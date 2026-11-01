#!/usr/bin/env bash
# Chapter 127: the nanochat backward pass against an independent reverse-mode
# oracle, central differences, coverage, sliding windows and descent steps.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax};mutate=0;jobs=4
while [[ $# -gt 0 ]];do case "$1" in --mutate)mutate=1;shift;;--jobs)jobs=$2;shift 2;;*)exit 2;;esac;done
python3 -I psionic/tests/nanochat_grad_oracle.py 2>/dev/null | cmp - psionic/fixtures/nanochat/grad-goldens.coax
for mode in debug release;do
 flags=();if [[ $mode == release ]];then flags=(--release);fi
 "$coax" test psionic/lib/nanochat/backward.coax "${flags[@]+"${flags[@]}"}"
 "$coax" run psionic/examples/nanochat-gradient.coax "${flags[@]+"${flags[@]}"}" | cmp - psionic/fixtures/nanochat-gradient.out
done
# The 140-position window check takes about a minute; once is enough.
"$coax" test psionic/lib/nanochat/backward-checks.coax --release --only window
if [[ $mutate == 1 ]];then "$coax" mutate psionic/lib/nanochat/backward.coax --root-only --jobs "$jobs";fi
echo 'nanochat backward: oracle gradients, central differences, coverage, windows, descent; example transcript debug/release'
