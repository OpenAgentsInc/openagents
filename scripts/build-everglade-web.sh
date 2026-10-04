#!/usr/bin/env bash
# Build Everglade for the browser.
#
# Writes into OUTDIR:
#
#   everglade_web.js        the wasm-bindgen glue (an ES module; call its
#                           default export, `init`)
#   everglade_web_bg.wasm   the module
#
# The page serves them with a `<canvas id="everglade-canvas">` and serves the
# pinned pack at `/everglade/pack/<PACK_SHA256>.vtp` on the same origin; read
# `crates/everglade-web/README.md`.
#
# With `--with-pack`, also writes a local test page: `index.html` and the
# committed pack at `everglade/pack/<PACK_SHA256>.vtp`, so any static server
# rooted at OUTDIR shows the glade, for example
# `python3 -m http.server -d OUTDIR 8080`.
#
# Prerequisites: the pinned toolchain's wasm32-unknown-unknown target; the
# `wasm-bindgen` CLI at the version Cargo.lock pins (`cargo install
# wasm-bindgen-cli --version VERSION --locked`); and a clang with the
# WebAssembly target for secp256k1's C, found on PATH or from Homebrew's
# `llvm`, or named in `CC_wasm32_unknown_unknown`. `wasm-opt` (Binaryen) is
# used when present.
#
# Usage: ./scripts/build-everglade-web.sh [--with-pack] OUTDIR
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
target="wasm32-unknown-unknown"
target_dir="${CARGO_TARGET_DIR:-$root/target}"

with_pack=false
out=""
for arg in "$@"; do
  case "$arg" in
    --with-pack) with_pack=true ;;
    -*) echo "unknown option $arg" >&2; exit 64 ;;
    *) out="$arg" ;;
  esac
done
if [ -z "$out" ]; then
  echo "usage: $0 [--with-pack] OUTDIR" >&2
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

# A clang that targets WebAssembly. Apple's does not.
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
cargo build --release --locked --target "$target" -p everglade-web
mkdir -p "$out"
wasm-bindgen --target web --no-typescript --out-dir "$out" \
  "$target_dir/$target/release/everglade_web.wasm"

if command -v wasm-opt >/dev/null 2>&1; then
  # The features Rust's wasm32 target enables by default.
  wasm-opt -O2 \
    --enable-bulk-memory --enable-mutable-globals --enable-nontrapping-float-to-int \
    --enable-sign-ext --enable-reference-types --enable-multivalue \
    "$out/everglade_web_bg.wasm" -o "$out/everglade_web_bg.wasm"
else
  echo "wasm-opt not found; the module is not size-optimized" >&2
fi

if [ "$with_pack" = true ]; then
  sha="$(awk -F'"' '/^pub const PACK_SHA256/ { print $2 }' crates/verse/src/zones/everglade_pack.rs)"
  mkdir -p "$out/everglade/pack"
  cp "assets/verse/everglade/$sha.vtp" "$out/everglade/pack/$sha.vtp"
  cp crates/everglade-web/index.html "$out/index.html"
fi

ls -l "$out"
