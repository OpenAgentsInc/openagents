#!/usr/bin/env bash
# One opt-in full-model capture; normal iteration uses recorded references.
# Usage: run_deepseek_original.sh ORIGINAL_DIRECTORY RECORD_DIRECTORY [TOKEN_LIMIT]
set -euo pipefail
cd "$(dirname "$0")/../.."
[[ $# -ge 2 && $# -le 3 ]] || { echo 'usage: run_deepseek_original.sh ORIGINAL_DIRECTORY RECORD_DIRECTORY [TOKEN_LIMIT]' >&2; exit 2; }
original=$(realpath "$1")
mkdir -p "$2"
record=$(realpath "$2")
limit=${3:-64}
[[ $limit =~ ^[1-9][0-9]{0,4}$ ]] && (( limit <= 16384 )) || {
    echo 'TOKEN_LIMIT must be an integer in 1..16384' >&2; exit 2;
}
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-deepseek-original.XXXXXX")
trap 'rm -rf "$out"' EXIT
cp psionic/fixtures/deepseek/original/r1.pst "$original/r1.pst"
python3 -I psionic/tests/deepseek_fixture.py --header > "$out/deepseek-tiny.h"
cp psionic/fixtures/deepseek/text-goldens.h "$out/text-goldens.h"
"$coax" c psionic/tests/deepseek-runtime.coax --library --header "$out/deepseek.h" --release > "$out/deepseek.c"
"$cc" -std=c11 -O3 -g -ffp-contract=off -fno-fast-math -I"$out" -Ipsionic/platform \
    "$out/deepseek.c" psionic/platform/deepseek-memory.c psionic/tests/deepseek_native.c -lm -o "$out/native"
"$coax" version > "$record/compiler.txt"
"$cc" --version > "$record/cc.txt"
sha256sum "$out/native" "$out/deepseek.c" psionic/fixtures/deepseek/original/qualification-policy.json > "$record/build.sha256"
"$out/native" original "$original" "$record" "$limit" > "$record/run.txt" 2> "$record/progress.txt"
cat "$record/run.txt"
