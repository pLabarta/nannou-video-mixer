#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

TARGET="x86_64-apple-darwin"

# Cross-compiling a GUI app (nannou links Cocoa/Metal via winit + wgpu) for
# macOS from Linux requires the real Apple SDK, provided through osxcross.
# There is no legal way to download that SDK except by extracting it from
# Xcode on a Mac you (or your org) own, so this script only works if you
# already set up osxcross locally.
#
# Reliable alternatives, in order of preference:
#   1. Build on a real Mac (`./scripts/build-linux.sh` equivalent there,
#      target is just the host).
#   2. Use the GitHub Actions workflow at .github/workflows/release.yml,
#      which builds on an actual macos-latest runner. This is the
#      recommended path and needs no local macOS toolchain at all.
#   3. Set up osxcross yourself and point OSXCROSS_ROOT at it, then re-run
#      this script.

if [ -z "${OSXCROSS_ROOT:-}" ] || ! command -v x86_64-apple-darwin-clang >/dev/null 2>&1; then
  cat >&2 <<'EOF'
error: osxcross toolchain not found (no x86_64-apple-darwin-clang on PATH
and/or OSXCROSS_ROOT is unset).

Cross-compiling for macOS from Linux needs the Apple SDK via osxcross,
which this machine doesn't have set up. Use one of:
  - GitHub Actions: .github/workflows/release.yml builds a real macOS
    binary on a macos-latest runner, no local setup required.
  - Build directly on a Mac.
  - Install osxcross (https://github.com/tpoechtrager/osxcross) with a
    legally obtained macOS SDK, export OSXCROSS_ROOT, and re-run this
    script.
EOF
  exit 1
fi

export CC_x86_64_apple_darwin="x86_64-apple-darwin-clang"
export CXX_x86_64_apple_darwin="x86_64-apple-darwin-clang++"
export CARGO_TARGET_X86_64_APPLE_DARWIN_LINKER="x86_64-apple-darwin-clang"

if ! rustup target list --installed | grep -q "^${TARGET}\$"; then
  echo "==> Adding rust target $TARGET"
  rustup target add "$TARGET"
fi

echo "==> Building for macOS ($TARGET)"
cargo build --release --target "$TARGET"

OUT_DIR="dist/macos"
mkdir -p "$OUT_DIR"
cp "target/$TARGET/release/nannou_sketch" "$OUT_DIR/"

echo "==> Done: $OUT_DIR/nannou_sketch"
