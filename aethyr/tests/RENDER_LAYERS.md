# The layer resource check

Run `check-render-layers.coax` against an existing compiled catalogue host and
shader package. It never builds a producer, shader or realm frame:

```sh
coax/web/tool.sh aethyr/tests/check-render-layers.coax \
  --wasm HOST.wasm --package PACKAGE_DIR --work-dir NEW_DIR --profile 7
```

`--surface-root DIR` selects an owned fixture tree containing `coax/web/render.js`,
`coax/web/render-face.c` and `apps/openagents-web/`; it defaults to the repository.
The output directory must be new. `--reference OLD_RECEIPT.json` compares all
stable resource status fields with a retained original observer result.

Coax owns VPK header/table/span traversal, selected image hashes, source fences,
exact upload ranges, capture metadata admission and `textures.config`. The small
reason-1 `render-layer-effects.js` performs WebAssembly, WebGPU, ImageBitmap and
HTTP effects. Hashes/counts of the exact compiled Wasm, admitted package and
decoded ImageBitmap inputs are joined to the locally fenced bytes. Production
`RenderHost.makeLayers` and its real GPU texture readback remain the pixel path.

Each input image/pack uses the shared 64 MiB file fence. A complete readback may
be up to the original 256 MiB POST envelope: Coax declares at most four 64 MiB
POST sinks, schedules each exact range, checks its ACK/count/hash, finalizes the
origin, then assembles and hashes all bytes before atomic publication. This
supports the 67,236,864-byte individual-layer fixture without increasing the
shared origin's bounds. No texture row, mip, layer or transfer padding is masked.
The browser observation retains its original 180-second deadline and 30-second
CDP calls. All browsers are headless and muted.

The resource receipt keeps schema `aethyr.render.layer-effects.v1`, the browser
version and complete legacy `status` observations. Additional fields name the
actual compiler/observer images, finalized origin, transport part count and
reference equality. `layers.bin` contains the complete physical readback;
`textures.config` describes its exact pitches and offsets. A failed source join
retains a structured failed receipt and the raw observation files, and publishes
no qualified layer artifact. An HTTP ACK alone cannot qualify it.

`render-layer-policy.coax` owns the mutation-checked decisions.
`render-layer-tool-checks.coax` compares every byte, all stable metadata/browser
fields, original shared source hashes and both texture configurations (only the
declared owned file path is normalized and canonicalized).
`render-layer-control-checks.coax` plants malformed pack, image and transport
controls in copied fixture trees. `render-layer-recovery-check.coax` joins the
repaired full int32 CLI refusal to its unchanged original observation.
`render-layer-source-check.coax` exercises source changes after capture and,
with `--after-post`, after the upload ACK but before final source qualification.

The qualified packed and individual GPU observations are reused after the
tested CLI/failed-receipt repairs because their exact Wasm, production renderer,
package and source-image closures are unchanged. Each actual tool image and its
source epoch are retained separately. This proves resource conformance and tool
parity; it does not qualify a whole-realm frame, new product emission, current-main
compiler, or a native/CPU texture implementation.

Qualification: `docs/verification/aethyr-render-layer-tool-port-2026-11-02.json`.
