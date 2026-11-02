#!/usr/bin/env bash
# Chapter 100: V4.1 Flash on one machine. The header manifest against the
# config, the host byte plan, and the composed 40-layer stack against the
# composed oracle. With DIR (from deepseek_v41_manifest.py --fetch DIR) the
# fetched headers are rechecked against their pins too.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
mutate=0;jobs=${COAX_MUTATE_JOBS:-8};headers=
while [[ $# -gt 0 ]];do
    case "$1" in
        --mutate) mutate=1;shift ;;
        --jobs) jobs=$2;shift 2 ;;
        --headers) headers=$2;shift 2 ;;
        *) echo 'usage: check_deepseek_flash.sh [--mutate] [--jobs N] [--headers DIR]' >&2;exit 2 ;;
    esac
done
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-flash.XXXXXX");trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/deepseek_flash_oracle.py
if [[ -n $headers ]];then python3 -I psionic/tests/deepseek_v41_manifest.py "$headers";fi
for mode in debug release;do
    flags=(--run-steps 40000000)
    if [[ $mode == release ]];then flags+=(--release);fi
    for unit in flash-manifest flash-plan flash flash-split flash-stages;do "$coax" test "psionic/lib/deepseek/$unit.coax" "${flags[@]}";done
    for example in deepseek-flash deepseek-flash-refuse deepseek-flash-stages;do
        "$coax" run "psionic/examples/$example.coax" "${flags[@]}" > "$out/$example.out"
        cmp "psionic/fixtures/deepseek-flash/$example.out" "$out/$example.out"
    done
done
if [[ $mutate == 1 ]];then
    for unit in flash-manifest flash-plan flash flash-split;do "$coax" mutate "psionic/lib/deepseek/$unit.coax" --root-only --jobs "$jobs" --run-steps 40000000;done
fi
echo 'V4.1 Flash: manifest, host plan, composed stack against the composed oracle, and both transcripts pass'
