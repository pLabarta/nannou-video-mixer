#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

./build-linux.sh
./build-windows.sh
./build-macos.sh || echo "==> Skipping macOS (see message above) — use .github/workflows/release.yml instead"
