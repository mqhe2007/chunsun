#!/usr/bin/env sh
set -e

BASE_URL="__CHUNSUN_CLI_DOWNLOAD_URL__"
INSTALL_DIR="$HOME/.local/bin"
COMMAND_NAME="chunsun"
TARGET="$INSTALL_DIR/$COMMAND_NAME"

echo "[install] 开始安装 chunsun CLI..."
echo "[install] Windows 用户请在 PowerShell 中执行: irm ${BASE_URL}/install.ps1 | iex"

# 检测操作系统
OS="$(uname -s)"
case "$OS" in
  Darwin) OS_NAME="darwin" ;;
  Linux)  OS_NAME="linux" ;;
  *)
    echo "[install] 错误: 不支持的操作系统: $OS" >&2
    exit 1
    ;;
esac

# 检测 CPU 架构
ARCH="$(uname -m)"
case "$ARCH" in
  x86_64 | amd64) ARCH_NAME="x64" ;;
  arm64 | aarch64) ARCH_NAME="arm64" ;;
  *)
    echo "[install] 错误: 不支持的 CPU 架构: $ARCH" >&2
    exit 1
    ;;
esac

BINARY_NAME="chunsun-cli-${OS_NAME}-${ARCH_NAME}"
BINARY_URL="${BASE_URL}/${BINARY_NAME}"
echo "[install] 平台: ${OS_NAME}-${ARCH_NAME}"

# 确保安装目录存在
mkdir -p "$INSTALL_DIR"

# 兼容旧版本通过软链接安装的场景：
# 若目标是软链接（尤其是失效软链接），curl -o 会跟随链接写入并可能报 No such file or directory。
if [ -L "$TARGET" ]; then
  rm -f "$TARGET"
fi

# 下载二进制文件
echo "[install] 正在下载: $BINARY_URL"
if command -v curl > /dev/null 2>&1; then
  curl -fSL "$BINARY_URL" -o "$TARGET"
elif command -v wget > /dev/null 2>&1; then
  wget -O "$TARGET" "$BINARY_URL"
else
  echo "[install] 错误: 需要 curl 或 wget，请先安装其中之一" >&2
  exit 1
fi

# 赋予执行权限
chmod 755 "$TARGET"

echo "[install] 安装完成: $TARGET"

# 检查 PATH
case ":$PATH:" in
  *":$INSTALL_DIR:"*)
    ;;
  *)
    echo "[install] 提示: $INSTALL_DIR 未加入 PATH"
    echo "[install] 请在 ~/.zshrc 或 ~/.bashrc 中添加以下内容并重启终端："
    echo ""
    echo "    export PATH=\"$INSTALL_DIR:\$PATH\""
    echo ""
    ;;
esac

# 验证安装
if command -v "$COMMAND_NAME" > /dev/null 2>&1; then
  echo "[install] 验证安装："
  "$COMMAND_NAME" --version
else
  echo "[install] 安装完成，请将 $INSTALL_DIR 加入 PATH 后执行: $COMMAND_NAME --version"
fi
