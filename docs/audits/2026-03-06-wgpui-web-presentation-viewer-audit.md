# 2026-03-06 WGPUI Web Presentation Viewer Audit

> Historical note: This audit is a point-in-time snapshot from its date. Current product and architecture authority lives in `README.md`, `docs/MVP.md`, `docs/OWNERSHIP.md`, and `crates/wgpui/README.md`. File paths, implementation-status claims, and historical comparisons here may be superseded by later commits.

- Author: Codex
- Status: complete
- Scope: audit of historical in-repo WGPUI web presentation/visualization work, current retained WGPUI/browser capability, ownership boundaries, and recommended repo layout for a new web-based presentation viewer

## Objective

Answer five questions:

1. What presentation-like or visualization-like browser work used to exist in this repo?
2. What reusable WGPUI/browser capability is still retained today?
3. What is missing if we want to ship a browser slide deck viewer now?
4. Where should the code live under the current MVP and ownership rules?
5. What is the lowest-regret path to build a web-based presentation viewer using WGPUI components?

## Sources Reviewed

Current authority and retained surfaces:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `Cargo.toml`
- `crates/wgpui/README.md`
- `crates/wgpui/docs/MVP_BOUNDARIES.md`
- `crates/wgpui/Cargo.toml`
- `crates/wgpui/src/lib.rs`
- `crates/wgpui/src/platform/web.rs`
- `crates/wgpui/src/components/`
- `crates/wgpui/src/markdown/`
- `crates/wgpui/examples/`
- `crates/wgpui/src/testing/README.md`

Historical browser surfaces recovered from git:

- `71a17941e` `feat(web): add /2026 page and improve /gfn page`
- `838492ab8` `Add the agent network transcript route`
- `2c8e5a981` `Prune autopilot MVP workspace`

Historical files inspected from git history:

- `crates/web/client/src/app.rs`
- `crates/web/client/src/views/gfn.rs`
- `crates/web/client/src/views/mod.rs`
- `crates/web/client/src/state.rs`
- `crates/web/worker/src/routes/gfn.rs`

## Executive Recommendation

Build the new presentation viewer as a **new app surface**, not inside `apps/autopilot-desktop` and not as a large workflow layer inside `crates/wgpui`.

Recommended split:

| Layer | Recommendation | Why |
| --- | --- | --- |
| Browser presentation app | New workspace member: `apps/presentation-viewer-web` | This is app behavior, routing, deck loading, navigation, and browser bootstrapping, which do not belong in `wgpui`. |
| Reusable UI primitives | Keep in `crates/wgpui` only if truly product-agnostic | This respects `docs/OWNERSHIP.md` and avoids turning `wgpui` into an app framework. |
| Demo/proving ground | Use `crates/wgpui/examples/storybook` and/or a tiny desktop example for component validation | Good for iterating on chrome/widgets before wiring the browser app. |
| Authoring/docs | Add deck authoring docs under `docs/` | The viewer needs an explicit content format, not just renderer code. |

Do **not** resurrect the old `crates/web/` stack wholesale. Reuse its lessons, visual language, and the viable parts of its architecture, but rebuild a smaller, cleaner browser app around the retained `wgpui` web lane.

## Bottom Line

The repo still has enough retained WGPUI infrastructure to support a browser presentation viewer, but it no longer has a retained browser app shell. The strongest path is:

1. Build a new small wasm app in `apps/presentation-viewer-web`.
2. Keep deck state, routing, slide parsing, keyboard navigation, and browser-specific event wiring there.
3. Only upstream generic pieces into `crates/wgpui` when they are clearly reusable outside presentations.

## What We Had

## 1. A real browser WGPUI app surface

At the start of 2026, the repo had a browser app stack under `crates/web/`:

- a wasm client using `wgpui::WebPlatform`,
- worker routes that served HTML shells and mounted a canvas,
- route-specific page flags like `window.GFN_PAGE = true`,
- an `AppState` / `AppView` model that selected different page views,
- custom DOM event translation into `wgpui::InputEvent`.

