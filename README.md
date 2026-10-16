# nannou_sketch

A simple [Nannou](https://nannou.cc) sketch — a ball bouncing around the window.

## Development (Linux)

Run it directly with cargo while iterating:

```
cargo run
```

Debug builds are slow to start under nannou/wgpu but fine for tweaking the sketch. For smoother frame rates while testing animation/timing, use a release build instead:

```
cargo run --release
```

Optional: auto-rebuild and restart on file changes with [`cargo-watch`](https://github.com/watchexec/cargo-watch):

```
cargo install cargo-watch
cargo watch -x run
```

## Building

Build scripts live in `scripts/` and produce binaries under `dist/<platform>/`. All are run from Linux.

### Linux (native)

```
./scripts/build-linux.sh
```

Output: `dist/linux/nannou_sketch`

### Windows (cross-compile)

Requires the mingw-w64 toolchain:

```
sudo apt install mingw-w64
./scripts/build-windows.sh
```

Output: `dist/windows/nannou_sketch.exe`

### macOS

Cross-compiling for macOS from Linux requires the Apple SDK via [osxcross](https://github.com/tpoechtrager/osxcross), which can't be obtained legally without owning a Mac. If you have osxcross set up, `./scripts/build-macos.sh` will use it.

Otherwise, build a real macOS binary with no local macOS toolchain by pushing to GitHub and running the included workflow (`.github/workflows/release.yml`), which builds on a `macos-latest` runner — trigger it manually from the Actions tab or by pushing a `v*` tag.

### All at once

```
./scripts/build-all.sh
```

Builds Linux and Windows, and macOS if osxcross is available; otherwise skips it with a pointer to the GitHub Actions workflow.
# nannou-video-mixer
