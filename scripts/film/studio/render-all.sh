#!/usr/bin/env bash
# Renders the launch film's studio shots (cut rows 3-6, #11432) with headless
# Blender: each shot's screen footage from ~/work/film-out/footage/ onto a
# device, to ~/work/film-out/studio/<shot>.mp4.
#
# Usage: scripts/film/studio/render-all.sh [--preview] [--standin] [SHOT...]
#   --preview   quarter-pixel, low-sample renders for checking moves
#   --standin   use ~/work/film-out/studio/standin/ clips (standin.py) when a
#               shot's real footage is missing, instead of skipping the shot
# BLENDER overrides Blender's path; FOOTAGE and OUT override the directories.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
blender=${BLENDER:-/Applications/Blender.app/Contents/MacOS/Blender}
footage=${FOOTAGE:-$HOME/work/film-out/footage}
out=${OUT:-$HOME/work/film-out/studio}
standin_dir=$out/standin
preview=() standin=0 shots=()
for a in "$@"; do
  case $a in
    --preview) preview=(--preview) ;;
    --standin) standin=1 ;;
    *) shots+=("$a") ;;
  esac
done
[ ${#shots[@]} -eq 0 ] && shots=(macbook-chat macbook-terminal macbook-sync iphone-approve)

clip_for() {
  case $1 in
    macbook-chat) echo web-chat ;;
    macbook-terminal) echo terminal-coder ;;
    macbook-sync) echo web-sync ;;
    iphone-approve) echo phone-approve ;;
    *) echo "" ;;
  esac
}

mkdir -p "$out"
status=0
for shot in "${shots[@]}"; do
  spec=$here/shots/$shot.json
  clip=$(clip_for "$shot")
  if [ ! -f "$spec" ] || [ -z "$clip" ]; then
    echo "render-all: unknown shot $shot" >&2; status=1; continue
  fi
  video=$footage/$clip.mp4
  if [ ! -f "$video" ]; then
    if [ $standin -eq 1 ] && [ -f "$standin_dir/$clip.mp4" ]; then
      echo "render-all: $shot: no $video yet, using stand-in $standin_dir/$clip.mp4"
      video=$standin_dir/$clip.mp4
    else
      echo "render-all: $shot: SKIPPED, footage $video is missing (record it, or pass --standin)"
      continue
    fi
  fi
  suffix=""; [ ${#preview[@]} -gt 0 ] && suffix=.preview
  start=$(date +%s)
  "$blender" -b --factory-startup -P "$here/studio.py" -- --shot "$spec" --video "$video" \
    --out "$out/$shot$suffix.mp4" "${preview[@]}" 2>&1 | grep -E "^studio:|Error|Traceback" || true
  if [ -f "$out/$shot$suffix.mp4" ] && [ "$(stat -f %m "$out/$shot$suffix.mp4" 2>/dev/null || stat -c %Y "$out/$shot$suffix.mp4")" -ge "$start" ]; then
    echo "render-all: $shot done in $(( $(date +%s) - start ))s -> $out/$shot$suffix.mp4"
  else
    echo "render-all: $shot FAILED" >&2; status=1
  fi
done
exit $status
