# 2026-03-09 TouchDesigner HTML Embedding And WGPUI Port Audit

> Historical note: This audit is a point-in-time snapshot from its date. Current product and architecture authority lives in `docs/MVP.md`, `docs/OWNERSHIP.md`, `crates/wgpui/README.md`, and current code in this repo.

- Author: Codex
- Status: complete
- Scope: audit `~/Downloads/idea-garden-v1/`, compare it to retained WGPUI/browser/desktop surfaces in this repo, and recommend how to embed TouchDesigner-like visuals into the Rust app without violating MVP or ownership boundaries

## Objective

Answer five questions:

1. What is `idea-garden-v1` actually built from?
2. What does the current repo already retain that is relevant to TouchDesigner-like visuals?
3. Can that HTML page be embedded directly into the current Rust/WGPUI desktop app?
4. What is the lowest-regret path to ship TouchDesigner-like visuals in this repo?
5. Where should each layer live under the current MVP and ownership rules?

## Sources Reviewed

Authority and ownership:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/wgpui/CRATE_BOUNDARIES.md`
- `crates/wgpui/README.md`
- `crates/wgpui/docs/MVP_BOUNDARIES.md`

Current retained implementation:

- `Cargo.toml`
- `apps/autopilot-desktop/Cargo.toml`
- `apps/autopilot-desktop/src/render.rs`
- `crates/wgpui/Cargo.toml`
- `crates/wgpui/src/lib.rs`
- `crates/wgpui/src/platform/web.rs`
- `crates/wgpui-core/src/scene.rs`
- `crates/wgpui-render/src/renderer.rs`
- `crates/wgpui/docs/rendering-pipelines.md`
- `apps/deck/Cargo.toml`
- `apps/deck/README.md`
- `apps/deck/src/app.rs`
- `docs/audits/2026-03-06-wgpui-web-presentation-viewer-audit.md`

External bundle inspected:

- `~/Downloads/idea-garden-v1/README.md`
- `~/Downloads/idea-garden-v1/garden.html`

## Executive Recommendation

Do not try to embed `garden.html` directly "inside WGPUI." The current WGPUI stack is a native `wgpu` scene renderer, not an HTML runtime, and `apps/autopilot-desktop` does not currently include any desktop webview lane.

The lowest-regret order is:

1. **Best fit for the MVP desktop app:** build our own TouchDesigner-like native scene in `apps/autopilot-desktop`, using WGPUI as the renderer and keeping product behavior in the app crate.
2. **Best fit if browser parity matters:** build a new small wasm visualization app using the retained `wgpui` web lane, following the `apps/deck` pattern, and only later consider embedding that in a desktop webview.
3. **Fastest way to reuse the existing HTML bundle:** add an app-owned embedded webview lane beside WGPUI, but treat that as a separate surface, not as a WGPUI feature.

Avoid a fourth path: trying to make WGPUI render arbitrary HTML, DOM, or a live Three.js canvas. The repo does not have that bridge, and forcing it into `crates/wgpui` would violate current ownership boundaries.

## Bottom Line

This repo is already set up for:

- native desktop `wgpu` rendering through WGPUI,
- browser `wgpu` rendering through WGPUI on wasm,
- product-specific scene logic in app crates.

It is **not** currently set up for:

- embedding HTML/JS content into the desktop app,
- hosting a desktop webview inside `apps/autopilot-desktop`,
- drawing foreign textures or canvases into WGPUI as a first-class public API.

So the real choice is not "can WGPUI embed TouchDesigner HTML right now?" It is:

- build a native WGPUI version,
- or add a separate embedded browser lane,
- or build a wasm app first and optionally embed that later.

## What `idea-garden-v1` Actually Is

`idea-garden-v1` is not a TouchDesigner project file. It is a standalone browser app in one HTML file.

### Structure

- `garden.html` is a single 4,941-line HTML file with inline CSS and JavaScript.
- It imports browser modules from CDN:
  - `three`
  - `OrbitControls`
  - `CSS2DRenderer`
  - `EffectComposer`
  - `RenderPass`
  - `UnrealBloomPass`
  - `Line2` / `LineMaterial` / `LineGeometry`
- It relies on browser-only APIs:
  - `window.showDirectoryPicker()` for vault/folder selection
  - IndexedDB for session/cache restore
  - `fetch()` for Gemini embeddings and cross-pollination requests
  - DOM overlays for setup screens, panels, controls, and timeline chrome
  - `requestAnimationFrame()` for its main loop

### Runtime Model

The app mixes three layers in one file:

1. **Data ingestion**
   - scans Markdown notes from a selected folder
   - parses tags/frontmatter
   - groups notes by subfolder or tag
   - optionally computes Gemini embeddings

2. **Scene generation**
   - creates L-system plants
   - places leaves as note nodes
   - runs a pollen particle system
   - uses bloom and label renderers
   - handles a camera with `OrbitControls`

3. **UI shell**
   - setup screen
   - stats and transport controls
   - detail side panel
   - timeline scrubber
   - runtime VFX control panel

This matters because it is not just "an effect." It is a full browser app shell plus a real-time 3D scene.

## What The Repo Already Retains

## 1. A native desktop `wgpu` app shell

`apps/autopilot-desktop` is a native Winit + `wgpu` app. Its main rendering path in `apps/autopilot-desktop/src/render.rs` creates a single native surface and uses `wgpui::renderer::Renderer` to paint the app scene.

That is a strong fit for:

- a native scene viewport,
- a deterministic render loop,
- app-owned file scanning and cache state,
- HUD-style overlays and detail panes.

It is not an HTML shell.

## 2. WGPUI already supports the primitives needed for a native port

The retained WGPUI stack is more capable than a simple 2D HUD:

- quads
- text
- curves/lines
- SVG quads
- indexed mesh primitives with normals/colors
- layered scene composition
- platform adapters for desktop and web

The mesh pipeline in `crates/wgpui-core/src/scene.rs` plus `crates/wgpui/docs/rendering-pipelines.md` means we can already submit generic triangle meshes and edge overlays through the native renderer. That is the main retained primitive needed for "TouchDesigner-like" custom geometry and motion.

That makes a native version of:

- branches/trunks,
- leaves,
- leader lines,
- particle trails,
- animated background geometry,
- HUD overlays,

feasible without introducing a browser runtime.

## 3. WGPUI also retains a browser lane

This repo still has a wasm/browser WGPUI path:

- `crates/wgpui` exposes the `web` feature
- `crates/wgpui/src/platform/web.rs` provides `WebPlatform`
- `apps/deck` is a live example of a browser app built on that lane

That means the repo can support "our own version of TouchDesigner-like visuals in Rust, running in browser" today as a proper app-owned lane.

This is important because it creates a third option between:

- full native desktop port, and
- keeping the current raw HTML/Three.js bundle unchanged.

## 4. The repo does not retain a desktop webview lane

Search across `Cargo.toml`, `apps/autopilot-desktop`, `crates/wgpui`, and docs shows no current dependency or retained integration lane for:

- `wry`
- `tao`-hosted webviews
- `WebView2`
- `WKWebView`
- `tauri`
- CEF

`apps/autopilot-desktop/Cargo.toml` depends on `wgpui`, `wgpu`, `winit`, and app/runtime crates, but not a browser embedding stack.

That means an embedded HTML surface is possible only as **new app work**, not by flipping on an existing feature.

## 5. The renderer has an image pipeline, but not a foreign-canvas embedding API

`crates/wgpui-render` clearly has an image pipeline, but the current public scene API exposes `SvgQuad`, not a general "draw any uploaded texture/canvas/video frame" interface.

Today, WGPUI is good at:

- drawing its own scene primitives,
- rasterizing and caching SVGs,
- rendering app-owned meshes and text.

Today, WGPUI is not yet a ready-made host for:

- a live Three.js canvas,
- a browser `canvas` copied into the scene,
- a TouchDesigner frame stream,
- a WebView texture composited inline with app widgets.

That gap is architectural, not cosmetic.

## Where The Ownership Boundaries Push Us

Per `docs/OWNERSHIP.md`:

- `apps/autopilot-desktop` owns app wiring, pane orchestration, and product behavior.
- `crates/wgpui` owns product-agnostic UI APIs and rendering support.
- product workflows must not move into `crates/wgpui`.

So:

- vault scanning, note grouping, embeddings, pollen business logic, and product scene orchestration belong in an app crate
- reusable camera/input or generic texture primitives could move into `crates/wgpui` only if they are clearly product-agnostic
- a browser/webview host for one product surface should begin in an app crate, not in `crates/wgpui`

This is consistent with MVP guidance too: keep changes small, verifiable, and desktop-first unless the new lane is clearly justified.

## Integration Options

## Option A: Native WGPUI Port Inside `apps/autopilot-desktop`

### What it means

Rebuild the visual system in Rust and render it natively through WGPUI:

- app-owned scene state in `apps/autopilot-desktop`
- WGPUI primitives for HUD chrome and overlays
- mesh primitives for branches/leaves/particles or billboards
- app-owned input controller for pan/zoom/orbit-style camera
- app-owned cache/storage using existing Rust runtime patterns instead of browser IndexedDB

### What maps cleanly

- note scanning: from browser picker to native folder picker / configured path
- tag/subfolder grouping: direct Rust port
- L-system generation: direct Rust port
- timeline filter: direct Rust port
- side panel and labels: WGPUI panes/components
- VFX settings: WGPUI controls or app pane
- embeddings/fetch: Rust `reqwest` lane instead of browser `fetch`

### What must be replaced

- `showDirectoryPicker()` -> native desktop file/folder selection lane
- IndexedDB -> app disk cache / state snapshot
- DOM/CSS panels -> WGPUI widgets or custom paint
- `OrbitControls` -> app-owned camera controller
- Three.js scene graph/materials -> WGPUI scene + mesh model
- `CSS2DRenderer` -> WGPUI text overlays / labels

### Why this is the best fit

- It keeps the MVP desktop-first.
- It keeps one render loop.
- It preserves WGPUI visual consistency.
- It keeps deterministic state in Rust instead of split JS/browser state.
- It does not require an additional browser stack in the desktop app.

### Main risk

The port is real engineering work. Three.js currently gives the bundle:

- scene graph helpers,
- camera controls,
- bloom/postprocessing,
- sprite helpers,
- label helpers.

Those would need to be recreated or simplified in our own scene implementation.

### Recommendation

This is the best choice if the goal is "our own version of TouchDesigner-like stuff" that feels first-class inside Autopilot.

## Option B: New WGPUI Web App, Following `apps/deck`

### What it means

Create a new browser visualization app, likely under a new `apps/*` member, using:

- `wgpui` with the `web` feature
- `WebPlatform`
- a tiny HTML shell
- app-owned visualization state and input bridge

This mirrors the already-retained `apps/deck` structure.

### Why it matters

This gives us a Rust-owned browser visualization lane without committing to:

- native desktop port first, or
- keeping the existing raw JS bundle.

It is the cleanest way to prototype "TouchDesigner-like visuals, but ours" while staying within the repo's architecture.

### When to choose it

Choose this if:

- browser delivery matters,
- you want fast visual iteration in web first,
- or you want a later path where the same wasm app could be hosted in a webview or external browser.

### Limits

- This does not by itself embed into the native app.
- To show it inside `apps/autopilot-desktop`, you would still need a desktop webview lane.

### Recommendation

This is the best choice if the goal is "own the visualization in Rust first, then decide whether it belongs in browser, desktop, or both."

## Option C: Add A Desktop WebView And Load `garden.html`

### What it means

Keep the bundle mostly as-is and embed it in the desktop app using a new webview dependency, with the WGPUI app and the webview living side-by-side.

### Benefits

- fastest path to see the existing experience inside a desktop window
- minimal porting of Three.js logic
- preserves existing bloom, labels, particle system, and HTML shell

### Costs

- introduces a second rendering/input/runtime stack into the desktop app
- requires new platform-specific webview plumbing
- file access behavior will differ from normal browser Chrome/Edge behavior
- `showDirectoryPicker()` support may not match the current browser assumptions
- IndexedDB/cache behavior will need retesting
- CDN-hosted Three.js imports and browser security assumptions need packaging work
- visual language will drift from WGPUI unless heavily restyled

### Architectural reality

This is not "via WGPUI." It is "next to WGPUI."

WGPUI can frame or orchestrate a region for the view, but the HTML content would still be rendered by the webview engine, not by WGPUI's renderer.

### Recommendation

Only choose this if speed of reusing the existing HTML is more important than desktop-native integration quality.

## Option D: True TouchDesigner Runtime Integration

If by "embed TouchDesigner" you literally mean integrating TouchDesigner output, that is a different problem from `idea-garden-v1`.

That path would usually involve:

- a shared frame stream or texture bridge such as Syphon/Spout/NDI,
- or IPC/network frame transport,
- plus a WGPU texture upload/composition path.

The current repo does not retain:

- TouchDesigner integration code,
- shared-texture bridges,
- or a public arbitrary-texture scene primitive in WGPUI.

So true TouchDesigner integration is feasible only as new work, and it is a worse fit than a native WGPUI implementation unless TouchDesigner itself is the source of truth you need to keep.

## What Not To Do

## 1. Do not put product scene logic into `crates/wgpui`

These do **not** belong in `crates/wgpui`:

- vault parsing
- note clustering
- Gemini integration
- Obsidian-specific ingest
- scene-specific pollen behavior
- product-side detail pane semantics

Keep them app-owned.

## 2. Do not try to make WGPUI an HTML renderer

That would add the wrong abstraction to the wrong crate. The repo already has:

- a desktop native lane,
- and a wasm browser lane.

It does not need a half-HTML, half-native rendering abstraction inside `wgpui`.

## 3. Do not couple MVP desktop surfaces to a giant browser dependency unless the reuse win is overwhelming

The MVP spec is desktop-first and performance-sensitive. If the visual system is intended to become a core Autopilot surface, the native path is cleaner.

## Recommended Build Order

## Path 1: Recommended For The Desktop MVP

1. Build a small app-owned "visual scene pane" in `apps/autopilot-desktop`.
2. Port only the core retained behaviors first:
   - folder-backed content ingest
   - grouping
   - procedural plant generation
   - camera controls
   - hover/select
   - side detail panel
3. Use existing WGPUI primitives for HUD and labels.
4. Add generic WGPUI primitives only if the native port proves they are broadly reusable:
   - billboards/sprites
   - generic uploaded-image quad
   - camera helper utilities
   - optional postprocess hooks

This gives the app a desktop-native "our own TouchDesigner-like scene" without dragging in a webview.

## Path 2: Recommended If Browser Iteration Comes First

1. Create a new wasm visualization app following `apps/deck`.
2. Rebuild the scene in Rust on top of `wgpui::WebPlatform`.
3. Keep app-specific state and content loading in that new app crate.
4. Only later decide whether to:
   - keep it browser-only,
   - launch it externally from desktop,
   - or embed it in a webview.

This is cleaner than trying to evolve the current single-file JS bundle into a long-term product lane.

## Path 3: Fast Reuse Prototype

1. Add an app-owned desktop webview lane.
2. Package `garden.html` and local assets.
3. Replace browser-only assumptions that break in an embedded context.
4. Bridge only the minimal host controls needed from Rust.

Use this only as a prototype or research surface.

## Concrete Repo Placement

| Concern | Recommended owner |
| --- | --- |
| Product visualization state for Autopilot | `apps/autopilot-desktop` |
| Browser-only visualization app | new `apps/*` crate, same pattern as `apps/deck` |
| Generic mesh/camera/input/render helpers | `crates/wgpui*` only if proven reusable |
| Embedded desktop webview host | `apps/autopilot-desktop` |
| Obsidian/note ingest, clustering, embeddings | app crate, not `wgpui` |

## Practical Recommendation For This Repo

If the real question is:

> "How do we get TouchDesigner-like visual systems into our Rust app through WGPUI?"

the answer is:

- **Use WGPUI as the renderer for our own scene, not as a host for arbitrary HTML.**
- **Use the existing `apps/deck` + `WebPlatform` pattern if you want a browser-first Rust scene.**
- **Add a webview only if you explicitly choose a parallel browser runtime inside the desktop app.**

If the real question is:

> "Can we stuff this downloaded HTML page directly into the current desktop app?"

the answer is:

- **Not with the retained stack as-is.**
- It requires a new desktop webview lane.
- That lane would sit beside WGPUI, not inside it.

## Final Recommendation

For OpenAgents as currently scoped, the best long-term move is:

1. Treat `idea-garden-v1` as a reference implementation, not a production surface.
2. Extract the transferable ideas:
   - procedural scene generation
   - particle/cross-pollination logic
   - timeline and clustering UX
   - information-dense side panels
3. Rebuild the core experience natively in an app-owned WGPUI scene for desktop.
4. If browser iteration is strategically important, create a new wasm app around the retained WGPUI web lane rather than preserving the current single-file JS app forever.

That path respects:

- `docs/MVP.md` desktop-first intent,
- `docs/OWNERSHIP.md` crate boundaries,
- and the retained WGPUI architecture already in this repo.
