#!/bin/sh
# Score every MetaHuman Creator preset's face against the reference (round 7).
#
#   scripts/verse/draconia_presets.sh [NAME ...]
#
# For each preset head exported by scripts/unreal/presets_export.py
# (~/work/film-out/draconia/metahuman/presets/NAME.fbx): builds a fast,
# face-only Verse model of it smiling (MetaHuman's archetype rig poses the
# smile; the preset's shape is matched to the rig's vertices by texture
# coordinate), writes its rest head in DNA centimetres for the 3D
# proportions, and renders it at the 0:54 full-face framing and light.
# Then MetaHuman's tracker measures every render (landmarks.py) and
# draconia_preset_score.py ranks them. Private MetaHuman data, never used
# to train AI models.
set -u
WT=$(cd "$(dirname "$0")/../.." && pwd)
MH=$HOME/work/film-out/draconia/metahuman
OUT=$HOME/work/film-out/draconia/parity/round-07/presets
B=${DRACONIA_PORTRAIT:-$HOME/work/openagents-target-agent-parity/release/examples/draconia_portrait}
S=$HOME/work/film-out/draconia/build/draconia_set_parity.glb
mkdir -p "$OUT" "$MH/build-presets"
names=${*:-$(ls "$MH/presets" | sed -n 's/\.fbx$//p')}
images=""
for n in $names; do
  d=$MH/build-presets/$n
  mkdir -p "$d"
  MH_FAST=1 MH_SHAPE=fit MH_FIT_FBX=$MH/presets/$n.fbx MH_SHAPE_OUT=$d/head.txt MH_NEUTRAL_DIR=$MH/archetype \
    /Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup --python "$WT/scripts/blender/metahuman_import.py" -- \
    "$MH/archetype-smile" "$MH/textures" "$d/mh.smile.glb" --tex 1024 > "$d/import.log" 2>&1
  "$B" "$OUT/$n.png" --model "$d/mh.smile.glb" --set "$S" --static --shot fullface --light ref --subframes 8 --size 960x540 2>&1 | tail -1
  images="$images $OUT/$n.png"
done
cd "$WT" || exit 1
DRACONIA_LANDMARK_IMAGES="$images" DRACONIA_LANDMARK_OUT=$OUT/landmarks caffeinate -dimsu sh scripts/unreal/run_ue.sh scripts/unreal/landmarks.py > "$OUT/landmarks.log" 2>&1
python3 "$WT/scripts/verse/draconia_preset_score.py" "$OUT" "$MH/build-presets"
