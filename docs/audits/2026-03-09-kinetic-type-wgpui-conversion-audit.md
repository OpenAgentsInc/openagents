# 2026-03-09 KineticType WGPUI Conversion Audit

> Historical note: This audit is a point-in-time snapshot from its date. Current product and architecture authority lives in `docs/MVP.md`, `docs/OWNERSHIP.md`, `crates/wgpui/README.md`, and current code in this repo.

- Author: Codex
- Status: complete
- Scope: audit `~/Downloads/KineticType-v1.1/`, identify what the effect actually does, and recommend how to convert it into a native WGPUI implementation with clear crate ownership

## Objective

Answer five questions:

1. What is KineticType actually doing under the hood?
2. Is it a good fit for a native WGPUI port?
3. Which parts belong in `crates/wgpui` versus an app crate?
4. What is the lowest-regret implementation order?
5. What retained repo capabilities already cover most of the work?

## Sources Reviewed

Authority and boundaries:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/wgpui/CRATE_BOUNDARIES.md`
- `crates/wgpui/README.md`
- `crates/wgpui/docs/MVP_BOUNDARIES.md`

Retained WGPUI surfaces:

- `crates/wgpui/src/lib.rs`
- `crates/wgpui/src/text.rs`
- `crates/wgpui/src/tools.rs`
- `crates/wgpui/src/effects/illuminator.rs`
- `crates/wgpui-core/src/scene.rs`
- `crates/wgpui-render/src/renderer.rs`
- `crates/wgpui/docs/rendering-pipelines.md`
- `apps/deck/README.md`
- `docs/audits/2026-03-06-wgpui-web-presentation-viewer-audit.md`

Bundle inspected:

- `~/Downloads/KineticType-v1.1/README.md`
- `~/Downloads/KineticType-v1.1/CHANGELOG.md`
- `~/Downloads/KineticType-v1.1/kinetic-type.js`
- `~/Downloads/KineticType-v1.1/index.html`
- `~/Downloads/KineticType-v1.1/example.html`

## Executive Recommendation

KineticType is a strong candidate for a native WGPUI conversion.

Unlike the Idea Garden bundle, this effect is not fundamentally an HTML app shell or a 3D scene. Its core loop is:

1. generate a low-resolution brightness field,
2. sample that field on a character grid,
3. draw visible glyphs and optional colored blobs.

That maps cleanly to retained WGPUI primitives:

- `TextSystem` for glyph measurement and layout,
- `Scene::draw_text` / `TextRun` for the text grid,
- `Quad` for character blobs and background,
- the existing animation/update loop for time-driven patterns.

The best path is:

1. implement a **generic** `KineticText`-style component in `crates/wgpui`,
2. prove it in a storybook/example lane first,
3. keep any editor/export tooling or product-specific usage in app-owned surfaces.

Do **not** port the browser editor UI into WGPUI as part of the first pass. The reusable asset here is the effect engine, not the whole HTML tool.

## Bottom Line

This effect does **not** require:

- HTML rendering,
- a webview,
- DOM spans,
- or a browser-only runtime.

It is mostly a deterministic text-grid effect. That makes it one of the cleaner “convert to WGPUI” candidates I have seen in these downloaded bundles.

The main limitations in the retained stack are not the core text effect. They are:

- no generic public texture quad API for app code,
- no retained video/GIF decode lane in desktop WGPUI,
- no generic custom fragment shader hook in the public component API,
- no built-in blur/postprocess lane for the blob backgrounds.

Those are follow-on refinements, not blockers for a strong v1.

## What KineticType Actually Does

KineticType is a single-file browser runtime plus editor, but the effect itself is simple.

## 1. It renders a tiny brightness buffer first

The runtime computes `baseCols` and `baseRows` from:

- container width/height,
- measured character width,
- measured line height,
- tile repetition settings.

Then it renders one brightness value per logical cell using one of two sources:

- a WebGL fragment shader preset, or
- a media source drawn into a tiny canvas and sampled as luminance or alpha.

Important detail: it does **not** render the final typography in WebGL. It renders a low-resolution mask and then uses that to decide which characters should appear.

## 2. It turns brightness into a text grid

After it has the brightness buffer, it:

- maps each visible grid position to a brightness value,
- applies tile mirroring,
- applies `invert` and `threshold`,
- toggles per-character or per-word visibility,
- writes characters into a grid of absolutely positioned DOM spans.

So the core visual is a binary or near-binary reveal mask over repeated text.

## 3. Optional “blob” backgrounds are just per-cell colored circles

The `charBg` mode adds an underlay:

- one DOM element per cell,
- sized around the character box,
- colored from a gradient based on cell position,
- optionally blurred,
- optionally tiled with the same mirror logic.

This is visually rich, but technically straightforward.

## 4. The browser editor is not the core effect

`index.html` is a configuration UI:

- controls,
- URL sync,
- export snippets,
- local file upload,
- media preview behavior.

That editor is useful reference material, but it is not the part that should drive the WGPUI conversion plan.

## Why This Fits WGPUI Better Than The Idea Garden Bundle

The key difference is architectural shape.

Idea Garden was:

- browser UI shell,
- file picker,
- large custom DOM layout,
- Three.js scene graph,
- orbit camera,
- particle scene,
- side labels and panels.

KineticType is:

- a brightness sampler,
- a text grid,
- optional cell underlays.

That is already close to the retained WGPUI model:

- compute state in Rust,
- emit scene primitives,
- render through one `wgpu` pipeline.

## What The Repo Already Has

## 1. Text measurement and layout

`crates/wgpui/src/text.rs` already gives us the main building blocks:

- `TextSystem::measure`
- `TextSystem::measure_styled`
- `TextSystem::measure_size`
- `TextSystem::layout`
- `TextSystem::layout_mono`

That covers the exact first requirement of KineticType: determine cell size from font metrics and lay out repeated glyph rows.

The default retained mono lane is already suitable too:

- JetBrains Mono is loaded into the WGPUI text system
- monospace layout is explicit

For a first pass, that is enough.

## 2. Scene-level text and quad primitives

`crates/wgpui-core/src/scene.rs` already exposes:

- `TextRun`
- `GlyphInstance`
- `Scene::draw_text`
- `Quad`
- `Scene::draw_quad`

That means the WGPUI version can render:

- rows of text,
- one-glyph runs if needed,
- colored circular or rounded background quads,
- multi-layer composition.

## 3. Animation/update primitives

The WGPUI crate already retains:

- animation helpers,
- timing utilities,
- live update loops in desktop/web platform lanes.

For KineticType, we only need a stable `time_seconds` input per frame and a repaint loop.

## 4. Static image decoding helpers

`crates/wgpui/src/tools.rs` already has:

- `load_image_from_bytes`
- `load_image_from_path`
- `load_image_from_url` behind `network`

That is enough for a first retained **static image mask** lane on desktop or web. The browser bundle’s image-mask concept can therefore be ported without first adding a full video pipeline.

## 5. Storybook/example proving grounds

Per the retained WGPUI docs and examples, the right place to validate this visually before app integration is:

- `crates/wgpui/examples/`
- or storybook/demo lanes behind feature flags

That matches crate ownership: the effect is generic, so prove it generically first.

## What WGPUI Does Not Already Have

## 1. No public “draw arbitrary uploaded texture” API for app code

WGPUI’s renderer has an image pipeline, but the public scene model exposed to callers is centered on:

- text,
- quads,
- SVG quads,
- meshes.

That is fine for KineticType v1 because the effect only needs brightness sampling, not source-image display.

It does mean:

- an exact source-preview mode,
- arbitrary mask texture display,
- or texture compositing helpers,

would need extra reusable API later.

## 2. No retained video/GIF media lane for desktop WGPUI

The browser version supports:

- `HTMLImageElement`
- `HTMLVideoElement`
- GIF decoding in the editor

The retained desktop WGPUI stack does not already own a video decode or animated GIF lane suitable for this effect. So parity should be staged:

- v1: shader presets + static images
- later: animated GIF/video if truly needed

## 3. No public custom fragment shader hook

The current public WGPUI API does not expose a “drop in custom fragment shader and sample it into a field” surface.

That means the easiest first port is **not** “port the GLSL literally.” The easiest first port is:

- port the preset math to Rust,
- compute brightness values on the CPU,
- feed those values into the text grid.

This is acceptable because the effect resolution is deliberately tiny: one sample per character cell.

## 4. No built-in blur/postprocess for blob backgrounds

KineticType’s `charBgBlur` is a browser CSS blur. WGPUI currently has:

- quads,
- layering,
- simple lighting-like effects such as `Illuminator`,

but not a generic blur filter for arbitrary quads.

That means blob parity should be staged:

- v1: crisp circular/rounded quads
- v2: approximate glow using layered alpha quads or a reusable blur-like effect

## Recommended Ownership Split

## `crates/wgpui`

This is the right home for the reusable effect engine if the API stays generic.

What belongs here:

- a generic `KineticText` or `GlyphField` component
- generic config types:
  - text
  - font size
  - letter spacing
  - threshold
  - invert
  - tile cols/rows
  - mirror mode
  - shader preset enum
  - blob background options
- generic scalar-field generation
- generic image-mask sampling from decoded pixel buffers

Why:

- the effect is product-agnostic,
- it is a UI/rendering surface,
- it can be reused by multiple apps/examples.

## `crates/wgpui-render` / `crates/wgpui-core`

Only touch these if profiling or parity proves a real reusable primitive is missing.

Possible later reusable additions:

- uploaded texture quad API
- generic scalar-field render pass
- blur-like postprocess utility
- lower-level glyph run builder helper

Do not start here.

## `apps/autopilot-desktop`

This is where product usage belongs, not the generic implementation.

What belongs here:

- pane placement
- product-specific copy/presets
- integration with app state
- any Autopilot-specific theme choices or triggers

## What The Conversion Should Look Like

## Phase 1: CPU Scalar Field + Text Grid

This should be the first real implementation.

Algorithm:

1. Measure cell width/height from the chosen font using `TextSystem`.
2. Compute `cols`, `rows`, `base_cols`, and `base_rows`.
3. Fill a `Vec<f32>` or `Vec<u8>` brightness field of size `base_cols * base_rows`.
4. For each visible grid cell:
   - map to tile-local coordinates,
   - apply mirror rules,
   - sample brightness,
   - apply threshold/invert,
   - decide visible character.
5. Build text runs for each row and draw them.
6. Optionally draw blob quads behind visible cells.

This exactly mirrors the browser implementation, minus the DOM.

### Why CPU-first is acceptable

The effect resolution is low by design.

At a representative desktop size:

- ~120 columns
- ~50 rows
- ~6,000 cells

Even a full brightness pass plus row-string regeneration is reasonable for a first implementation, especially because:

- the glyph atlas is cached,
- the preset math is simple,
- the effect does not require per-pixel full-screen rendering.

## Phase 2: Shader Preset Port As Pure Rust Functions

Port the eight presets as pure Rust brightness functions over normalized UV and time:

- `ripple`
- `column`
- `sine`
- `vortex`
- `slant`
- `erosion`
- `fisheye`
- `corner`

This gives near-total effect parity without any new renderer work.

The `erosion` preset is the only one that needs extra care because it depends on simplex noise. Even that is tractable as a Rust utility function.

## Phase 3: Blob Backgrounds

Implement `charBg` using WGPUI quads.

Mapping:

- browser circular `div` -> rounded `Quad`
- padding -> expanded bounds
- offset -> shifted quad bounds
- gradient sampling -> color choice from normalized cell position
- threshold hide/show -> same visibility mask as glyphs

Suggested v1 limitation:

- support `step` and `linear` palette sampling
- omit true blur until profiling or product need justifies it

## Phase 4: Static Image Mask Support

Implement a generic pixel-buffer mask source:

- decode image bytes via `load_image_from_path` / `load_image_from_bytes`
- resample to `base_cols x base_rows`
- compute luminance or alpha
- feed into the same brightness field

That preserves the cleanest v1.1 addition from the browser bundle without dragging in video or GIF complexity.

## Phase 5: Optional Optimization Or Exact-Shader Path

Only if needed after measurement:

- add a reusable offscreen scalar-field pass in WGPUI render,
- port presets from GLSL to WGSL,
- or add lower-level glyph batching helpers to reduce per-frame layout work.

This is optimization, not the starting point.

## Specific Feature Parity Assessment

| Feature | WGPUI conversion difficulty | Notes |
| --- | --- | --- |
| Repeated text grid | Low | Direct fit for `TextSystem` + `TextRun` |
| Threshold/invert reveal | Low | Simple brightness post-process |
| Tile cols/rows | Low | Pure math |
| Mirror modes | Low | Pure math |
| Character mode | Low | Direct |
| Word mode | Low-Medium | Same row scan logic as browser version |
| Shader presets except erosion | Low | Straight Rust math |
| Erosion noise preset | Medium | Needs reusable noise function |
| Static image mask | Medium | Decode + luminance sampling |
| Blob backgrounds without blur | Low | Quads |
| Blob blur | Medium-High | Approximation or new effect primitive |
| GIF/video mask | High | New media pipeline |
| Browser editor UI | Not recommended for first port | Keep as separate tooling if wanted |

## Recommended API Shape

The generic WGPUI component should probably look something like:

```rust
KineticText::new()
    .text("KINETIC TYPE")
    .font_size(16.0)
    .letter_spacing(2.0)
    .preset(KineticPreset::Ripple)
    .speed(1.0)
    .frequency(8.0)
    .amplitude(0.5)
    .threshold(0.0)
    .invert(false)
    .tile(KineticTile::new(3, 2).mirror_xy())
    .blob_background(KineticBlobBackground::default())
