#!/usr/bin/env bash
# Build only the new-land public reader, with separately compiled Coax units.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
source "$repo/coax/web/toolchain.sh"
out="$1"; mkdir -p "$(dirname "$out")"
work="$(mktemp -d)";trap 'rm -rf "$work"' EXIT
objects=()
for unit in landing-browser landing-fold; do
  (cd "$repo" && coax/bin/coax c "${here#"$repo"/}/$unit.coax" --library --header "$work/$unit.h") > "$work/$unit.c"   # repo-relative: bytes don't depend on the checkout
  "$COAXIAL_CLANG" --target=wasm32-wasip1 --sysroot="$COAXIAL_SYSROOT" -resource-dir="$COAXIAL_RESOURCES" -std=c11 -O2 -ffp-contract=off -fno-fast-math -Wno-unused-function -Wno-unused-variable -c "$work/$unit.c" -o "$work/$unit.o"
  objects+=("$work/$unit.o")
done
for unit in landing-reader landing-fold; do
  "$COAXIAL_CLANG" --target=wasm32-wasip1 --sysroot="$COAXIAL_SYSROOT" -resource-dir="$COAXIAL_RESOURCES" -std=c11 -O2 -I"$work" -c "$here/$unit-face.c" -o "$work/$unit-face.o"
  objects+=("$work/$unit-face.o")
done
"$COAXIAL_CLANG" --target=wasm32-wasip1 --sysroot="$COAXIAL_SYSROOT" -resource-dir="$COAXIAL_RESOURCES" -O2 -mexec-model=reactor "${objects[@]}" -Wl,--import-memory,--max-memory=67108864,-z,stack-size=1048576,--export=__heap_base,--gc-sections,--strip-debug -o "$work/reader.wasm"
mv "$work/reader.wasm" "$out"
