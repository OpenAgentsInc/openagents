#!/usr/bin/env bash
# Completed Psionic chapters, local/owned machines only. No model downloads or services.
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD
coax=${COAX:-$root/coax/bin/coax}
cc=${COAX_CC:-cc}
# The text fixtures pin Unicode 17.0.0; use a Python whose database matches.
text_python=${PSIONIC_TEXT_PYTHON:-}
if [[ -z $text_python ]]; then
    for candidate in python3 python3.15; do
        if command -v "$candidate" >/dev/null && [[ $("$candidate" -I -c 'import unicodedata; print(unicodedata.unidata_version)') == 17.0.0 ]]; then
            text_python=$candidate; break
        fi
    done
fi
if [[ -z $text_python ]]; then echo "Psionic text checks need a Python with Unicode 17.0.0 (set PSIONIC_TEXT_PYTHON)" >&2; exit 1; fi
if [[ ! -x "$coax" ]]; then
    if [[ -n ${COAX:-} ]]; then echo "COAX is not executable: $coax" >&2; exit 1; fi
    coax/build.sh
fi
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-check.XXXXXX")
trap 'rm -rf "$out"' EXIT
"$coax" version
python3 -I psionic/tests/ops_oracle.py > "$out/ops-oracle.h"
python3 -I psionic/tests/mxfp4_oracle.py --header > "$out/mxfp4-oracle.h"
python3 -I psionic/tests/moe_oracle.py --header > "$out/moe-oracle.h"
python3 -I psionic/tests/gptoss_oracle.py --header > "$out/gptoss-oracle.h"
python3 -I psionic/tests/harmony_fixtures.py
python3 -I psionic/tests/harmony_oracle.py --header > "$out/harmony-goldens.h"
python3 -I psionic/tests/weights_fixtures.py --header > "$out/weights-fixtures.h"
# The weights root checks/mutates every imported model-io unit plus JSON/SHA-256.
"$text_python" -I psionic/tests/text_fixtures.py
"$text_python" -I psionic/tests/text_oracle.py --header > "$out/text-goldens.h"
python3 -I psionic/tests/attention_oracle.py --header > "$out/attention-oracle.h"
python3 -I psionic/tests/delta_oracle.py --header > "$out/delta-oracle.h"
python3 -I psionic/tests/qwen_attention_oracle.py --header > "$out/qwen-attention-oracle.h"
python3 -I psionic/tests/qwen_block_oracle.py --header > "$out/qwen-block-oracle.h"
python3 -I psionic/tests/qwen_linear_oracle.py --header > "$out/qwen-linear-oracle.h"
python3 -I psionic/tests/qwen_full_oracle.py --header > "$out/qwen-full-oracle.h"
python3 -I psionic/tests/qwen_decision_oracle.py --header > "$out/qwen-decision-oracle.h"
python3 -I psionic/tests/readout_oracle.py
python3 -I psionic/tests/answer_oracle.py
python3 -I psionic/tests/quality_corpus.py
python3 -I psionic/tests/qwen_model_oracle.py
python3 -I psionic/tests/qwen_tokens_oracle.py
python3 -I psionic/tests/elephant_layer0_fixture.py
python3 -I psionic/tests/elephant_reference_fixture.py
python3 -I psionic/tests/elephant_reference_fixture.py --check-output psionic/fixtures/models/elephant/psionic-reference.txt
python3 -I psionic/tests/session_fixture.py
python3 -I psionic/tests/prepare_oracle.py --header > "$out/prepare-golden.h"
for unit in harmony/admission harmony/messages gptoss-math gptoss-attention moe mxfp4 learn tensor ops-contract ops model-io/weights text attention delta qwen-attention qwen-matvec qwen-block qwen-linear-layer qwen-full-layer qwen-decision qwen-prefix qwen-model qwen-tokens qwen-rope qwen-request decision-models decision-prepare decision-session decision-answer readout model-io/access model-io/layer-map model-io/model-map model-io/rank5-bundle decision-calibration decision-json decision-quality; do
    "$coax" test "psionic/lib/$unit.coax"
    "$coax" test "psionic/lib/$unit.coax" --release
    "$coax" mutate "psionic/lib/$unit.coax"
