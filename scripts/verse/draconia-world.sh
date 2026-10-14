#!/bin/sh
# Draconia in the castle: walk round her in a candlelit potions hall.
# Local only, from the private MetaHuman pipeline's exports in
# ~/work/film-out (never committed, never used for AI training).
# See docs/verse/draconia/world.md.
#
#   scripts/verse/draconia-world.sh            # the window
#   scripts/verse/draconia-world.sh --benchmark 20
#   scripts/verse/draconia-world.sh --capture ~/Downloads
#   scripts/verse/draconia-world.sh --walk ~/Downloads/draconia-world-walk.mp4
set -eu

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/work/openagents-target-castle}"
FILM="$HOME/work/film-out/draconia"
MH="$FILM/metahuman"
LOWER="$FILM/world/draconia_lower.glb"
BLENDER="${BLENDER:-/Applications/Blender.app/Contents/MacOS/Blender}"

# Her latest round: the parity pipeline's stable path, else its rig build.
MODEL="$MH/latest/mh.neutral.glb"
[ -f "$MODEL" ] || MODEL="$MH/build-rig-neutral/mh.neutral.glb"
BODY="$MH/rig-body-down/body_lod0_mesh.obj"
[ -f "$BODY" ] || BODY="$MH/archetype-body-down/body_lod0_mesh.obj"

if [ ! -f "$MODEL" ] || [ ! -f "$BODY" ]; then
  echo "Draconia's private MetaHuman assets are not on this computer."
  echo "Expected her export at $MODEL"
  echo "and her body at $BODY."
  echo "They come from the private pipeline (docs/verse/draconia/parity.md) and are never in git."
  echo "To walk the castle without her: $0 --no-draconia"
  case " $* " in *" --no-draconia "*) ;; *) exit 1 ;; esac
else
  # Her lower body (legs, shoes, skirt, the robe to the shin) is rebuilt
  # whenever a new round or a new body lands.
  if [ ! -f "$LOWER" ] || [ "$MODEL" -nt "$LOWER" ] || [ "$BODY" -nt "$LOWER" ]; then
    if [ -x "$BLENDER" ]; then
      echo "draconia-world: building her lower body for this round (Blender, about 10 s)"
      "$BLENDER" -b --factory-startup --python "$ROOT/scripts/verse/draconia_world_body.py" -- \
        --model "$MODEL" --body "$BODY" --out "$LOWER" >/dev/null
    elif [ ! -f "$LOWER" ]; then
      echo "draconia-world: her lower body needs Blender at $BLENDER (set BLENDER=...)"
      exit 1
    fi
  fi
  set -- --model "$MODEL" --lower "$LOWER" "$@"
fi

cargo build --release --quiet --manifest-path "$ROOT/Cargo.toml" \
  -p verse-imported --features imported-desktop,crypt-fight --example draconia_world
exec "$CARGO_TARGET_DIR/release/examples/draconia_world" "$@"
