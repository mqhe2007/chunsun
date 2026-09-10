#!/usr/bin/env bash
# 以根 package.json 为唯一来源，将版本号同步到所有 Rust crate 的 Cargo.toml。
# 也可先指定新版本号（会同时更新 package.json），再一键同步。
#
# 用法:
#   pnpm run version:sync            # 同步当前 package.json 的版本号到各 Cargo.toml
#   pnpm run version:sync -- 1.0.0   # 先把版本号改为 1.0.0，再同步
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# 需要同步版本号的 Cargo.toml 清单
CARGO_MANIFESTS=(
  "packages/backend/Cargo.toml"
  "packages/cli/Cargo.toml"
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
    echo "  ✅ $manifest"
  else
    rm "$tmp"
    echo "  ⏭️  ${manifest}（已是 ${version}）"
  fi
}

echo "[version:sync] 同步 Cargo.toml:"
for manifest in "${CARGO_MANIFESTS[@]}"; do
  update_cargo_version "$manifest" "$PKG_VERSION"
done

echo ""
echo "[version:sync] 完成。校验结果:"
bash "$ROOT/scripts/check-version-consistency.sh"
