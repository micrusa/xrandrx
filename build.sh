#!/usr/bin/env bash
set -euo pipefail

echo "Building xrandrx in release mode..."
cargo build --release

echo "Build complete: target/release/xrandrx"
