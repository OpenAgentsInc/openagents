> Imported unchanged from Aethyrforge `docs/research/2026-10-24-gpu-native.md` at `2b1ccb7ef5` (2026-10-24). Relative links point into that repository at that commit. Decisions here are superseded by [`docs/adr/`](../../adr/README.md); this is background.

# GPU Native: one GPU-drawn interface for every OpenAgents device

Date: 2026-10-24. Status: exploratory strategy. Nothing here is selected or
implemented, and no code changed. This document builds on the
[consolidation audit](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/audits/2026-10-24-consolidate-into-openagents.md) and
uses its reconciled decision record numbers (ADR 0001–0014) unless it names an
Aethyrforge record such as [0006](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/decisions/0006-cross-platform-engine-core.md).

Labels used throughout:

- **Fact** is read from a file named here, with its path.
- **Measured** is a number from a recorded run. Single samples are marked.
- **Inference** is a conclusion from reading code or docs without running them.
- **Speculation** is a proposal or estimate. Every recommendation is speculation
  until the owner decides.

## Summary

The owner asked (2026-10-24) whether the openagents "Rust Native" idea should
become something like "GPU Native": for any device, get the best access to its
hardware-accelerated graphics and draw the screen's pixels directly on the GPU,
with one framework across mobile, desktop, and browser and one language for
setting levels of detail from each device's resources.

- **Recommendation:** yes, adopt the principle. One retained scene and UI model
  and one renderer draw 3D worlds *and* application UI on every platform. Thin
  platform hosts supply a surface, input, lifecycle, and the few services that
  must stay native or DOM: text input and IME, the accessibility tree, sign-in,
  payments, and web pages that need indexing.
- **Recommendation:** keep the Rust core over the `webgpu.h`-shaped `af-gpu`
  crate (Aethyrforge 0006, audit ADR 0004 and 0005). Fix the measured Rust-to-JS
  call cost with a packed command-list import. Do not switch languages: frames
  are GPU-bound, so language is not the lever, and the Rust investment is large.
- **Recommendation:** keep the Rust Native *view vocabulary* (semantic nodes,
  typed intents, revisions) as the UI tree, and replace its per-platform widget
  adapters with one GPU adapter plus platform bridges for accessibility and
  text input. This reverses Rust Native's "native widgets" premise; it does not
  discard its data contracts.
- **Recommendation:** keep GPUI on the desktop for now, with no new investment.
  Carry forward its scene primitive set and ideas, and the OpenAgents text
  layout crate, into the new framework. Retire the Zeron/Zed fork when the
  desktop app moves, last.