```

Keep media and shader inputs generic:

- `KineticSource::Preset(KineticPreset)`
- `KineticSource::ImageMask(ImageData, MaskChannel)`

That keeps the component reusable and avoids browser-specific types in the public API.

## What Not To Port First

## 1. Do not port `index.html` as a WGPUI product surface

The editor is not the core value. It is a convenience tool around the effect.

## 2. Do not begin with custom WGSL or renderer surgery

The effect can be reproduced without changing the renderer. Start with the simplest architecture that proves the look.

## 3. Do not drag video/GIF support into v1

Those are real media-pipeline concerns and not required to validate the effect in WGPUI.

## 4. Do not make it app-specific if the intent is reusable typography infrastructure

This effect is generic enough that keeping it in `crates/wgpui` is justified if the API stays clean.

## Suggested Build Order

1. Add a tiny prototype in a WGPUI example or storybook lane.
2. Port cell measurement, scalar-field presets, and text reveal.
3. Add blob backgrounds.
4. Profile CPU cost with representative viewport sizes.
5. If the effect holds up, promote it into a reusable `wgpui` component.
6. Add static image masks.
7. Only then decide whether renderer-level optimization is worth it.

## Final Recommendation

KineticType should be treated as a **native WGPUI conversion candidate**, not as something to embed through a browser lane.

The correct first implementation is:

- generic,
- Rust-native,
- CPU scalar-field driven,
- text-grid based,
- and staged around retained WGPUI text/quads.

This is one of the rare downloaded effects where the best answer is not “use a webview” and not “keep it app-owned.” The effect engine itself is generic enough to belong in `crates/wgpui`, with product usage remaining in app crates.
