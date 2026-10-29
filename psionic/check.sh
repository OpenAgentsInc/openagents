#!/usr/bin/env bash
# Chapters 1–8, local/owned machines only. No model downloads or services.
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD
coax=${COAX:-$root/coax/bin/coax}
cc=${COAX_CC:-cc}
if [[ ! -x "$coax" ]]; then
    if [[ -n ${COAX:-} ]]; then echo "COAX is not executable: $coax" >&2; exit 1; fi
    coax/build.sh
fi
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-check.XXXXXX")
trap 'rm -rf "$out"' EXIT
"$coax" version
python3 -I psionic/tests/ops_oracle.py > "$out/ops-oracle.h"
python3 -I psionic/tests/weights_fixtures.py --header > "$out/weights-fixtures.h"
# The weights root checks/mutates every imported model-io unit plus JSON/SHA-256.
python3 -I psionic/tests/text_fixtures.py
python3 -I psionic/tests/text_oracle.py --header > "$out/text-goldens.h"
python3 -I psionic/tests/attention_oracle.py --header > "$out/attention-oracle.h"
python3 -I psionic/tests/delta_oracle.py --header > "$out/delta-oracle.h"
python3 -I psionic/tests/qwen_attention_oracle.py --header > "$out/qwen-attention-oracle.h"
python3 -I psionic/tests/qwen_block_oracle.py --header > "$out/qwen-block-oracle.h"
python3 -I psionic/tests/qwen_linear_oracle.py --header > "$out/qwen-linear-oracle.h"
python3 -I psionic/tests/qwen_full_oracle.py --header > "$out/qwen-full-oracle.h"
python3 -I psionic/tests/qwen_decision_oracle.py --header > "$out/qwen-decision-oracle.h"
for unit in learn tensor ops-contract ops model-io/weights text attention delta qwen-attention qwen-matvec qwen-block qwen-linear-layer qwen-full-layer qwen-decision qwen-prefix; do
    "$coax" test "psionic/lib/$unit.coax"
    "$coax" test "psionic/lib/$unit.coax" --release
    "$coax" mutate "psionic/lib/$unit.coax"
done
for mode in debug release; do
    flags=()
    opt=-O0
    if [[ $mode == release ]]; then flags+=(--release); opt=-O2; fi
    for unit in learn tensor ops weights text attention delta qwen-attention qwen-matvec qwen-block qwen-linear-layer qwen-full-layer qwen-decision qwen-prefix; do
        source="psionic/lib/$unit.coax"
        if [[ $unit == weights ]]; then source=psionic/lib/model-io/weights.coax; fi
        "$coax" c "$source" --library --header "$out/$unit.h" "${flags[@]}" > "$out/$unit.c"
        case "$unit" in
            learn) adapter=psionic/platform/memory.c; harness=psionic/tests/native.c ;;
            tensor) adapter=psionic/platform/tensor-memory.c; harness=psionic/tests/tensor.c ;;
            ops) adapter=psionic/platform/ops-memory.c; harness=psionic/tests/ops.c ;;
            weights) adapter=psionic/platform/weights-memory.c; harness=psionic/tests/weights.c ;;
            text) adapter=psionic/platform/text-memory.c; harness=psionic/tests/text.c ;;
            attention) adapter=psionic/platform/attention-memory.c; harness=psionic/tests/attention.c ;;
            delta) adapter=psionic/platform/delta-memory.c; harness=psionic/tests/delta.c ;;
            qwen-attention) adapter=psionic/platform/qwen-attention-memory.c; harness=psionic/tests/qwen_attention.c ;;
            qwen-matvec) adapter=psionic/platform/qwen-matvec-memory.c; harness=psionic/tests/qwen_matvec.c ;;
            qwen-block) adapter=psionic/platform/qwen-block-memory.c; harness=psionic/tests/qwen_block.c ;;
            qwen-linear-layer) adapter=psionic/platform/qwen-linear-layer-memory.c; harness=psionic/tests/qwen_linear.c ;;
            qwen-full-layer) adapter=psionic/platform/qwen-full-layer-memory.c; harness=psionic/tests/qwen_full.c ;;
            qwen-decision) adapter=psionic/platform/qwen-decision-memory.c; harness=psionic/tests/qwen_decision.c ;;
            qwen-prefix) adapter=psionic/platform/qwen-prefix-memory.c; harness=psionic/tests/qwen_prefix.c ;;
        esac
        for native in plain sanitized; do
            sanitizers=()
            if [[ $native == sanitized ]]; then sanitizers=(-fsanitize=address,undefined -fno-omit-frame-pointer); fi
            "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math \
                -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter \
                "${sanitizers[@]}" -I"$out" -Ipsionic/platform \
                "$out/$unit.c" "$adapter" "$harness" -lm -o "$out/$unit-native"
            echo "native $unit $mode $native:"
            ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/$unit-native"
        done
    done
done
python3 -I psionic/tests/book.py --coax "$coax"
echo "Psionic chapters 1–8 pass: Coax debug/release checks, zero-survivor mutation, native debug/release, ASan/UBSan and book"