That matters because it proves the repo already had a working pattern for:

- full-screen WGPUI browser pages,
- browser resize handling,
- keyboard/mouse/wheel event adaptation,
- route-specific views rendered into a single canvas.

## 2. The `/gfn` visualization was effectively a mini presentation app

The clearest historical fit for your current goal is the old `/gfn` page.

It was not a slide deck, but it already had most of the presentation DNA:

- full-viewport canvas rendering,
- a designed visual language using `Frame::corners()` and `DotsGrid`,
- a scrollable card surface,
- an interactive slider,
- comparison diagrams,
- explanatory sections,
- a CTA,
- custom page-local state (`GfnState`) with hover, scrolling, animation, and input bounds.

The visual grammar was already presentation-grade:

- dark HUD background,
- centered framed content,
- animated frame entrance,
- high-contrast accent colors,
- custom scrollbar treatment,
- clear typography hierarchy,
- narrative structure from title -> visualization -> explanation -> CTA.

## 3. The old worker route architecture was simple and effective

The `/gfn` route itself was extremely thin:

- return HTML,
- mount a `<canvas>`,
- load the wasm bundle,
- call `start_demo("canvas")`,
- set page flags.

That is useful because it shows the browser viewer does **not** need a huge frontend stack to work. A Rust wasm app plus a tiny HTML shell is enough.

## 4. The old web client also contained anti-patterns

The old retained browser app was functional, but it had clear costs that should not be copied:

- one large `AppState` for many unrelated surfaces,
- route/view selection by global flags,
- browser event handling centralized in a large wasm client module,
- product pages, repo views, HUD logic, and experimental pages mixed together,
- page-local state embedded in a large shared app state model.

For a new presentation viewer, those are avoidable.

## What We Have Now

## 1. `wgpui` still retains a viable web lane

This is the most important positive result of the audit.

`crates/wgpui` still explicitly supports a web build via the `web` feature:

- `wasm-bindgen`
- `web-sys`
- `js-sys`
- `console_error_panic_hook`
- browser `wgpu` backend selection with WebGPU/WebGL2 fallback

The retained `WebPlatform` in `crates/wgpui/src/platform/web.rs` already handles:

- locating or receiving a browser canvas,
- device-pixel-ratio sizing,
- surface/device/queue creation,
- WebGPU-first and WebGL2-fallback boot,
- resize reconfiguration,
- rendering `Scene` output to the canvas,
- a browser animation loop helper,
- a resize observer helper.

That means the rendering substrate for a browser deck viewer still exists.

## 2. `wgpui` has a strong reusable visual toolkit

For slide deck work, the retained component set is already substantial:

- HUD chrome: `Frame`, `DotsGrid`, `StatusBar`, `Notifications`, `Hotbar`, `Reticle`, `Heatmap`, `RingGauge`, `SignalMeter`
- general UI: `Button`, `Tabs`, `Dropdown`, `Modal`, `TextInput`
- containers: `ScrollView`, `VirtualList`
- content rendering: `MarkdownRenderer`, `MarkdownView`, `StreamingMarkdown`
- text/layout/render primitives: `Scene`, `Quad`, `TextSystem`, theme tokens, geometry/input APIs

That is enough to build:

- title slides,
- bullet slides,
- side-by-side content slides,
- code slides,
- HUD-styled chapter dividers,
- speaker-note panels,
- agenda/index overlays,
- animated transitions between slides,
- overview mode or filmstrip mode.

## 3. The repo still has good non-product proving grounds

The current repo retains two valuable places to validate presentation UI ideas before wiring a browser app:

- `crates/wgpui/examples/storybook`
- `crates/wgpui/src/testing/`

This is useful because the presentation viewer will likely need new generic components like:

- slide title/header chrome,
- presenter progress indicator,
- deck index/overview strip,
- notes panel,
- citation/footer bar,
- full-screen code block layout,
- asset placeholders and media frames.

