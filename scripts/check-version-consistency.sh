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

BACKEND_VERSION=$(read_cargo_version "packages/backend/Cargo.toml")
CLI_VERSION=$(read_cargo_version "packages/cli/Cargo.toml")

echo "package.json:        $PKG_VERSION"
echo "backend Cargo.toml:  $BACKEND_VERSION"
echo "cli Cargo.toml:      $CLI_VERSION"
echo ""

MISMATCH=0
if [[ "$PKG_VERSION" != "$BACKEND_VERSION" ]]; then
  echo "::error::backend 版本号不一致：期望 ${PKG_VERSION}，实际 ${BACKEND_VERSION}"
  MISMATCH=1
fi
if [[ "$PKG_VERSION" != "$CLI_VERSION" ]]; then
  echo "::error::cli 版本号不一致：期望 ${PKG_VERSION}，实际 ${CLI_VERSION}"
  MISMATCH=1
fi

if [[ "$MISMATCH" -ne 0 ]]; then
  echo ""
  echo "::error::版本号不一致，请以根 package.json 为唯一来源同步修改"
  echo "  一键同步: pnpm run version:sync"
  exit 1
fi

echo "✅ 版本号一致：${PKG_VERSION}"
