#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: builds the bundle and the Wasm module, serves them, and drives headless Chrome)
# Psionic chapter 59: chapter 54's search in the browser. Builds the bundle
# (coax/web/tool.sh psionic/tools/book-search.coax --bundle) with the native
# result line of every held-out query, compiles search.coax to Wasm
# (coax/web/wasm.sh), serves both with `coax serve --static` on 127.0.0.1,
# runs the page in headless, muted Chrome (coax/web/tool.sh shot) on the
# bundle and on three refused bundles, compares the browser's result lines
# with the native ones byte for byte, and writes the receipt.
#   psionic/web/book-search/check.sh [RECEIPT]   (default psionic/fixtures/browser-search/receipt.json)
# Exit 0 only when the lines are equal and each refused bundle is refused
# with its own verdict.
set -euo pipefail
cd "$(dirname "$0")/../../.."
receipt=${1:-psionic/fixtures/browser-search/receipt.json}
out=coax/web/build/book-search-web; port=${CH59_PORT:-8859}; wait_ms=${CH59_WAIT_MS:-15000}
mkdir -p "$out"
coax/web/tool.sh psionic/tools/book-search.coax --bundle "$out/bundle" --reference psionic/fixtures/book-search/heldout.tsv 2>&1 | grep '^book-search:' >&2
coax/web/wasm.sh psionic/web/book-search/search.coax psionic/web/book-search/face.c "$out/search.wasm"
cp psionic/web/book-search/index.html psionic/fixtures/book-search/heldout.tsv "$out/"

# Three bundles the page must refuse. wrong-encoder: the manifest and index are
# the real ones, the encoder's last byte (in its query vector) differs.
# other-encoder: that changed encoder with a manifest naming it (same size,
# same 64 numbers per word), beside the real index. stale: the real encoder
# and index with a manifest whose corpus digest names other books.
for v in wrong-encoder other-encoder stale; do rm -rf "${out:?}/$v"; cp -r "$out/bundle" "$out/$v"; rm -f "$out/$v/reference.txt"; done
for v in wrong-encoder other-encoder; do
  size=$(stat -c %s "$out/$v/encoder.bin")
  last=$(od -An -tu1 -j $((size - 1)) -N1 "$out/$v/encoder.bin" | tr -d ' ')
  printf "$(printf '\\%03o' $(((last + 1) % 256)))" | dd of="$out/$v/encoder.bin" bs=1 seek=$((size - 1)) conv=notrunc status=none
done
sum=$(sha256sum "$out/other-encoder/encoder.bin" | cut -d' ' -f1)
sed -i "s/^encoder [0-9a-f]* /encoder $sum /" "$out/other-encoder/manifest.txt"
other=$(printf 'other books' | sha256sum | cut -d' ' -f1)
sed -i "s/^corpus .*/corpus $other/" "$out/stale/manifest.txt"

coax/web/serve.sh "$out" --listen "127.0.0.1:$port" --static > "$out/serve.log" 2>&1 &
server=$!
trap 'kill $server 2>/dev/null || true' EXIT
for _ in $(seq 50); do curl -sf "http://127.0.0.1:$port/index.html" > /dev/null && break; sleep 0.1; done
for v in bundle wrong-encoder other-encoder stale; do
  coax/web/tool.sh shot "http://127.0.0.1:$port/index.html?bundle=$v&queries=heldout.tsv" "$out/$v.png" 1000x700 "$wait_ms" \
    --eval 'JSON.stringify(window.bookSearch)' > "$out/$v.shot"
  sed -n 1p "$out/$v.shot" > "$out/$v.json"
done
python3 -I psionic/web/book-search/receipt.py "$out" "$receipt"
