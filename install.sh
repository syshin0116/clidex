#!/bin/sh
set -e

# clidex installer — downloads the latest release binary for your platform

REPO="syshin0116/clidex"

# Choose install directory:
# 1. User override via env var
# 2. ~/.cargo/bin if it exists (avoid conflict with cargo install)
# 3. ~/.local/bin as default
if [ -n "$CLIDEX_INSTALL_DIR" ]; then
  INSTALL_DIR="$CLIDEX_INSTALL_DIR"
elif [ -d "$HOME/.cargo/bin" ]; then
  INSTALL_DIR="$HOME/.cargo/bin"
else
  INSTALL_DIR="$HOME/.local/bin"
fi

# Detect platform
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
  linux)  OS_TARGET="unknown-linux-gnu" ;;
  darwin) OS_TARGET="apple-darwin" ;;
  *)      echo "Unsupported OS: $OS"; exit 1 ;;
esac

case "$ARCH" in
  x86_64|amd64) ARCH_TARGET="x86_64" ;;
  aarch64|arm64) ARCH_TARGET="aarch64" ;;
  *)             echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

TARGET="${ARCH_TARGET}-${OS_TARGET}"
ARCHIVE="clidex-${TARGET}.tar.gz"

echo "Detected platform: ${TARGET}"

# Get latest release URL
DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/${ARCHIVE}"

echo "Downloading from: ${DOWNLOAD_URL}"

# Create temp directory
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

# Download and extract
curl -fsSL "$DOWNLOAD_URL" -o "$TMP_DIR/$ARCHIVE"
tar xzf "$TMP_DIR/$ARCHIVE" -C "$TMP_DIR"

# Install
mkdir -p "$INSTALL_DIR"
mv "$TMP_DIR/clidex" "$INSTALL_DIR/clidex"
chmod +x "$INSTALL_DIR/clidex"

echo ""
echo "Installed clidex to: $INSTALL_DIR/clidex"

# Warn if another clidex binary exists elsewhere in PATH
OTHER_CLIDEX=$(which clidex 2>/dev/null || true)
if [ -n "$OTHER_CLIDEX" ] && [ "$OTHER_CLIDEX" != "$INSTALL_DIR/clidex" ]; then
  echo ""
  echo "Warning: another clidex found at $OTHER_CLIDEX"
  echo "  This may shadow the newly installed version."
  echo "  Remove it with: rm $OTHER_CLIDEX"
fi

# Check if install dir is in PATH
case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *) echo "Warning: $INSTALL_DIR is not in your PATH"
     echo "Add this to your shell profile:"
     echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
     ;;
esac

echo ""
echo "Get started:"
echo "  clidex update       # Download the tool index"
echo "  clidex \"json tool\"  # Search for tools"
