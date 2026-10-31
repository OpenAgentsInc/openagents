#!/usr/bin/env bash
# Everglade natively against the browser's wasm (G3, docs/verse/native-gpu.md):
# the same Coax and face built both ways, run through the same steps (a walk,
# a turn and a jump, the clock fixed, golden mode). Every frame's vertices,
# draw list, counts and particles must be the same bytes; the uniforms and
# stores must be the same words or within libm's rounding (realm-run.c).
#   verse/native/check.sh HOST    HOST is a Linux machine with nix (coderos-4080)
# Needs the pack at apps/openagents-web/realms/everglade/assets/everglade.pack
# (built by verse/tools/pack_realm.py) and the wasm toolchain (coax/web/toolchain.sh).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../.." && pwd -P)"
host="${1:?usage: check.sh HOST}"
pack="$root/apps/openagents-web/realms/everglade/assets/everglade.pack"
[[ -f "$pack" ]] || { echo "check.sh: no $pack (verse/tools/pack_realm.py builds it)" >&2; exit 1; }
work="$(mktemp -d "${TMPDIR:-/tmp}/verse-native.XXXXXX")"
trap 'rm -rf "$work"' EXIT
m="$root/verse"
"$root/coax/web/wasm.sh" "$root/apps/openagents-web/everglade.coax" "$root/apps/openagents-web/everglade-face.c" "$work/everglade.wasm" \
  "$m/player.coax" "$m/grid.coax" "$m/everglade/river.coax" "$m/everglade/guard-guide-policy.coax"
"$here/build.sh" everglade "$work/c"
cp "$pack" "$here/everglade.facts" "$here/build.sh" "$work/c/"
mkdir -p "$work/c/wasm"
steps=(--frames 240 --every 30 --golden --extra 9000 --keys 10:-6:1,40:-4:1,80:-4:0,150:-6:0,160:32:1,162:32:0)
VERSE_NATIVE_CLOCK_MS=0 bun "$here/wasm-run.ts" "$work/everglade.wasm" --pack "$pack" --facts "$here/everglade.facts" "${steps[@]}" --dump "$work/c/wasm" > "$work/wasm.txt"
COPYFILE_DISABLE=1 tar --no-xattrs -C "$work/c" -cf - . | ssh -o BatchMode=yes "$host" "set -e
d=\$(mktemp -d); trap 'rm -rf \$d' EXIT; cd \$d; tar -xf - 2>/dev/null
mkdir -p verse/native; cp build.sh verse/native/
nix-shell -p gcc --run 'COAX=/nonexistent verse/native/build.sh everglade \$PWD' > /dev/null
VERSE_NATIVE_CLOCK_MS=0 ./realm-run --pack everglade.pack --facts everglade.facts ${steps[*]} --against wasm" > "$work/native.txt" || status=$?
grep -v "^frame" "$work/native.txt"
# The frames, without the uniforms' and stores' hashes (compared above, within rounding).
strip() { grep "^frame" "$1" | sed -E 's/ u [0-9a-f]+//; s/ s[0-9] [0-9]+ [0-9a-f]+//g'; }
if diff <(strip "$work/wasm.txt") <(strip "$work/native.txt"); then
  echo "check.sh: $(grep -c "^frame" "$work/native.txt") frames, the same vertices, draws and particles natively as in the wasm"
else
  echo "check.sh: the frames differ"; exit 1
fi
exit "${status:-0}"
