#!/bin/sh
# Cross-build the Windows release binaries, so Unix-only code that a Windows
# build reaches is caught before a release rather than during one.
#
# Builds what scripts/release/coder.sh ships for windows-x86_64: Coder
# (coder-new), the openagents CLI, Microcoder, and the coder-boundary helper.
# Needs cargo-zigbuild, zig, and the x86_64-pc-windows-gnu Rust target.
# Run it under the build lease:
#
#   openagents lease build --keep-target-dir -- scripts/check-windows.sh
set -eu
cd "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"

for tool in cargo-zigbuild zig; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "check-windows: $tool is not installed" >&2
        exit 1
    fi
done

exec cargo zigbuild --release --locked --target x86_64-pc-windows-gnu \
    -p coder-new --bin coder-new \
    -p openagents-cli --bin openagents \
    -p microcoder --bin microcoder \
    -p coder-boundary --bin coder-boundary
