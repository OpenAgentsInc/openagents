# Exact renderer golden observations

Run the complete observer with supplied, already-built products:

```sh
coax/web/tool.sh aethyr/tests/check-render-goldens.coax \
  PRODUCER.wasm PACKAGE PAGE.html EMPTY_OUTPUT \
  --frames 90 --size=320x180
```

The output must be a new or empty directory. The checker refuses a nonempty
directory before writing instrumentation, including when a supplied input is
named `observed-host.js` inside that directory. It does not build a producer,
shader, or package.

The original controls remain available: `--start`, `--frames N` (1–600),
`--size=WIDTHxHEIGHT` (each 1–8192), `--query=FORM`, `--legacy-wgsl FILE`,
`--legacy-particles FILE`, and either `--reuse-generic DIR` or
`--reuse-legacy DIR`. The first occurrence wins and unknown options retain
their original ignored behavior. Frame numbers preserve JavaScript Number
spellings and integer rounding; viewport components remain ASCII digits.
Query encoding follows URLSearchParams, including malformed UTF-8 replacement
and the original positions of the three probe flags. `--surface-root DIR`
explicitly selects a retained browser surface instead of the current checkout.

Coax owns HTML and anchored observation transforms, origin routing, the exact
CDP command sequence, readiness, fixed-step counts, deadlines, event admission,
upload ranges, source fences, reuse and verdicts. `render-golden-effects.js`
only calls the browser SDK, reads typed arrays, hashes them and sends the
requested byte ranges. Chrome stays headless and muted, with the original
three background flags and device metrics.

Both routes publish `gpu.bin`, `u.bin`, `hud.bin`, `draws.bin` and a receipt.
CDP commands/messages and Runtime/Log events remain beside the receipt. The
loopback origin records original and served hashes, every request, WebSocket
lifecycle, POST acknowledgement, and final ownership. Complete captures use
up to eight explicit 64 MiB sinks per field; the 512 MiB envelope includes an
8192×8192 RGBA16F image. Coax assembles, hashes and compares the raw bytes in
64 KiB chunks. No image mask or tolerance is applied.

A comparison refuses different producer Wasm, uniform/HUD/draw identities,
physical image shapes, changed sources, Runtime exceptions or error-level
Log entries. A difference in the images remains a diagnostic: exit 0 with
`identical: false` and exact channel/group-of-four/max-byte counters when all
those joins pass. This is the original migration-observer behavior.

Reuse verifies the original route, controls, validation arrays, first Wasm
identity, and every canonical captured file's length and hash before copying
the receipt unchanged. A reused receipt keeps its historical observer and
source provenance; it is not promoted to a current-source capture. Its old
source files need not still be installed. Node-style lexical absolute paths
preserve override control spellings such as `/tmp`, independently of the
canonical paths required by the owned byte sinks.

The bounded port qualification is recorded in
`docs/verification/aethyr-render-golden-tool-port-2026-11-02.json`. It uses
frozen small realm products, not a new Grid, renderer-math, producer or
current-compiler release qualification.
