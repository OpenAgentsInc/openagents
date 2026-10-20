#!/usr/bin/env bash
# Build the signed-in GPUI web chat (apps/openagents/web/chat) for the
# browser.
#
# Writes into OUTDIR:
#
#   openagents_web_chat.js        wasm-bindgen's `--target web` glue
#   openagents_web_chat_bg.wasm   the module
#   openagents_web_files.js       the file panel's glue and module, loaded
#   openagents_web_files_bg.wasm  when the panel first opens (#12271)
#   openagents_web_room.js        a computer chat's live transcript (its
#   openagents_web_room_bg.wasm   chat2 room and Loro doc), loaded when one
#                                 first opens (#12224)
#   openagents_web_dictation.js   dictation's glue and module, loaded the
#   openagents_web_dictation_bg.wasm  first time the microphone starts (#12292)
#   openagents_web_work.js        the work panel's glue and module, loaded
#   openagents_web_work_bg.wasm   when it first opens (#12225)
#   openagents_web_settings.js    Settings' glue and module, loaded the
#   openagents_web_settings_bg.wasm  first time Settings opens (#12288)
#   start.js                      the page's one script: checks, loads, launches
#   fonts/*.ttf                   the Geist faces the chat draws, fetched apart
#   *.gz, *.br                    gzip and brotli copies, sent when asked
#
# openagents.com's server shows them at /gpui when started with
# `openagents-web --gpui-chat OUTDIR` (default /srv/gpui-chat in the image).
#
# Prerequisites: as build-web-demo.sh, the pinned toolchain's
# wasm32-unknown-unknown target and the `wasm-bindgen` CLI at the version
# web/Cargo.lock pins. `wasm-opt`, `brotli`, and Python 3 are required.
#
# Usage: apps/openagents/scripts/build-web-chat.sh OUTDIR
set -euo pipefail

app="$(cd "$(dirname "$0")/.." && pwd)"
web="$app/web"
target="wasm32-unknown-unknown"
target_dir="${CARGO_TARGET_DIR:-$web/target}"

out="${1:-}"
if [ -z "$out" ] || [ "$#" -ne 1 ]; then
  echo "usage: $0 OUTDIR" >&2
  exit 64
fi

locked="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' "$web/Cargo.lock")"
installed="$(wasm-bindgen --version 2>/dev/null | awk '{ print $2 }')"
if [ "$installed" != "$locked" ]; then
  echo "wasm-bindgen ${installed:-is not installed}; this build needs $locked: cargo install wasm-bindgen-cli --version $locked --locked" >&2
  exit 69
fi

for tool in wasm-opt brotli python3; do
  command -v "$tool" >/dev/null 2>&1 || { echo "$tool is not installed" >&2; exit 69; }
done

cargo build --locked --manifest-path "$web/Cargo.toml" -p openagents-web-chat \
  --release --target "$target"

mkdir -p "$out/fonts"
wasm-bindgen --target web --no-typescript --out-dir "$out" \
  "$target_dir/$target/release/openagents_web_chat.wasm"
wasm-opt -Os --enable-bulk-memory --enable-nontrapping-float-to-int \
    --enable-sign-ext --enable-mutable-globals --enable-reference-types \
    "$out/openagents_web_chat_bg.wasm" -o "$out/openagents_web_chat_bg.wasm"
# The file panel (#12271): its own module, which start.js loads the first
# time the panel opens.
cargo build --locked --manifest-path "$web/Cargo.toml" -p openagents-web-files \
  --release --target "$target"
wasm-bindgen --target web --no-typescript --out-dir "$out" \
  "$target_dir/$target/release/openagents_web_files.wasm"
wasm-opt -Os --enable-bulk-memory --enable-nontrapping-float-to-int \
    --enable-sign-ext --enable-mutable-globals --enable-reference-types \
    "$out/openagents_web_files_bg.wasm" -o "$out/openagents_web_files_bg.wasm"
# A computer chat's live transcript (#12224): its own module, which
# start.js loads the first time a computer chat opens.
cargo build --locked --manifest-path "$web/Cargo.toml" -p openagents-web-room \
  --release --target "$target"
wasm-bindgen --target web --no-typescript --out-dir "$out" \
  "$target_dir/$target/release/openagents_web_room.wasm"
wasm-opt -Os --enable-bulk-memory --enable-nontrapping-float-to-int \
    --enable-sign-ext --enable-mutable-globals --enable-reference-types \
    "$out/openagents_web_room_bg.wasm" -o "$out/openagents_web_room_bg.wasm"
# Dictation (#12292): its own module, which start.js loads the first time
# the composer's microphone starts.
cargo build --locked --manifest-path "$web/Cargo.toml" -p openagents-web-dictation \
  --release --target "$target"
wasm-bindgen --target web --no-typescript --out-dir "$out" \
  "$target_dir/$target/release/openagents_web_dictation.wasm"
wasm-opt -Os --enable-bulk-memory --enable-nontrapping-float-to-int \
    --enable-sign-ext --enable-mutable-globals --enable-reference-types \
    "$out/openagents_web_dictation_bg.wasm" -o "$out/openagents_web_dictation_bg.wasm"
# The work panel (#12225): its own module too, loaded when it first opens.
cargo build --locked --manifest-path "$web/Cargo.toml" -p openagents-web-work \
  --release --target "$target"
wasm-bindgen --target web --no-typescript --out-dir "$out" \
  "$target_dir/$target/release/openagents_web_work.wasm"
wasm-opt -Os --enable-bulk-memory --enable-nontrapping-float-to-int \
    --enable-sign-ext --enable-mutable-globals --enable-reference-types \
    "$out/openagents_web_work_bg.wasm" -o "$out/openagents_web_work_bg.wasm"
# Settings (#12288): its own module, which start.js loads the first time
# Settings opens.
cargo build --locked --manifest-path "$web/Cargo.toml" -p openagents-web-settings \
  --release --target "$target"
wasm-bindgen --target web --no-typescript --out-dir "$out" \
  "$target_dir/$target/release/openagents_web_settings.wasm"
wasm-opt -Os --enable-bulk-memory --enable-nontrapping-float-to-int \
    --enable-sign-ext --enable-mutable-globals --enable-reference-types \
    "$out/openagents_web_settings_bg.wasm" -o "$out/openagents_web_settings_bg.wasm"
cp "$web/chat/start.js" "$out/start.js"
# The faces start.js asks for, by the names in its FONTS list.
fonts="$(sed -n 's/^  "\([A-Za-z-]*\.ttf\)",$/\1/p' "$web/chat/start.js")"
for font in $fonts; do
  cp "$app/crates/ui/assets/fonts/$font" "$out/fonts/$font"
done
compress() {
  gzip -9 -n -c "$1" > "$1.gz"
  if command -v brotli >/dev/null 2>&1; then
    brotli -q 11 -f -o "$1.br" "$1"
  fi
}
# The first-frame painter: the page paints the frame this browser kept
# from the last visit before the module loads (#12244).
"$app/scripts/build-first-frame-painter.sh" "$out/first-frame"
python3 "$app/scripts/package-web-build.py" "$out" openagents_web_chat openagents_web_files openagents_web_room openagents_web_dictation openagents_web_work openagents_web_settings
for font in $fonts; do
  compress "$out/fonts/$font"
done
ls -l "$out" "$out/fonts"
