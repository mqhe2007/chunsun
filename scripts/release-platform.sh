#!/usr/bin/env bash
# 全量发布：各平台 CLI → 前端 → 按指定目标交叉编译平台二进制。
# 默认 linux-x64（最常见的部署环境）。
#
# 用法:
#   pnpm run platform:release
#   pnpm run platform:release -- linux-arm64
#   pnpm run platform:release -- linux-x64 linux-arm64
#   pnpm run platform:release -- all
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
# shellcheck source=lib-release.sh
source "$ROOT/scripts/lib-release.sh"

usage() {
  cat <<EOF
用法: pnpm run platform:release -- [目标...]

目标（可多选，默认 linux-x64）:
  linux-x64      Linux x86_64（musl，默认）
  linux-arm64    Linux ARM64（musl）
  darwin-arm64   macOS Apple Silicon
  darwin-x64     macOS Intel
  windows-x64    Windows x64
  all            以上全部

产物目录: dist/platform/
EOF
}

ALIASES=()
if [[ $# -eq 0 ]]; then
  ALIASES=(linux-x64)
else
  for arg in "$@"; do
    case "$arg" in
      -h|--help)
        usage
        exit 0
        ;;
      all)
        ALIASES=("${ALL_PLATFORM_ALIASES[@]}")
        break
        ;;
      linux-x64|linux-arm64|darwin-arm64|darwin-x64|windows-x64)
        ALIASES+=("$arg")
        ;;
      *)
        echo "[release] 未知参数: $arg" >&2
        usage >&2
        exit 1
        ;;
    esac
  done
fi

if [[ ${#ALIASES[@]} -eq 0 ]]; then
  ALIASES=(linux-x64)
fi

ensure_cross_compile
ensure_rust_targets

echo "[release] 1/3 交叉编译 CLI（供实例 /cli 下载，始终打全平台）"
"$ROOT/scripts/build-cli-dist.sh"

echo "[release] 2/3 构建前端（官网 + 控制台）"
pnpm --filter website build
pnpm --filter console build

OUT="$ROOT/dist/platform"
mkdir -p "$OUT"
BE_MANIFEST="$ROOT/packages/backend/Cargo.toml"
BE_TARGET_DIR="$ROOT/packages/backend/target"

echo "[release] 3/3 构建平台二进制: ${ALIASES[*]}"
for alias in "${ALIASES[@]}"; do
  triple="$(platform_triple "$alias")"
  echo "[release] 编译平台 ($alias → $triple)…"
  cargo zigbuild --release --target "$triple" --manifest-path "$BE_MANIFEST"

  artifact="$(platform_artifact_name "$alias")"
  if [[ "$alias" == windows-x64 ]]; then
    src="$BE_TARGET_DIR/$triple/release/chunsun.exe"
  else
    src="$BE_TARGET_DIR/$triple/release/chunsun"
  fi
  if [[ ! -f "$src" ]]; then
    echo "[release] 未找到编译产物: $src" >&2
    exit 1
  fi
  cp "$src" "$OUT/$artifact"
  if [[ "$alias" != windows-x64 ]]; then
    chmod 755 "$OUT/$artifact"
  fi
  echo "[release] 已写出 $OUT/$artifact ($(wc -c < "$OUT/$artifact") bytes)"
done

echo "[release] 完成。把对应系统的文件拷到服务器运行即可，默认端口 11111，首次打开 /console/setup。"
echo "[release] 本机产物:"
ls -lh "$OUT"
