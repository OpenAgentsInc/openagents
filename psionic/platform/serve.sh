#!/usr/bin/env bash
# Build the release /v1/systemone host and run it on 127.0.0.1.
#   psionic/platform/serve.sh --toy [--port N] [--read-ms N] [--deadline-ms N] [--capacity N]
#   psionic/platform/serve.sh --elephant DIR [the same options]
# The Elephant files are supplied by the caller; nothing is downloaded.
set -euo pipefail
cd "$(dirname "$0")/../.."
build=${PSIONIC_SERVE_BUILD:-${TMPDIR:-/tmp}/psionic-serve-build}
psionic/platform/serve-build.sh "$build" release plain
args=("$@")
for a in "$@"; do if [[ $a == --elephant ]]; then args+=(--table psionic/fixtures/text/ch05/qwen35.pst); fi; done
exec "$build/psionic-serve" "${args[@]}"
