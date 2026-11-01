#!/usr/bin/env bash
# Chapter 178: measure a replica's start, cold and warm, and record the receipt.
#   psionic/tests/startup.sh [--bytes N] [--elephant DIR [--hash]] [--record]
# Writes a weights file of N bytes (default 256 MiB) under a fresh directory
# on the same disk as the build, so eviction hints reach a real page cache.
# --elephant DIR observes DIR/model.safetensors read-only: verification by its
# existing verified record and a full mapping; --hash also hashes all of it.
# The receipt names the machine, compiler and flags, and is never compared
# byte for byte. --record writes psionic/fixtures/startup/<host>.json.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax};cc=${COAX_CC:-cc};record=0;bytes=268435456;elephant="";hash=0
while [[ $# -gt 0 ]];do case "$1" in
    --record)record=1;shift;;
    --bytes)bytes=$2;shift 2;;
    --elephant)elephant=$2;shift 2;;
    --hash)hash=1;shift;;
    *)exit 2;;esac;done
out=$(mktemp -d "${PSIONIC_STARTUP_DIR:-$HOME/.cache}/psionic-startup.XXXXXX");trap 'rm -rf "$out"' EXIT
common=(-std=c11 -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas)
"$coax" c psionic/lib/startup.coax --library --header "$out/startup.h" > "$out/debug.c"
"$cc" "${common[@]}" -O0 -fsanitize=address,undefined -fno-omit-frame-pointer -I"$out" "$out/debug.c" psionic/platform/startup.c -lm -o "$out/sanitized"
"$coax" c psionic/lib/startup.coax --release --library --header "$out/startup.h" > "$out/release.c"
flags="coax c --release; ${common[*]} -O2"
"$cc" "${common[@]}" -O2 -I"$out" "$out/release.c" psionic/platform/startup.c -lm -o "$out/startup"
# The sanitized build runs the same starts on a 4 MiB file; its timings are discarded.
mkdir "$out/small" "$out/big"
head -c 4194304 /dev/urandom > "$out/small/weights.bin"
small=$(sha256sum "$out/small/weights.bin" | cut -d' ' -f1)
ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/sanitized" "$out/small" 4194304 "$small" sanitized sanitized > "$out/small.json"
python3 -I -c 'import json,sys; json.load(open(sys.argv[1]))' "$out/small.json"
head -c "$bytes" /dev/urandom > "$out/big/weights.bin"
sync -f "$out/big/weights.bin"
sha=$(sha256sum "$out/big/weights.bin" | cut -d' ' -f1)
extra=()
if [[ -n $elephant ]];then
    file="$elephant/model.safetensors"
    st=$(stat -c '%d-%i' "$file")
    extra=("$file" 682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba "${XDG_CACHE_HOME:-$HOME/.cache}/psionic/verified/$st.rec" "$hash")
fi
"$out/startup" "$out/big" "$bytes" "$sha" "$("$cc" --version | head -1)" "$flags" "${extra[@]}" > "$out/receipt.json"
python3 -I -c 'import json,sys; json.load(open(sys.argv[1]))' "$out/receipt.json"
cat "$out/receipt.json"
if [[ $record == 1 ]];then mkdir -p psionic/fixtures/startup;cp "$out/receipt.json" "psionic/fixtures/startup/$(hostname -s).json";fi
