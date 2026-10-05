#!/bin/sh
# Render every Verse particle sprite sheet with Blender headless into
# assets/verse/fx/. See docs/verse/particles.md.
#
# Usage: scripts/blender/build-fx.sh [SHEET...]
#
# With no arguments it renders every sheet; otherwise only the named ones
# (fireball, smoke, sparks). BLENDER names the Blender binary (default: the
# macOS app bundle's). FX_SAMPLES lowers the Cycles samples for a quick
# draft (default 64). FX_PREVIEW names a directory that receives each sheet
# composited over a dark field, alpha-blended and added side by side.
#
# Then look at the effects in the engine:
#   cargo run --release -p verse --example fx_preview -- EFFECT OUT_DIR
set -eu

here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
out="$repo/assets/verse/fx"
blender=${BLENDER:-/Applications/Blender.app/Contents/MacOS/Blender}

if [ "$#" -eq 0 ]; then
  set -- fireball smoke sparks
fi
mkdir -p "$out"
for sheet in "$@"; do
  case "$sheet" in
    fireball | smoke) cell=128 ;;
    sparks) cell=256 ;;
    *)
      echo "unknown sheet: $sheet" >&2
      exit 2
      ;;
  esac
  "$blender" -b --factory-startup --python "$here/fx/$sheet.py" -- "$out/$sheet.png" "$cell" 2>&1 |
    grep -E '^SHEET|Error|Traceback' || true
done