- **Naming:** "GPU Native" works as the name of the *principle*. For the
  framework itself, prefer an ownable name tied to the engine; see
  [naming](#naming).
- **Plan headline:** Phase 0 turns the Kiki MVP into the first GPU Native app
  on the web (render-lab path, one token set, one device profile). Phase 1 runs
  the engine core natively inside the mobile app. Phase 2 refactors the mobile
  app into a shell that loads zones. Phase 3 moves the desktop.

## Where we are today

### Five ways of drawing UI

**Fact:** the repositories contain at least five UI drawing paths.

| Path | Where | How it draws |
| --- | --- | --- |
| Rust Native | `openagents/crates/rust-native` (+ `rust-native-desktop`, `rust-native-web`) | Semantic views (`rust-native.view.v2`/`v3`) rendered by platform adapters. Its architecture doc says "An iOS button should be a native button" (`docs/coder/rust-native/architecture.md`). Transcript layout is computed in Rust; adapters paint the runs |
| GPUI desktop | `openagents/apps/openagents` | GPUI from `zeronsh/zui` rev `361ff20`, "extracted from wingleeio/zed e2ddcc6", plus OpenAgents patches for backdrop blur, edge fades, rounded clips, and GPU memory bounds (comments in `apps/openagents/Cargo.toml`) |
| Mobile display lists | `apps/openagents/crates/mobile` | Rust lays out display lists (`layout/`, `visual/`); UIKit on iOS and Kotlin on Android paint them over a UniFFI core (`AGENTS.md`, "One OpenAgents desktop app and one mobile app") |
| React web UI | `openagents/apps/verse-ui`, `aethyrforge/apps/editor` | React DOM, Effect Atom, Effect 4, shadcn/ui (audit ADR 0011) |
| Engine gameplay UI | `aethyrforge/docs/game-systems.md` | Proposed `ui.screen.v1` declarative screens rendered as DOM by the player host |

**Fact:** the same audit counts five renderers for overlapping worlds: the
Aethyrforge TypeScript WebGPU renderer, the Rust render lab, Verse v1 on `wgpu`
(`crates/verse*`), the iOS `medieval_town` `wgpu` Metal renderer
(`apps/openagents/crates/mobile/src/medieval_town`, with its own `town.wgsl`),
and `apps/haunt-preview`.

**Inference:** the duplication is the problem the owner is describing. Every new
surface currently needs a world renderer decision *and* a UI drawing decision,
made separately per platform.

### What already points toward GPU Native

- **Fact:** Aethyrforge [0006](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/decisions/0006-cross-platform-engine-core.md)
  selects one Rust core, including the renderer, over a thin `webgpu.h`-shaped
  crate (`crates/gpu`), with browser WebGPU on the web, Dawn natively, and
  `wgpu` as fallback. [0007](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/decisions/0007-gpu-driven-rendering.md) selects
  GPU-driven rendering, temporal anti-aliasing, cluster LOD, and crowd tiers.
  [0008](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/decisions/0008-shader-toolchain.md) selects Slang compiled to WGSL.
- **Fact:** `crates/render-lab` is a working Rust renderer on browser WebGPU
  with runtime switches for path (CPU draws, GPU culling with indirect draws,
  visibility buffer), anti-aliasing (`Aa::Taa`, MSAA), occlusion, and LOD
  (`crates/render-lab/src/lab.rs`, `Config`).
- **Fact:** `crates/gpu/src/lib.rs` declares a `native` module behind the
  `native` feature, but `crates/gpu/src/` contains only `lib.rs`, `null.rs`, and
  `web.rs`. **Inference:** the native Dawn/`wgpu-native` backend is not written
  yet, so the `native` feature does not build today.
- **Fact:** `apps/openagents/crates/text` (`openagents-text`) shapes with
  `rustybuzz`, breaks lines with ICU4X, and its doc comment reports matching
  CoreText line starts in 100% of 48k test cases. It already powers the mobile
  transcript.
- **Fact:** `apps/openagents/crates/visual-gpu` renders agent visuals offscreen
  with `wgpu` 29 on every desktop; the mobile crate also links `wgpu` 29 with
  the `metal` feature for `medieval_town`.

### Measured facts that shape the choice

- **Measured:** Town and battle scenes are GPU-bound. CPU preparation is
  2–4 ms against a 22–42 ms main HDR pass, and C versus TypeScript made no
  frame-rate difference ([C Town results](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/research/c-medieval-town-results-2026-10-23.md),
  [0007](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/decisions/0007-gpu-driven-rendering.md)).
- **Measured (single samples, not a benchmark):** on an M5 Max in Chrome,
  encoding about 1,500 draws through Rust-to-JS imports cost about
  0.22–0.66 ms, against about 0.10–0.15 ms when TypeScript calls WebGPU
  directly. **Inference:** a packed command-list import (Rust writes a compact
  command buffer to Wasm memory; one JS call replays it) would remove most of
  the per-call crossing cost.
- **Measured:** headless Linux Chrome on CoderOS (RTX 4080) fell back to
  SwiftShader, a CPU rasterizer. Headless Chrome on a Mac uses Metal.
  **Inference:** GPU performance evidence cannot come from headless Linux
  Chrome on CoderOS as configured; qualification needs real-GPU hosts.

## What "GPU Native" would mean

**Definition (speculation):** an application is GPU Native when one retained
scene and UI model, and one renderer, produce every pixel of its interactive
surface on the GPU, on every platform. Platform code is a thin host that
supplies a surface (browser `<canvas>`, iOS `CAMetalLayer`, Android
`SurfaceView`, desktop window), input, lifecycle, and a short list of
deliberately native services.

### Compared with the alternatives

| Approach | What draws a button | Same pixels everywhere | Native feel for free | One renderer for UI and worlds |
| --- | --- | --- | --- | --- |
| React Native | The platform's native widget | No | Yes | No |
| Rust Native today | A per-platform adapter (SwiftUI/UIKit, Android widgets, HTML, `winit` painter) | No, by design | Mostly | No |
| GPUI (Zed) | GPUI's own GPU renderer, desktop only | Yes, on desktop | No | No (no 3D worlds) |
| Flutter | Its own engine (Skia, now Impeller) | Yes | No | No (2D focus) |
| **GPU Native** | The engine's renderer | Yes | No | **Yes** |

**Inference:** GPU Native trades "native feel for free" for "one look, one
LOD language, and UI that composes with 3D worlds" (cards floating over a
planet, panels with backdrop blur of the live scene). Kiki's journey, read for
product intent only, is exactly that: sign-in and option cards in a sci-fi
style over a 3D planetary view, editing panels over a live world, and award
cards. Those are hard to do well with native widgets layered over a GPU view.