Those can be proven in storybook or desktop examples before deciding whether they belong in `wgpui`.

## 4. The markdown lane is useful, but not slide-ready by itself

The retained markdown support is real and reusable:

- headings
- paragraphs
- unordered and ordered lists
- blockquotes
- tables
- code blocks with highlighting
- task lists
- streaming updates

But it is not yet a complete presentation authoring system.

Current limitations that matter for slide decks:

- `MarkdownConfig::default()` gives headers the same size as body text (`header_sizes: [1.0; 6]`), so the default hierarchy is weak for presentation use.
- Links are styled, but `MarkdownView` does not currently implement link interaction/hit targets.
- Images are ignored by the parser (`Tag::Image { .. } => {}`).
- There is no deck model such as `Deck`, `Slide`, `SlideTransition`, `SpeakerNotes`, or `PresenterState`.
- There is no built-in concept of slide separators, fragments, incremental reveal, or presenter notes.

Conclusion: markdown is a **building block**, not the whole solution.

## 5. The browser app shell is gone

This is the biggest current gap.

The repo no longer has:

- a retained web app workspace member,
- a static HTML shell,
- route serving for browser pages,
- wasm bundle bootstrapping around a presentation surface,
- a retained DOM-to-`InputEvent` adapter,
- URL/hash routing for current slide,
- a retained hosting/deploy lane for browser-only demos.

This is the main reason the presentation viewer should be a new app surface rather than “just a WGPUI component.”

## What Is Missing

## 1. No retained browser input/app framework on top of `WebPlatform`

`WebPlatform` solves rendering and resize, but not full app behavior.

A new browser viewer still needs code for:

- mouse move / mouse down / mouse up translation,
- wheel handling,
- keyboard input mapping,
- focus behavior,
- pointer cursor changes,
- URL hash or query-state synchronization,
- loading deck content and assets,
- animation loop state updates,
- fullscreen/presenter mode toggles.

Historically this lived in `crates/web/client/src/app.rs`. Today it does not exist in retained form.

## 2. No retained deck domain model

The repo currently has:

- rendering primitives,
- components,
- markdown blocks,
- demo/storybook state,

but it does not have:

- `Deck`
- `Slide`
- `SlideKind`
- `SlideTheme`
- `DeckManifest`
- `SpeakerNotes`
- `AssetRef`
- `SlideTransition`
- `DeckRouter`

Without that model, any presentation work risks collapsing into one large state file like the old browser client.

## 3. No retained asset/content pipeline for slide authoring

There is currently no standard path for:

- deck markdown files,
- slide-local images,
- SVG/chart assets,
- code samples,
- theme manifests,
- precomputed deck JSON,
- content hot reload in browser mode.

This must be designed up front or the viewer will become hard-coded.

## 4. No retained browser hosting/deploy path

The old browser surface used worker routes and a wasm bundle. The current repo has neither a retained worker app nor any HTML/package-managed browser shell.

You do not need React/Vite to solve this, but you do need **some** explicit hosting story:

- static HTML + wasm assets,
- a tiny Rust server,
- or a new deployable web app surface.

## Where Stuff Should Go

## 1. Do not put the presentation viewer in `apps/autopilot-desktop`

This would violate the spirit of the current MVP scope.

Reasons:

- `docs/MVP.md` is desktop-first and tightly scoped to the Autopilot earn loop.
- `apps/autopilot-desktop` owns product behavior, pane orchestration, and MVP flows.
- A browser presentation viewer is not part of the current desktop MVP path.
- Mixing it into the desktop app would increase product/runtime surface area for no MVP gain.

## 2. Do not put the full presentation app inside `crates/wgpui`

This would violate `docs/OWNERSHIP.md`.

`crates/wgpui` owns:

- product-agnostic UI APIs,
- rendering and component surfaces,
- platform abstraction.

It must not own:

- product/app workflows,
- app-specific business logic.

