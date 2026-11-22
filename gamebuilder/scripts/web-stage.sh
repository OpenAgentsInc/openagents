#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around cp, shasum, sed and gzip; holds no logic).
# Stages gamebuilder.org from a finished web/build.sh output:
#   scripts/web-stage.sh DIR            unversioned site tree (/, /credits, /fated-dragon, /splicebots, /whispering-spring)
#   scripts/web-stage.sh DIR release    moves immutable release code/shaders/fonts under /b/NAME/ and rewrites HTML; prints NAME
#   scripts/web-stage.sh DIR stored     gzips .js/.wasm/.wgsl/.gbd1 under /b/ in place for Content-Encoding: gzip
set -euo pipefail
gb="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo="$(cd "$gb/.." && pwd)"
export GB_ROOT="$gb" GB_VENDOR="$repo" COAX="${COAX:-$repo/coax/bin/coax}"
stage="$1"; web="$GB_ROOT/web"

if [[ "${2:-}" == "" ]]; then
  mkdir -p "$stage/build" "$stage/coaxial" "$stage/fonts" "$stage/img" "$stage/api/auth" \
    "$stage/fated-dragon/build" "$stage/fated-dragon/assets" \
    "$stage/splicebots/build" "$stage/splicebots/fonts" "$stage/splicebots/packs" \
    "$stage/whispering-spring/build"
  cp "$web/index.html" "$web/index.md" "$web/credits.html" "$web/404.html" "$stage/"
  cp "$web/fonts/"* "$stage/fonts/"
  cp "$web/img/"* "$stage/img/"
  cp "$web/coaxial/"*.js "$stage/coaxial/"
  cp "$web/build/frame-plan.wasm" "$web/build/host-layers.wasm" "$web/build/host-packs.wasm" "$stage/build/"
  # Fated Dragon
  cp "$web/fated-dragon/index.html" "$web/fated-dragon/glue.js" "$web/fated-dragon/fated-dragon.wgsl" "$stage/fated-dragon/"
  cp "$web/fated-dragon/build/fated-dragon.wasm" "$stage/fated-dragon/build/"
  cp "$web/fated-dragon/assets/dragon.gbd1" "$stage/fated-dragon/assets/"
  # SPLICEBOTS
  cp "$web/splicebots/index.html" "$web/splicebots/splice-jam.wgsl" "$stage/splicebots/"
  cp "$web/splicebots/build/splice-jam.wasm" "$stage/splicebots/build/"
  cp "$web/splicebots/fonts/"* "$stage/splicebots/fonts/"
  cp "$web/splicebots/packs/"*.pack "$stage/splicebots/packs/"
  # The Whispering Spring (audio-manifest.json stays unversioned: the artist's files swap in without a release)
  cp "$web/whispering-spring/index.html" "$web/whispering-spring/glue.js" "$web/whispering-spring/audio.js" "$web/whispering-spring/audio-manifest.json" "$web/whispering-spring/whispering-spring.wgsl" "$stage/whispering-spring/"
  cp "$web/whispering-spring/build/whispering-spring.wasm" "$stage/whispering-spring/build/"
  # Empty response for host.js optional viewer check so the console stays free of 404s.
  : > "$stage/api/auth/viewer"
  exit
fi

