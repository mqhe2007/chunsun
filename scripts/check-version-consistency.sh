#!/usr/bin/env bash
# 校验根 package.json 与各 Rust crate 的版本号一致性。
# 单一来源：根 package.json 的 version 字段。
#
# 用法:
#   bash scripts/check-version-consistency.sh
#   pnpm run version:check
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# 用 node 解析 package.json，避免 grep/sed 被注释、空格或格式变化影响
PKG_VERSION=$(node -p "require('./package.json').version")

# 只在 [package] 段内匹配 version，避免被 dependencies 里的 version 字段干扰
read_cargo_version() {
  local manifest="$1"
  if [[ ! -f "$manifest" ]]; then
    echo "::error::找不到文件: $manifest" >&2
    return 1
  fi
  awk '
    /^\[package\]/ { in_pkg=1; next }
    /^\[/ { in_pkg=0 }
    in_pkg && /^version[[:space:]]*=/ {
      gsub(/^version[[:space:]]*=[[:space:]]*"/, "")
      gsub(/".*/, "")
      print
      found=1
      exit
    }
    END {
      if (!found) {
        print "::error::未在 [package] 段找到 version 字段" >"/dev/stderr"
        exit 1
      }
    }
  ' "$manifest"
}

# 读取 Cargo.lock 中本 crate（name = "chunsun"）的 version
read_cargo_lock_version() {
  local lock="$1"
  if [[ ! -f "$lock" ]]; then
    echo "::error::找不到文件: $lock" >&2
    return 1
  fi
  awk '
    /^\[\[package\]\]/ { in_pkg=1; is_chunsun=0; next }
    /^\[\[/ { in_pkg=0; is_chunsun=0 }
    in_pkg && /^name[[:space:]]*=[[:space:]]*"chunsun"/ { is_chunsun=1; next }
    in_pkg && is_chunsun && /^version[[:space:]]*=/ {
      gsub(/^version[[:space:]]*=[[:space:]]*"/, "")
      gsub(/".*/, "")
      print
      found=1
      exit
    }
    END {
      if (!found) {
        print "::error::未在 Cargo.lock 找到 name=\"chunsun\" 的 version" >"/dev/stderr"
        exit 1
      }
    }
  ' "$lock"
}

BACKEND_VERSION=$(read_cargo_version "packages/backend/Cargo.toml")
CLI_VERSION=$(read_cargo_version "packages/cli/Cargo.toml")
BACKEND_LOCK_VERSION=$(read_cargo_lock_version "packages/backend/Cargo.lock")
CLI_LOCK_VERSION=$(read_cargo_lock_version "packages/cli/Cargo.lock")

echo "package.json:        $PKG_VERSION"
echo "backend Cargo.toml:  $BACKEND_VERSION"
echo "backend Cargo.lock:  $BACKEND_LOCK_VERSION"
echo "cli Cargo.toml:      $CLI_VERSION"
echo "cli Cargo.lock:      $CLI_LOCK_VERSION"
echo ""

MISMATCH=0
if [[ "$PKG_VERSION" != "$BACKEND_VERSION" ]]; then
  echo "::error::backend Cargo.toml 版本号不一致：期望 ${PKG_VERSION}，实际 ${BACKEND_VERSION}"
  MISMATCH=1
fi
if [[ "$PKG_VERSION" != "$BACKEND_LOCK_VERSION" ]]; then
  echo "::error::backend Cargo.lock 版本号不一致：期望 ${PKG_VERSION}，实际 ${BACKEND_LOCK_VERSION}"
  MISMATCH=1
fi
if [[ "$PKG_VERSION" != "$CLI_VERSION" ]]; then
  echo "::error::cli Cargo.toml 版本号不一致：期望 ${PKG_VERSION}，实际 ${CLI_VERSION}"
  MISMATCH=1
fi
if [[ "$PKG_VERSION" != "$CLI_LOCK_VERSION" ]]; then
  echo "::error::cli Cargo.lock 版本号不一致：期望 ${PKG_VERSION}，实际 ${CLI_LOCK_VERSION}"
  MISMATCH=1
fi

# ---- CLI dist 构建产物版本校验 ----
# 源码版本号 bump 后若忘记重建 dist，终端用户 update 会下载到旧版本二进制。
# 本机可运行的平台直接执行 --version 严格比对；交叉编译的产物无法在本机运行，
# 通过检查二进制中是否嵌入精确版本字节串（env! 注入，实测新旧版本互不误报）校验。
DIST="$ROOT/packages/cli/dist"
# 产物文件名带版本号（与 build-cli-dist.sh 一致），版本号单一来源：根 package.json
DIST_FILES=(
  "chunsun-cli-darwin-arm64-v${PKG_VERSION}"
  "chunsun-cli-darwin-x64-v${PKG_VERSION}"
  "chunsun-cli-linux-x64-v${PKG_VERSION}"
  "chunsun-cli-linux-arm64-v${PKG_VERSION}"
  "chunsun-cli-windows-x64-v${PKG_VERSION}.exe"
)

OS_NAME="$(uname -s)"

can_run() {
  case "$OS_NAME" in
    Darwin)
      case "$1" in
        chunsun-cli-darwin-*) return 0 ;;
      esac
      ;;
    Linux)
      case "$1" in
        chunsun-cli-linux-*) return 0 ;;
      esac
      ;;
    MINGW* | MSYS* | CYGWIN*)
      case "$1" in
        *.exe) return 0 ;;
      esac
      ;;
  esac
  return 1
}

if [[ -d "$DIST" ]]; then
  echo ""
  echo "CLI dist 构建产物:"
  for f in "${DIST_FILES[@]}"; do
    if [[ ! -f "$DIST/$f" ]]; then
      echo "::error::CLI dist 缺少产物: ${f}（请先运行 pnpm run cli:dist）"
      MISMATCH=1
      continue
    fi
    if can_run "$f"; then
      actual="$("$DIST/$f" --version 2>/dev/null | awk '{print $NF}')"
      mode="运行 --version"
    else
      if python3 -c "import sys; sys.exit(0 if sys.argv[2].encode() in open(sys.argv[1],'rb').read() else 1)" "$DIST/$f" "$PKG_VERSION" 2>/dev/null; then
        actual="$PKG_VERSION"
        mode="嵌入版本串检查"
      else
        actual="未知/不匹配"
        mode="嵌入版本串检查"
      fi
    fi
    if [[ "$actual" != "$PKG_VERSION" ]]; then
      echo "::error::CLI dist 版本不一致：$f 实际 ${actual}（${mode}），期望 ${PKG_VERSION}。请运行 pnpm run cli:dist 重建并重新部署 /cli 目录。"
      MISMATCH=1
    else
      echo "  ✅ ${f}（${mode}）= ${actual}"
    fi
  done
else
  echo "::warning::CLI dist 目录不存在（${DIST}），跳过产物校验（CI 无构建产物；本地请先 pnpm run cli:dist）"
fi

if [[ "$MISMATCH" -ne 0 ]]; then
  echo ""
  echo "::error::版本号不一致，请以根 package.json 为唯一来源同步修改"
  echo "  一键同步: pnpm run version:sync"
  echo "  CLI dist 重建: pnpm run cli:dist"
  exit 1
fi

echo ""
echo "✅ 版本号一致：${PKG_VERSION}"