### The hard parts of drawing your own UI

Drawing your own pixels means owning what the platform used to give away.
The list below is from general industry knowledge, not verified in this
session.

| Concern | Risk | Proposal |
| --- | --- | --- |
| Text shaping and line breaking | Complex scripts, emoji, bidirectional text | Carry `openagents-text` forward (rustybuzz + ICU4X, CoreText-matching breaks). Bundle fonts; fall back to system fonts by name where the platform allows |
| Text input and IME | Composition (CJK, dictation), autocorrect, autofill, password managers | **Keep native.** A platform text field (or a hidden DOM `<input>`/`contenteditable` on the web) owns composition; the GPU draws the field's chrome and, optionally, its committed text |
| Accessibility | Screen readers see a single opaque surface | **Mirror.** Build a platform accessibility tree from the same semantic view: UIAccessibility elements on iOS, `AccessibilityNodeProvider` on Android, a DOM mirror on the web, NSAccessibility on desktop. AccessKit (Rust) is a candidate for desktop and possibly Android |
| Selection and copy | Platform selection handles and menus | Rust Native's `selection::Selection` already owns grapheme-safe selection; use native handles and menus where available |
| Scrolling physics | iOS rubber-banding and Android overscroll feel wrong when imitated badly | Per-platform scroll curves in the shell layer, or drive scrolling from a native scroll view's offsets |
| System fonts and Dynamic Type | User text-size settings | Read the platform's scale and feed it to layout (the transcript layout already accepts a scale curve) |
| Autofill, passkeys, sign-in | Password managers and passkeys need real fields | **Keep native or DOM.** Sign-in is a native sheet or a DOM form over the canvas |
| Payments | Store rules and trust | **Keep native.** Store payment sheets and the wallet's confirmation step are shell-owned |
| Web SEO, links, sharing | Canvas text is invisible to crawlers | **Keep DOM** for marketing and shareable pages (already Rust/Maud on openagents.com). GPU Native is for the app surface, not the website |

How others handle these (general knowledge, to verify before relying on it):

- **Flutter** draws everything, connects to the platform IME through a text
  input channel, and builds a semantics tree that it maps to platform
  accessibility APIs; on the web it creates a DOM semantics overlay. Its
  "platform views" embed native widgets at a compositing cost.
- **Zed and GPUI** draw everything on desktop with Metal on macOS and other
  backends elsewhere (the pinned fork's comments name `wgpu` on Linux and a
  Windows path). Accessibility is a known gap.
- **Slint** and **egui** draw their own widgets and integrate AccessKit for
  accessibility; egui on the web uses a hidden text element for IME.
- **Bevy UI** is an ECS-based UI with an AccessKit integration.
- **Unity UI Toolkit** and most game engines draw their own UI, show the
  platform's software keyboard for text, and have weak screen-reader support.

**Inference:** the frameworks that took accessibility seriously (Flutter, Slint,
egui) all converge on the same design: a semantic tree beside the pixel tree.
Rust Native already *is* a semantic tree. That makes it a better starting point
for GPU Native than GPUI, whose element tree is pixel-first.

