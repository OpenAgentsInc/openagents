# Coast C2 kit verification

The retained pack is `105ec8dc07ec7fbd5efa0acc3a57232b95435086e81680b1a3dad58619873d7c`:
487,425 bytes, 108 static meshes (36 models at three detail levels), and four
animated wildlife rigs. Decoded textures occupy 393,216 bytes. All assets
are original Reference-mode work; no licensed content or lighting bake runs.

`base-*.jpg` and `lod-*.jpg` retain reviewed Eevee raster previews.
`effects.jpg` shows the 16-frame analytic surf-spray and sea-mist sheets.
The native captures place each static model in the coast. The four wildlife
captures add the admitted bind-pose rigs to the capture scene only; C4 owns
live wildlife. `capture.json` retains the pack identity, adapter, camera,
image digest, and static GPU allocation. This is visual evidence, not a
frame-time benchmark. C6 owns complete climate and tier timing qualification.

Validation uses the artifact compiler's filtered test, the coast library's
CPU tests (including exact source-to-pin reproduction, LOD coverage,
collision, terrain seams, and resident bounds), formatting, and the coast's
WASM check. The resident bound reserves 64 MiB for water in addition to all
static geometry and decoded textures and stays below Low's 160 MiB budget.

Run the ignored `coast_kit_capture` example test under a GPU lease with
`COAST_KIT_CAPTURE_OUTPUT`, `VERSE_QUALITY=high`, and
`RUST_MIN_STACK=67108864`. Compile with a build lease and the agent's external
target directory. The capture uses offscreen raster rendering only.
