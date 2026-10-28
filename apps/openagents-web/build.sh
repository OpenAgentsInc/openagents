#!/usr/bin/env bash
# Builds the splash for the browser: build/splash.wasm.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$here/build"
"$here/../../coax/web/wasm.sh" "$here/splash.coax" "$here/face.c" "$here/build/splash.wasm"