A presentation viewer app includes workflow logic:

- deck loading,
- slide routing,
- presenter controls,
- browser history,
- content manifests,
- keyboard navigation semantics,
- speaker-notes logic.

That belongs in an app, not the toolkit.

## 3. Recommended new app surface

Create:

- `apps/presentation-viewer-web`

This is the cleanest fit because it keeps:

- browser bootstrapping,
- deck parsing/loading,
- viewer state,
- route handling,
- presentation-specific controls

in an app-owned lane, while allowing `wgpui` to stay reusable.

## 4. What can move into `crates/wgpui`

Only move code into `wgpui` if it is reusable outside presentations.

Good candidates:

- a generic `PresentationChrome`-style frame component if it is truly app-agnostic
- reusable viewport/pager helpers
- generic keyboard navigation helpers
- generic image/SVG/media components
- markdown improvements that help any app, not just presentations
- generic “deck overview strip” only if it is framed as a generic pager/index component

Bad candidates:

- your deck manifest format
- slide sequencing logic
- speaker notes semantics
- URL routing behavior
- page-specific narrative content
- presentation-specific app state

## Proposed Repo Layout

Recommended shape:

```text
apps/presentation-viewer-web/
  Cargo.toml
  src/
    lib.rs
    app.rs
    state.rs
    input.rs
    routes.rs
    deck/
      mod.rs
      model.rs
      parser.rs
      manifest.rs
    views/
      mod.rs
      slide.rs
      overview.rs
      presenter.rs
    widgets/
      deck_frame.rs
      progress_bar.rs
      notes_panel.rs
  static/
    index.html
docs/presentations/
  README.md
  deck-format.md
  sample-deck.md
```

Optional WGPUI additions only if justified by reuse:

```text
crates/wgpui/src/components/organisms/
  media_frame.rs
  pager_strip.rs
crates/wgpui/examples/
  presentation_chrome_demo.rs
```

## Recommended Authoring Model

## 1. Start with markdown-plus-metadata, not raw scene code

The best low-regret starting point is:

- one deck file per presentation,
- markdown content for text-heavy slides,
- explicit slide separators,
- frontmatter or slide metadata for layout/theme,
- structured asset references for images/SVG/code.

Example direction:

```md
---
title: WGPUI Presentation Viewer
theme: hud
---

# Why this exists

- Browser-native deck viewer
- Rust/WGPU stack
- Reuses OpenAgents visual language

---
layout: two-column
title: Current State
left:
  - WGPUI web platform retained
  - No retained browser app shell
right:
  - Markdown exists
  - Images/links need work
```

I would **not** start by treating `MarkdownView` as “the presentation engine.” Use it as a renderer for slide bodies, notes, or text regions.

## 2. Separate deck parsing from slide rendering

Keep these layers explicit:

- `deck::parser`: turns source files into typed deck models
- `deck::model`: typed slide definitions
- `views::slide`: chooses WGPUI layout/components for a given slide kind
- `app::state`: navigation, fullscreen, notes visibility, current slide, transitions

This is exactly the separation the historical `crates/web` lane lacked.

## 3. Support two slide lanes early

Start with:

1. `MarkdownSlide`
2. `CustomSlide`

`MarkdownSlide` is for:

- title slides
- bullets
- simple tables
- code listings

`CustomSlide` is for:

- charts
- animated diagrams
- side-by-side HUD compositions
- timeline or network views
- interactive demos

That gives you the flexibility of the old `/gfn` page without hard-coding every slide as scene math.

## Recommended Feature Set For V1

## Must-have

- Full-screen canvas deck viewer
- Left/right arrow navigation
- `Home` / `End` navigation
- Clickable next/prev zones
- URL hash for current slide
- Slide counter/progress indicator
- HUD frame and background theme
- Markdown slide body rendering
- Code block rendering
- Simple two-column slide layout
- Presenter notes toggle

## Strongly recommended

