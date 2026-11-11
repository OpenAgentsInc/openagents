#!/usr/bin/env bash
# The Microcoder tools against a scratch folder with three Swift files.
#   coax/lib/microcoder/tools_smoke.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../../.." && pwd)"
if [[ "$(uname -s)" == Darwin ]]; then cc="${COAX_CC:-/usr/bin/clang}"; else cc="${COAX_CC:-${CC:-gcc}} -D_DEFAULT_SOURCE"; fi; coax="${COAX:-$repo/coax/bin/coax}"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
mkdir -p "$work/folder"
for f in a b c; do printf 'print("%s")\n' "$f" > "$work/folder/$f.swift"; done
echo secret > "$work/outside.txt"
"$coax" c "$here/tools_smoke.coax" > "$work/smoke.c"
$cc -std=c11 -O1 -g -o "$work/smoke" "$work/smoke.c" "$repo/coax/platform/process-native.c" "$repo/coax/platform/store-file-native.c" -lm
MC_SMOKE_DIR="$work/folder" "$work/smoke" | tee "$work/out"
tail -1 "$work/out" | grep -q '^smoke: ok$'
