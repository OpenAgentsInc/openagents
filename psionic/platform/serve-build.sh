#!/usr/bin/env bash
# Build the chapter-16 /v1/systemone host: OUT MODE(debug|release) NATIVE(plain|sanitized) [gpu].
# Coax emits one C library from psionic/lib/serve-proto.coax (which loads
# serve-run.coax); the host links it.
# gpu adds the CUDA engine (--elephant-gpu): the device plan's C library, the
# generated kernels next to the binary, and the CUDA host, loaded at run time.
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ $# != 3 && ( $# != 4 || $4 != gpu ) ]]; then echo "usage: psionic/platform/serve-build.sh OUT debug|release plain|sanitized [gpu]" >&2; exit 2; fi
out=$1 mode=$2 native=$3 gpu=${4:-}
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
mkdir -p "$out"
flags=() opt=-O0 sanitizers=()
if [[ $mode == release ]]; then flags+=(--release); opt=-O2; elif [[ $mode != debug ]]; then exit 2; fi
if [[ $native == sanitized ]]; then sanitizers=(-fsanitize=address,undefined -fno-omit-frame-pointer); elif [[ $native != plain ]]; then exit 2; fi
"$coax" c psionic/lib/serve-proto.coax --library --header "$out/serve.h" "${flags[@]}" > "$out/serve.c"
for unit in decision-session decision-prepare decision-answer; do printf '#include "serve.h"\n' > "$out/$unit.h"; done
python3 -I psionic/tests/elephant_manifest.py --header > "$out/elephant-inventory.h"
common=(-std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter
    -Wno-unknown-pragmas "${sanitizers[@]}" -I"$out" -Ipsionic/platform)
"$cc" "${common[@]}" -c "$out/serve.c" -o "$out/serve.o"
"$cc" "${common[@]}" -c psionic/platform/decision-session-memory.c -o "$out/session.o"
"$cc" "${common[@]}" -c psionic/platform/decision-prepare-memory.c -o "$out/prepare.o"
"$cc" "${common[@]}" -c psionic/platform/decision-answer-memory.c -o "$out/answer.o"
if [[ $gpu == gpu ]]; then
    "$coax" c psionic/lib/device/plan.coax --library --release --header "$out/plan.h" > "$out/plan.c"
    for unit in project layer warp; do "$coax" cuda "psionic/lib/device/$unit.coax" --release > "$out/$unit.cu"; done
    "$cc" "${common[@]}" -Wno-missing-field-initializers -c "$out/plan.c" -o "$out/plan.o"
    "$cc" "${common[@]}" -Wno-missing-field-initializers -c psionic/platform/elephant-gpu.c -o "$out/elephant-gpu.o"
    "$cc" "${common[@]}" -Wno-missing-field-initializers -c psionic/platform/cuda-host.c -o "$out/cuda-host.o"
    "$cc" "${common[@]}" -Wno-missing-field-initializers -DPSIONIC_SERVE_GPU psionic/platform/serve-host.c "$out/serve.o" "$out/session.o" \
        "$out/plan.o" "$out/elephant-gpu.o" "$out/cuda-host.o" -ldl -lm -o "$out/psionic-serve"
else
    "$cc" "${common[@]}" psionic/platform/serve-host.c "$out/serve.o" "$out/session.o" -lm -o "$out/psionic-serve"
fi
