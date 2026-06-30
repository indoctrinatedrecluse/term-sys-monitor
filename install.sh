#!/bin/bash
# install.sh - Installer for Linux and macOS

set -e

REPO="indoctrinatedrecluse/term-sys-monitor"
BINARY_NAME="term-sys-monitor-linux"
INSTALL_DIR="/usr/local/bin"

# Determine fallback local binary directory if no root access
if [ "$EUID" -ne 0 ]; then
    INSTALL_DIR="$HOME/.local/bin"
    echo "Notice: Running without sudo. Installing to $INSTALL_DIR..."
fi

# Ensure install directory exists
mkdir -p "$INSTALL_DIR"

echo "Fetching latest release version from GitHub..."
LATEST_TAG=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$LATEST_TAG" ]; then
    echo "Error: Could not retrieve latest release version. Make sure there is a release available."
    exit 1
fi

DOWNLOAD_URL="https://github.com/$REPO/releases/download/$LATEST_TAG/$BINARY_NAME"

echo "Downloading term-sys-monitor ($LATEST_TAG)..."
curl -L -o "$INSTALL_DIR/term-sys-monitor" "$DOWNLOAD_URL"

echo "Configuring execution permissions..."
chmod +x "$INSTALL_DIR/term-sys-monitor"

echo "=================================================="
echo "   term-sys-monitor successfully installed!      "
echo "=================================================="
echo "Location: $INSTALL_DIR/term-sys-monitor"
echo "Simply run: term-sys-monitor"
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo "Warning: $INSTALL_DIR is not in your PATH. You may need to add it."
fi
echo "=================================================="
