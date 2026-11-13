#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: finds the Wasm toolchain, then runs the Coax tool that builds a realm script)
# Builds a realm script: a `#script` Coax unit in, a checked WebAssembly module and its manifest out
# (metaverse-engine/tools/script-build.coax, docs/research/2026-11-13-realm-scripts.md). Run at the
# repository root:
#   coax/web/script.sh SOURCE.coax [--out DIR] [--quiet]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../.." && pwd)"
source "$here/toolchain.sh"
export COAXIAL_CLANG COAXIAL_RESOURCES PATH
export COAX="${COAX:-$repo/coax/bin/coax}"
exec "$here/tool.sh" "$repo/metaverse-engine/tools/script-build.coax" "$@"
