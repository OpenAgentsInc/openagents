#!/usr/bin/env bash
# Build the chapter-20 /v1/systemone host: OUT MODE(debug|release) NATIVE(plain|sanitized) [gpu|metal].
# Coax emits one C library from psionic/lib/serve-proto.coax (which loads
# serve-run.coax); the host links it.
# gpu adds the CUDA engine (--elephant-gpu): the device plan's C library, the
# generated kernels next to the binary, and the CUDA host, loaded at run time.
# metal adds the Apple GPU engine (--elephant-metal, chapter 47): the same plan,
# the generated .metal kernels next to the binary, and the Metal host.
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ $# != 3 && ( $# != 4 || ( $4 != gpu && $4 != metal ) ) ]]; then echo "usage: psionic/platform/serve-build.sh OUT debug|release plain|sanitized [gpu|metal]" >&2; exit 2; fi
out=$1 mode=$2 native=$3 gpu=${4:-}
coax=${COAX:-$PWD/coax/bin/coax}
metal_coax=${COAX_METAL:-$coax}
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
"$cc" "${common[@]}" -c psionic/platform/verified-file.c -o "$out/verified-file.o"
build_id() {
    python3 -I psionic/tests/serve_build_id.py "$out" "$coax" "$cc" "$mode" "$native" "${gpu:-cpu}"
}
if [[ $gpu == gpu ]]; then
    "$coax" c psionic/lib/device/plan.coax --library --release --header "$out/plan.h" > "$out/plan.c"
    for unit in project layer warp; do "$coax" cuda "psionic/lib/device/$unit.coax" --release > "$out/$unit.cu"; done
    "$cc" "${common[@]}" -Wno-missing-field-initializers -c "$out/plan.c" -o "$out/plan.o"
    "$cc" "${common[@]}" -Wno-missing-field-initializers -c psionic/platform/elephant-gpu.c -o "$out/elephant-gpu.o"
    "$cc" "${common[@]}" -Wno-missing-field-initializers -Wno-misleading-indentation -c psionic/platform/cuda-host.c -o "$out/cuda-host.o"
    build_id
    "$cc" "${common[@]}" -Wno-missing-field-initializers -DPSIONIC_SERVE_GPU psionic/platform/serve-host.c psionic/platform/serve-notify.c psionic/platform/serve-busy.c "$out/serve.o" "$out/session.o" "$out/verified-file.o" \
        "$out/plan.o" "$out/elephant-gpu.o" "$out/cuda-host.o" -ldl -lm -o "$out/psionic-serve"
elif [[ $gpu == metal ]]; then
    "$coax" c psionic/lib/device/plan.coax --library --release --header "$out/plan.h" > "$out/plan.c"
    for unit in project layer warp tile; do "$metal_coax" metal-compute "psionic/lib/device/$unit.coax" --release > "$out/$unit.metal"; done
    "$cc" "${common[@]}" -Wno-missing-field-initializers -c "$out/plan.c" -o "$out/plan.o"
    "$cc" "${common[@]}" -Wno-missing-field-initializers -c psionic/platform/elephant-metal.c -o "$out/elephant-metal.o"
    "$cc" -fobjc-arc -mmacosx-version-min=15.0 "$opt" -g -Wall -Wextra -Werror "${sanitizers[@]}" -c psionic/platform/metal-host.m -o "$out/metal-host.o"
    build_id
    "$cc" "${common[@]}" -Wno-missing-field-initializers -DPSIONIC_SERVE_METAL psionic/platform/serve-host.c psionic/platform/serve-notify.c psionic/platform/serve-busy.c "$out/serve.o" "$out/session.o" "$out/verified-file.o" \
        "$out/plan.o" "$out/elephant-metal.o" "$out/metal-host.o" -framework Metal -framework Foundation -lm -o "$out/psionic-serve"
else
    build_id
    "$cc" "${common[@]}" psionic/platform/serve-host.c psionic/platform/serve-notify.c psionic/platform/serve-busy.c "$out/serve.o" "$out/session.o" "$out/verified-file.o" -lm -o "$out/psionic-serve"
fi

"$cc" "${common[@]}" psionic/platform/serve-supervisor.c "$out/serve.o" -lm -o "$out/psionic-supervise"
