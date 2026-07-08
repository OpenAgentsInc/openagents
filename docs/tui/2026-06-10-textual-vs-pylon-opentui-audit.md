# Textual vs. Pylon TUI (Bun/Effect/OpenTUI) — Architecture Audit, Gap Analysis, and What to Learn

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-10
Author: agent audit (Claude Code)
Reference repo: `projects/repos/textual` (workspace-root reference lane, read-only)
Our surface: `apps/pylon/src/index.ts` and `apps/pylon/packages/runtime/src/opentui-renderer.ts`
OpenTUI source (reference): `projects/repos/opentui`

## Purpose

Textual is the most mature retained-mode terminal UI framework in existence,
written in Python by Will McGugan (also the author of Rich). Our TUI surface —
the Pylon dashboard — is written in Bun/TypeScript on Effect and OpenTUI. The
language and runtime are different, but the architectural problems are the
same: layout, partial repaint, virtualization, input dispatch, focus,
testability. This audit compares the two stacks layer by layer, identifies
where Textual is simply ahead, and extracts the specific ideas worth porting
into our Bun/Effect/OpenTUI world — without copying any code.

## Scale snapshot

| Dimension | Textual | Pylon TUI |
|---|---|---|
| Core framework LOC | ~82,400 (`src/textual/`, 247 files) | OpenTUI provides the framework (~50+ TS files + Zig core); we consume it |
| App-level TUI LOC | n/a (framework) | ~1,700 (`apps/pylon/src/index.ts`) + 157 (`opentui-renderer.ts`) |
| Built-in widgets | 58 widget files (DataTable, TextArea, Tree, Markdown, …) | ~16 OpenTUI renderables; we use 6 (Box, Text, ScrollBox, Markdown, Textarea, + runtime Code/Diff) |
| Test files | ~410 (~46,600 LOC), incl. SVG snapshot tests | 24 (pylon) + 41 (runtime), zero TUI-specific |
| Screens/routing | Screen stack, modes, modal screens | Single fixed dashboard, no routing |
| Styling | Full CSS dialect (TCSS) with cascade/specificity | Imperative property assignment + flexbox props |
| Maturity | Production framework, large ecosystem | Observational dashboard for a contributor node |

This is not an apples-to-apples comparison — Textual is a framework and Pylon
is an app on top of a younger framework (OpenTUI). The useful comparison is:
(a) what does OpenTUI already give us that matches Textual, (b) what is missing
at the framework layer, and (c) what is missing at our app layer regardless of
framework.

## Layer-by-layer comparison

### 1. Event/concurrency model

