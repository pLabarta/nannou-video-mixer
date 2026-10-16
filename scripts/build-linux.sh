#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

TARGET="x86_64-unknown-linux-gnu"

echo "==> Building for Linux ($TARGET)"
cargo build --release --target "$TARGET"

OUT_DIR="dist/linux"
mkdir -p "$OUT_DIR"
cp "target/$TARGET/release/nannou_sketch" "$OUT_DIR/"

echo "==> Done: $OUT_DIR/nannou_sketch"
