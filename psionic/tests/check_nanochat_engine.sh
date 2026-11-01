#!/usr/bin/env bash
# Chapter 126: nanochat text to text (render for completion, prefill, cached
# generation through chapter 53's sampler, decode) against an independent oracle.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax};mutate=0;jobs=4
while [[ $# -gt 0 ]];do case "$1" in --mutate)mutate=1;shift;;--jobs)jobs=$2;shift 2;;*)exit 2;;esac;done
# The oracle tokenizes with Unicode 17.0.0 classes.
py=${PSIONIC_TEXT_PYTHON:-}
if [[ -z $py ]];then for candidate in python3 python3.15;do
 if command -v "$candidate" >/dev/null && [[ $("$candidate" -I -c 'import unicodedata; print(unicodedata.unidata_version)') == 17.0.0 ]];then py=$candidate;break;fi
done;fi
if [[ -z $py ]];then echo "nanochat engine: needs a Python with Unicode 17.0.0 (set PSIONIC_TEXT_PYTHON)" >&2;exit 1;fi
"$py" -I psionic/tests/nanochat_engine_oracle.py
for mode in debug release;do
 flags=();if [[ $mode == release ]];then flags=(--release);fi
 "$coax" test psionic/lib/nanochat/engine.coax "${flags[@]+"${flags[@]}"}" | tail -1
 "$coax" test psionic/lib/nanochat/engine-checks.coax --steps 400000000 "${flags[@]+"${flags[@]}"}" | tail -1
 "$coax" run psionic/examples/nanochat-reply.coax "${flags[@]+"${flags[@]}"}" | cmp - psionic/fixtures/nanochat-reply.out
done
if [[ $mutate == 1 ]];then "$coax" mutate psionic/lib/nanochat/engine.coax --root-only --jobs "$jobs" | tail -1;fi
echo 'nanochat engine: oracle prompts, replies, stops and text; full-pass replay of every pick; example transcript debug/release'
