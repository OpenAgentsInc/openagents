#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax, clang, and Metal host runner; holds no logic)
# End-to-end verification gate for Executor W3.1 (hybrid-exec-v1, GitHub #13920):
#   1. Coax unit tests (debug & release) on hybrid-exec-policy, hybrid-lora, hybrid-exec.
#   2. Zero-survivor mutation tests on all three #authority units.
#   3. Pure-Coax runnable verification example (psionic/examples/hybrid-exec.coax).
#   4. Native Apple Silicon Metal 4 build & live paired held-out benchmark on real
#      Inkling-2-Mini-IQ4_NL.gguf + ~/.openagents/modules/ W2.2 dense F64 cores.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-/usr/bin/clang}
if [[ ! -x "$coax" ]]; then
    if [[ -n ${COAX:-} ]]; then echo "COAX is not executable: $coax" >&2; exit 1; fi
    coax/build.sh
fi
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-hybrid-exec.XXXXXX")
trap 'rm -rf "$out"' EXIT

"$coax" version
for mode in debug release; do
    flags=()
    if [[ $mode == release ]]; then flags=(--release); fi
    "$coax" test psionic/lib/hybrid-exec-policy.coax "${flags[@]}"
    "$coax" test psionic/lib/hybrid-lora.coax "${flags[@]}"
    "$coax" test psionic/lib/hybrid-exec.coax "${flags[@]}"
done

"$coax" mutate psionic/lib/hybrid-exec-policy.coax --root-only --native --jobs "${PSIONIC_JOBS:-8}"
"$coax" mutate psionic/lib/hybrid-lora.coax --root-only --native --jobs "${PSIONIC_JOBS:-8}"
"$coax" mutate psionic/lib/hybrid-exec.coax --root-only --native --jobs "${PSIONIC_JOBS:-8}"

"$coax" run psionic/examples/hybrid-exec.coax --release

if [[ $(uname) == Darwin ]]; then
    mkdir -p "$out/obj"
    "$coax" metal-compute psionic/lib/inkling/kernels.coax --release > "$out/inkling-kernels.metal"
    "$coax" c psionic/platform/hybrid-exec-bridge.coax --library --release --header "$out/hx-bridge.h" > "$out/hx-bridge.c"
    cflags=(-mmacosx-version-min=14.0 -std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -I"$out" -Ipsionic/platform)
    "$cc" -fobjc-arc -mmacosx-version-min=14.0 -O2 -Ipsionic/platform -c psionic/platform/metal-host.m -o "$out/obj/metal-host.o"
    "$cc" "${cflags[@]}" -c psionic/platform/inkling-gguf.c -o "$out/obj/inkling-gguf.o"
    "$cc" "${cflags[@]}" -c psionic/platform/inkling-metal.c -o "$out/obj/inkling-metal.o"
    "$cc" "${cflags[@]}" -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -Wno-missing-field-initializers -c "$out/hx-bridge.c" -o "$out/obj/hx-bridge.o"
    "$cc" "${cflags[@]}" -Werror -c psionic/platform/hybrid-exec-host.c -o "$out/obj/hybrid-exec-host.o"
    "$cc" -mmacosx-version-min=14.0 -O2 \
        "$out/obj/metal-host.o" "$out/obj/inkling-gguf.o" "$out/obj/inkling-metal.o" "$out/obj/hx-bridge.o" "$out/obj/hybrid-exec-host.o" \
        -framework Metal -framework Foundation -lm -o "$out/hybrid-exec-metal"

    model="${HX_MODEL:-$HOME/.openagents/models/inkling-2-mini/Inkling-2-Mini-IQ4_NL.gguf}"
    modules="${HX_MODULES:-$HOME/.openagents/modules}"
    if [[ -f "$model" && -d "$modules" ]]; then
        "$out/hybrid-exec-metal" --model "$model" --metal "$out/inkling-kernels.metal" --modules "$modules"
    else
        echo "hybrid-exec-metal built cleanly; set HX_MODEL and HX_MODULES to run live Metal 4 benchmark"
    fi
fi

echo "Executor W3.1 gate passed"
