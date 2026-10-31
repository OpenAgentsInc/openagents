#!/usr/bin/env bash
# Chapter 20 native checks: build psionic-serve in debug and release, plain and
# with ASan/UBSan, and drive each over loopback with psionic/tests/serve.c.
set -euo pipefail
cd "$(dirname "$0")/../.."
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-serve.XXXXXX")
trap 'rm -rf "$out"' EXIT
for mode in debug release; do
    for native in plain sanitized; do
        dir="$out/$mode-$native"
        psionic/platform/serve-build.sh "$dir" "$mode" "$native"
        opt=-O0; if [[ $mode == release ]]; then opt=-O2; fi
        sanitizers=(); if [[ $native == sanitized ]]; then sanitizers=(-fsanitize=address,undefined -fno-omit-frame-pointer); fi
        "$cc" -std=c11 "$opt" -g -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter "${sanitizers[@]}" \
            -I"$dir" -Ipsionic/platform psionic/tests/serve.c "$dir/serve.o" "$dir/prepare.o" "$dir/answer.o" -lm -o "$dir/serve-test"
        mkdir -p "$dir/logs"
        echo "native serve $mode $native:"
        ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1:print_stacktrace=1 "$dir/serve-test" "$dir/psionic-serve" "$dir/logs"
    done
done
