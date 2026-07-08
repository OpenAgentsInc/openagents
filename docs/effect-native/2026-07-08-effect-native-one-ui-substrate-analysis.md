# Effect Native — One Typed Component Set Across Web, Mobile, and Native Renderers

Date: 2026-07-08
Status: **OWNER DECISION (2026-07-08) — pivot now.** This supersedes the
2026-07-04 "one UI ecosystem = React + Tailwind everywhere" clause in
`CLAUDE.md` for **new** UI surfaces. The owner's call, recorded verbatim
in essence:

> Multiple stacks + React-slop-on-React-slop produces brittle software and
> hits a brick wall fast. We need to make **thousands of changes per day**;
> that demands a resilient, typed system designed up front, not short-term
> React tooling that pulls us out of the typesafe world. Do this in an
> **Effect/Blueprint-driven** way immediately. Design **ONE set of
> components** that works on web and mobile — **even if the underlying
> render is not React Native** (it may be Swift or Android native, with
> React Native code ported in as needed). Start **SIMPLE and
> data-driven.**

This doc records the decision, the design (**Effect Native**), and the
honest, staged path that captures the resilience without stalling the
sales push. The Hyperview analysis (§1) and the substrate rationale (§2–§4)
are retained from the prior revision because they are the evidence base;
the recommendation (§5–§8) is rewritten to the decision.

## 1. What Hyperview is (the reference for the native half)

Hyperview (Instawork, MIT; `projects/repos/hyperview`) is **HTMX's idea
for native mobile**: a hypermedia format (HXML, an XML dialect) plus a
generic React Native client that fetches screens from any HTTP server and
renders them as *real native* components. The parts worth stealing the
*shape* of:

- **A small, closed element vocabulary** (~18 `hv-*` elements in
  `src/elements/`: view, text, image, list, section-list, text-field,
  select, switch, date-field, picker, spinner, web-view, plus
  navigator/route/screen). Each tag maps to a native component. A *closed
  catalog* — the same discipline as our SSE `oa.component` channel.
- **A behavior algebra** (`src/services/behaviors/`): declarative
  attributes — `trigger` (press/refresh/load/visible) → `action`
  (replace/append/prepend/reload/navigate) → `target` (subtree by id).
  Interactions are **data, not code**; a press swaps a subtree and
  `shallowCloneToRoot` produces a new immutable tree.
- **A tiny generic core** (xmldom, lodash, tiny-emitter, url-parse) meant
  to embed in an RN app. All app-specificity lives in the server-emitted
  tree; the client is a pure interpreter.

The essence, stripped of XML: **the UI is a serializable, typed tree +
a fixed behavior algebra, and the renderer is a generic interpreter.**
That is the idea we adopt. The XML/XSD encoding is *not* — we encode in
Effect Schema.

## 2. Why this is ours, not a foreign import

We already believe and partly ship the load-bearing ideas, in Effect form:

- **UI as a typed, serializable description.** `arbiter-effect` is a typed
  `GraphSpec` (nodes/pins/links) rendered by a *pluggable* renderer today;
  `three-effect` renders a typed scene graph; the SSE `oa.component`
  channel validates a closed component catalog at the boundary. "One typed
  spec, swappable renderer" is already in the codebase.
- **Interaction as typed effect.** Behavior contracts already make UX
  expectations typed, testable oracles. A behavior algebra is the runtime
  twin: interactions as typed intents an interpreter runs — Effect
  programs, naturally.
- **Server-driven, typed, offline-first data.** Khala Sync already makes
  the *data* plane data-driven and durable. Effect Native makes the *view*
  plane data-driven the same way. We have the harder half already.

This is also the **Blueprint** move applied to UI: a component is a typed
object; an interaction is a typed action submission; the view is governed,
validated, and receipt-friendly. It keeps UI inside the typesafe substrate
instead of exiling it to React land.

## 3. Effect Native — the architecture

**One component set, defined once, data-driven; renderers are thin and
swappable.** Three layers:

