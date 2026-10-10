#!/usr/bin/env bash
# Build the sealed-inference demo (openagents.com/att) for the browser.
#
# Writes into OUTDIR:
#
#   att_web.js        the wasm-bindgen glue (an ES module)
#   att_web_bg.wasm   the module
#   start.js          the page's one script: imports the glue and starts it
#   att.css           the page's stylesheet (the page has no inline styles)
#   *.gz              a gzip copy of each, which the site sends when asked
#
# The site serves them beside its /att page. With `--with-page`, also
# writes a local test page, `index.html`, so any static server rooted at
# OUTDIR shows the demo, for example `python3 -m http.server -d OUTDIR 8080`.
#
# Prerequisites: the pinned toolchain's wasm32-unknown-unknown target and the
# `wasm-bindgen` CLI at the version Cargo.lock pins (`cargo install
# wasm-bindgen-cli --version VERSION --locked`). `wasm-opt` (Binaryen) is
# used when present.
#
# Usage: ./scripts/build-att-web.sh [--with-page] OUTDIR
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

# A clang that targets WebAssembly, for secp256k1's C (Apple's does not).
wasm_clang() {
  local candidate
  for candidate in "$(command -v clang || true)" /opt/homebrew/opt/llvm*/bin/clang /usr/local/opt/llvm*/bin/clang; do
    if [ -x "$candidate" ] && "$candidate" --print-targets 2>/dev/null | grep -q wasm32; then
      echo "$candidate"
      return 0
    fi
  done
  return 1
}
if [ -z "${CC_wasm32_unknown_unknown:-}" ]; then
  if ! CC_wasm32_unknown_unknown="$(wasm_clang)"; then
    echo "no clang with the WebAssembly target; install LLVM (for example \`brew install llvm\`) or set CC_wasm32_unknown_unknown" >&2
    exit 69
  fi
  export CC_wasm32_unknown_unknown
fi
if [ -z "${AR_wasm32_unknown_unknown:-}" ] && [ -x "$(dirname "$CC_wasm32_unknown_unknown")/llvm-ar" ]; then
  export AR_wasm32_unknown_unknown="$(dirname "$CC_wasm32_unknown_unknown")/llvm-ar"
fi

cd "$root"
# ATT_FEATURES=demo builds the capture page, a scripted round without the
# network.
cargo build --release --locked --target "$target" -p att-web ${ATT_FEATURES:+--features "$ATT_FEATURES"}
mkdir -p "$out"
wasm-bindgen --target web --no-typescript --out-dir "$out" \
  "$target_dir/$target/release/att_web.wasm"

if command -v wasm-opt >/dev/null 2>&1; then
  # The features Rust's wasm32 target enables by default.
  wasm-opt -Oz \
    --enable-bulk-memory --enable-mutable-globals --enable-nontrapping-float-to-int \
    --enable-sign-ext --enable-reference-types --enable-multivalue \
    "$out/att_web_bg.wasm" -o "$out/att_web_bg.wasm"
else
  echo "wasm-opt not found; the module is not size-optimized" >&2
fi

cp crates/att-web/start.js "$out/start.js"
cp crates/att-web/static/att.css "$out/att.css"
for file in att_web.js att_web_bg.wasm start.js att.css; do
  gzip -9 -n -c "$out/$file" > "$out/$file.gz"
done

if [ "$with_page" = true ]; then
  cp crates/att-web/index.html "$out/index.html"
fi

ls -l "$out"
