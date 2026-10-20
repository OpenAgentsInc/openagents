#!/usr/bin/env bash
# Fetch the hand landmarker /lander9 runs in the browser, pinned by digest,
# into OUTDIR, which the site serves from its own origin at /lander9/hands/
# (`openagents-web --lander9 DIR` reads DIR/hands/):
#
#   OUTDIR/vision_bundle.mjs              MediaPipe Tasks Vision 0.10.35, the JS API
#   OUTDIR/wasm/vision_wasm_internal.js   its wasm loader
#   OUTDIR/wasm/vision_wasm_internal.wasm its runtime (SIMD build)
#   OUTDIR/hand_landmarker.task           the hand landmarker model (float16, version 1)
#   OUTDIR/NOTICE                         where each file came from and its license
#
# with brotli and gzip copies of each beside it. Both are Google's, under
# Apache-2.0. The owner approved running MediaPipe in the browser for
# /lander9 on 2026-10-20; the gestures, smoothing, and interactions are ours,
# in Rust (apps/openagents/web/hands, apps/openagents/web/graph-card).
#
# Move a pin by changing its URL and digest together.
#
# Usage: scripts/fetch-hand-landmarker.sh OUTDIR
set -euo pipefail

out="${1:-}"
if [ -z "$out" ] || [ "$#" -ne 1 ]; then
  echo "usage: $0 OUTDIR" >&2
  exit 64
fi
command -v brotli >/dev/null 2>&1 || { echo "brotli is not installed" >&2; exit 69; }

version="0.10.35"
package_url="https://registry.npmjs.org/@mediapipe/tasks-vision/-/tasks-vision-$version.tgz"
package_sha256="84597a25e13d123b5f4cbe768bb72e97a2c28c7a465f0ace287d8cbe5246bff0"
model_url="https://storage.googleapis.com/mediapipe-models/hand_landmarker/hand_landmarker/float16/1/hand_landmarker.task"
model_sha256="fbc2a30080c3c557093b5ddfc334698132eb341044ccee322ccf8bcf3607cde1"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

fetch() { # URL SHA256 DEST
  curl -fsSL --retry 3 -o "$3" "$1"
  local got
  got="$( (sha256sum "$3" 2>/dev/null || shasum -a 256 "$3") | awk '{ print $1 }')"
  if [ "$got" != "$2" ]; then
    echo "$1: SHA-256 $got, expected $2" >&2
    exit 65
  fi
}

fetch "$package_url" "$package_sha256" "$work/package.tgz"
fetch "$model_url" "$model_sha256" "$work/hand_landmarker.task"
mkdir -p "$work/package"
tar -xzf "$work/package.tgz" -C "$work/package"

mkdir -p "$out/wasm"
# The bundle's source map isn't shipped; drop its pointer.
grep -v '^//# sourceMappingURL=' "$work/package/package/vision_bundle.mjs" > "$out/vision_bundle.mjs"
cp "$work/package/package/wasm/vision_wasm_internal.js" "$work/package/package/wasm/vision_wasm_internal.wasm" "$out/wasm/"
cp "$work/hand_landmarker.task" "$out/"
cat > "$out/NOTICE" <<NOTICE
MediaPipe Tasks Vision $version (@mediapipe/tasks-vision on npm), Copyright Google LLC,
Apache License 2.0: vision_bundle.mjs, wasm/vision_wasm_internal.js, wasm/vision_wasm_internal.wasm.
$package_url (SHA-256 $package_sha256)

MediaPipe hand landmarker model, float16, version 1, Copyright Google LLC, Apache License 2.0:
hand_landmarker.task.
$model_url (SHA-256 $model_sha256)

License text: https://www.apache.org/licenses/LICENSE-2.0
NOTICE
for file in vision_bundle.mjs wasm/vision_wasm_internal.js wasm/vision_wasm_internal.wasm hand_landmarker.task; do
  gzip -9 -n -c "$out/$file" > "$out/$file.gz"
  brotli -q 11 -f -o "$out/$file.br" "$out/$file"
done
ls -l "$out" "$out/wasm"
