# Effect Native — One Effect-Based UI Substrate Across HTML, React Native, Canvas, and Terminal

Date: 2026-07-08
Status: exploratory analysis / thought piece (owner-prompted). Not a
decision and not a reversal of the standing ONE-UI direction. This doc
examines whether the Foldkit/Effect "declarative UI as an Effect program"
thesis extends cleanly to **React Native** and other render targets — via
a shared, Effect-Schema-typed view description with pluggable renderers —
using Hyperview (`projects/repos/hyperview`) as the reference for the
server-driven-native half of the idea.

Standing context this must respect: the owner UI decision (2026-07-04,
`CLAUDE.md`) is **one UI ecosystem — React + Tailwind** across web
(TanStack Start), Khala Code desktop (Electrobun + React), and mobile
(Expo React Native), with **Effect as the services/logic substrate
everywhere** and existing Foldkit surfaces treated as migration-era legacy.
"Effect Native" is a research thesis about the *substrate below the
renderer*, not a pitch to swap React out.

## 1. What Hyperview actually is (brief analysis)

Hyperview (Instawork, MIT) is **HTMX's idea for native mobile**: a
hypermedia format (**HXML** — an XML dialect) plus a React Native client
that fetches HXML screens from any HTTP server and renders them as *real
native* components. The server drives layout, content, and available
actions; the client is a thin, generic interpreter.

The architecture, from the source:

- **A small typed element vocabulary** (~18 elements in `src/elements/`):
  `hv-view`, `hv-text`, `hv-image`, `hv-list`, `hv-section-list`,
  `hv-text-field`, `hv-select-single/multiple`, `hv-switch`,
  `hv-date-field`, `hv-picker-field`, `hv-spinner`, `hv-web-view`, plus
  navigation elements (`hv-navigator`, `hv-route`, `hv-screen`). Each maps
  an HXML tag to a React Native component. This is a *closed catalog* —
  exactly the discipline our typed-component-over-SSE channel already uses
  on the web (`khala-component-channel.ts`).
- **Behaviors** (`src/services/behaviors/`): declarative interaction
  attributes on elements — `trigger` (press/refresh/load/visible),
  `action` (replace/append/prepend/reload/navigate/…), `href`, `target`,
  `verb`. A press fetches HXML and swaps a subtree by id; a
  `shallowCloneToRoot` produces a new immutable DOM. This is HTMX's
  swap/trigger/target model transplanted onto a native tree — *the
  interaction model is data, not code*.
- **A tiny, dependency-light core** (`@instawork/xmldom`, lodash,
  tiny-emitter, url-parse) designed to embed into an existing RN app; RN
  navigation and gesture libs are peer deps. The client is deliberately
  generic — all app-specificity lives in the HXML the server emits.