**Browsers specifically:** a canvas-only UI loses DOM accessibility, find-in-page,
text selection, translation, and browser autofill unless the app mirrors them.
The DOM mirror must be kept in sync each revision and tested with real screen
readers. This is the largest cost of moving the web UI off React.

## A unified capability and LOD language

### Idea

One device profile and budget model drives both world LOD (0007's techniques:
render scale, temporal anti-aliasing, occlusion, cluster LOD bias, crowd tier
distances) and UI fidelity (backdrop blur, shadows, animation, effect layers,
glyph atlas resolution). It is measured at startup, then adjusted continuously
from frame timing and platform signals. Zones and UI components declare which
knobs they expose and the floor below which they refuse to degrade.

### Data model sketch (speculation)

```rust
/// Probed once, refreshed on platform events. Never inferred from user agent.
pub struct DeviceProfile {
    pub gpu: GpuInfo,          // adapter, WebGPU features and limits, class
    pub memory: MemoryInfo,    // budget_mb from platform or probe
    pub display: DisplayInfo,  // size_px, scale, refresh_hz, hdr
    pub input: InputKinds,     // touch, pointer, keyboard, gamepad
    pub power: PowerState,     // battery, low_power, thermal: Nominal..Critical
    pub network: NetworkInfo,  // metered, save_data, estimated_mbps
    pub prefs: UserPrefs,      // reduced_motion, reduced_transparency, text_scale
}

/// What the current session may spend. Set by profile + context.
pub struct Budget {
    pub frame_ms: f32,         // e.g. 16.7 or 33.3; 0007 qualification picks it
    pub gpu_ms: f32,
    pub memory_mb: u32,
    pub download_mb: u32,
}

/// One scalar the controller moves; each subsystem maps it through its curve.
pub struct Detail(pub f32);    // 0.0 = floor, 1.0 = full

pub enum Knob {
    RenderScale, Aa, Occlusion, ClusterLodBias, CrowdTierDistance,
    ShadowQuality, FoliageDensity,                 // world
    BackdropBlur, UiShadows, Motion, UiEffects,    // interface
}

pub struct KnobDecl { pub knob: Knob, pub curve: Curve, pub floor: f32 }
```

A controller reads timestamp queries (an optional WebGPU feature, per 0007)
or frame pacing when timestamps are unavailable, applies hysteresis so detail
does not oscillate, and writes `Detail` once per second or on a platform event.
Platform signals feed `PowerState`: on iOS, the process thermal state and Low
Power Mode; on Android, the power manager's thermal status; in browsers there
is no thermal API, so the controller relies on sustained frame time. User
preferences such as reduced motion and reduced transparency are hard limits,
not budget inputs.

Rules (speculation):

- Text, focus indicators, and hit targets have no knob. Readability is a floor,
  matching 0007's rule that UI and text render at output resolution.
- A zone release declares its knobs and floors in its manifest; the shell
  refuses to open a zone whose floor exceeds the device's best case and says
  so, rather than rendering a broken world.
- Every frame record and failure report includes the profile and current
  `Detail`, extending [platform support](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/platform-support.md)'s rule to
  record the selected fallback quality.

**Inference:** `render-lab`'s `Config` (path, `aa`, `occlusion`, `lod`) is
already the world half of this. The UI half does not exist yet; GPUI's blur and
edge-fade primitives and the fork's GPU memory bounds are the obvious first
UI knobs.

## Language and stack options

The owner is not attached to Rust. The options below are judged on portability,
the measured GPU-bound finding, reuse of existing work, and app-store rules.

