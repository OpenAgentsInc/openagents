#!/usr/bin/env bash
# Builds the room server's simulation for the browser's local host (GitHub #12784; Aethyr book
# chapter 95): the units build.sh links for oa-room (room.coax and the city's), turned into C by the
# same compiler and linked to wasm32 with room-worker.c in place of room-host.c. Offline Everglade
# runs it in a Web Worker (coax/web/room-worker.js), so offline play is a one-seat room.
#   coax/web/room/worker.sh [OUT.wasm]      (default apps/openagents-web/build/everglade-room.wasm)
# The C of each unit is kept under the SHA-256 of the compiler and the unit's #load closure
# (COAXIAL_WASM_CACHE, default ~/.cache/coax-wasm), so a build whose sources didn't change
# compiles no Coax. COAXIAL_MODE=dev links at -O1 without LTO, as wasm.sh does.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../../.." && pwd -P)"
out="${1:-$root/apps/openagents-web/build/everglade-room.wasm}"
mkdir -p "$(dirname "$out")"; out="$(cd "$(dirname "$out")" && pwd -P)/$(basename "$out")"
coax="${COAX:-$root/coax/bin/coax}"
source "$root/coax/web/toolchain.sh"
clang="$COAXIAL_CLANG"
work="$(mktemp -d "${TMPDIR:-/tmp}/coax-room-worker.XXXXXX")"
trap 'rm -rf "$work"' EXIT
units=(coax/web/room/room.coax apps/openagents-web/everglade.coax apps/openagents-web/everglade-step.coax verse/grid.coax verse/everglade/river.coax verse/everglade/guard-guide-policy.coax)
cache="${COAXIAL_WASM_CACHE-$HOME/.cache/coax-wasm}"
[[ -z "$cache" ]] || mkdir -p "$cache"
compiler_sum="$(shasum -a 256 < "$coax" | cut -c1-64)"
emit() {   # unit: its C and header into $work (NAME.c, NAME.h), from the cache or the compiler
  local u="$1" n key="" list
  n="$(basename "$u" .coax)"
  if [[ -n "$cache" ]] && list="$(cd "$root" && coax/web/tool.sh wasm-size sources "$u")" && [[ -n "$list" ]]; then
    key="$({ printf '%s\n' coax-room-worker-c-1 "$compiler_sum" "$u"; (cd "$root" && shasum -a 256 $list); } | shasum -a 256 | cut -c1-64)"
    if [[ -f "$cache/$key.h" && -f "$cache/$key.c" ]]; then cp "$cache/$key.c" "$work/$n.c"; cp "$cache/$key.h" "$work/$n.h"; touch "$cache/$key.c" "$cache/$key.h"; return 0; fi
  fi
  (cd "$root" && "$coax" c "$u" --library --header "$work/$n.h") > "$work/$n.c"
  if [[ -n "$key" ]]; then cp "$work/$n.c" "$cache/.$key.c" && mv -f "$cache/.$key.c" "$cache/$key.c"; cp "$work/$n.h" "$cache/.$key.h" && mv -f "$cache/.$key.h" "$cache/$key.h"; fi
}
pids=()
for u in "${units[@]}"; do emit "$u" & pids+=($!); done
for p in "${pids[@]}"; do wait "$p"; done
# The room's unit and the city's both carry apps/openagents-web/analytics.coax's exports (build.sh
# makes the room's copies local to it): here the room's are renamed in its object, room_dup_NAME.
names() { grep -oE '^[A-Za-z_][A-Za-z0-9_ *]* \**([a-z_][A-Za-z0-9_]*)\(' "$1" | sed -E 's/.*[ *]([A-Za-z_][A-Za-z0-9_]*)\($/\1/' | sort -u; }
names "$work/room.h" > "$work/room.syms"
: > "$work/other.syms"
for u in "${units[@]:1}"; do names "$work/$(basename "$u" .coax).h" >> "$work/other.syms"; done
sort -u "$work/other.syms" -o "$work/other.syms"
renames=()
while read -r s; do [[ -n "$s" ]] && renames+=("-D$s=room_dup_$s"); done < <(comm -12 "$work/room.syms" "$work/other.syms")
"$root/coax/web/tool.sh" "$here/room-catalog.coax" "$root/apps/openagents-web/realms/everglade/assets/everglade.pack" "$work/room-catalog.c"
mode="${COAXIAL_MODE:-release}"
opt=-Os; lto=(-flto)
if [[ "$mode" == dev ]]; then opt=-O1; lto=(); fi
cflags=(--target=wasm32-wasip1 --sysroot="$COAXIAL_SYSROOT" -resource-dir="$COAXIAL_RESOURCES" -std=c11 "$opt" ${lto[@]+"${lto[@]}"}
        -ffp-contract=off -fno-fast-math -w)
# the C library's sines, exponentials and powers left to room-math.c, which oa-room links too (build.sh, #12893)
for f in sin cos tan exp log pow; do cflags+=("-fno-builtin-$f" "-fno-builtin-${f}f"); done
objs=()
# (no LTO for it: it stands in for the C library's functions, which the link resolves after the bitcode)
mflags=(); for f in "${cflags[@]}"; do [[ "$f" == -flto ]] || mflags+=("$f"); done
"$clang" "${mflags[@]}" -c "$here/room-math.c" -o "$work/room-math.o" & pids=($!); objs+=("$work/room-math.o")
"$clang" "${cflags[@]}" ${renames[@]+"${renames[@]}"} -I"$work" -c "$work/room.c" -o "$work/room.o" & pids+=($!); objs+=("$work/room.o")
for u in "${units[@]:1}"; do
  n="$(basename "$u" .coax)"
  "$clang" "${cflags[@]}" -I"$work" -c "$work/$n.c" -o "$work/$n.o" & pids+=($!); objs+=("$work/$n.o")
done
"$clang" "${cflags[@]}" -I"$work" -I"$here" -c "$here/room-world.c" -o "$work/room-world.o" & pids+=($!); objs+=("$work/room-world.o")
"$clang" "${cflags[@]}" -c "$work/room-catalog.c" -o "$work/room-catalog.o" & pids+=($!); objs+=("$work/room-catalog.o")
"$clang" "${cflags[@]}" -I"$work" -I"$here" -c "$here/room-worker.c" -o "$work/room-worker.o" & pids+=($!); objs+=("$work/room-worker.o")
for p in "${pids[@]}"; do wait "$p"; done
"$clang" --target=wasm32-wasip1 --sysroot="$COAXIAL_SYSROOT" -resource-dir="$COAXIAL_RESOURCES" -mexec-model=reactor -o "$work/output.wasm" "${objs[@]}" \
  -Wl,--import-memory,--max-memory=268435456,-z,stack-size=4194304,--export=__heap_base,--gc-sections,--strip-debug
if [[ "$mode" != dev ]]; then
  wasm_opt="${COAXIAL_WASM_OPT:-$(command -v wasm-opt || true)}"
  if [[ -n "$wasm_opt" ]]; then "$wasm_opt" "$work/output.wasm" -O2 -o "$work/optimized.wasm" && mv "$work/optimized.wasm" "$work/output.wasm"; fi
  # its budget line (apps/openagents-web/wasm-budgets.txt everglade-room)
  (cd "$root" && coax/web/tool.sh wasm-size budget apps/openagents-web/wasm-budgets.txt "$(basename "$out" .wasm)" "$work/output.wasm") || { s=$?; [[ $s == 4 && "$out" != "$root/apps/openagents-web/build/"* ]] || exit 1; }
fi
mv "$work/output.wasm" "$out"
echo "built $out ($(wc -c < "$out" | tr -d ' ') bytes)"
