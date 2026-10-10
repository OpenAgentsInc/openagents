#!/usr/bin/env bash
# Build Grow Little Bunny for the browser.
#
# Writes into OUTDIR:
#
#   bunny_web.js        the wasm-bindgen glue (an ES module)
#   bunny_web_bg.wasm   the module
#   start.js            the page's one script: imports the glue and starts it
#   *.gz                a gzip copy of each, which the site sends when asked
#
# The site serves them at /games/grow-little-bunny/ from the directory it was
# started with (`openagents-web --bunny OUTDIR`). With `--with-page`, also
# writes a local test page, `index.html`, so any static server rooted at
# OUTDIR shows the game, for example `python3 -m http.server -d OUTDIR 8080`.
#
# Prerequisites: the pinned toolchain's wasm32-unknown-unknown target and the
# `wasm-bindgen` CLI at the version Cargo.lock pins (`cargo install
# wasm-bindgen-cli --version VERSION --locked`). `wasm-opt` (Binaryen) is
# used when present.
#
# Usage: ./scripts/build-bunny-web.sh [--with-page] OUTDIR
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
target="wasm32-unknown-unknown"
target_dir="${CARGO_TARGET_DIR:-$root/target}"

with_page=false
out=""
for arg in "$@"; do
  case "$arg" in
    --with-page) with_page=true ;;
    -*) echo "unknown option $arg" >&2; exit 64 ;;
    *) out="$arg" ;;
  esac
done
if [ -z "$out" ]; then
  echo "usage: $0 [--with-page] OUTDIR" >&2
  exit 64
fi

# The CLI must be the library's exact version, or the glue does not match.
locked="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' "$root/Cargo.lock")"
if ! command -v wasm-bindgen >/dev/null 2>&1; then
  echo "wasm-bindgen is not installed; run: cargo install wasm-bindgen-cli --version $locked --locked" >&2
  exit 69
fi
installed="$(wasm-bindgen --version | awk '{ print $2 }')"
if [ "$installed" != "$locked" ]; then
  echo "wasm-bindgen $installed does not match Cargo.lock's $locked; run: cargo install wasm-bindgen-cli --version $locked --locked" >&2
  exit 69
fi

cd "$root"
# BUNNY_FEATURES=autoplay builds the capture build, where the bot plays.
cargo build --release --locked --target "$target" -p bunny-web ${BUNNY_FEATURES:+--features "$BUNNY_FEATURES"}
mkdir -p "$out"
wasm-bindgen --target web --no-typescript --out-dir "$out" \
  "$target_dir/$target/release/bunny_web.wasm"

if command -v wasm-opt >/dev/null 2>&1; then
  # The features Rust's wasm32 target enables by default.
  wasm-opt -Oz \
    --enable-bulk-memory --enable-mutable-globals --enable-nontrapping-float-to-int \
    --enable-sign-ext --enable-reference-types --enable-multivalue \
    "$out/bunny_web_bg.wasm" -o "$out/bunny_web_bg.wasm"
else
  echo "wasm-opt not found; the module is not size-optimized" >&2
fi

cp crates/bunny-web/start.js "$out/start.js"
for file in bunny_web.js bunny_web_bg.wasm start.js; do
  gzip -9 -n -c "$out/$file" > "$out/$file.gz"
done

if [ "$with_page" = true ]; then
  cp crates/bunny-web/index.html "$out/index.html"
fi

ls -l "$out"
