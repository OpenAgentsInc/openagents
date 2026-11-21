#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around cp, shasum, sed and gzip; holds no logic).
# gamebuilder.org's files as a browser is given them (issue 12), from a finished build (web/build.sh).
# scripts/web-deploy.sh uploads what this lays out; scripts/web-load.sh measures it.
#
#   scripts/web-stage.sh DIR            the site as the dev server serves it: /index.html, /host/app.js,
#                                       /build/*.wasm, /fonts/ and the rest
#   scripts/web-stage.sh DIR release    then the files a first visit loads (and the village's pack) are moved under /b/NAME/,
#                                       NAME being a digest of them, and index.html names them there:
#                                       a file under /b/ never changes, so a browser keeps it for good
#                                       and a second visit asks for the page alone. Prints NAME.
#   scripts/web-stage.sh DIR stored     then the script, the modules and the shaders under /b/ are
#                                       gzipped in place, under their own names: the bytes the bucket
#                                       holds and a browser is sent (Content-Encoding: gzip)
set -euo pipefail
gb="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo="$(cd "$gb/.." && pwd)"
export GB_ROOT="$gb" GB_VENDOR="$repo" COAX="${COAX:-$repo/coax/bin/coax}"
stage="$1"; web="$GB_ROOT/web"

if [[ "${2:-}" == "" ]]; then
  mkdir -p "$stage/build" "$stage/host" "$stage/fonts" "$stage/coaxial" "$stage/img" "$stage/assets" "$stage/api/auth"
  cp "$web/index.html" "$web/index.md" "$web/404.html" "$stage/"
  cp "$web/build/"*.wasm "$web/build/hud.wgsl" "$stage/build/"
  # /agents' icons, and their notice (Apps SDK UI, MIT): asked for by name, only there
  mkdir -p "$stage/build/icons"; cp "$web/build/icons/"* "$stage/build/icons/"
  cp "$web/build/player-catalog.json" "$web/build/player-permission.html" "$web/build/player-focus.html" "$stage/build/"
  # the page's one script, and beside it the pinned tree's files its workers are asked for by name
  cp "$web/host/"*.js "$stage/host/"
  cp "$web/fonts/"* "$stage/fonts/"
  cp "$web/img/"* "$stage/img/"
  # the village's pack (0 A.D. art by Wildfire Games, CC BY-SA 3.0), its credit and what was changed, and the tool that made it: all public, as the licence asks
  cp "$web/assets/"* "$stage/assets/"; cp "$GB_ROOT/tools/village-pack.coax" "$stage/assets/village-pack.coax"
  # the pinned tree's hosts under the address its own tools ask for them at (no logic; its own files)
  cp "$GB_VENDOR/coax/web/"*.js "$stage/coaxial/"
  # The browser host asks /api/auth/viewer on every page (who is logged in, for apps that read it;
  # this site's world does not: its login is the page's own, /api/auth/account). Only the login's
  # and the chat's paths go to the API from this site, so the question fell to the bucket and came
  # back 404 in the console. An empty answer is the host's own word for "nobody" and is not an error.
  : > "$stage/api/auth/viewer"
  exit
fi

if [[ "$2" == release ]]; then
  name="$(cat "$stage/host/app.js" "$stage/build/"*.wasm "$stage/build/hud.wgsl" "$stage/fonts/"*.woff2 "$stage/assets/village.pack" "$stage/img/"kind-*.jpg | shasum -a 256 | cut -c1-16)"
  mkdir -p "$stage/b/$name/build" "$stage/b/$name/fonts" "$stage/b/$name/assets" "$stage/b/$name/img"
  mv "$stage/host" "$stage/b/$name/host"
  mv "$stage/build/"*.wasm "$stage/build/hud.wgsl" "$stage/b/$name/build/"
  mv "$stage/fonts/"*.woff2 "$stage/b/$name/fonts/"
  # the village's pack is asked for only when the village is picked, and is kept like the rest. The
  # copy at /assets/ stays, beside its credit: the licence asks that the pack be public there.
  cp "$stage/assets/village.pack" "$stage/b/$name/assets/"
  # the game types' pictures, which the home page's ring of portals shows (web/hud-glue.js asks for them beside the
  # script, ../img/): kept like the rest, so a second visit asks for none of them. The copies at /img/ stay.
  cp "$stage/img/"kind-*.jpg "$stage/b/$name/img/"
  # index.html names the script, the fonts, the shaders, the pack and the HUD's module where they now are; the
  # hosts find every other module beside the script (../build/NAME.wasm)
  sed -i.was -e "s|\"/host/|\"/b/$name/host/|g" -e "s|/fonts/\([A-Za-z-]*\)\.woff2|/b/$name/fonts/\1.woff2|g" -e "s|\"/build/hud.wgsl\"|\"/b/$name/build/hud.wgsl\"|" -e "s|pack:/assets/|pack:/b/$name/assets/|" -e "s|data-app=\"hud\"|data-app=\"hud\" data-wasm=\"/b/$name/build/hud.wasm\"|" "$stage/index.html"
  rm "$stage/index.html.was"
  echo "$name"
  exit
fi

if [[ "$2" == stored ]]; then
  find "$stage/b" -type f \( -name '*.js' -o -name '*.wasm' -o -name '*.wgsl' \) -exec sh -c 'gzip -9 -n "$1" && mv "$1.gz" "$1"' _ {} \;
  exit
fi
echo "usage: scripts/web-stage.sh DIR [release|stored]" >&2; exit 2