- Overview mode / slide index
- Fade or slide transition animation
- Theme presets (`hud`, `minimal`, `code`, `diagram`)
- Footer slot for source/citation
- SVG asset support
- Browser fullscreen mode

## Later

- Incremental reveal / fragments
- Speaker view in second window
- Live remote control
- Embedded audio/video
- Export to image/PDF

## Key Findings

## 1. The old `/gfn` work should be treated as design precedent, not code to restore

What is worth keeping from it:

- full-screen WGPUI browser composition
- HUD chrome
- page-local state model
- explicit interaction bounds
- route-specific browser shell
- simple wasm boot path

What should not be copied:

- giant shared app state
- page selection by global flags
- one large browser client owning unrelated surfaces
- hard-coded coupling to product-specific app views

## 2. `wgpui` is already good enough for presentation visuals

The blocker is not drawing capability.

The blocker is missing browser-app glue and missing deck/domain structure.

That is a good problem to have because it means the core rendering investment already exists.

## 3. The current markdown pipeline is a partial accelerator, not the destination

It saves time for:

- slide body copy
- bullet lists
- code samples
- notes

But it is insufficient as-is for:

- image-heavy slides
- clickable references
- presentation-grade heading hierarchy
- incremental reveal
- theme-aware layout variants

## 4. Storybook and desktop demos are valuable, but they are not the shipping home

They should be used to:

- develop generic components,
- tune chrome and motion,
- test layout primitives,
- validate visual polish.

They should not become the production browser viewer.

## Suggested Build Order

## Phase 1: Recover the browser lane cleanly

Deliver:

- new `apps/presentation-viewer-web`
- wasm entrypoint
- `WebPlatform` boot
- canvas HTML shell
- input translation
- one deck with arrow-key navigation

Goal:

- prove the browser app surface exists again in minimal form

## Phase 2: Add deck model and authoring format

Deliver:

- typed `Deck` / `Slide`
- markdown deck parser
- slide separators and metadata
- title slide + bullet slide + code slide + two-column slide

Goal:

- stop hard-coding slides

## Phase 3: Add presentation chrome

Deliver:

- reusable frame/background theme
- progress bar / slide count
- overview mode
- notes panel
- URL-state sync

Goal:

- make it presentation-ready

## Phase 4: Upstream only proven generic pieces

Potential upstreams to `crates/wgpui`:

- image/SVG/media primitives
- pager/index widgets
- markdown improvements
- reusable chrome widgets

Goal:

- keep app logic in the app and toolkit logic in the toolkit

## Risks

## 1. Risk: building too much inside `wgpui`

If deck logic goes into `wgpui`, the crate will become an app framework and violate ownership boundaries.

## 2. Risk: rebuilding the old `crates/web` sprawl

If the new viewer starts as “a page inside a general web client,” it will likely inherit the same shared-state bloat the old browser stack had.

## 3. Risk: overcommitting to markdown as the only slide abstraction

This will become painful as soon as the deck needs diagrams, images, choreography, or presentation-specific layout semantics.

## 4. Risk: treating storybook as deployment

Storybook is a proving ground, not a browser product surface.

## Final Recommendation

If the goal is “I want to do slide deck presentations in the browser using WGPUI,” the repo is in a surprisingly good position, but only if you separate the problem correctly:

1. **Use the retained `wgpui` web lane as the renderer.**
2. **Create a new browser app surface for the viewer.**
3. **Use markdown as one slide content source, not the entire presentation model.**
4. **Borrow visual language and interaction patterns from the old `/gfn` page, but do not restore the old `crates/web` architecture.**
5. **Keep `apps/autopilot-desktop` out of it and keep `crates/wgpui` product-agnostic.**

In short: the old repo already proved that WGPUI can do browser-native presentation-like experiences. The current repo still has the rendering core, the HUD vocabulary, the markdown lane, and the demo/test tooling. What it does not have anymore is the browser app shell and the deck model. That is exactly what the next implementation should add, in a new app-owned lane.
