#!/usr/bin/env bash
# Packing, checkpoint records and exact resume (psionic-resume-v1). CPU only.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax};cc=${COAX_CC:-cc};mutate=0;jobs=4
while [[ $# -gt 0 ]];do case "$1" in --mutate)mutate=1;shift;;--jobs)jobs=$2;shift 2;;*)exit 2;;esac;done
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-resume.XXXXXX");trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/resume_oracle.py
python3 -I psionic/tests/resume_oracle.py --header > "$out/resume-oracle.h"
units=(psionic/lib/data/pack.coax psionic/lib/checkpoint/record.coax psionic/lib/checkpoint/resume.coax)
for mode in debug release;do
 flags=();opt=-O0;if [[ $mode == release ]];then flags+=(--release);opt=-O3;fi
 for u in "${units[@]}";do "$coax" test "$u" "${flags[@]+"${flags[@]}"}";done
 "$coax" c psionic/lib/checkpoint/resume.coax --library --header "$out/resume.h" "${flags[@]+"${flags[@]}"}" > "$out/resume-$mode.c"
 for kind in plain sanitized;do
  san=();if [[ $kind == sanitized ]];then san=(-fsanitize=address,undefined -fno-omit-frame-pointer);fi
  "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas "${san[@]+"${san[@]}"}" -I"$out" -Ipsionic/platform "$out/resume-$mode.c" psionic/platform/checkpoint-files.c psionic/tests/resume_native.c -lm -o "$out/native-$mode-$kind"
  mkdir "$out/run-$mode-$kind"
  ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native-$mode-$kind" "$out/run-$mode-$kind"
 done
done
"$coax" run psionic/examples/resume.coax > "$out/example.out";cmp psionic/fixtures/resume.out "$out/example.out"
"$coax" run psionic/examples/resume.coax --release > "$out/example-release.out";cmp psionic/fixtures/resume.out "$out/example-release.out"
if [[ $mutate == 1 ]];then for u in "${units[@]}";do "$coax" mutate "$u" --root-only --jobs "$jobs";done;fi
python3 -I psionic/tests/book.py