```
┌────────────────────────────────────────────────────────────────┐
│  1. COMPONENT SET (defined once, in Effect Schema)               │
│     A closed, versioned catalog of typed components + props +    │
│     a typed behavior/intent algebra. This is "the ONE set."      │
│     A screen/view is DATA: a serializable typed tree of these.   │
│     Blueprint-shaped: components are typed objects, interactions  │
│     are typed intents, everything validates at the boundary.     │
├────────────────────────────────────────────────────────────────┤
│  2. RUNTIME (Effect)                                             │
│     Interprets the view tree; interactions run as Effect         │
│     programs; data binds from Khala Sync; state transitions are  │
│     typed; validation + receipts as everywhere else.             │
├────────────────────────────────────────────────────────────────┤
│  3. RENDERERS (thin, per platform — the ONLY platform-specific   │
│     code; the contract above them is renderer-agnostic)          │
│       • web      → HTML/DOM (no React required; React optional    │
│                    as an interim host, e.g. inside Sarah/Next)    │
│       • mobile   → React Native FIRST (reuse the 94 shipping RN   │
│                    primitives as adapter #1), with a path to a    │
│                    native Swift / Jetpack Compose renderer later, │
│                    per component, as fidelity demands             │
│       • canvas   → three-effect (already exists)                  │
│       • (later)  → terminal, if the Toad/wterm lane justifies it  │
└────────────────────────────────────────────────────────────────┘
```

The decisive design commitment: **the component set and behavior algebra
are the stable, typed contract; a renderer is an implementation detail you
can have several of, and replace one at a time.** React Native is adopted
as *the first mobile renderer* — not as the architecture — so nothing
shipping is thrown away, and a native Swift/Compose renderer can be swapped
in per component without touching the component definitions or the app
code that uses them. "React Native code ported in as needed" is exactly
this: RN is the fast first adapter; native is the resilience upgrade.

## 4. Why this is the resilient choice for a thousand-changes-a-day codebase

- **One definition, not N.** A component (Button, Text, Stack, Field, List,
  Card) is defined once as typed data. Web and mobile consume the *same*
  definition; only the thin renderer differs. Today "React everywhere"
  shares component *concepts* but forks render code per platform and per
  vendor library — three dialects of slop that drift.
- **The type system is the guardrail at scale.** When agents make thousands
  of edits/day, the failure mode is silent drift and brittle glue. A
  closed Effect-Schema catalog + typed intents means a bad change fails at
  the boundary, is snapshot-testable, and binds to behavior contracts and
  visual baselines by construction. This is the QAM discipline made native
  to the UI layer.
- **Agent-authored UI is safe by construction.** An agent emitting a typed
  view tree (validated against the catalog) cannot emit arbitrary JSX. This
  is the SSE component-channel safety, generalized — and it is the right
  substrate for Sarah's streamed components and generative UI.
- **Renderer independence is strategic insurance.** If React Native, a
  vendor UI kit, or a framework churns (they do), we swap a renderer, not
  the product. It also makes the eventual "native Swift/Compose for
  fidelity/perf" move a migration, not a rewrite.

## 5. The decision, and the line that keeps it safe

**Adopt Effect Native as the UI substrate now.** But the owner's "20-minute
sprint to rip out the React shit" needs an honest split, because the word
"rip out" applied to *shipping* code mid-sales-push is where this turns
from resilient into reckless:

- **Greenfield → pivot immediately (this is the real win, and it is
  cheap).** The new openagents.com app (`apps/start`) is **empty** — WEB-1
  has not written a line yet. Sarah's branded UI (S-10) is **not built**.
  These are exactly the surfaces about to accrete React slop. Pivot them
  to Effect Native *before* they exist in React. This is the "don't
  sacrifice the last few weeks of Effect work" move — you stop the
  bleeding at the greenfield boundary.