| Option | For | Against |
| --- | --- | --- |
| **(a) Rust core + `webgpu.h` (current 0006)** | Builds for Wasm and native; reuses `af-kernel`, `render-lab`, `rust-native`, `openagents-text`, the mobile core, Verse v1 (about 408k lines per the audit), and about 612k lines of Rust in `apps/openagents/crates` (measured with `wc -l` for this doc); one language with the domain services | Rust-to-JS crossing cost on the web (measured above, fixable); Wasm binary size and startup; compile times |
| (b) C++ core (Dawn and Filament style) | Industry standard; Dawn is C++; strongest native tooling | Abandons the Rust investment; 0006 records no frame benefit from C; a third product language against `openagents` policy |
| (c) Zig | Small binaries, good C interop, easy Dawn linking | Young ecosystem, no UI or text stack, rewrite of everything; no measured benefit |
| (d) TypeScript web-first, native shells running a JS engine + WebGPU (Dawn bindings, or WKWebView/WebView) | Fastest web iteration; today's production renderer is TypeScript; the measured direct-call path is cheapest | iOS forbids third-party JIT, so a bundled JS engine runs interpreted; WebView loses native control of thermal state and lifecycle (0006 lists it only as an interim channel); duplicates domain logic already in Rust |
| (e) Adopt an engine or UI framework (Flutter, Unity, Bevy, Slint) | Mature text, IME, accessibility (Flutter); mature 3D (Unity) | None of them gives both GPU-driven 3D on WebGPU *and* an owned semantic UI with typed intents; ownership concerns in Aethyrforge 0001; Unity is not open source; Flutter's 3D story is immature; Bevy UI and Slint lack our world pipeline |

**Recommendation:** option (a), with three amendments.

1. **Packed command import.** Replace per-call imports with a command buffer in
   Wasm memory that one JS function replays per pass. This is the measured
   bottleneck of the Rust path, and 0007 already shrinks submissions to tens of
   commands per frame.
2. **Write the native `af-gpu` backend next.** Without it there is no native
   GPU Native. Try Dawn first per 0006; fall back to `wgpu-native` per target.
3. **TypeScript shrinks to host glue and the DOM mirror.** This *proposes
   evolving audit ADR 0011*, which keeps React/Effect as the web UI exception.
   Under this plan React stays for the editor and website-embedded pages until
   the GPU UI passes its accessibility and input gates; in-world UI moves first.
   ADR 0011 changes only when the owner accepts a gate result.

**Reasons:** frames are GPU-bound, so the language decides portability and
reuse, not speed. Rust is the only option that runs the existing kernel, text
layout, mobile core, and domain crates on every target today.

**Evidence that would change this recommendation:**

