#!/usr/bin/env bash
# 由发布脚本 source。约定：调用方已 set -euo pipefail，且 ROOT 指向仓库根。

ensure_cross_compile() {
  if ! command -v cargo >/dev/null 2>&1; then
    echo "[release] 缺少命令: cargo。请先安装 Rust：https://rustup.rs" >&2
    exit 1
  fi
  if ! command -v rustup >/dev/null 2>&1; then
    echo "[release] 缺少命令: rustup" >&2
    exit 1
  fi
  if ! command -v zig >/dev/null 2>&1; then
    echo "[release] 缺少命令: zig（cargo-zigbuild 用来交叉编译）。macOS: brew install zig" >&2
    exit 1
  fi
  if ! cargo zigbuild --help >/dev/null 2>&1; then
    echo "[release] 未检测到 cargo-zigbuild，正在安装…"
    cargo install cargo-zigbuild
  fi
}

# 短名 → rustc triple
platform_triple() {
  case "$1" in
    linux-x64) echo x86_64-unknown-linux-musl ;;
    linux-arm64) echo aarch64-unknown-linux-musl ;;
    darwin-arm64) echo aarch64-apple-darwin ;;
    darwin-x64) echo x86_64-apple-darwin ;;
    windows-x64) echo x86_64-pc-windows-gnu ;;
    *)
      echo "[release] 未知平台: $1" >&2
      echo "[release] 可选: linux-x64 linux-arm64 darwin-arm64 darwin-x64 windows-x64" >&2
      return 1
      ;;
  esac
}

ALL_PLATFORM_ALIASES=(linux-x64 linux-arm64 darwin-arm64 darwin-x64 windows-x64)

ALL_RUST_TARGETS=(
  aarch64-apple-darwin
  x86_64-apple-darwin
  x86_64-unknown-linux-musl
  aarch64-unknown-linux-musl
  x86_64-pc-windows-gnu
)

ensure_rust_targets() {
  echo "[release] 确保 Rust target 已安装…"
  local t
  for t in "${ALL_RUST_TARGETS[@]}"; do
    rustup target add "$t" >/dev/null
  done
}

# 平台发布包：chunsun-<os>-<arch>（与 CLI 的 chunsun-cli-* 区分）
platform_artifact_name() {
  case "$1" in
    windows-x64) echo chunsun-windows-x64.exe ;;
    *) echo "chunsun-$1" ;;
  esac
}