done
for mode in debug release; do
    flags=()
    opt=-O0
    if [[ $mode == release ]]; then flags+=(--release); opt=-O2; fi
    for unit in harmony gptoss-attention moe mxfp4 learn tensor ops weights text attention delta qwen-attention qwen-matvec qwen-block qwen-linear-layer qwen-full-layer qwen-decision qwen-prefix qwen-model qwen-tokens qwen-rope qwen-request decision-models decision-prepare decision-session decision-answer readout rank5 rank5-bundle access layer-map model-map decision-calibration decision-json decision-quality; do
        source="psionic/lib/$unit.coax"
        if [[ $unit == weights ]]; then source=psionic/lib/model-io/weights.coax; fi
        if [[ $unit == rank5 ]]; then source=psionic/lib/model-io/rank5.coax; fi
        if [[ $unit == rank5-bundle ]]; then source=psionic/lib/model-io/rank5-bundle.coax; fi
        if [[ $unit == access ]]; then source=psionic/lib/model-io/access.coax; fi
        if [[ $unit == layer-map ]]; then source=psionic/lib/model-io/layer-map.coax; fi
        if [[ $unit == model-map ]]; then source=psionic/lib/model-io/model-map.coax; fi
        "$coax" c "$source" --library --header "$out/$unit.h" "${flags[@]}" > "$out/$unit.c"
        case "$unit" in
            harmony) adapter=psionic/platform/harmony-memory.c; harness=psionic/tests/harmony.c ;;
            gptoss-attention) adapter=psionic/platform/gptoss-attention-memory.c; harness=psionic/tests/gptoss_attention.c ;;
            moe) adapter=psionic/platform/moe-memory.c; harness=psionic/tests/moe.c ;;
            mxfp4) adapter=psionic/platform/mxfp4-memory.c; harness=psionic/tests/mxfp4.c ;;
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
            qwen-model) adapter=psionic/platform/qwen-model-memory.c; harness=psionic/tests/qwen_model.c ;;
            qwen-tokens) adapter=psionic/platform/qwen-tokens-memory.c; harness=psionic/tests/qwen_tokens.c ;;
            qwen-rope) adapter=psionic/platform/qwen-rope-memory.c; harness=psionic/tests/qwen_rope.c ;;
            qwen-request) adapter=psionic/platform/qwen-request-memory.c; harness=psionic/tests/qwen_request.c ;;
            decision-models) adapter=psionic/platform/decision-models-memory.c; harness=psionic/tests/decision_models.c ;;
            decision-prepare) adapter=psionic/platform/decision-prepare-memory.c; harness=psionic/tests/decision_prepare.c ;;
            decision-session) adapter=psionic/platform/decision-session-memory.c; harness=psionic/tests/decision_session.c ;;
            decision-answer) adapter=psionic/platform/decision-answer-memory.c; harness=psionic/tests/decision_answer.c ;;
            readout) adapter=psionic/platform/readout-memory.c; harness=psionic/tests/readout.c ;;
            rank5) adapter=psionic/platform/rank5-memory.c; harness=psionic/tests/rank5.c ;;
            rank5-bundle) adapter=psionic/platform/rank5-bundle-memory.c; harness=psionic/tests/rank5_bundle.c ;;
            access) adapter=psionic/platform/access-memory.c; harness=psionic/tests/access.c ;;
            layer-map) adapter=psionic/platform/layer-map-memory.c; harness=psionic/tests/layer_map.c ;;
            model-map) adapter=psionic/platform/model-map-memory.c; harness=psionic/tests/model_map.c ;;
            decision-calibration) adapter=psionic/platform/decision-calibration-memory.c; harness=psionic/tests/decision_calibration.c ;;
            decision-json) adapter=psionic/platform/decision-json-memory.c; harness=psionic/tests/decision_json.c ;;
            decision-quality) adapter=psionic/platform/decision-quality-memory.c; harness=psionic/tests/decision_quality.c ;;
        esac
        for native in plain sanitized; do
            sanitizers=()
            if [[ $native == sanitized ]]; then sanitizers=(-fsanitize=address,undefined -fno-omit-frame-pointer); fi
            "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math \
                -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas \
                "${sanitizers[@]}" -I"$out" -Ipsionic/platform \
                "$out/$unit.c" "$adapter" "$harness" -lm -o "$out/$unit-native"
            echo "native $unit $mode $native:"
            ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/$unit-native"
        done
    done
done
python3 -I psionic/tests/book.py --coax "$coax"
echo "Psionic completed chapters pass: Coax debug/release checks, zero-survivor mutation, native debug/release, ASan/UBSan and book"
