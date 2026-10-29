#!/usr/bin/env bash
# Standalone local probe; ordinary site builds never visit this hidden folder.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../../../.." && pwd)"
mkdir -p "$here/build"
"$root/coax/web/wasm.sh" "$here/probe.coax" "$here/face.c" "$here/build/probe.wasm" "$here/verifier.coax"
cp "$root/apps/openagents-web/splash.wgsl" "$here/build/probe.wgsl"