**Textual.** Every widget is a `MessagePump` (`src/textual/message_pump.py`)
with its own asyncio message queue. Events (50+ types in `events.py`) are
messages that bubble up the DOM tree until handled or stopped. Handlers are
registered declaratively via an `@on(Message, selector)` decorator scanned at
class-creation time by a metaclass; selector matching reuses the CSS query
engine, so a parent can write `@on(Button.Pressed, "#save")`. Context vars
(`active_app`, `active_message_pump`) give async-safe ambient context. There is
a `prevent()` context manager to suppress message categories during
programmatic mutation (e.g. set an input's value without firing `Changed`).

**Pylon.** Direct callback registration on renderables (`onKeyDown`,
`onMouseScroll`, `onSubmit`) plus hand-rolled SGR mouse-sequence parsing in
`index.ts`. Background work runs as fire-and-forget Effect fibers via a
`runBackgroundEffect` helper (`Effect.runPromise` with error logging, no fiber
handles, no interruption, no supervision). UI state lives in module-level
mutable refs (`globalRenderer`, `logScrollBox`, `balanceTextRenderable`, …).

**Assessment.** This is the largest conceptual gap. Textual's insight is that a
TUI is a distributed system of tiny actors: each widget owns a queue, messages
bubble, and the framework guarantees ordering and lifecycle. Our equivalent
primitive already exists and is arguably better suited: Effect fibers, `Queue`,
`PubSub`, `Scope`, and `Layer`. We are using almost none of it — Pylon's
background loops are unsupervised and uninterruptible, and UI mutation is
scattered global-ref pokes. Ironically we have a stricter concurrency runtime
than asyncio and use it less rigorously than Textual uses asyncio.

**Learn:**
- Model each pane as an Effect service with a typed inbound message queue
  (`Queue<PaneMsg>`), supervised in a `Scope` so shutdown interrupts cleanly.
  This is the Effect-native translation of `MessagePump`.
- Adopt message bubbling for UI events: a single `PubSub<UiEvent>` with typed
  events replaces ad hoc callback wiring and makes cross-pane behavior (Tab
  focus cycling, global shortcuts) declarative.
- Port the `prevent()` idea: when programmatically setting composer content or
  scroll position, suppress the resulting change events. Cheap to do with a
  `FiberRef<Set<EventTag>>`.

### 2. Rendering pipeline and partial repaint

**Textual.** A compositor (`src/textual/_compositor.py`, 44KB) maintains a map
of widget → screen-space geometry, tracks a dirty-widget set, and emits minimal
terminal updates (`ChopsUpdate` spans, cursor-positioned region writes).
Widgets render immutable `Strip`s (lines of Rich `Segment`s) which are cached
at several levels: per-strip FIFO caches for crop/divide/style, a per-widget
`StylesCache` for border+padding line composition, and LRU caches on hot
functions. The "Line API" lets virtualized widgets implement
`render_line(y) -> Strip` so a million-row table renders only ~40 visible
lines per frame.

**Pylon/OpenTUI.** OpenTUI owns the frame loop (`createCliRenderer`,
`targetFps`) with a native Zig core doing buffer diffing and yoga-layout
(flexbox) for geometry. Our app code does zero rendering work beyond property
assignment and `.add()`-ing MarkdownRenderables into a ScrollBox; the only raw
ANSI we emit is a scroll-lock toggle.

**Assessment.** OpenTUI's Zig double-buffer diffing is a legitimately different
(and simpler) answer to the same problem Textual solves with dirty regions and
strip caches: Textual minimizes *recomputation*, OpenTUI minimizes *terminal
writes* and brute-forces recomputation at native speed. For our current
dashboard this is fine. It stops being fine the moment we render large
scrollback or tables: our log pane keeps up to 1,000 MarkdownRenderables alive
in a ScrollBox, every one of them a layout node, regardless of visibility.
Textual would never do this — RichLog and DataTable render only visible lines
from a backing data structure.

**Learn:**
- The Line API is the single most portable idea in Textual. For the log feed,
  replace "one renderable per log entry" with one virtualized renderable backed
  by an array of pre-rendered lines, rendering only the visible window.
  OpenTUI's `TextBuffer`/`EditBuffer` primitives are the natural substrate.
- Cache rendered markdown: Pylon re-parses markdown on streaming updates.
  Textual's pattern — immutable rendered lines, append-only, cache keyed on
  content+width — applies directly.
- Adopt width-keyed caching discipline: every Textual cache key includes
  available width, so resize invalidates correctly for free.

### 3. Layout and styling

**Textual.** A full CSS dialect (TCSS): tokenizer/parser
(`src/textual/css/parse.py`), specificity, cascade, pseudo-classes
(`:focus`, `:hover`, `:dark`), `fr` fractional units resolved with exact
`Fraction` arithmetic, hot-reloadable stylesheets, and six layout algorithms
(vertical/horizontal/grid/dock/stream). Styling is separated from widget logic;
themes are CSS variable swaps.

**Pylon.** Inline flexbox props and `parseColor` literals hardcoded in
`index.ts`. One GitHub-Dark `SyntaxStyle` defined inline. No theming, no
hover/focus styling beyond what OpenTUI defaults provide.

**Assessment.** We should *not* build a CSS engine — that's ~150KB of Textual's
source and most of its complexity budget, justified only for a general-purpose
framework. But two sub-ideas are worth stealing cheaply: (1) centralized theme
tokens instead of scattered color literals, and (2) state-conditional styling
(focused pane gets a highlighted border) expressed as data, not imperative
toggling.

**Learn:**
- Extract a `theme.ts` token module (colors, border styles, syntax style) and
  forbid inline `parseColor` calls in view code. This is the 1% of TCSS that
  pays for itself immediately.
- Represent focus styling declaratively: a tiny `applyFocusStyle(pane, focused)`
  driven by the focus service, rather than per-callsite border mutation.

### 4. Reactivity and state → UI flow

**Textual.** `reactive()` descriptors (`src/textual/reactive.py`) with
`watch_x` callbacks, `compute_x` derived values, validation, and per-attribute
declarations of consequence (`layout=True`, `repaint=True`, `recompose=True`).
Setting `self.count = 5` automatically schedules exactly the right amount of
re-render.

**Pylon.** Manual: poll loop fetches wallet balance → `Effect.sync(() => {
balanceTextRenderable.content = ... })`. State and view are fused; there is no
model layer to test.

**Assessment.** Effect has a strictly more powerful primitive here:
`SubscriptionRef` / `Stream`. Textual's reactive system is a workaround for
Python lacking observable state; we have observable state natively and don't
use it. The right architecture for Pylon is: each domain service exposes a
`SubscriptionRef<State>`; the view layer subscribes once and maps state changes
onto renderable mutations. That gives us Textual's watch/compute semantics,
plus testability (assert on the ref, no renderer needed), in idiomatic Effect.

**Learn:**
- Introduce a model layer: `WalletState`, `TelemetryState`, `OperatorState`,
  `LogFeedState` as `SubscriptionRef`s owned by their service. The TUI becomes
  a pure subscriber. This is the Effect translation of reactive+watch and the
  prerequisite for everything in §7 (testing).
- Steal the *consequence declaration* idea: when wiring a state field to the
  view, declare whether a change is content-only (cheap mutation) or
  structural (rebuild children). Today everything is implicit.

### 5. Widget depth and virtualization

**Textual.** 58 widgets; the flagship ones are serious engineering: DataTable
(2,864 LOC — virtualized, three-level cell/line/row LRU caches, row/column keys
decoupled from visual position), TextArea (2,798 LOC — tree-sitter syntax
highlighting, document model, multi-line selection), Tree (1,601 LOC — lazy
loading), Markdown, OptionList, TabbedContent, Collapsible.

**Pylon.** Six renderables in a fixed arrangement. OpenTUI itself offers ~16
(including Select, Input, Slider, TabSelect, Code, Diff) of which the dashboard
uses a third.

**Assessment.** We don't need 58 widgets. We need maybe three good ones that we
currently lack, all of which Textual demonstrates the design for:
- a **virtualized log/feed** (Textual: RichLog + Line API) — see §2;
- a **table** for assignments/jobs/payouts (Textual: DataTable's key-vs-position
  separation is the part to copy — rows keyed by job id survive re-sorting);
- a **tabbed or stacked screen container** (Textual: Screen stack + modes) the
  moment the dashboard grows a second surface (forum, wallet ops, assignments).

### 6. Input, focus, and keybindings

**Textual.** A first-class `Binding` system: widgets/screens/apps declare
`BINDINGS = [Binding("ctrl+s", "save", "Save")]`; a Footer widget auto-displays
them; a built-in command palette (ctrl+p) exposes every action fuzzily. Focus
is a framework concern with traversal order, and 200+ named keys are
normalized across terminals by the driver layer.

**Pylon.** Tab toggles between exactly two focusables (composer, log box);
arrow/page keys are forwarded manually inside `onKeyDown` handlers; the key map
exists only as code. No discoverability — a user cannot find out that Tab
switches panes without reading source.

**Learn:**
- Declare bindings as data: a `bindings: Array<{ key, action, description }>`
  table per pane, dispatched by one global handler. Render the table in a
  one-line footer. This is low-effort and transforms usability — it is the
  most user-visible item in this audit.
- A command palette is a natural later step (OpenTUI has Select; a modal
  fuzzy-filter overlay over the action table is a weekend project) and fits
  Pylon's grow-into-control-panel trajectory.

### 7. Testing — the widest gap

**Textual.** `Pilot` (`src/textual/pilot.py`) drives a real app headlessly:
`async with app.run_test() as pilot: await pilot.press("enter")`. A
`HeadlessDriver` makes this fast and CI-safe. `pytest-textual-snapshot` renders
the app to SVG and diffs frames — ~410 test files including visual regression
for nearly every widget.

**Pylon.** 65 test files across pylon+runtime, all backend. Zero tests exercise
the TUI; there is no way to instantiate the dashboard without a TTY, because
view construction, state, and service startup are fused inside `runPylonNode`.

**Learn (in order):**
1. The refactor in §4 (state as `SubscriptionRef`s) makes 80% of "TUI logic"
   testable with no renderer at all — assert that a wallet poll transitions
   `WalletState` to OFFLINE, not that a string turned red.
2. Check whether OpenTUI exposes a headless/string-buffer renderer (its Zig
   core renders to a buffer that should be dumpable); if so, build a tiny
   Pilot-equivalent: construct dashboard → inject key events → snapshot the
   text buffer with `bun:test` snapshots. If not, this is the single most
   valuable upstream contribution we could make to OpenTUI.
3. Snapshot-test the layout at 2–3 terminal sizes to catch flexbox regressions.

### 8. Lifecycle, supervision, and modes

**Textual.** App owns a screen stack with push/pop/switch, modal screens with
typed results (`push_screen_wait` returns a value), workers with automatic
cancellation when their widget unmounts, and a clean three-phase lifecycle
(compose → mount → unmount) per widget. Also: inline mode (render below the
prompt and exit) and `textual serve` (same app served to a browser over
WebSocket).

**Pylon.** One screen, `Effect.never` at the bottom of main, fibers leak on
exit (process death is the cleanup strategy). No modal/confirm primitive —
a real gap for a node that handles money: there is no way to ask "really send
50,000 sats?" in the TUI.

**Learn:**
- Wrap the whole TUI in a `Scope`; launch every background loop with
  `Effect.forkScoped`. Ctrl+C then interrupts fibers, flushes the renderer,
  and restores the terminal deliberately rather than by process death.
- Add one modal primitive: a centered Box overlay + focus capture returning
  `Effect<boolean>`. Textual's `push_screen_wait` shows the right API shape —
  awaitable, typed result. Wallet sends and payout-target admissions should go
  through it.
- Note the web-parity idea for later: Textual proves a terminal app can serve
  itself to a browser by shipping the same frame stream over WebSocket.
  OpenTUI's buffer-diff core makes this plausible for us too, and it aligns
  with the operator-surface direction (watch a Pylon node from anywhere).

### 9. Performance notes

Textual's tricks (strip caches, Fraction-based fractional scrolling, GC pause
during scroll) mostly compensate for Python being slow. Bun + a Zig render core
makes whole categories of those tricks unnecessary — we should not port them
reflexively. The two that survive the language change are **virtualization**
(visible-window rendering, §2/§5 — no runtime speed saves you from holding
100k layout nodes) and **width-keyed render caching** for markdown/code, since
tree-sitter parsing and markdown layout are expensive in any language.

