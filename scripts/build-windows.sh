#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

TARGET="x86_64-pc-windows-gnu"

if ! command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
  echo "error: mingw-w64 linker not found." >&2
  echo "Install it first, e.g. on Debian/Ubuntu:" >&2
  echo "    sudo apt install mingw-w64" >&2
  exit 1
fi

if ! rustup target list --installed | grep -q "^${TARGET}\$"; then
  echo "==> Adding rust target $TARGET"
  rustup target add "$TARGET"
fi

echo "==> Building for Windows ($TARGET)"
cargo build --release --target "$TARGET"

OUT_DIR="dist/windows"
mkdir -p "$OUT_DIR"
cp "target/$TARGET/release/nannou_sketch.exe" "$OUT_DIR/"

echo "==> Done: $OUT_DIR/nannou_sketch.exe"
