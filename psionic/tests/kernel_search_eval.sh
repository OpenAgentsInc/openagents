#!/usr/bin/env bash
# Chapter 247's evaluator: admit one norm implementation to the CUDA compute
# subset, run chapter 247's cases on the CPU, then on the L4 with timings, and
# keep the whole result in the ledger. The evaluator, the cases and the
# ledger are outside any candidate's writable directory.
#   psionic/tests/kernel_search_eval.sh FILE PREFIX [cpu]
# FILE exports PREFIX_knobs, PREFIX_plan, PREFIX_pass0 and PREFIX_pass1 (see
# psionic/lib/device/norm-search.coax). Candidates under the agent directory
# share a budget of 8 evaluations, failures included. With `cpu` the device
# leg is skipped and nothing is written to the ledger. KS_CONFIRM=1 re-measures
# an already evaluated candidate outside the budget; the entry says so.
set -euo pipefail
cd "$(dirname "$0")/../.."
file=$1 prefix=$2 mode=${3:-device}
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-/usr/bin/cc}
ledger=psionic/kernels/norm/ledger
agent_dir=psionic/kernels/norm/agent
budget=8
vm=${KS_VM:-psionic-l4-elephant} zone=${KS_ZONE:-us-central1-a} project=${KS_PROJECT:-openagentsgemini}
export CLOUDSDK_CONFIG=${CLOUDSDK_CONFIG:-/Users/christopherdavid/work/.secrets/gcloud-sa-config}

[[ $prefix =~ ^[a-z][a-z0-9]{0,7}$ ]] || { echo "refused: prefix must be a lowercase name of up to 8 characters"; exit 2; }
[ -f "$file" ] || { echo "refused: no file $file"; exit 2; }
mkdir -p "$ledger"
if [ "$mode" != cpu ] && [[ $file == $agent_dir/* ]] && [ "${KS_CONFIRM:-0}" != 1 ]; then
    used=$({ grep -l "^source $agent_dir/" "$ledger"/*.log 2>/dev/null || true; } | wc -l | tr -d ' ')
    [ "$used" -lt "$budget" ] || { echo "refused: the agent budget of $budget evaluations is spent"; exit 3; }
fi
gen=$(mktemp -d "${TMPDIR:-/tmp}/kernel-search.XXXXXX")
trap 'rm -rf "$gen"' EXIT
log=$gen/result.log
{
    echo "source $file sha256 $(shasum -a 256 "$file" | cut -d' ' -f1)"
    echo "prefix $prefix"
    [ "${KS_CONFIRM:-0}" = 1 ] && echo "confirmation outside the agent budget"
    echo "when $(TZ=America/Chicago date '+%Y-%m-%d %H:%M:%S %Z')"
    echo "commit $(git rev-parse --short HEAD)"
    echo "evaluator $(cat psionic/lib/kernel-search.coax psionic/tests/kernel_search.c psionic/platform/cuda-host.c | shasum -a 256 | cut -d' ' -f1)"
} > "$log"
finish() {
    cat "$log"
    if [ "$mode" != cpu ]; then
        n=$(ls "$ledger" | wc -l | tr -d ' ')
        cp "$log" "$ledger/$(printf '%03d' $((n + 1)))-$prefix.log"
    fi
}
start=$(date +%s)
if ! "$coax" cuda "$file" --release > "$gen/candidate.cu" 2> "$gen/admission.err"; then
    echo "admission refused: $(grep -m1 'error' "$gen/admission.err")" >> "$log"
    finish; exit 0
fi
for k in knobs plan pass0 pass1; do
    grep -q "__global__ void ${prefix}_$k(" "$gen/candidate.cu" || { echo "admission refused: no kernel ${prefix}_$k" >> "$log"; finish; exit 0; }
done
echo "admission ok: $(grep -c '__global__' "$gen/candidate.cu") kernels" >> "$log"
"$coax" c "$file" --library --release --header "$gen/candidate.h" > "$gen/candidate.c"
"$coax" c psionic/lib/kernel-search.coax --library --release --header "$gen/kernel-search.h" > "$gen/kernel-search.c"
cp psionic/tests/kernel_search.c psionic/platform/cuda-host.c psionic/platform/cuda-host.h "$gen/"
flags="-std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -Wno-missing-field-initializers"
if ! "$cc" $flags -DPFX="$prefix" -I"$gen" "$gen/candidate.c" "$gen/kernel-search.c" "$gen/cuda-host.c" "$gen/kernel_search.c" \
    -ldl -lm -o "$gen/kernel-search" 2> "$gen/build.err"; then
    echo "build refused: $(grep -m1 'error' "$gen/build.err")" >> "$log"
    finish; exit 0
fi
"$gen/kernel-search" cpu >> "$log"
if grep -q '^cpu .* ok$' "$log" && [ "$mode" != cpu ]; then
    COPYFILE_DISABLE=1 tar -C "$gen" -czf "$gen/ship.tgz" candidate.cu candidate.c candidate.h kernel-search.c kernel-search.h cuda-host.c cuda-host.h kernel_search.c
    remote="ks-$prefix-$$"
    gcloud compute scp "$gen/ship.tgz" "$vm:/tmp/$remote.tgz" --zone "$zone" --project "$project" --tunnel-through-iap --quiet > /dev/null 2>&1
    gcloud compute ssh "$vm" --zone "$zone" --project "$project" --tunnel-through-iap --quiet --command "set -e
        mkdir -p /tmp/$remote && tar -C /tmp/$remote -xzf /tmp/$remote.tgz && cd /tmp/$remote
        gcc $flags -Wno-misleading-indentation -DPFX=$prefix -I. candidate.c kernel-search.c cuda-host.c kernel_search.c -ldl -lm -o kernel-search
        PSIONIC_NVRTC=\$HOME/venv/lib/python3.12/site-packages/nvidia/cu13/lib/libnvrtc.so.13 ./kernel-search device candidate.cu
        cd /tmp && rm -rf /tmp/$remote /tmp/$remote.tgz" 2> "$gen/remote.err" >> "$log" || echo "device run failed: $(grep -v -i 'numpy\|tunnel\|^$\|WARNING' "$gen/remote.err" | tail -1)" >> "$log"
fi
echo "seconds $(( $(date +%s) - start ))" >> "$log"
finish
