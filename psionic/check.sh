#!/usr/bin/env bash
# Completed Psionic chapters, local/owned machines only. No model downloads or services.
#   psionic/check.sh                      every unit: the release gate
#   psionic/check.sh --only UNIT...       just these units, for the inner loop
#   psionic/check.sh --jobs N ...         N parallel mutants and native builds (default: cores)
# A UNIT is a library path under psionic/lib without .coax (text, model-io/layer-map,
# harmony/admission) or a native harness name (layer-map, rank5, weights). A
# directory name selects every unit in it (harmony). --only runs the oracles,
# the named units' Coax debug/release checks and zero-survivor mutation, their
# native debug/release plain and ASan/UBSan harnesses, and the book check.
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD
only=()
jobs=${PSIONIC_JOBS:-$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)}
while [[ $# -gt 0 ]]; do
    case "$1" in
        --only) shift; while [[ $# -gt 0 && $1 != --* ]]; do only+=("$1"); shift; done
                [[ ${#only[@]} -gt 0 ]] || { echo "--only needs at least one unit" >&2; exit 2; } ;;
        --jobs) [[ $# -ge 2 && $2 =~ ^[1-9][0-9]*$ ]] || { echo "--jobs needs a positive count" >&2; exit 2; }
                jobs=$2; shift 2 ;;
        *) echo "usage: psionic/check.sh [--jobs N] [--only UNIT...]" >&2; exit 2 ;;
    esac
done
coax=${COAX:-$root/coax/bin/coax}
cc=${COAX_CC:-cc}
# A chapter-only request avoids unrelated oracle/package setup.
if [[ ${#only[@]} -eq 1 ]]; then
    chapter=${only[0]%.coax}; chapter=${chapter#psionic/lib/}
    if [[ $chapter == deepseek/front || $chapter == deepseek/front-* ]]; then
        exec env COAX="$coax" COAX_CC="$cc" psionic/tests/check_deepseek_front.sh --mutate --jobs "$jobs"
    fi
    if [[ $chapter == deepseek ]]; then
        env COAX="$coax" COAX_CC="$cc" psionic/tests/check_deepseek.sh --mutate --jobs "$jobs"
        exec env COAX="$coax" COAX_CC="$cc" psionic/tests/check_deepseek_front.sh --mutate --jobs "$jobs"
    fi
    if [[ $chapter == deepseek || $chapter == deepseek/* ]]; then
        exec env COAX="$coax" COAX_CC="$cc" psionic/tests/check_deepseek.sh --mutate --jobs "$jobs"
    fi
fi
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
began=$(date +%s); last=$began
phase() { local t; t=$(date +%s); echo "== $1: $((t - last)) s (total $((t - began)) s)"; last=$t; }
trap 'rm -rf "$out"' EXIT
"$coax" version
python3 -I psionic/tests/ops_oracle.py > "$out/ops-oracle.h"
python3 -I psionic/tests/mxfp4_oracle.py --header > "$out/mxfp4-oracle.h"
python3 -I psionic/tests/moe_oracle.py --header > "$out/moe-oracle.h"
python3 -I psionic/tests/trace_oracle.py --header > "$out/trace-oracle.h"
python3 -I psionic/tests/soft_oracle.py --header > "$out/soft-oracle.h"
python3 -I psionic/tests/gate_oracle.py --header > "$out/gate-oracle.h"
python3 -I psionic/tests/gptoss_oracle.py --header > "$out/gptoss-oracle.h"
"$text_python" -I psionic/tests/harmony_fixtures.py
"$text_python" -I psionic/tests/harmony_oracle.py --header > "$out/harmony-goldens.h"
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
"$text_python" -I psionic/tests/prepare_oracle.py --header > "$out/prepare-golden.h"
coax_units=(trace-machine exact-memory soft-memory relu-arith gate-graph harmony/admission harmony/messages gptoss-math gptoss-attention moe mxfp4 learn tensor ops-contract ops model-io/weights text attention delta qwen-attention qwen-matvec qwen-block qwen-linear-layer qwen-full-layer qwen-decision qwen-prefix qwen-model qwen-tokens qwen-rope qwen-request decision-models decision-prepare decision-session decision-answer readout model-io/access model-io/layer-map model-io/model-map model-io/rank5-bundle decision-calibration decision-json decision-quality)
native_units=(exact-memory soft-memory gate-graph harmony gptoss-attention moe mxfp4 learn tensor ops weights text attention delta qwen-attention qwen-matvec qwen-block qwen-linear-layer qwen-full-layer qwen-decision qwen-prefix qwen-model qwen-tokens qwen-rope qwen-request decision-models decision-prepare decision-session decision-answer readout rank5 rank5-bundle access layer-map model-map decision-calibration decision-json decision-quality)
native_source() {
    case "$1" in
        weights|rank5|rank5-bundle|access|layer-map|model-map) echo "psionic/lib/model-io/$1.coax" ;;
        *) echo "psionic/lib/$1.coax" ;;
    esac
}
native_harness() {
    unit=$1
    case "$unit" in
        harmony) adapter=psionic/platform/harmony-memory.c; harness=psionic/tests/harmony.c ;;
        gptoss-attention) adapter=psionic/platform/gptoss-attention-memory.c; harness=psionic/tests/gptoss_attention.c ;;
        exact-memory) adapter=psionic/platform/trace-memory.c; harness=psionic/tests/trace.c ;;
        soft-memory) adapter=psionic/platform/soft-spans.c; harness=psionic/tests/soft.c ;;
        gate-graph) adapter=psionic/platform/gate-spans.c; harness=psionic/tests/gate.c ;;
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
}
# --only: a name selects the units it equals, contains (a directory), or names
# by basename; a library path also selects the native harness of its file or
# of its top-level directory (harmony/admission -> harmony).
# picks_coax UNIT NAME...: true if a name selects the library unit.
picks_coax() {
    local unit=$1 n; shift
    for n in "$@"; do
        n=${n%.coax}; n=${n#psionic/lib/}
        [[ $unit == "$n" || $unit == "$n"/* || ${unit##*/} == "$n" ]] && return 0
    done
    return 1
}
# picks_native UNIT NAME...: true if a name selects the native harness.
picks_native() {
    local unit=$1 n source; shift
    source=$(native_source "$unit")
    for n in "$@"; do
        n=${n%.coax}; n=${n#psionic/lib/}
        [[ $unit == "$n" || $unit == "${n##*/}" || $unit == "${n%%/*}" || $source == "psionic/lib/$n.coax" ]] && return 0
    done
    return 1
}
selected_coax=(); selected_native=(); gptoss=0; deepseek=0
picks_deepseek() { local n=${1%.coax}; n=${n#psionic/lib/}; [[ $n == deepseek || $n == deepseek/* ]]; }
picks_gptoss() { local n=${1%.coax}; n=${n#psionic/lib/}; [[ $n == gptoss || $n == gptoss/* ]]; }
if [[ ${#only[@]} -eq 0 ]]; then
    selected_coax=("${coax_units[@]}"); selected_native=("${native_units[@]}"); gptoss=1; deepseek=1
else
    for unit in "${coax_units[@]}"; do if picks_coax "$unit" "${only[@]}"; then selected_coax+=("$unit"); fi; done
    for unit in "${native_units[@]}"; do if picks_native "$unit" "${only[@]}"; then selected_native+=("$unit"); fi; done
    for n in "${only[@]}"; do
        hit=0
        if picks_gptoss "$n"; then hit=1; gptoss=1; fi
        if picks_deepseek "$n"; then hit=1; deepseek=1; fi
        for unit in "${coax_units[@]}"; do if picks_coax "$unit" "$n"; then hit=1; fi; done
        for unit in "${native_units[@]}"; do if picks_native "$unit" "$n"; then hit=1; fi; done
        [[ $hit == 1 ]] || { echo "psionic/check.sh: no unit named '$n'" >&2; exit 2; }
    done
fi
phase oracles
echo "units: ${selected_coax[*]+${selected_coax[*]}} | native: ${selected_native[*]+${selected_native[*]}} | jobs $jobs"

# Run "label command..." tasks from stdin, $jobs at a time, and print each
# task's log in input order. Fails if any task failed.
run_parallel() {
    local dir="$out/logs.$1" i=0 failed=0
    mkdir -p "$dir"
    while IFS= read -r line; do
        printf '%s\n' "$line" > "$dir/$i.task"; i=$((i+1))
    done
    [[ $i -gt 0 ]] || return 0
    seq 0 $((i-1)) | xargs -P "$jobs" -I{} bash -c 'bash -euo pipefail -c "$(cat "$1/$2.task")" > "$1/$2.log" 2>&1 || echo $? > "$1/$2.fail"' _ "$dir" {}
    for ((k = 0; k < i; k++)); do
        cat "$dir/$k.log"
        if [[ -f $dir/$k.fail ]]; then echo "FAILED: $(cat "$dir/$k.task")" >&2; failed=1; fi
    done
    return $failed
}
export coax cc out

# Coax checks in both profiles, in parallel across units.
for unit in "${selected_coax[@]+"${selected_coax[@]}"}"; do
    printf '"$coax" test %q\n' "psionic/lib/$unit.coax"
    printf '"$coax" test %q --release\n' "psionic/lib/$unit.coax"
done | run_parallel coax
phase "Coax debug/release checks"
# Mutation: one unit at a time, each using every job.
for unit in "${selected_coax[@]+"${selected_coax[@]}"}"; do
    "$coax" mutate "psionic/lib/$unit.coax" --jobs "$jobs"
done
phase mutation

# Native debug/release harnesses, plain and ASan/UBSan, in parallel.
native_one() {
    local unit=$1 mode=$2 opt=-O0 source dir native
    local flags=()
    if [[ $mode == release ]]; then flags+=(--release); opt=-O2; fi
    source=$(native_source "$unit")
    native_harness "$unit"
    dir="$out/$mode"; mkdir -p "$dir"
    "$coax" c "$source" --library --header "$dir/$unit.h" "${flags[@]+"${flags[@]}"}" > "$dir/$unit.c"
    for native in plain sanitized; do
        sanitizers=()
        if [[ $native == sanitized ]]; then sanitizers=(-fsanitize=address,undefined -fno-omit-frame-pointer); fi
        "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math \
            -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas \
            "${sanitizers[@]+"${sanitizers[@]}"}" -I"$dir" -I"$out" -Ipsionic/platform \
            "$dir/$unit.c" "$adapter" "$harness" -lm -o "$dir/$unit-$native"
        echo "native $unit $mode $native:"
        ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$dir/$unit-$native"
    done
}
export -f native_one native_source native_harness
for mode in debug release; do
    for unit in "${selected_native[@]+"${selected_native[@]}"}"; do
        echo "native_one $unit $mode"
    done
done | run_parallel native
phase "native debug/release, plain and ASan/UBSan"
# The gpt-oss chapters (45-48) have their own runner; --only gptoss selects it.
if [[ $gptoss == 1 ]]; then
    COAX="$coax" COAX_CC="$cc" psionic/tests/check_gptoss.sh
    COAX="$coax" COAX_CC="$cc" psionic/tests/check_gptoss_device.sh
    phase "gpt-oss chapters"
fi

if [[ $deepseek == 1 ]]; then
    COAX="$coax" COAX_CC="$cc" psionic/tests/check_deepseek.sh --mutate --jobs "$jobs"
    COAX="$coax" COAX_CC="$cc" psionic/tests/check_deepseek_front.sh --mutate --jobs "$jobs"
    phase "DeepSeek chapters 56–57"
fi
python3 -I psionic/tests/book.py --coax "$coax" --jobs "$jobs"
phase book
if [[ ${#only[@]} -gt 0 ]]; then
    echo "Psionic units ${only[*]} pass: Coax debug/release checks, zero-survivor mutation, native debug/release, ASan/UBSan and book"
    echo "(--only checks the named units; run psionic/check.sh without --only before release)"
else
    echo "Psionic completed chapters pass: Coax debug/release checks, zero-survivor mutation, native debug/release, ASan/UBSan and book"
fi
