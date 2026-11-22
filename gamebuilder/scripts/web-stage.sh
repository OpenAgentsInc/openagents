#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around cp, shasum, sed and gzip; holds no logic).
# Stages gamebuilder.org from a finished web/build.sh output:
#   scripts/web-stage.sh DIR            unversioned site tree (/, /credits, /fated-dragon, /splicebots, /whispering-spring, /ice-cream-man, /scrappers-arena)
#   scripts/web-stage.sh DIR release    moves immutable release code/shaders/fonts under /b/NAME/ and rewrites HTML; prints NAME
#   scripts/web-stage.sh DIR stored     gzips .js/.wasm/.wgsl/.gbd1/.wsg/.indices/.gbm under /b/ in place for Content-Encoding: gzip
set -euo pipefail
gb="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo="$(cd "$gb/.." && pwd)"
export GB_ROOT="$gb" GB_VENDOR="$repo" COAX="${COAX:-$repo/coax/bin/coax}"
stage="$1"; web="$GB_ROOT/web"

if [[ "${2:-}" == "" ]]; then
  mkdir -p "$stage/build" "$stage/coaxial" "$stage/fonts" "$stage/img" "$stage/api/auth" \
    "$stage/fated-dragon/build" "$stage/fated-dragon/assets" \
    "$stage/splicebots/build" "$stage/splicebots/fonts" "$stage/splicebots/packs" \
    "$stage/whispering-spring/build" "$stage/whispering-spring/assets" \
    "$stage/ice-cream-man/build" "$stage/ice-cream-man/assets" \
    "$stage/scrappers-arena/build" "$stage/scrappers-arena/assets"
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
  cp "$web/whispering-spring/index.html" "$web/whispering-spring/glue.js" "$web/whispering-spring/audio.js" "$web/whispering-spring/room.js" "$web/whispering-spring/audio-manifest.json" "$web/whispering-spring/whispering-spring.wgsl" "$stage/whispering-spring/"
  cp "$web/whispering-spring/build/whispering-spring.wasm" "$stage/whispering-spring/build/"
  cp "$web/whispering-spring/assets/grove.wsg" "$stage/whispering-spring/assets/"
  # Ice Cream Man
  cp "$web/ice-cream-man/index.html" "$web/ice-cream-man/icecream.wgsl" "$stage/ice-cream-man/"
  cp "$web/ice-cream-man/build/icecream.wasm" "$stage/ice-cream-man/build/"
  cp "$web/ice-cream-man/assets/IceCream.indices" "$stage/ice-cream-man/assets/"
  # Scrappers Arena
  cp "$web/scrappers-arena/index.html" "$web/scrappers-arena/glue.js" "$web/scrappers-arena/scrappers-arena.wgsl" "$stage/scrappers-arena/"
  cp "$web/scrappers-arena/build/scrappers-arena.wasm" "$stage/scrappers-arena/build/"
  cp "$web/scrappers-arena/assets/scrappers.gbm" "$stage/scrappers-arena/assets/"
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
    "$stage/whispering-spring/room.js" \
    "$stage/whispering-spring/build/whispering-spring.wasm" \
    "$stage/whispering-spring/whispering-spring.wgsl" \
    "$stage/whispering-spring/assets/grove.wsg" \
    "$stage/ice-cream-man/build/icecream.wasm" \
    "$stage/ice-cream-man/icecream.wgsl" \
    "$stage/ice-cream-man/assets/IceCream.indices" \
    "$stage/scrappers-arena/glue.js" \
    "$stage/scrappers-arena/build/scrappers-arena.wasm" \
    "$stage/scrappers-arena/scrappers-arena.wgsl" \
    "$stage/scrappers-arena/assets/scrappers.gbm" \
    | shasum -a 256 | cut -c1-16)"
  mkdir -p "$stage/b/$name/coaxial" "$stage/b/$name/build" "$stage/b/$name/fonts" \
    "$stage/b/$name/fated-dragon/build" "$stage/b/$name/fated-dragon/assets" \
    "$stage/b/$name/splicebots/build" "$stage/b/$name/splicebots/fonts" \
    "$stage/b/$name/whispering-spring/build" "$stage/b/$name/whispering-spring/assets" \
    "$stage/b/$name/ice-cream-man/build" "$stage/b/$name/ice-cream-man/assets" \
    "$stage/b/$name/scrappers-arena/build" "$stage/b/$name/scrappers-arena/assets"
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
  # The Whispering Spring: its code is versioned; its page and audio-manifest.json are not.
  # Move audio-manifest.json under /manifests/whispering-spring/ so no whispering-spring/ prefix
  # collides with the extensionless /whispering-spring GCS object.
  mv "$stage/whispering-spring/glue.js" "$stage/whispering-spring/audio.js" "$stage/whispering-spring/room.js" "$stage/whispering-spring/whispering-spring.wgsl" "$stage/b/$name/whispering-spring/"
  mv "$stage/whispering-spring/build/whispering-spring.wasm" "$stage/b/$name/whispering-spring/build/"
  rmdir "$stage/whispering-spring/build"
  mv "$stage/whispering-spring/assets/grove.wsg" "$stage/b/$name/whispering-spring/assets/"
  rmdir "$stage/whispering-spring/assets"
  mkdir -p "$stage/manifests/whispering-spring"
  mv "$stage/whispering-spring/audio-manifest.json" "$stage/manifests/whispering-spring/"
  # Ice Cream Man
  mv "$stage/ice-cream-man/icecream.wgsl" "$stage/b/$name/ice-cream-man/"
  mv "$stage/ice-cream-man/build/icecream.wasm" "$stage/b/$name/ice-cream-man/build/"
  rmdir "$stage/ice-cream-man/build"
  mv "$stage/ice-cream-man/assets/IceCream.indices" "$stage/b/$name/ice-cream-man/assets/"
  rmdir "$stage/ice-cream-man/assets"
  # Scrappers Arena
  mv "$stage/scrappers-arena/glue.js" "$stage/scrappers-arena/scrappers-arena.wgsl" "$stage/b/$name/scrappers-arena/"
  mv "$stage/scrappers-arena/build/scrappers-arena.wasm" "$stage/b/$name/scrappers-arena/build/"
  rmdir "$stage/scrappers-arena/build"
  mv "$stage/scrappers-arena/assets/scrappers.gbm" "$stage/b/$name/scrappers-arena/assets/"
  rmdir "$stage/scrappers-arena/assets"

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
    -e "s|data:/whispering-spring/assets/grove\.wsg|data:/b/$name/whispering-spring/assets/grove.wsg|g" \
    "$stage/whispering-spring/index.html"
  rm -f "$stage/whispering-spring/index.html.was"
  sed -i.was \
    -e "s|\"/whispering-spring/audio-manifest\.json\"|\"/manifests/whispering-spring/audio-manifest.json\"|g" \
    "$stage/b/$name/whispering-spring/glue.js"
  rm -f "$stage/b/$name/whispering-spring/glue.js.was"
  sed -i.was \
    -e "s|/fonts/\([A-Za-z-]*\)\.woff2|/b/$name/fonts/\1.woff2|g" \
    -e "s|\"/coaxial/host\.js\"|\"/b/$name/coaxial/host.js\"|g" \
    -e "s|\"/ice-cream-man/build/icecream\.wasm\"|\"/b/$name/ice-cream-man/build/icecream.wasm\"|g" \
    -e "s|\"/ice-cream-man/icecream\.wgsl\"|\"/b/$name/ice-cream-man/icecream.wgsl\"|g" \
    -e "s|index:/ice-cream-man/assets/IceCream\.indices|index:/b/$name/ice-cream-man/assets/IceCream.indices|g" \
    "$stage/ice-cream-man/index.html"
  rm -f "$stage/ice-cream-man/index.html.was"
  sed -i.was \
    -e "s|/fonts/\([A-Za-z-]*\)\.woff2|/b/$name/fonts/\1.woff2|g" \
    -e "s|\"/coaxial/host\.js\"|\"/b/$name/coaxial/host.js\"|g" \
    -e "s|\"/scrappers-arena/build/scrappers-arena\.wasm\"|\"/b/$name/scrappers-arena/build/scrappers-arena.wasm\"|g" \
    -e "s|\"/scrappers-arena/scrappers-arena\.wgsl\"|\"/b/$name/scrappers-arena/scrappers-arena.wgsl\"|g" \
    -e "s|data:/scrappers-arena/assets/scrappers\.gbm|data:/b/$name/scrappers-arena/assets/scrappers.gbm|g" \
    -e "s|\"/scrappers-arena/glue\.js\"|\"/b/$name/scrappers-arena/glue.js\"|g" \
    "$stage/scrappers-arena/index.html"
  rm -f "$stage/scrappers-arena/index.html.was"
  echo "$name"
  exit
fi

if [[ "$2" == stored ]]; then
  find "$stage/b" -type f \( -name '*.js' -o -name '*.wasm' -o -name '*.wgsl' -o -name '*.gbd1' -o -name '*.wsg' -o -name '*.indices' -o -name '*.gbm' \) \
    -exec sh -c 'gzip -9 -n "$1" && mv "$1.gz" "$1"' _ {} \;
  exit
fi
echo "usage: scripts/web-stage.sh DIR [release|stored]" >&2; exit 2
