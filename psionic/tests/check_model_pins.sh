#!/usr/bin/env bash
# Pinned Elephant, Mammoth and nanochat contracts; offline, metadata and a synthetic file only.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax};mutate=0;jobs=4
while [[ $# -gt 0 ]];do case "$1" in --mutate)mutate=1;shift;;--jobs)jobs=$2;shift 2;;*)exit 2;;esac;done
for mode in debug release;do
 flags=();if [[ $mode == release ]];then flags=(--release);fi
 "$coax" test psionic/lib/model-pins.coax "${flags[@]+"${flags[@]}"}" | tail -1
done
coax/web/tool.sh psionic/tools/model-pins.coax check psionic/fixtures/models/pins
python3 -I psionic/tests/model_pins_oracle.py
if [[ $mutate == 1 ]];then "$coax" mutate psionic/lib/model-pins.coax --root-only --jobs "$jobs" | tail -1;fi