## Gap analysis summary

| Capability | Textual | Pylon today | Severity | Owner of fix |
|---|---|---|---|---|
| Supervised concurrency / clean shutdown | message pumps, workers, unmount cancellation | fire-and-forget fibers, exit by process death | High | Pylon (pure Effect work) |
| State/view separation | reactive + watch/compute | global refs mutated in place | High | Pylon |
| TUI testability | Pilot + headless driver + SVG snapshots | none | High | Pylon + possibly OpenTUI upstream |
| Log/feed virtualization | Line API, RichLog | 1,000 live MarkdownRenderables in a ScrollBox | Medium-High | Pylon |
| Keybinding declaration + discoverability | Binding system, Footer, command palette | hardcoded handlers, undocumented keys | Medium | Pylon |
| Modal/confirm flows | modal screens, awaitable results | none (money flows with no confirm UI) | Medium | Pylon |
| Multi-screen routing | screen stack + modes | single dashboard | Medium (rises as features grow) | Pylon |
| Theming | TCSS, themes, live reload | inline color literals | Low-Medium | Pylon (token module) |
| Rich tables/trees | DataTable, Tree | none | Low today, Medium when assignments UI lands | Pylon (OpenTUI primitives suffice) |
| CSS engine | full TCSS | n/a | Not a gap — deliberately skip | — |
| Web serving of TUI | textual-serve | n/a | Opportunity, not a gap | OpenTUI/Pylon later |