- **The payoff it sells:** ship app changes by deploying your backend; no
  store review, no version fragmentation (every user runs "the current
  screen"), any backend can host it, and you can even serve static XML.

The essence: **the UI is a serializable, server-authored, typed tree; the
client is a pure interpreter of that tree plus a fixed behavior algebra.**
That framing is the interesting part — not the XML syntax, which is an
implementation choice (and, honestly, the weakest part for us: XML +
XSD schemas vs. our Effect-Schema world).

## 2. Why this rhymes with our stack

We already believe the three load-bearing ideas underneath Hyperview, in
Effect form, and ship two of them:

1. **UI as a typed, serializable description.** Foldkit models a surface
   as a declarative structure driven by Effect; `three-effect` renders a
   typed scene graph; Arbiter (`@openagentsinc/arbiter-effect`) is a typed
   `GraphSpec` (nodes/pins/links) rendered by a pluggable renderer — the
   *same* spec drives an SVG/HTML renderer today and could drive others.
   Our whole "typed component catalog validated at the boundary" pattern
   (the SSE `oa.component` channel; Sarah's components) is Hyperview's
   closed-element vocabulary by another name.
2. **Interaction as data/effect, not imperative glue.** Behavior contracts
   already treat UX expectations as typed, testable oracles. A behavior
   algebra (trigger → action → target) is the runtime analog: interactions
   as values an interpreter runs, which is natural to express as small
   Effect programs.
3. **Server-driven surfaces.** Khala Sync already makes our *data* plane
   server-driven and local-first (SQLite mirror, durable cursors, typed
   mutations/events). Hyperview makes the *view* plane server-driven. We
   have the harder half (durable, exact, offline data) and lack only the
   view half — and the view half is precisely what would let mobile UI
   change without a store release, which is the same pain the OTA server
   (`apps/oa-updates`) exists to reduce.

So "Effect Native" is not a new religion — it's asking whether the
substrate we already run under DOM (typed view + Effect logic) can also run
under React Native, canvas/Three, and the terminal, with **one description
type and swappable renderers**.

## 3. The "Effect Native" thesis

**One Effect-based UI core, many render targets.** Concretely, three
layers:

```
┌──────────────────────────────────────────────────────────┐
│  View description  — Effect Schema-typed UI tree           │
│  (closed element/component catalog + typed props + a       │
│   typed behavior algebra: trigger → action → target)       │
├──────────────────────────────────────────────────────────┤
│  Behavior/runtime  — interactions as Effect programs;      │
│  data via Khala Sync; validation at the boundary; the      │
│  same authority/receipt discipline as everything else      │
├──────────────────────────────────────────────────────────┤
│  Renderer adapters — pluggable, per target:                │
│    • dom       (HTML/React on web — TanStack Start)         │
│    • native    (React Native / Expo — "Effect Native")     │
│    • canvas3d  (three-effect scene graph)                   │
│    • tty       (terminal — Toad/Textual-style, cf. wterm)   │
└──────────────────────────────────────────────────────────┘
```

The key insight from Hyperview: if the **view description and the behavior
algebra are the stable contract**, the renderer is a detail you can have
several of. React (web), React Native (mobile), Three (canvas), and a
terminal driver are all just interpreters of the same typed tree. Arbiter
already proves the "one spec, pluggable renderer" pattern in our codebase
at small scale.

Two independent axes worth separating, because they're often conflated:

- **Axis A — multi-target rendering** (one typed UI description → web +
  native + canvas + tty renderers). This is the "Effect Native" naming
  idea and is *renderer plurality*.
- **Axis B — server-driven UI** (the description is fetched/streamed from
  the server and swapped by id, Hyperview/HTMX-style). This is
  *authoring locus* and is orthogonal — you can have multi-target
  rendering with client-authored trees, or single-target with
  server-authored trees, or both.

Hyperview does A (native only) + B together. Our SSE component channel
does B on web with a closed catalog. The full "Effect Native" would be A
across our targets, with B available where it pays (mobile, to dodge store
review; Sarah's streamed components; the QA swarm board).

## 4. What it would buy us

- **Instant mobile updates without store review** (Axis B on native): the
  standing pain the OTA server only partly solves — OTA still ships a JS
  bundle; server-driven screens change with a backend deploy. For a
  sales-and-agent product iterating fast, that's the same leverage
  Hyperview sells, and it composes with Khala Sync (data) to make the
  mobile app a thin, durable interpreter over server-authored views +
  server-synced data.
- **One design system, honestly one system** (Axis A): the ONE-UI goal is
  "one UI ecosystem." Today that's "React everywhere," which shares
  *components* but still forks *render code* per platform (DOM vs RN vs
  canvas). A shared typed description would share the *description* and
  localize only the renderer — a stronger form of the same goal, and it
  would fold `three-effect` (canvas) and any terminal surface into the
  same substrate instead of leaving them as islands.
- **Testability and receipts for free.** A typed view tree + behavior
  algebra is trivially snapshot-testable and lends itself to the QAM
  visual-baseline and behavior-contract discipline — the description is
  the oracle. Agent-authored UI (Sarah's components, generative UI)
  becomes schema-validated by construction, which is exactly the
  `json-render`/closed-catalog safety we already want.
- **Agent-native by construction.** An agent emitting a typed view tree is
  safer and more legible than an agent emitting JSX. This is the same bet
  behind the SSE component channel, generalized to native.

## 5. The honest costs and conflicts

- **It runs against the 2026-07-04 ONE-UI decision as currently written.**
  That decision deliberately chose React + Tailwind + NativeWind + shadcn
  to *stop* framework proliferation and get velocity from a huge ecosystem.
  A bespoke Effect-native UI core is the opposite move — it's building
  framework, not using one. The Foldkit shell migration was explicitly
  *halted* for this reason. Reviving a Foldkit-descended UI substrate needs
  to clear that bar, and today it does not.
- **Renderer adapters are where frameworks go to die.** "One description,
  many renderers" is easy to draw and brutal to maintain at fidelity —
  every native gesture, every accessibility affordance, every keyboard
  behavior has to be re-expressed per adapter. React Native already *is*
  the "write once, render native" bet with a decade of investment; a
  thinner homegrown layer on top competes with that from behind.
- **We'd be reinventing Hyperview's hard-won parts** (native list
  virtualization, navigation integration, form controls, web-view escape
  hatch) unless we vendored/forked Hyperview itself — which is XML/XSD and
  would need an Effect-Schema reskin to fit our world.
- **The current product needs velocity, not a substrate.** P0–P2 (mobile
  MVP, Sarah, Codex cutover) and the sales push are the priority. Effect
  Native is infrastructure that pays off over years; committing to it now
  trades near-term shipping for long-term elegance at exactly the wrong
  moment.

## 6. Where the idea is actually strong (the narrow, adoptable slice)

Strip the maximalism and there's a real, low-risk kernel already aligned
with the roadmap:

1. **Standardize the typed-view-tree + closed-catalog pattern we already
   use** (SSE `oa.component`, Sarah's components, Arbiter's `GraphSpec`)
   into one small shared Effect-Schema contract — a "typed UI description"
   package — *without* claiming to replace React. Renderers stay
   React/RN/three; the shared thing is the description + validation +
   behavior-contract binding. This is Axis A's *contract* without Axis A's
   *renderer rewrite*, and it's genuinely useful today (Sarah, generative
   UI, agent-authored surfaces).
2. **Prototype server-driven screens for one throwaway mobile surface**
   (Axis B) behind a flag — a settings pane or a promo screen — to measure
   whether "change a screen by deploying the backend" is worth the
   interpreter cost *for us*, given we already have Khala Sync + OTA. If it
   is, that's the compelling wedge and the real "Effect Native" case; if
   not, we learned it cheaply.
3. **Keep `three-effect` and any terminal surface as the proof that "Effect
   + pluggable renderer" already works** — they're the existing evidence
   that the substrate idea isn't hypothetical, and the honest scope of how
   far it's been taken.

## 7. Recommendation

Treat **Effect Native as a research lane, not a pivot.** The thesis is
sound and genuinely ours-shaped — a typed, serializable, Effect-driven view
description with pluggable renderers and an optional server-driven mode is
the natural generalization of what we already ship on three surfaces. But
the maximal version (a homegrown multi-renderer UI framework) conflicts
with the deliberate ONE-UI velocity decision and competes with React
Native from behind, and the moment calls for shipping the MVP, Sarah, the
Codex cutover, and sales.

So: adopt the **narrow slice** (§6.1 — one shared typed-UI-description
contract, renderers unchanged), run the **cheap server-driven-native
experiment** (§6.2) when a low-stakes surface is handy, and revisit the
full "Effect Native" framework only if (a) server-driven native measurably
beats OTA for our iteration pace, or (b) renderer-fork cost across web /
native / canvas / tty becomes a real tax rather than a theoretical one.
Name it now, build it small, let the product decide whether it earns the
capital.

## 8. Open questions

1. Does server-driven native (Axis B) beat our OTA + Khala Sync stack for
   real iteration speed, or is it redundant given we already ship JS OTA
   and sync data live?
2. Could a single Effect-Schema "UI description" contract unify the SSE
   component channel, Sarah's components, Arbiter, and a future native
   catalog *without* forcing a renderer rewrite — i.e., is Axis A's
   contract adoptable independent of Axis A's renderers?
3. If we ever wanted server-driven native, do we fork/reskin Hyperview
   (proven native interpreter, XML→Effect-Schema retag) or grow it from
   `three-effect`'s renderer-adapter pattern? The former is faster; the
   latter is more ours.
4. Terminal as a first-class target: does the Toad/Textual/`wterm`
   direction justify a `tty` renderer in the same substrate, or stay a
   separate lane?
5. How does this reconcile, on paper, with the ONE-UI decision — is Effect
   Native a *layer below* React (the description/behavior substrate, React
   as one renderer) rather than a competitor to it? That framing is the
   only one that doesn't reopen a settled decision.
