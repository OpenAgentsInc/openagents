#!/bin/sh
# Rebuild every generated model under assets/verse/generated with Blender
# headless, then render the gallery.
#
# Usage: scripts/blender/build-models.sh [GALLERY_DIR]
#
# BLENDER names the Blender binary (default: the macOS app bundle's).
# ENEMY_PACK names Quaternius's "Easy Animated Enemy Pack - Jan 2019.zip"
# (default: ~/Downloads); without it, the converted creatures are skipped.
# Set GALLERY_ONLY=1 to render the gallery from the committed models.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
out="$repo/assets/verse/generated"
blender=${BLENDER:-/Applications/Blender.app/Contents/MacOS/Blender}
pack=${ENEMY_PACK:-"$HOME/Downloads/Easy Animated Enemy Pack - Jan 2019.zip"}
gallery=${1:-"${TMPDIR:-/tmp}/verse-models-gallery"}

run() {
  script=$1
  shift
  "$blender" -b --factory-startup --python "$here/$script.py" -- "$@" 2>&1 | grep -E '^MODEL|Error|Traceback' || true
}

if [ "${GALLERY_ONLY:-0}" != 1 ]; then
  if [ -f "$pack" ]; then
    run enemy_pack "$pack" "$out"
  else
    echo "skipping the enemy pack: $pack not found" >&2
  fi
  for script in sledgehammer fountain observatory bandshell market_stall training_dummy animals dragon; do
    run "$script" "$out"
  done
  "$blender" -b --factory-startup --python "$here/buildings.py" -- "$out/buildings" 2>&1 | grep -E '^BUILT|Error|Traceback' || true
  run street_props "$out/street"
  run town_props "$out/town"
fi
run gallery "$out" "$gallery"
echo "gallery: $gallery/gallery.png, $gallery/contact_sheet.png, $gallery/previews/"