## Recommended sequence

1. **State layer first** (§4): domain `SubscriptionRef`s + view-as-subscriber.
   Unblocks testing and every later item; pure Effect work, no OpenTUI changes.
2. **Scoped supervision** (§8): `Effect.forkScoped` everything, deliberate
   shutdown path.
3. **Bindings-as-data + footer** (§6): biggest usability win per line of code.
4. **Virtualized log feed** (§2/§5): replaces the per-entry renderable model;
   needed before the log pane becomes the execution-evidence surface.
5. **Headless render harness + snapshot tests** (§7): investigate OpenTUI
   buffer dumping; contribute upstream if missing.
6. **Modal confirm primitive** (§8): gate wallet sends behind it.
7. **Screen stack** (§8): only when a second surface (assignments table, forum)
   actually lands.

## What not to take from Textual

- **The CSS engine.** Highest-complexity subsystem, justified only for a
  general-purpose framework with external users. Theme tokens give us 80% of
  the value at 1% of the cost.
- **The message-pump-per-widget actor model wholesale.** Effect fibers +
  queues + streams are our native idiom; translate the *guarantees* (ordering,
  supervision, bubbling), not the mechanism.
- **Python-performance workarounds** (GC pausing, Fraction layout math,
  aggressive micro-caching of segment ops). The Zig core makes these moot.
- **The 58-widget surface area.** Pylon is one product's dashboard; build the
  three widgets the product needs.

## Closing observation

Textual's deepest lesson is not any single mechanism — it is that the team
treated the terminal as a *platform* and budgeted accordingly: a real
compositor, a real style system, a real test harness, real docs. Our stack
choice (Bun + Effect + OpenTUI's Zig core) starts from a stronger runtime
foundation than Python ever offered Textual, but the Pylon dashboard currently
uses that foundation at the level of a status script: global refs, unsupervised
fibers, untested UI. The gap is not the framework — OpenTUI's renderer model is
sound — it is architectural discipline at our app layer, and Effect gives us
better tools for it than Textual had. Items 1–3 above are cheap and would close
most of the discipline gap this sprint.
