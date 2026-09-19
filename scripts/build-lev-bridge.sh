#!/usr/bin/env bash
# Builds the Lev bridge helper, the Swift executable that reaches Apple's
# on-device foundation model.
#
# The helper needs macOS 26 or later on Apple Silicon with Apple Intelligence
# enabled. Everything else in `crates/lev` builds and tests without it.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
package="$root/swift/lev-bridge"

if ! command -v swift >/dev/null 2>&1; then
  echo "swift is not on PATH; install the Xcode command line tools" >&2
  exit 1
fi

swift build --package-path "$package" -c release

binary="$package/.build/release/lev-bridge"
echo "built $binary"
codesign --verify --strict "$binary" && echo "signature verified"
