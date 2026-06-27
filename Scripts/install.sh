#!/usr/bin/env bash
set -euo pipefail

REPO="pamod-madubashana/Cotrex"
INSTALL_DIR="${COTREX_INSTALL_DIR:-$HOME/.local/bin}"

echo "Installing Cotrex..."

# Detect platform
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS/$ARCH" in
    Linux/x86_64)   FILENAME_PATTERN="cotrex-VERSION-linux-x86_64.tar.gz" ;;
    Linux/aarch64)  FILENAME_PATTERN="cotrex-VERSION-linux-aarch64.tar.gz" ;;
    Darwin/arm64)   FILENAME_PATTERN="cotrex-VERSION-macos-arm64.tar.gz" ;;
    Darwin/x86_64)  FILENAME_PATTERN="cotrex-VERSION-macos-x86_64.tar.gz" ;;
    *) echo "Unsupported platform: $OS/$ARCH"; exit 1 ;;
esac

# Get latest release tag
echo "Fetching latest release..."
TAG="$(curl -sL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')"
if [ -z "$TAG" ]; then
    echo "Failed to fetch latest release."
    exit 1
fi

VERSION="${TAG#v}"
FILENAME="${FILENAME_PATTERN/VERSION/$VERSION}"
URL="https://github.com/$REPO/releases/download/$TAG/$FILENAME"

# Download and extract
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

echo "Downloading $FILENAME..."
curl -sL "$URL" -o "$TMPDIR/$FILENAME"

echo "Extracting..."
tar -xzf "$TMPDIR/$FILENAME" -C "$TMPDIR"

# Install
mkdir -p "$INSTALL_DIR"
cp "$TMPDIR/cotrex" "$INSTALL_DIR/cotrex"
chmod +x "$INSTALL_DIR/cotrex"

echo ""
echo "Installed: $INSTALL_DIR/cotrex"

# PATH hint
if ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
    echo ""
    echo "Add to your shell profile (~/.bashrc, ~/.zshrc):"
    echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
fi

echo ""
"$INSTALL_DIR/cotrex" --version
