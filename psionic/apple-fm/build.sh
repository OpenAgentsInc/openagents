#!/usr/bin/env bash
# Builds the Apple FM bridge: macOS 26 or later on Apple Silicon, with the
# Xcode command line tools. Running it also needs Apple Intelligence on.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ "$(uname -s)" != "Darwin" ]; then echo "apple-fm: macOS only" >&2; exit 1; fi
command -v swift >/dev/null 2>&1 || { echo "apple-fm: swift is not on PATH; install the Xcode command line tools (xcode-select --install)" >&2; exit 1; }
swift build --package-path "$here" -c release
bin="$here/.build/release/apple-fm-bridge"
codesign --verify --strict "$bin"
echo "$bin"