if [[ "$2" == release ]]; then
  name="$(cat \
    "$stage/coaxial/"*.js \
    "$stage/build/"*.wasm \
    "$stage/fonts/"*.woff2 \
    "$stage/fated-dragon/glue.js" \
    "$stage/fated-dragon/build/fated-dragon.wasm" \
    "$stage/fated-dragon/fated-dragon.wgsl" \
    "$stage/fated-dragon/assets/dragon.gbd1" \
    "$stage/splicebots/build/splice-jam.wasm" \
    "$stage/splicebots/splice-jam.wgsl" \
    "$stage/splicebots/fonts/"* \
    "$stage/whispering-spring/glue.js" \
    "$stage/whispering-spring/audio.js" \
    "$stage/whispering-spring/build/whispering-spring.wasm" \
    "$stage/whispering-spring/whispering-spring.wgsl" \
    | shasum -a 256 | cut -c1-16)"
  mkdir -p "$stage/b/$name/coaxial" "$stage/b/$name/build" "$stage/b/$name/fonts" \
    "$stage/b/$name/fated-dragon/build" "$stage/b/$name/fated-dragon/assets" \
    "$stage/b/$name/splicebots/build" "$stage/b/$name/splicebots/fonts" \
    "$stage/b/$name/whispering-spring/build"
  mv "$stage/coaxial/"*.js "$stage/b/$name/coaxial/"
  rmdir "$stage/coaxial"
  mv "$stage/build/"*.wasm "$stage/b/$name/build/"
  rmdir "$stage/build"
  mv "$stage/fonts/"*.woff2 "$stage/b/$name/fonts/"
  mv "$stage/fated-dragon/glue.js" "$stage/fated-dragon/fated-dragon.wgsl" "$stage/b/$name/fated-dragon/"
  mv "$stage/fated-dragon/build/fated-dragon.wasm" "$stage/b/$name/fated-dragon/build/"
  rmdir "$stage/fated-dragon/build"
  mv "$stage/fated-dragon/assets/dragon.gbd1" "$stage/b/$name/fated-dragon/assets/"
  rmdir "$stage/fated-dragon/assets"
  mv "$stage/splicebots/splice-jam.wgsl" "$stage/b/$name/splicebots/"
  mv "$stage/splicebots/build/splice-jam.wasm" "$stage/b/$name/splicebots/build/"
  rmdir "$stage/splicebots/build"
  mv "$stage/splicebots/fonts/"* "$stage/b/$name/splicebots/fonts/"
  rmdir "$stage/splicebots/fonts"
  mkdir -p "$stage/packs/splicebots"
  mv "$stage/splicebots/packs/"*.pack "$stage/packs/splicebots/"
  rmdir "$stage/splicebots/packs"
  # The Whispering Spring: its code is versioned; its page and audio-manifest.json are not
  mv "$stage/whispering-spring/glue.js" "$stage/whispering-spring/audio.js" "$stage/whispering-spring/whispering-spring.wgsl" "$stage/b/$name/whispering-spring/"
  mv "$stage/whispering-spring/build/whispering-spring.wasm" "$stage/b/$name/whispering-spring/build/"
  rmdir "$stage/whispering-spring/build"

  sed -i.was -e "s|/fonts/\([A-Za-z-]*\)\.woff2|/b/$name/fonts/\1.woff2|g" "$stage/index.html" "$stage/credits.html"
  rm -f "$stage/index.html.was" "$stage/credits.html.was"
  sed -i.was \
    -e "s|/fonts/\([A-Za-z-]*\)\.woff2|/b/$name/fonts/\1.woff2|g" \
    -e "s|\"/coaxial/host\.js\"|\"/b/$name/coaxial/host.js\"|g" \
    -e "s|\"/fated-dragon/build/fated-dragon\.wasm\"|\"/b/$name/fated-dragon/build/fated-dragon.wasm\"|g" \
    -e "s|\"/fated-dragon/fated-dragon\.wgsl\"|\"/b/$name/fated-dragon/fated-dragon.wgsl\"|g" \
    -e "s|data:/fated-dragon/assets/dragon\.gbd1|data:/b/$name/fated-dragon/assets/dragon.gbd1|g" \
    -e "s|\"/fated-dragon/glue\.js\"|\"/b/$name/fated-dragon/glue.js\"|g" \
    "$stage/fated-dragon/index.html"
  rm -f "$stage/fated-dragon/index.html.was"
  sed -i.was \
    -e "s|/fonts/\([A-Za-z-]*\)\.woff2|/b/$name/fonts/\1.woff2|g" \
    -e "s|\"/coaxial/host\.js\"|\"/b/$name/coaxial/host.js\"|g" \
    -e "s|\"/splicebots/build/splice-jam\.wasm\"|\"/b/$name/splicebots/build/splice-jam.wasm\"|g" \
    -e "s|\"/splicebots/splice-jam\.wgsl\"|\"/b/$name/splicebots/splice-jam.wgsl\"|g" \
    -e "s|\"/splicebots/fonts/|\"/b/$name/splicebots/fonts/|g" \
    -e "s|pack:/splicebots/packs/|pack:/packs/splicebots/|g" \
    "$stage/splicebots/index.html"
  rm -f "$stage/splicebots/index.html.was"
  sed -i.was \
    -e "s|\"/coaxial/host\.js\"|\"/b/$name/coaxial/host.js\"|g" \
    -e "s|\"/whispering-spring/build/whispering-spring\.wasm\"|\"/b/$name/whispering-spring/build/whispering-spring.wasm\"|g" \
    -e "s|\"/whispering-spring/whispering-spring\.wgsl\"|\"/b/$name/whispering-spring/whispering-spring.wgsl\"|g" \
    -e "s|\"/whispering-spring/glue\.js\"|\"/b/$name/whispering-spring/glue.js\"|g" \
    "$stage/whispering-spring/index.html"
  rm -f "$stage/whispering-spring/index.html.was"
  echo "$name"
  exit
fi

if [[ "$2" == stored ]]; then
  find "$stage/b" -type f \( -name '*.js' -o -name '*.wasm' -o -name '*.wgsl' -o -name '*.gbd1' \) \
    -exec sh -c 'gzip -9 -n "$1" && mv "$1.gz" "$1"' _ {} \;
  exit
fi
echo "usage: scripts/web-stage.sh DIR [release|stored]" >&2; exit 2
