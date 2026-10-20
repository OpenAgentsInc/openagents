#!/usr/bin/env bash
# The font files the GPUI web apps share (#12233): the desktop's Geist and
# Geist Mono faces that /gpui, /demo/gpui, and AgentGraph (/agent/foreman)
# draw. Each app's page names the faces it needs with `<link rel=preload
# data-gpui-font>` and its module takes them from the page, so the browser
# downloads and caches each face once for every app.
#
# Writes into OUTDIR, served by openagents.com at /gpui-fonts/
# (`openagents-web --gpui-fonts OUTDIR`, or /srv/gpui-fonts in the image):
#
#   NAME.HASH.ttf     each face under a content-hashed name, so it can be
#                     cached for good
#   *.gz, *.br        gzip and brotli copies
#   fonts.json        {"Geist.ttf": "Geist.0123456789ab.ttf", ...}
#
# Usage: apps/openagents/scripts/build-web-fonts.sh OUTDIR
set -euo pipefail

app="$(cd "$(dirname "$0")/.." && pwd)"
fonts="$app/crates/ui/assets/fonts"
out="${1:-}"
if [ -z "$out" ] || [ "$#" -ne 1 ]; then
  echo "usage: $0 OUTDIR" >&2
  exit 64
fi
# The faces, as `openagents_web_chat_view::assets::FONT_FILES` lists them;
# AgentGraph draws the first, third, fourth, and eighth.
names="$(sed -n 's/^    "\([A-Za-z-]*\.ttf\)",$/\1/p' "$app/web/chat-view/src/assets.rs")"
[ -n "$names" ] || { echo "no font names in chat-view/src/assets.rs" >&2; exit 65; }

hash() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -c1-12
  else
    shasum -a 256 "$1" | cut -c1-12
  fi
}

mkdir -p "$out"
manifest="{"
sep=""
for name in $names; do
  hashed="${name%.ttf}.$(hash "$fonts/$name").ttf"
  cp "$fonts/$name" "$out/$hashed"
  gzip -9 -n -c "$out/$hashed" > "$out/$hashed.gz"
  if command -v brotli >/dev/null 2>&1; then
    brotli -q 11 -f -o "$out/$hashed.br" "$out/$hashed"
  fi
  manifest="$manifest$sep\"$name\": \"$hashed\""
  sep=", "
done
printf '%s}\n' "$manifest" > "$out/fonts.json"
cat "$out/fonts.json"
