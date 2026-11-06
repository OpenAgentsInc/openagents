#!/usr/bin/env bash
# Builds the Orbit League room server (room.coax, room-host.c) to OUT
# (default coax/web/room/bin/room).
#   coax/web/room/build.sh [OUT]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../../.." && pwd -P)"
out="${1:-$here/bin/room}"
mkdir -p "$(dirname "$out")"
work="$(mktemp -d "${TMPDIR:-/tmp}/coax-room-build.XXXXXX")"
trap 'rm -rf "$work"' EXIT
coax="${COAX:-$root/coax/bin/coax}"
source_root="$root"
if [[ -n "${COAX_FAST_EDIT:-}" ]]; then
  cmp -s "$coax" "$root/coax/bin/coax" || { echo 'fast-edit: compiler differs from the sealed default' >&2; exit 2; }
  [[ "$(command -v cc)" == "$(sed -n '5p' "$COAX_FAST_EDIT/toolchain")" ]] || { echo 'fast-edit: native compiler differs from the sealed baseline' >&2; exit 2; }
  source_root="$COAX_FAST_EDIT/source"
fi
"$coax" c "$source_root/coax/web/room/room.coax" --library --header "$work/room.h" > "$work/room.c"
# Everglade's headless world for the everglade rooms (room-world.c): the city's unit (everglade.coax,
# nothing drawn), the controller's (everglade-step.coax, with aethyr/player.coax) and the units the
# city links beside them, as apps/openagents-web/build.sh links the page's.
units=(apps/openagents-web/everglade.coax apps/openagents-web/everglade-step.coax verse/grid.coax verse/everglade/river.coax verse/everglade/guard-guide-policy.coax)
for u in "${units[@]}"; do
  n="$(basename "$u" .coax)"
  (cd "$source_root" && "$coax" c "$source_root/$u" --library --header "$work/$n.h" > "$work/$n.c")
done
# The simulation's floats as the browser's local host computes them (worker.coax, #12893): no fused
# a * b + c, and the C library's sines, exponentials and powers left to room-math.c, which both hosts link.
strict=(-ffp-contract=off -fno-fast-math)
fast_objects=(); fast_flags=(); fast_libs=()
if [[ -n "${COAX_FAST_EDIT:-}" && "${COAX_FAST_EDIT_ORACLE:-0}" != 1 ]]; then
  for c in "$work/"*.c; do "$root/coax/web/tool.sh" fast-edit instrument "$COAX_FAST_EDIT" "$c"; done
  "$coax" c "$root/coax/web/fast-edit-offer.coax" --library --header "$work/fast-edit-offer.h" -o "$work/fast-edit-policy.c"
  cc -std=c11 -O2 -w -c "$work/fast-edit-policy.c" -o "$work/fast-edit-policy.o"
  cc -std=c11 -O2 -I"$work" -I"$root/coax/web" -c "$root/coax/web/fast-edit-native.c" -o "$work/fast-edit-native.o"
  fast_objects=("$work/fast-edit-policy.o" "$work/fast-edit-native.o")
  fast_flags=(-DCOAX_FAST_EDIT_ENABLED -I"$root/coax/web")
  [[ "$(uname)" != Linux ]] || fast_libs=(-ldl)
fi
for f in sin cos tan exp log pow; do strict+=("-fno-builtin-$f" "-fno-builtin-${f}f"); done
objs=("$work/room.o")
cc -std=c11 -O2 "${strict[@]}" -w -c "$work/room.c" -o "$work/room.o" &
for u in "${units[@]}"; do
  n="$(basename "$u" .coax)"
  # at -O2, as the image builds them (Dockerfile): the tick runs them for every room 30 times a second
  cc -std=c11 -O2 "${strict[@]}" -w -c "$work/$n.c" -o "$work/$n.o" &
  objs+=("$work/$n.o")
done
wait
# Units that load the same Coax module each carry its #export_c procedures (the room's unit and the
# city's both carry apps/openagents-web/analytics.coax's; since 010c25bbed guard-guide-policy carries
# aethyr/input.coax's input_runs, coax.json's S_hash/S_hex and the crypto bytes' cb_* that the city and
# the room have too). The same source makes the same code, so one copy is the program's: the city's
# units in order, then the room's, each defining globally only what no unit before it did; a unit's
# later copies are made local to it (the Dockerfile does the same).
seen="$work/seen.syms"; : > "$seen"
own=("$work/everglade.o")
for u in "${units[@]:1}"; do own+=("$work/$(basename "$u" .coax).o"); done
own+=("$work/room.o")
objs=()
for o in "${own[@]}"; do
  nm -g --defined-only "$o" | awk '{print $3}' | sort -u > "$o.syms"
  comm -12 "$o.syms" "$seen" > "$o.dup"
  t="$o"
  if [[ -s "$o.dup" ]]; then
    t="${o%.o}-own.o"
    if [[ "$(uname)" == Darwin ]]; then ld -r "$o" -o "$t" -unexported_symbols_list "$o.dup"
    else objcopy --localize-symbols="$o.dup" "$o" "$t"; fi
  fi
  objs+=("$t")
  sort -u "$seen" "$o.syms" -o "$seen"
done
cc -std=c11 -O2 "${strict[@]}" -w -I"$work" -c "$here/room-world.c" -o "$work/room-world.o"
cc -std=c11 -O2 "${strict[@]}" -Wall -Wextra -c "$here/room-math.c" -o "$work/room-math.o"
# the pack's catalog, the bounds the world's pieces break with (room-catalog.coax)
"$root/coax/web/tool.sh" "$here/room-catalog.coax" "$root/apps/openagents-web/realms/everglade/assets/everglade.pack" "$work/room-catalog.c"
cc -std=c11 -O2 -w -c "$work/room-catalog.c" -o "$work/room-catalog.o"
cc -std=c11 -O2 "${strict[@]}" -Wall -Wextra -D_DEFAULT_SOURCE -I"$work" -I"$here" -I"$root/verse/voice-host" "$here/room-host.c" "$root/verse/voice-host/ws.c" \
  "${fast_flags[@]}" "$work/room-world.o" "$work/room-catalog.o" "$work/room-math.o" "${objs[@]}" "${fast_objects[@]}" -lm "${fast_libs[@]}" -o "$out"
# the simulation calls none of the C library's (room-math.c's in their place)
if nm -u "$out" | grep -E '^_?_*(sin|cos|tan|exp|log|pow)f?$|sincos'; then echo "build.sh: $out calls the C library's math above (room-math.c, #12893)" >&2; exit 1; fi
# the started server holds every declared default (room-host.c --start-check, room.coax rm_start_check)
"$out" --start-check
echo "built $out"
