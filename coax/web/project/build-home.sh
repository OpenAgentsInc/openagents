#!/usr/bin/env bash
# Homepage renderer, public verifier and project fold in separate Coax units; the same module is
# the Grid (/realms/the-grid, data-page 1), with its session as a library.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
coax_driver="${COAX:-$repo/coax/bin/coax}"
render="${COAXIAL_RENDER:-1}"
[[ "$render" != auto ]] || render=1
case "$render" in 0|1) ;; *) echo "COAXIAL_RENDER must be auto,0 or1" >&2; exit 2 ;; esac
run_steps=()
if [[ -n "${COAXIAL_RUN_STEPS:-}" ]]; then run_steps=(--run-steps "$COAXIAL_RUN_STEPS")
elif [[ "$render" == 1 ]]; then run_steps=(--run-steps 128000);fi
source "$repo/coax/web/toolchain.sh"
out="$1"; mkdir -p "$(dirname "$out")"
# This shared homepage/Grid module needs its 400k-vertex arena inside 64 MiB.
# The retained 174-export Wasm audit bounds normal-entry linear stack at
# 1,953,936 bytes, including replay. Reserve 4 MiB with that measured margin;
# see docs/verification/aethyr-grid-final-stack-2026-11-02.json for scope.
grid_stack_bytes=4194304
work="$(mktemp -d)";trap 'rm -rf "$work"' EXIT
objects=(); units=()
for unit in newland landing-browser landing-fold; do
  directory=coax/web/project; [[ "$unit" != newland ]] || directory=apps/openagents-web
  # By repo-relative path, so the module's bytes don't depend on the checkout (wasm.sh).
  source_unit="$directory/$unit.coax"
  if [[ "$render" == 1 && "$unit" == landing-browser ]]; then source_unit=aethyr/render/carriers/landing-browser.coax; fi
  (cd "$repo" && "$coax_driver" c "$source_unit" --library --header "$work/$unit.h" ${run_steps[@]+"${run_steps[@]}"}) > "$work/$unit.c"
  units+=("$unit")
done
# The Grid (/realms/the-grid) is this module walked (newland-walk.coax): its session (you, the
# phone's thumbs and the record, on Aethyr's engine core) is a library of its own (Gridwalk.h).
for library in verse/gridwalk.coax; do
  base="$(basename "$library" .coax)"; base="$(printf '%s' "${base:0:1}" | tr a-z A-Z)${base:1}"
  (cd "$repo" && "$coax_driver" c "$library" --library --header "$work/$base.h" ${run_steps[@]+"${run_steps[@]}"}) > "$work/$base.c"
  units+=("$base")
done
if [[ "$render" == 1 ]]; then cp "$work/landing-browser.h" "$work/Render.h"; fi
if [[ -n "${COAXIAL_ADMITTED_DIR:-}" ]]; then mkdir -p "$COAXIAL_ADMITTED_DIR";cp "$work/"*.c "$work/"*.h "$COAXIAL_ADMITTED_DIR/";fi
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
if [[ "$render" == 1 ]]; then
  "$COAXIAL_CLANG" --target=wasm32-wasip1 --sysroot="$COAXIAL_SYSROOT" -resource-dir="$COAXIAL_RESOURCES" -std=c11 -O2 -I"$work" -c "$repo/coax/web/render-face.c" -o "$work/Render-face.o"
  objects+=("$work/Render-face.o")
fi
"$COAXIAL_CLANG" --target=wasm32-wasip1 --sysroot="$COAXIAL_SYSROOT" -resource-dir="$COAXIAL_RESOURCES" -O2 -mexec-model=reactor "${objects[@]}" -Wl,--import-memory,--max-memory=67108864,-z,stack-size="$grid_stack_bytes",--export=__heap_base,--gc-sections,--strip-debug -o "$work/reader.wasm"
mv "$work/reader.wasm" "$out"
