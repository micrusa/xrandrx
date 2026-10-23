#!/usr/bin/env bash
set -euo pipefail

# Ensure binary is built
if [ ! -f "target/release/xrandrx" ]; then
    echo "Binary not found. Building release binary first..."
    cargo build --release
fi

# Determine destination directory
DEST_DIR="${1:-}"

if [ -z "$DEST_DIR" ]; then
    if [ -d "$HOME/.local/bin" ] || [[ ":$PATH:" == *":$HOME/.local/bin:"* ]]; then
        DEST_DIR="$HOME/.local/bin"
    elif [ -w "/usr/local/bin" ]; then
        DEST_DIR="/usr/local/bin"
    else
        DEST_DIR="$HOME/.local/bin"
    fi
fi

mkdir -p "$DEST_DIR"

echo "Installing xrandrx to $DEST_DIR..."
install -m 755 target/release/xrandrx "$DEST_DIR/xrandrx"

echo "Successfully installed xrandrx to $DEST_DIR/xrandrx"
if [[ ":$PATH:" != *":$DEST_DIR:"* ]]; then
    echo "Note: Make sure $DEST_DIR is in your PATH."
fi