- After the packed import, Rust submission stays more than 0.5 ms slower than
  TypeScript at p95 on a pinned web profile (0006's own revisit condition).
- The Wasm build makes first load materially slower on a mid-range phone browser
  (threshold to be set in qualification).
- Neither Dawn nor `wgpu-native` qualifies on iOS or Android within the mobile
  milestone.
- The DOM accessibility mirror cannot pass screen-reader checks on the web,
  which would argue for keeping React for all web app UI permanently.

## GPUI: carry forward, own, or keep

**Fact:** GPUI here is a fork of a fork: Zed's GPUI, extracted by
`wingleeio/zed` at `e2ddcc6`, published as `zeronsh/zui`, pinned at rev
`361ff20`, with OpenAgents patches listed in `apps/openagents/Cargo.toml`
comments and `docs/downstream-patches.md`. The app pins Rust 1.98.1 for it and
ports upstream fixes by hand (`apps/openagents/FORK.md`). The `ui` crate is
about 194k lines and `chat-view` about 53k (measured with `wc -l`).
**Inference:** the pinned platform features (`wayland`, `x11`) and the absence
of mobile or Wasm targets mean this GPUI is desktop-only; the mobile app does
not use it.

### Worth carrying forward

- **The scene primitive set** and its batching: quads with corner radii and
  borders, shadows, paths, sprite atlases for glyphs and images, underlines, and
  the OpenAgents additions (backdrop blur, edge fades, rounded clips). These map
  directly onto instanced WGSL pipelines on `af-gpu`.
- **The OpenAgents patches' lessons:** bounded blur scratch, idle release of
  intermediates, and atlas eviction (`ImageSource::evict`). GPU memory bounds are
  a first-class UI concern on phones.
- **Layout approach:** flexbox-style layout (GPUI uses Taffy) for general UI,
  with Rust Native's analytic transcript layout for long text.
- **Not GPUI, but the real text asset:** `openagents-text` and Rust Native's
  Markdown, selection, and incremental parsing.

### Worth leaving

- GPUI's application, entity, and context model. Rust Native's immutable views
  with typed intents and revisions are a better fit for a cross-platform,
  agent-driven, accessibility-mirrored UI.
- GPUI's platform layer beyond the desktop. Mobile and web hosts are already
  thinner and different.
- The fork chain as a dependency of the long-term framework.

### Options

1. **Keep GPUI for desktop only, indefinitely.** Cheapest now; leaves two UI
   renderers forever and the desktop cannot draw the engine's UI.
2. **Fork and own a minimal subset** (scene, renderer, text atlas) inside
   openagents. Removes the fork chain, but the subset is entangled with GPUI's
   element and window model, and it still renders through Metal and `wgpu`
   instead of `af-gpu`.
3. **Port ideas into the new framework**, keeping GPUI on desktop until the
   desktop phase. **Recommended.** The primitive set is small and well
   understood; reimplementing it over `af-gpu` with WGSL is bounded work
   (speculation: weeks, not months), and it avoids a third fork.

**Risks of staying on the fork:** upstream GPUI API churn with no stable
release; manual ports across three layers; the toolchain pin; each OpenAgents
patch (blur, fades, clips) must be rebased on every bump; and engine UI and
desktop UI drift apart.

## Naming

**Assessment of "GPU Native":** it says the right thing in two words and
invites the React Native comparison the owner intends. Its weaknesses: it is a
generic phrase that will be hard to search for and hard to own; "native" is
confusing for a framework whose first target is the browser; and it describes a
technique, which competitors can claim too. No trademark search was done for
any name below.

| Name | Pros | Cons |
| --- | --- | --- |
| GPU Native | Instantly understood; parallels React Native and Rust Native | Generic, unsearchable, "native" misleads on the web |
| Aethyr (framework), Aethyrforge (engine and toolchain) | Ownable, searchable, ties UI and engine into one family; "forge" builds what "Aethyr" runs | Spelling; says nothing about GPUs; depends on ADR 0001 keeping the Aethyrforge name |
| OpenAgents Surface | Plain; matches Rust Native's `Surface` element and the platform "surface" | Generic; collides with the existing `Surface` element meaning |
| Pixelfront | Descriptive of drawing pixels directly; distinctive | Sounds like a game studio; less clear for app UI |
| Lens | Short; suggests a view onto zones | Heavily used in software; poor searchability |

**Recommendation:** use **GPU Native** as the name of the architecture
principle in docs ("OpenAgents is GPU Native"), lowercase-hyphenated as an
adjective ("a GPU-native zone"). Name the framework itself within the engine
family, for example **Aethyr** for the runtime and UI layer that every device
runs, with Aethyrforge as the engine and authoring toolchain. Crate names would
follow the audit's `af-` prefix (for example `af-ui`). The owner decides.

## The mobile app as a gateway to zones

### Shape (speculation)

The OpenAgents app becomes a **shell** plus **zones**.

- **Shell:** the store binary. It contains the engine core, the GPU UI
  framework, the device profile and LOD controller, and shell services:
  identity and sign-in, the wallet and payments, notifications and push, voice,
  the accessibility and text-input bridges, and the zone launcher.
- **Zones:** versioned releases that the shell loads. Examples: the Haunted
  House (a Kiki tutorial world), the Grid (the coding-agent workspace: sessions,
  computers, transcripts, approvals), and standard interfaces such as chat,
  settings, and wallet.

**Fact:** the current app's Verse tab is already labeled "The Grid"
(`openagents/docs/verse/mobile.md`): a bare Verse world with a ground grid, sticks,
and a portal to Everglade. **Inference:** the owner's new meaning (the zone that
concentrates coding-agent capabilities) needs reconciling with that label.

### Zone packaging

- A zone is a release in the existing sense: a manifest whose ID is the SHA-256
  of its bytes ([publishing](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/publishing.md), PUB-001), listing compiled WGSL
  (0008), scene and asset data, UI screens in a declarative schema, behavior
  bytecode, and its LOD knobs and floors.
