#!/usr/bin/env bash
# Build the release /v1/systemone host and run it on 127.0.0.1.
#   psionic/platform/serve.sh --toy [--port N] [--read-ms N] [--deadline-ms N] [--capacity N]
#   psionic/platform/serve.sh --elephant DIR [the same options]
#   psionic/platform/serve.sh --elephant-gpu DIR [the same options]   (CUDA device)
#   psionic/platform/serve.sh --elephant-metal DIR [the same options] (Apple GPU)
#   psionic/platform/serve.sh --mammoth[-gpu] DIR [the same options]
# Original model files are supplied by the caller; nothing is downloaded.
set -euo pipefail
cd "$(dirname "$0")/../.."
build=${PSIONIC_SERVE_BUILD:-${TMPDIR:-/tmp}/psionic-serve-build}
gpu=
for a in "$@"; do if [[ $a == --elephant-gpu || $a == --mammoth-gpu ]]; then gpu=gpu; elif [[ $a == --elephant-metal ]]; then gpu=metal; fi; done
psionic/platform/serve-build.sh "$build" release plain $gpu
args=("$@")
for a in "$@"; do if [[ $a == --elephant || $a == --elephant-gpu || $a == --elephant-metal || $a == --mammoth || $a == --mammoth-gpu ]]; then args+=(--table psionic/fixtures/text/ch05/qwen35.pst); fi; done
if [[ ${PSIONIC_SUPERVISE:-0} == 1 ]];then
    exec "$build/psionic-supervise" -- "$build/psionic-serve" "${args[@]}"
fi
exec "$build/psionic-serve" "${args[@]}"
