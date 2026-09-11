#!/usr/bin/env bash
# 以根 package.json 为唯一来源，将版本号同步到：
#   - 各 Rust crate 的 Cargo.toml
#   - 对应 Cargo.lock 中本 crate（name = "chunsun"）的 version
#   - README 版本徽章
# 也可先指定新版本号（会同时更新 package.json），再一键同步。
#
# 用法:
#   pnpm run version:sync            # 同步当前 package.json 的版本号
#   pnpm run version:sync -- 1.0.0   # 先把版本号改为 1.0.0，再同步
#
# 说明：Cargo.lock 的本包 version 原先要等 cargo build 才会刷新；
# 升版提交前必须由本脚本一并写入，避免 bump commit 漏 lock、部署后才发现脏文件。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# 需要同步版本号的 Cargo 清单（toml + 同目录 lock）
CARGO_CRATES=(
  "packages/backend"
  "packages/cli"
)

# 如果传了新版本号，先更新 package.json
# 跳过 pnpm/npm run 传递的 -- 分隔符
if [[ "${1:-}" == "--" ]]; then
  shift
fi
if [[ $# -ge 1 && -n "${1:-}" ]]; then
  NEW_VERSION="$1"
  # 用 node 更新 package.json，避免 sed 破坏 JSON 格式
  node -e "
    const fs = require('fs');
    const pkg = JSON.parse(fs.readFileSync('package.json', 'utf8'));
    pkg.version = '$NEW_VERSION';
    fs.writeFileSync('package.json', JSON.stringify(pkg, null, 2) + '\n');
  "
  echo "[version:sync] 已更新 package.json → $NEW_VERSION"
fi

# 读取 package.json 的版本号作为唯一来源
PKG_VERSION=$(node -p "require('./package.json').version")
echo "[version:sync] 目标版本号: $PKG_VERSION"
echo ""

# 更新单个 Cargo.toml 的 [package] 段内 version
update_cargo_version() {
  local manifest="$1"
  local version="$2"

  if [[ ! -f "$manifest" ]]; then
    echo "  ⚠️  跳过（文件不存在）: $manifest"
    return 0
  fi

  # 用 awk 只替换 [package] 段内的 version，不影响 dependencies
  local tmp
  tmp="$(mktemp)"
  awk -v ver="$version" '
    /^\[package\]/ { in_pkg=1; print; next }
    /^\[/ { in_pkg=0 }
    in_pkg && /^version[[:space:]]*=/ {
      print "version = \"" ver "\""
      next
    }
    { print }
  ' "$manifest" > "$tmp"

  # 只有内容变化时才覆盖，避免不必要的文件改动
  if ! diff -q "$manifest" "$tmp" >/dev/null 2>&1; then
    mv "$tmp" "$manifest"
    echo "  ✅ ${manifest}"
  else
    rm "$tmp"
    echo "  ⏭️  ${manifest}（已是 ${version}）"
  fi
}

# 更新 Cargo.lock 中本 crate（name = "chunsun"）的 version 行。
# 只改本地包条目，不动依赖图；等价于 cargo build 后 lock 里那一行的刷新。
update_cargo_lock_version() {
  local lock="$1"
  local version="$2"

  if [[ ! -f "$lock" ]]; then
    echo "  ⚠️  跳过（文件不存在）: $lock"
    return 0
  fi

  local tmp
  tmp="$(mktemp)"
  awk -v ver="$version" '
    /^\[\[package\]\]/ { in_pkg=1; is_chunsun=0; print; next }
    /^\[\[/ { in_pkg=0; is_chunsun=0 }
    in_pkg && /^name[[:space:]]*=[[:space:]]*"chunsun"/ { is_chunsun=1; print; next }
    in_pkg && is_chunsun && /^version[[:space:]]*=/ {
      print "version = \"" ver "\""
      next
    }
    { print }
  ' "$lock" > "$tmp"

  if ! diff -q "$lock" "$tmp" >/dev/null 2>&1; then
    mv "$tmp" "$lock"
    echo "  ✅ ${lock}"
  else
    rm "$tmp"
    echo "  ⏭️  ${lock}（已是 ${version}）"
  fi
}

echo "[version:sync] 同步 Cargo.toml / Cargo.lock:"
for crate_dir in "${CARGO_CRATES[@]}"; do
  update_cargo_version "${crate_dir}/Cargo.toml" "$PKG_VERSION"
  update_cargo_lock_version "${crate_dir}/Cargo.lock" "$PKG_VERSION"
done

# 同步 README 版本徽章（badge 中 version-v<版本> 为唯一出现处）
update_readme_badge() {
  local readme="$1"
  local version="$2"

  if [[ ! -f "$readme" ]]; then
    echo "  ⚠️  跳过（文件不存在）: $readme"
    return 0
  fi

  local tmp
  tmp="$(mktemp)"
  sed -E "s/version-v[0-9][0-9.]*/version-v${version}/g" "$readme" > "$tmp"

  if ! diff -q "$readme" "$tmp" >/dev/null 2>&1; then
    mv "$tmp" "$readme"
    echo "  ✅ ${readme}"
  else
    rm "$tmp"
    echo "  ⏭️  ${readme}（徽章已是 v${version}）"
  fi
}

echo ""
echo "[version:sync] 同步 README 徽章:"
update_readme_badge "$ROOT/README.md" "$PKG_VERSION"

echo ""
echo "[version:sync] 完成。校验结果:"
bash "$ROOT/scripts/check-version-consistency.sh"