- **Speculation:** merge `ui.screen.v1` ([game systems](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/game-systems.md)) and
  the Rust Native view schema into one UI vocabulary, so a zone's cards and the
  shell's cards are the same kind of thing. Today they are two schemas with
  nearly the same limits (256 nodes and depth 16 against 1,024 nodes and
  depth 16).
- First-party standard interfaces (chat, settings, wallet) are compiled into
  the shell as zones with the same interfaces, so they work offline and pass
  store review as ordinary app features.

### Sandboxing creator code

- Creator behavior runs as metered bytecode in a Rust VM (audit ADR 0010,
  Aethyrforge [0004](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/decisions/0004-safe-behaviors.md)), with capability-checked
  host calls. A zone never receives the shell's credentials; it asks the shell
  for a capability (for example "show the payment sheet for this item") and the
  shell shows native UI the zone cannot draw over.
- On the web, zones keep running in separate origins through `aethyrforge-host`
  (ADR 0012). Natively, the equivalent is a separate VM instance with no
  ambient authority.
- Creator WGSL is compiled by the GPU driver, not executed on the CPU. It still
  needs validation and resource limits ([security](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/security.md) already
  isolates untrusted shaders).

### App-store code-loading rules

To verify against current text before shipping; this is a summary from general
knowledge, not a legal reading.

- **Apple:** the Developer Program License Agreement (section 3.3.2 in recent
  versions) and App Review Guideline 2.5.2 forbid downloading code that changes
  the app's features, with exceptions for interpreted code that does not change
  the app's primary purpose. Guideline 4.7 allows mini apps and mini games with
  conditions. Third-party JIT is not available on iOS.
- **Google Play:** the Device and Network Abuse policy forbids downloading
  executable code (DEX, native libraries) from outside Play; code in a VM or
  interpreter is allowed.
- **Consequence (inference):** zones may ship data, WGSL, and bytecode run by an
  interpreter in the shell (or Wasm run by an interpreter). They may not ship
  native code. All native capability lives in the shell and is updated through
  the stores.

### The same zones on the web and desktop

The browser runs the same engine core as Wasm on browser WebGPU, inside the
`aethyrforge-host` origin model; the desktop runs the native core, first inside
GPUI as an engine surface, later as the whole window. A zone release does not
know which host it is in, beyond its device profile.

## Phased plan

Each phase keeps the live Kiki routes (`/new-lander`, `/new-lander/verse`,
`/new-lander/house`) on their current pins until a replacement passes the
audit's browser checks. Effort figures are rough speculation in agent-assisted
engineer-weeks and assume the audit's import is done.

### Relationship to the consolidation audit

The audit's phases 0–4 move Aethyrforge into the monorepo without behavior
change. GPU Native work should start **after audit phase 2** (import with equal
artifact hashes) so it does not disturb that comparison. GPU Native phase 0 then
runs alongside audit phases 3–5; its renderer work *is* audit phase 5 step 7
(Rust renderer parity), and its UI work *extends* phase 5 step 2 (one web UI).

### Phase 0: the Kiki MVP becomes GPU Native v1 (web)

Changes to the current web stack:

1. **Packed command import** in `af-gpu`'s web backend.
   Gate: Rust encoding of the 1,500-draw lab scene within 0.1 ms of the
   TypeScript path at p95 over a recorded series (not single samples) on the
   M5 Max and one mid-range phone browser.
2. **Render-lab passes reach parity** on the pinned Town and battle profiles
   per 0006's migration rule, then the Haunted House zone renders through Rust.
   Gate: image parity and equal or better GPU time.
3. **One token set.** A Rust crate defines color, type scale, spacing, radii,
   blur, and motion for the Grid-style visual language, emitted as CSS
   variables for React and as constants for the GPU UI. Gate: React and GPU
   cards render the same tokens in a side-by-side capture.
4. **Device profile and LOD controller** in Rust, wired first to the existing
   TypeScript renderer's quality switches so it lands before renderer parity.
   Gate: a sustained-load test on a phone browser holds the frame budget by
   lowering `Detail` without dropping below floors.
5. **First GPU-drawn UI:** in-world option, award, and editing cards drawn by
   the engine, with a DOM accessibility mirror. Sign-in and text fields stay
   DOM. Gate: VoiceOver and NVDA can reach and activate every card.
