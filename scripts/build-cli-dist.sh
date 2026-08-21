#!/usr/bin/env bash
# 交叉编译全部 CLI 目标，写入 packages/cli/dist（chunsun-cli-*，与平台包 chunsun-* 区分）。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=lib-release.sh
source "$ROOT/scripts/lib-release.sh"

CLI_MANIFEST="$ROOT/packages/cli/Cargo.toml"
DIST="$ROOT/packages/cli/dist"

ensure_cross_compile
ensure_rust_targets

# 若未显式指定，从仓库根 .env 的 PUBLIC_ORIGIN 推导（与 CLI build.rs 一致）。
# 空默认会导致终端用户 update 后刷新模板打到 localhost。
if [[ -z "${CHUNSUN_DEFAULT_API_URL:-}" && -f "$ROOT/.env" ]]; then
  # shellcheck disable=SC1091
  set -a
  source "$ROOT/.env"
  set +a
  if [[ -n "${PUBLIC_ORIGIN:-}" ]]; then
    origin="${PUBLIC_ORIGIN%/}"
    export CHUNSUN_DEFAULT_API_URL="${origin}/api/v1"
    export CHUNSUN_DEFAULT_CLI_DOWNLOAD_URL="${origin}/cli"
    echo "[release] CLI 默认 API：$CHUNSUN_DEFAULT_API_URL"
  fi
fi
if [[ -z "${CHUNSUN_DEFAULT_API_URL:-}" ]]; then
  echo "[release] 警告：未设置 CHUNSUN_DEFAULT_API_URL / PUBLIC_ORIGIN，CLI 将不内嵌默认实例地址。" >&2
fi

CLI_FILES=(
  chunsun-cli-darwin-arm64
  chunsun-cli-darwin-x64
  chunsun-cli-linux-x64
  chunsun-cli-linux-arm64
  chunsun-cli-windows-x64.exe
)

mkdir -p "$DIST"
rm -f "$DIST"/.keep \
  "$DIST"/chunsun-darwin-arm64 \
  "$DIST"/chunsun-darwin-x64 \
  "$DIST"/chunsun-linux-x64 \
  "$DIST"/chunsun-linux-arm64 \
  "$DIST"/chunsun-windows-x64.exe \
  "${CLI_FILES[@]/#/$DIST/}"

build_one() {
  local triple="$1"
  echo "[release] 编译 CLI ($triple)…"
  cargo zigbuild --release --target "$triple" --manifest-path "$CLI_MANIFEST"
}

build_one aarch64-apple-darwin
build_one x86_64-apple-darwin
build_one x86_64-unknown-linux-musl
build_one aarch64-unknown-linux-musl
build_one x86_64-pc-windows-gnu

CLI_TARGET_DIR="$ROOT/packages/cli/target"
cp "$CLI_TARGET_DIR/aarch64-apple-darwin/release/chunsun" "$DIST/chunsun-cli-darwin-arm64"
cp "$CLI_TARGET_DIR/x86_64-apple-darwin/release/chunsun" "$DIST/chunsun-cli-darwin-x64"
cp "$CLI_TARGET_DIR/x86_64-unknown-linux-musl/release/chunsun" "$DIST/chunsun-cli-linux-x64"
cp "$CLI_TARGET_DIR/aarch64-unknown-linux-musl/release/chunsun" "$DIST/chunsun-cli-linux-arm64"
cp "$CLI_TARGET_DIR/x86_64-pc-windows-gnu/release/chunsun.exe" "$DIST/chunsun-cli-windows-x64.exe"
chmod 755 "$DIST"/chunsun-cli-darwin-arm64 "$DIST"/chunsun-cli-darwin-x64 \
  "$DIST"/chunsun-cli-linux-x64 "$DIST"/chunsun-cli-linux-arm64

missing=0
for f in "${CLI_FILES[@]}"; do
  if [[ ! -f "$DIST/$f" ]]; then
    echo "[release] 缺少产物: $DIST/$f" >&2
    missing=1
  else
    echo "[release] 已就绪 $f ($(wc -c < "$DIST/$f") bytes)"
  fi
done

if [[ "$missing" -ne 0 ]]; then
  exit 1
fi

echo "[release] CLI dist 完成: $DIST"