- **Shipping React → migrate onto the substrate, do NOT rip out.** The
  mobile MVP is **94 `.tsx` files**, live, on TestFlight, mid-P0; Sarah's
  realtime app is live and mid-build. You cannot rewrite those in 20
  minutes or 20 days without torching the MVP and the sales push. Instead:
  the mobile app's existing RN primitives **become renderer adapter #1**
  (they are already the "mobile renderer" — we're just putting a typed
  contract above them), and its screens migrate to consume the component
  set incrementally, screen by screen, under the QAM gate.

The honest framing: **Effect Native is a greenfield-first pivot plus a
staged migration — not a big-bang rewrite.** That is the only version that
is both resilient *and* compatible with shipping the MVP, Sarah, and the
outbound sales engine on schedule. A big-bang rewrite would sacrifice the
one thing that matters more than architecture right now: revenue.

## 6. Effect Native v0 — start simple and data-driven (the first sprint)

Deliberately tiny. The Hyperview lesson is that ~18 elements cover most
screens; we start with **~8** and grow only on demand.

- **`packages/effect-native` (new): the component set.** Effect-Schema
  definitions for a v0 catalog — `Stack` (row/column, the layout
  primitive), `Text`, `Button`, `Image`, `TextField`, `List`, `Card`,
  `Spacer` — each with typed props (bounded: spacing/color/size tokens
  from the Protoss-blue token set, not arbitrary strings). A `View` is a
  typed tree of these. A typed **intent** algebra: `onPress`,
  `onChange`, `onSubmit` → named intents resolved by the runtime, never
  inline closures in the data.
- **The runtime**: a small Effect interpreter that walks a `View`, binds
  data (from props or a Khala Sync scope), and dispatches intents as
  Effect programs. Pure, snapshot-testable.
- **Renderer adapter #1 — React Native** (`@effect-native/render-rn`):
  maps each catalog component to the existing khala-mobile RN primitive
  (reuse `khala-button`, `khala-text`, etc. — they become the adapter's
  targets). This proves "the mobile app renders the component set" with
  zero new native work.
- **Renderer adapter #2 — DOM/web** (`@effect-native/render-dom`): maps
  each component to a plain typed DOM/HTML output (Tailwind classes from
  the same tokens). **No React required** — this is the point; the web
  renderer can be a direct DOM renderer, and where React is convenient
  (Sarah/Next interim) a `render-react` shim hosts the same output.
- **Proof of the thesis:** one real screen — e.g. the mobile Settings
  pane or a Sarah card — defined once as a `View` and rendered
  identically by both adapters, snapshot-tested, contract-bound. That
  single "same definition, two renderers" receipt is v0's definition of
  done.
- **Data-driven from the start:** the view tree is data; where it's
  static it's a literal, where it's dynamic it binds a Khala Sync scope —
  and (the Hyperview/Axis-B option, deferred but designed-for) it *could*
  be served from the backend to change a screen without a release. v0
  does not require server-driven; the contract just doesn't preclude it.

Explicitly **not** in v0: a native Swift/Compose renderer (that's the
later fidelity upgrade the contract enables, not the starting point);
navigation/routing abstraction; animation system; the full component
surface. Grow the catalog only when a screen needs an element — never
speculatively.

## 7. Reconciling the in-flight work (immediate follow-ups)

This decision touches several standing artifacts. Flagged here as the
next actions (not silently rewritten in this doc):

- **`CLAUDE.md` ONE-UI clause (2026-07-04):** amend from "React + Tailwind
  everywhere" to "**Effect Native** typed component set as the UI
  substrate; React/React Native are renderer adapters, not the
  architecture; Effect remains the logic substrate." New surfaces start on
  Effect Native.
