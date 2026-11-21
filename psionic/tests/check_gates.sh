#!/usr/bin/env bash
# Focused gate for chapters 228 to 230; no models, services or GPU needed.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
if [[ ! -x "$coax" ]]; then
    if [[ -n ${COAX:-} ]]; then echo "COAX is not executable: $coax" >&2; exit 1; fi
    coax/build.sh
fi
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-gates.XXXXXX")
trap 'rm -rf "$out"' EXIT
"$coax" version
python3 -I psionic/tests/gate_oracle.py
python3 -I psionic/tests/gate_oracle.py --header > "$out/gate-oracle.h"
python3 -I psionic/tests/gate_oracle.py --harness-header > "$out/harness-oracle.h"
for mode in debug release; do
    flags=(); opt=-O0
    if [[ $mode == release ]]; then flags=(--release); opt=-O2; fi
    "$coax" test psionic/lib/relu-arith.coax "${flags[@]}"
    "$coax" test psionic/lib/gate-graph.coax "${flags[@]}"
    "$coax" test psionic/lib/compiler-harness.coax "${flags[@]}"
    "$coax" test psionic/lib/exec-vm-core.coax "${flags[@]}"
    "$coax" test psionic/lib/exec-vm.coax "${flags[@]}"
    "$coax" test psionic/lib/exec-vm-harness.coax "${flags[@]}"
    "$coax" test psionic/lib/exec-vm-programs.coax "${flags[@]}"
    "$coax" test psionic/lib/hull-kv.coax "${flags[@]}"
    "$coax" test psionic/lib/wasm-isa.coax "${flags[@]}"
    "$coax" test psionic/lib/wasm-transformer.coax "${flags[@]}"
    "$coax" test psionic/lib/wasm-transformer-benchmarks.coax "${flags[@]}"
    "$coax" c psionic/lib/gate-graph.coax --library --header "$out/gate-graph.h" "${flags[@]}" > "$out/gate-graph.c"
    "$coax" c psionic/lib/compiler-harness.coax --library --header "$out/compiler-harness.h" "${flags[@]}" > "$out/compiler-harness.c"
    for native in plain sanitized; do
        san=()
        if [[ $native == sanitized ]]; then san=(-fsanitize=address,undefined -fno-omit-frame-pointer); fi
        "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math \
            -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter \
            "${san[@]}" -I"$out" -Ipsionic/platform "$out/gate-graph.c" \
            psionic/platform/gate-spans.c psionic/tests/gate.c -lm -o "$out/native"
        echo "Gate native $mode $native:"
        ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native"
        "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math \
            -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter \
            "${san[@]}" -I"$out" -Ipsionic/platform "$out/compiler-harness.c" \
            psionic/platform/harness-memory.c psionic/tests/harness.c -lm -o "$out/harness"
        echo "Harness native $mode $native:"
        ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/harness"
    done
    for example in relu-gadgets gate-block catch-the-compiler; do
        "$coax" run "psionic/examples/$example.coax" "${flags[@]}" > "$out/example.out"
        cmp "psionic/fixtures/$example.out" "$out/example.out"
    done
done
python3 -I psionic/tests/exec_vm_oracle.py
"$coax" run psionic/examples/exec-vm.coax --release
python3 -I psionic/tests/wasm_exec_oracle.py
"$coax" run psionic/examples/wasm-exec.coax --release
"$coax" mutate psionic/lib/relu-arith.coax --jobs "${PSIONIC_JOBS:-8}"
"$coax" mutate psionic/lib/gate-graph.coax --jobs "${PSIONIC_JOBS:-8}"
"$coax" mutate psionic/lib/compiler-harness.coax --jobs "${PSIONIC_JOBS:-8}"
"$coax" mutate psionic/lib/exec-vm-core.coax --root-only --native --jobs "${PSIONIC_JOBS:-8}"
"$coax" mutate psionic/lib/exec-vm.coax --root-only --native --jobs "${PSIONIC_JOBS:-8}"
"$coax" mutate psionic/lib/exec-vm-harness.coax --root-only --native --jobs "${PSIONIC_JOBS:-8}"
"$coax" mutate psionic/lib/hull-kv.coax --root-only --native --jobs "${PSIONIC_JOBS:-8}"
"$coax" mutate psionic/lib/wasm-isa.coax --root-only --native --jobs "${PSIONIC_JOBS:-8}"
COAX_NATIVE_CHECKS_OPT=1 "$coax" mutate psionic/lib/wasm-transformer.coax --root-only --native --jobs "${PSIONIC_JOBS:-8}"
echo "Gate chapters pass: independent oracles, debug/release, ASan/UBSan, examples and zero-survivor mutation"