6. **Real-GPU qualification hosts.** Use Mac headless Chrome or headed Linux
   with a working GPU; record the adapter so SwiftShader runs are rejected.

Effort (speculation): 8–14 engineer-weeks. Risks: temporal anti-aliasing
readability on text and silhouettes (0007 keeps MSAA as fallback); DOM-mirror
maintenance; Kiki's schedule pressure on the live MVP.

### Phase 1: the engine core inside the mobile app

1. Write the native `af-gpu` backend (Dawn first, `wgpu-native` fallback).
2. Host an engine surface in the iOS `CAMetalLayer` and Android `SurfaceView`
   paths, replacing `medieval_town`.
3. Run the Haunted House release natively.

Gates: the same release ID renders with image parity against the web build;
30 minutes of sustained play without thermal shutdown or memory warnings on a
pinned mid-range phone; suspend and resume without device loss. Effort
(speculation): 10–16 engineer-weeks, dominated by Dawn build and size work.

### Phase 2: refactor the mobile app into shell and zones

1. A GPU adapter for the Rust Native view schema: the Rust-laid-out display
   lists that UIKit and Kotlin paint today are painted by the engine instead.
2. Platform bridges for accessibility, text input, selection handles, and
   scrolling curves.
3. Zone launcher, manifests, capability broker, and first-party zones for chat,
   settings, and wallet; the Grid and the Haunted House as loadable zones.

Gates: accessibility audits on iOS and Android equal or better than the current
app; text input parity (IME, dictation, autofill, passkeys); store review
passes with downloadable zones. Effort (speculation): 16–30 engineer-weeks.
Risk: users notice non-native feel; mitigate with per-platform scroll and
haptic conventions.

### Phase 3: desktop

The GPUI app first hosts engine surfaces for zones, then panes move one at a
time to the GPU UI framework until GPUI can be removed. Gate: each pane reaches
feature parity and equal input latency. Effort (speculation): open-ended; the
`ui` crate alone is about 194k lines.

### What to stop doing now

- No new features in `apps/openagents/crates/mobile/src/medieval_town`; it is a
  second renderer of the same town (audit ADR 0003 already freezes it).
- No new Verse v1 renderer features (audit ADR 0005).
- No new cards written separately per shell (already an `AGENTS.md` rule) and
  no fourth UI vocabulary.
- No new React components for in-world UI that phase 0 will draw on the GPU.
- No GPU performance claims from headless Linux Chrome on CoderOS.
- No new OpenAgents patches to the GPUI fork beyond fixes the desktop needs.

## Open questions and decisions for the owner

1. **Principle:** adopt GPU Native (one renderer for UI and worlds, native or
   DOM only for text input, accessibility mirror, sign-in, payments, and SEO
   pages), reversing Rust Native's native-widget premise while keeping its
   view schema?
2. **Name:** "GPU Native" for the principle and an engine-family name (for
   example "Aethyr") for the framework, or "GPU Native" for both?
3. **Stack:** keep Rust over `af-gpu` with the packed import, and accept the
   evidence list above as the conditions for revisiting?
4. **ADR 0011:** allow in-world and app UI on the web to move from React to the
   GPU UI behind accessibility gates, with React kept for the editor until then?
5. **GPUI:** keep it desktop-only with no new investment and retire it in
   phase 3, rather than forking a subset now?
6. **The Grid:** does the name move from the current Verse tab to the
   coding-agent zone, and what does the Verse tab become?
7. **One UI vocabulary:** merge `ui.screen.v1` and the Rust Native view schema?
8. **Zones and stores:** accept "data, WGSL, and interpreted bytecode only" as
   the rule for downloadable zones, and is a legal review of current Apple and
   Google text needed before phase 2?
9. **Accessibility bar:** what is the required level (for example, full
   VoiceOver, TalkBack, and NVDA operation of every control) before a GPU-drawn
   surface replaces a native or DOM one?
10. **Timing:** start phase 0 after audit phase 2, or run parts of it (the
    token set and device profile) in Aethyrforge before the cutover?