- **`MASTER_ROADMAP.md`:** P1 Track A (WEB-1 #8565) and Sarah's S-10/S-15
  retarget from "TanStack + React landing" to "Effect Native web renderer"
  — **launch-ui becomes visual/design reference, its shadcn/Tailwind
  ported into the DOM renderer's token set, not adopted as React
  components.** Add an **EN-1..N** lane family for the substrate.
- **WEB-1 (#8565):** rescope — the openagents.com landing is the **first
  Effect Native web surface** (greenfield; ideal pilot), with launch-ui as
  the look, not the framework. Update the delegation accordingly.
- **Sarah repo:** S-10 (branded UI) and S-15 (TanStack port) fold into
  "render Sarah's UI via the Effect Native web renderer"; the realtime
  voice loop and eve are untouched (they're not the UI layer).
- **Mobile (P0):** no change to shipping code; open an EN migration lane to
  wrap the 94 RN primitives as adapter #1 and migrate screens
  incrementally under the QAM gate — after the MVP ships, not during.

I have **not** made these edits in this commit — they are cross-cutting
owner-decision changes; say the word and I'll cascade them (CLAUDE.md,
MASTER_ROADMAP, WEB-1 rescope, EN lane issues) in one pass.

## 8. Honest risks (so the decision is made with eyes open)

- **This is building framework, not using one.** The velocity argument cuts
  both ways: a typed substrate is more resilient at 1000 edits/day, but the
  substrate itself must be *built and maintained* by us while the ecosystem
  gives React/RN away. v0 must stay ruthlessly small or it becomes the
  brick wall it's meant to avoid.
- **Native renderers (Swift/Compose) are a large surface** if pursued.
  The contract makes them optional and incremental — but "one component
  set renders truly native on iOS and Android" is, taken to completion, a
  multi-quarter effort. Keep it as the *insurance the contract enables*,
  not a v0 commitment. RN stays the mobile renderer until a specific screen
  proves it needs native.
- **Timing risk.** The current priority is MVP → Sarah → Codex cutover →
  sales. Effect Native must ride *inside* the greenfield web work (WEB-1,
  Sarah UI) that was going to happen anyway, not open a separate rewrite
  front. If it starts stealing hours from the sales push, it has failed its
  own justification.
- **Two-renderers fidelity tax** is real (every gesture/a11y/keyboard
  behavior re-expressed per adapter). v0 mitigates by starting with a
  handful of components and reusing RN as adapter #1 rather than
  reimplementing native.

## 9. Recommendation (to the decision already made)

Proceed — with the greenfield-first, start-simple discipline of §5–§6 as
the guardrail:

1. Build **`packages/effect-native` v0** (§6): ~8 typed components, the
   Effect runtime, RN adapter (reusing khala-mobile primitives) + DOM
   adapter, one screen rendered by both, contract- and snapshot-tested.
2. Make **WEB-1 the first Effect Native web surface** — launch-ui as the
   look, the DOM renderer as the framework. This captures the greenfield
   before it becomes React slop, exactly as the owner wants.
3. **Do not touch shipping React** (mobile MVP, Sarah voice loop) beyond
   wrapping RN as adapter #1; migrate screens incrementally, post-MVP,
   under the QAM gate.
4. Keep the **native Swift/Compose renderer as the designed-for upgrade**,
   pursued per-component only when fidelity/perf demands it — the contract
   guarantees it's a migration, never a rewrite.

Cascade the doc reconciliation (§7) on the owner's go.

## 10. Open questions

1. Encoding of the component set: pure Effect Schema types (compile-time,
   our world) with an optional serialized form for the server-driven/agent
   cases — confirm the serialized shape reuses the SSE `oa.component`
   catalog rather than inventing a parallel one.
2. Tokens: the Protoss-blue design tokens become the single source both
   renderers read — where do they live (a `@effect-native/tokens` package)
   and how do they relate to the existing shadcn/Tailwind tokens WEB-1's
   launch-ui reference uses?
3. Navigation: deferred in v0, but web routing (TanStack Router) and RN
   navigation are real — does Effect Native own a typed navigation intent,
   or delegate to per-platform routers below the adapter line?
4. The Axis-B (server-driven) option: worth wiring for mobile to dodge
   store review, or does OTA + Khala Sync already cover it? (Design-for,
   decide-later — v0 doesn't need it.)
5. How aggressively to migrate the 94 mobile `.tsx` files — full migration,
   or only new/changed screens on Effect Native while legacy stays wrapped?
   (Recommend the latter: never rewrite a working screen just to move it.)
