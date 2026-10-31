#!/usr/bin/env bash
# A realm natively against the browser's wasm (G3, docs/verse/native-gpu.md):
# the same Coax and face built both ways, run through the same steps (Everglade:
# a walk, a turn and a jump; Hold the Stair: a wave started, a defender picked,
# fast; the clock fixed; Everglade in golden mode). Every frame's vertices,
# draw list, counts and particles must be the same bytes; the uniforms and
# stores must be the same words or within libm's rounding (realm-run.c).
#   verse/native/check.sh HOST [REALM]    HOST is a Linux machine with nix (coderos-4080);
#                                          REALM everglade (the default), stair, belt, waterlab, orbitleague, newland or verse (the galaxy)
# Everglade needs its pack at apps/openagents-web/realms/everglade/assets/everglade.pack
# (built by verse/tools/pack_realm.py); both need the wasm toolchain (coax/web/toolchain.sh).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../.." && pwd -P)"
host="${1:?usage: check.sh HOST [REALM]}"
realm="${2:-everglade}"
work="$(mktemp -d "${TMPDIR:-/tmp}/verse-native.XXXXXX")"
trap 'rm -rf "$work"' EXIT
m="$root/verse"
web="$root/apps/openagents-web"
case "$realm" in
  everglade)
    pack="$web/realms/everglade/assets/everglade.pack"
    [[ -f "$pack" ]] || { echo "check.sh: no $pack (verse/tools/pack_realm.py builds it)" >&2; exit 1; }
    libraries=("$m/player.coax" "$m/grid.coax" "$m/everglade/river.coax" "$m/everglade/guard-guide-policy.coax")
    given=(--pack "$pack" --facts "$here/everglade.facts"); there="--pack everglade.pack --facts everglade.facts"
    steps=(--frames 240 --every 30 --golden --extra 9000 --keys 10:-6:1,40:-4:1,80:-4:0,150:-6:0,160:32:1,162:32:0) ;;
  stair)
    pack=""; libraries=(); given=(); there=""
    # not golden: golden mode holds the Stair still (its golden is one frame)
    steps=(--frames 900 --every 60 --keys 10:32:1,11:32:0,20:49:1,21:49:0,30:102:1,31:102:0) ;;
  newland)
    # the Grid: the homepage's module walked (data-page 1)
    pack=""; libraries=("$m/player.coax" "$m/grid.coax"); given=(--page 1); there="--page 1"
    steps=(--frames 600 --every 60 --keys 20:32:1,21:32:0,60:-4:1,120:-4:0,140:87:1,260:87:0) ;;
  verse)
    # the galaxy (page 1): a world chosen with the arrows and opened with Enter
    pack=""; libraries=(); given=(--page 1); there="--page 1"
    steps=(--frames 300 --every 30 --keys 20:-5:1,21:-5:0,40:-5:1,41:-5:0,60:-2:1,61:-2:0) ;;
  belt|waterlab|orbitleague)
    pack=""; libraries=(); given=(); there=""
    # a click, space, the arrows and W held a while
    steps=(--frames 600 --every 60 --keys 20:32:1,21:32:0,60:-4:1,120:-4:0,140:87:1,260:87:0) ;;
  *) echo "check.sh: no realm $realm" >&2; exit 2 ;;
esac
if [[ "$realm" == newland ]]; then "$root/coax/web/project/build-home.sh" "$work/$realm.wasm"   # the homepage's module, as the site builds it
else "$root/coax/web/wasm.sh" "$web/$realm.coax" "$web/$realm-face.c" "$work/$realm.wasm" ${libraries[@]+"${libraries[@]}"}; fi
"$here/build.sh" "$realm" "$work/c"
cp "$here/everglade.facts" "$here/build.sh" ${pack:+"$pack"} "$work/c/"
mkdir -p "$work/c/wasm"
VERSE_NATIVE_CLOCK_MS=0 bun "$here/wasm-run.ts" "$work/$realm.wasm" ${given[@]+"${given[@]}"} "${steps[@]}" --dump "$work/c/wasm" > "$work/wasm.txt"
COPYFILE_DISABLE=1 tar --no-xattrs -C "$work/c" -cf - . | ssh -o BatchMode=yes "$host" "set -e
d=\$(mktemp -d); trap 'rm -rf \$d' EXIT; cd \$d; tar -xf - 2>/dev/null
mkdir -p verse/native; cp build.sh verse/native/
nix-shell -p gcc --run 'COAX=/nonexistent verse/native/build.sh $realm \$PWD' > /dev/null
./realm-run --test-travel
VERSE_NATIVE_CLOCK_MS=0 ./realm-run $there ${steps[*]} --against wasm" > "$work/native.txt" || status=$?
grep -v "^frame" "$work/native.txt"
# The frames, without the uniforms' and stores' hashes (compared above, within rounding).
strip() { grep "^frame" "$1" | sed -E 's/ u [0-9a-f]+//; s/ s[0-9] [0-9]+ [0-9a-f]+//g'; }
if diff <(strip "$work/wasm.txt") <(strip "$work/native.txt"); then
  echo "check.sh: $(grep -c "^frame" "$work/native.txt") frames, the same vertices, draws and particles natively as in the wasm"
else
  echo "check.sh: the frames differ"; exit 1
fi
exit "${status:-0}"
