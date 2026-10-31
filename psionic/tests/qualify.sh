#!/usr/bin/env bash
# Chapter 13 checks. No model files, downloads or services.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-qualify.XXXXXX")
trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/qualify_oracle.py
for unit in qualify qualify-elephant; do
    "$coax" test "psionic/lib/$unit.coax"
    "$coax" test "psionic/lib/$unit.coax" --release
    "$coax" mutate "psionic/lib/$unit.coax"
done
# Each case runs as its own process; its exit status must be the verdict's,
# and its output must be the book's transcript for that case.
expected=(2 0 3 1 1 2 1 1)
for mode in debug release; do
    flags=(); opt=-O0
    if [[ $mode == release ]]; then flags+=(--release); opt=-O2; fi
    "$coax" c psionic/lib/qualify-gate.coax --library --header "$out/qualify-gate.h" "${flags[@]}" > "$out/qualify-gate.c"
    for native in plain sanitized; do
        sanitizers=()
        if [[ $native == sanitized ]]; then sanitizers=(-fsanitize=address,undefined -fno-omit-frame-pointer); fi
        "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
            -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter "${sanitizers[@]}" -I"$out" \
            "$out/qualify-gate.c" psionic/tests/qualify.c -lm -o "$out/gate"
        : > "$out/transcript"
        for k in "${!expected[@]}"; do
            set +e
            ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/gate" "$k" >> "$out/transcript" 2> "$out/stderr"
            status=$?
            set -e
            if [[ $status != "${expected[$k]}" || -s "$out/stderr" ]]; then
                echo "qualify gate $mode $native case $k: exit status $status, expected ${expected[$k]}" >&2; cat "$out/stderr" >&2; exit 1
            fi
            printf 'exit status %s\n\n' "$status" >> "$out/transcript"
        done
        cmp "$out/transcript" psionic/fixtures/qualify-elephant.out
        for bad in "" 8 -1 x 1x; do
            set +e; "$out/gate" $bad > /dev/null 2>&1; status=$?; set -e
            if [[ $status != 2 ]]; then echo "qualify gate refused argument '$bad' with $status" >&2; exit 1; fi
        done
        echo "native qualify gate $mode $native: 8 exit statuses and transcript match"
    done
done
echo "chapter 17 passes: oracle, Coax debug/release checks, zero-survivor mutation, native gate"
