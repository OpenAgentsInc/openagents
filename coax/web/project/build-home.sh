#!/usr/bin/env bash
# Homepage renderer, public verifier and project fold in separate Coax units; the same module is
# the Grid (/realms/the-grid, data-page 1), with its session as a library.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
source "$repo/coax/web/toolchain.sh"
out="$1"; mkdir -p "$(dirname "$out")"
work="$(mktemp -d)";trap 'rm -rf "$work"' EXIT
objects=(); units=()
for unit in newland landing-browser landing-fold; do
  directory=coax/web/project; [[ "$unit" != newland ]] || directory=apps/openagents-web
  # By repo-relative path, so the module's bytes don't depend on the checkout (wasm.sh).
  (cd "$repo" && coax/bin/coax c "$directory/$unit.coax" --library --header "$work/$unit.h") > "$work/$unit.c"
  units+=("$unit")
done
# The Grid (/realms/the-grid) is this module walked (newland-walk.coax): its session (you, the
# phone's thumbs and the record, on Aethyr's engine core) is a library of its own (Gridwalk.h).
for library in verse/gridwalk.coax; do
  base="$(basename "$library" .coax)"; base="$(printf '%s' "${base:0:1}" | tr a-z A-Z)${base:1}"
  (cd "$repo" && coax/bin/coax c "$library" --library --header "$work/$base.h") > "$work/$base.c"
  units+=("$base")
done
# Struct defaults come from the compiler (coax_default_T in each unit's header):
# the Grid's face calls them for its globals.
for unit in "${units[@]}"; do
  "$COAXIAL_CLANG" --target=wasm32-wasip1 --sysroot="$COAXIAL_SYSROOT" -resource-dir="$COAXIAL_RESOURCES" -std=c11 -O2 -ffp-contract=off -fno-fast-math -Wno-unused-function -Wno-unused-variable -c "$work/$unit.c" -o "$work/$unit.o"
  objects+=("$work/$unit.o")
done
for unit in newland landing-reader landing-fold landing-cache; do
  directory="$here"; [[ "$unit" != newland ]] || directory="$repo/apps/openagents-web"
  face="$directory/$unit-face.c"
  "$COAXIAL_CLANG" --target=wasm32-wasip1 --sysroot="$COAXIAL_SYSROOT" -resource-dir="$COAXIAL_RESOURCES" -std=c11 -O2 -I"$work" -I"$directory" -c "$face" -o "$work/$unit-face.o"
  objects+=("$work/$unit-face.o")
done
"$COAXIAL_CLANG" --target=wasm32-wasip1 --sysroot="$COAXIAL_SYSROOT" -resource-dir="$COAXIAL_RESOURCES" -O2 -mexec-model=reactor "${objects[@]}" -Wl,--import-memory,--max-memory=67108864,-z,stack-size=8388608,--export=__heap_base,--gc-sections,--strip-debug -o "$work/reader.wasm"
mv "$work/reader.wasm" "$out"
