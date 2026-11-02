#!/usr/bin/env bash
# Chapter 183: a decoder split over host threads. Checks in debug and
# release, the example transcripts, the independent oracle's constants, a
# short run under ThreadSanitizer, then the release harness: 1, 2, 4 and 8
# devices against tl_run bit for bit and chapter 113's forward within the
# frozen tolerance, and a speed receipt.
# Usage: psionic/tests/check_tensor_parallel.sh [--mutate] [REPEATS]
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
mutate=0
if [[ ${1:-} == --mutate ]]; then mutate=1; shift; fi
repeats=${1:-10}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-tensor-parallel.XXXXXX")
trap 'rm -rf "$out"' EXIT
plan=psionic/lib/tensor-parallel.coax
unit=psionic/lib/tensor-parallel-run.coax
python3 -I psionic/tests/decoder_oracle.py --split | cmp - <(grep -E '^TP_ORACLE_LOGITS ::' psionic/tests/tensor-parallel-checks.coax)
for file in "$plan" "$unit"; do
    "$coax" test "$file"
    "$coax" test "$file" --release
done
for name in split-model split-model-refuse; do
    for mode in "" --release; do
        "$coax" run "psionic/examples/$name.coax" $mode > "$out/$name.out"
        cmp "psionic/fixtures/$name.out" "$out/$name.out"
    done
done
flags=(-std=c11 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -Wno-missing-field-initializers -Wno-misleading-indentation)
"$coax" c "$unit" --library --release --header "$out/tensor-parallel.h" > "$out/tensor-parallel.c"
"$cc" "${flags[@]}" -O1 -g -fsanitize=thread -I"$out" "$out/tensor-parallel.c" psionic/tests/tensor_parallel_threads.c -lm -pthread -o "$out/threads-tsan"
TSAN_OPTIONS=halt_on_error=1 "$out/threads-tsan" 1 4 > /dev/null
"$cc" "${flags[@]}" -O2 -I"$out" "$out/tensor-parallel.c" psionic/tests/tensor_parallel_threads.c -lm -pthread -o "$out/threads"
"$out/threads" "$repeats"
if [[ $mutate == 1 ]]; then
    "$coax" mutate "$plan" --root-only
    "$coax" mutate "$unit" --root-only
fi
echo "chapter 183 passes"
