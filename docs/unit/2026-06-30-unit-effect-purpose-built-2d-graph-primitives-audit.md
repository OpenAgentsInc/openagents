# Purpose-Built 2D Dataflow-Graph Primitives (a `unit-effect`)

Date: 2026-06-30
Status: Audit / proposal (no code changes)
Author: Raynor (agent)

## Scope correction

We are **not** adopting [Unit](https://github.com/samuelmtimbo/unit)
(`@_unit/unit`) as a dependency. Unit is a single-author, beta, multi-MB
browser-OS with its own homegrown component framework, an uncommitted codegen
step, and ~850 standard-library units — embedding it whole would drag all of
that into our stack and fight Foldkit/Effect on every seam. It stays a
**read-only reference repo** under `projects/repos/unit/`, exactly like our other
reference lanes.

What we want is our **own streamlined, purpose-built version of Unit's
primitives** — the small set of ideas that make a live 2D dataflow graph feel
good — implemented natively on our stack (Effect + Foldkit), the same way
`three-effect` is "React-Three-Fiber rewritten in Effect" rather than a fork of
R3F.

This audit (a) distills which Unit primitives are actually worth rebuilding,
then (b) fleshes out the three ways to ship them — a new **`unit-effect`**
sibling package, **folding 2D graph primitives into `three-effect`**, or
**porting SVG node/pin/link rendering straight into Foldkit / `packages/ui`** —
with a recommendation.

## Primitives worth stealing from Unit (and nothing else)

Unit's value isn't its runtime; it's a handful of well-shaped abstractions. We
rebuild these, minimally:

1. **The unit/node model — MIMO.** A node is a Multi-Input Multi-Output box with
   named, **typed pins**. (Unit calls them units; formally MIMO finite state
   machines. We don't need the FSM runtime — we need the shape: a box with ports
   that carry typed values.) Unit ref: `src/Class/Unit/index.ts`, `src/Pin.ts`.
2. **Merges = links.** A link connects an output pin to an input pin. Edges are
   first-class, addressable, and can be lit/animated when data flows. Unit ref:
   `src/Class/Merge.ts`, `src/Class/Graph/index.ts`.
3. **The spec/serialization model.** A graph is plain JSON: `{ units, merges,
   inputs, outputs, metadata }`, where `metadata` carries **node positions** —
   logic and layout serialized together. This is the format we generate from our
   own domain data and persist/share. Unit ref: `src/types/GraphSpec.ts`,
   `src/bundle.ts`.
4. **Force/auto-layout.** Unit's "live objects" feel comes from a hand-rolled
   D3-force simulation (`src/client/simulation.ts`) — nodes settle physically,
   no manual placement required. We want a small force/spring layout, not a
   dependency on `d3`.
5. **Direct manipulation.** Pins are *writable*. Dragging a link, pausing a
   node, retuning a value — the diagram is a control surface, not a chart. This
   is the differentiator over every read-only flow widget. Unit ref: the
   `move*` family + `buildMoveMap` in `src/Class/Graph/`.
6. **Live datum inspection.** Nodes/pins surface their current value inline (the
   Datum / DataTree components). For us: the live token count, approval state,
   current tool call shown right on the node.
7. **Evidence binding (our addition).** Same discipline as the Verse's "no
   animation without a receipt": a link only lights when a real Khala receipt /
   event flows. The graph is a *projection* of real state, never state-of-record.

Everything else in Unit — the language, the 850-unit stdlib, the web-OS, the
shadow-DOM app, voice/gesture editing — we deliberately drop.

## Where this sits relative to our existing stack

- **`@openagentsinc/three-effect`** — Effect-native **3D** (Three.js/WebGL).
  Owns the Verse: Pylons, avatars, crackling Khala arcs, settlement bursts.
  "Where you watch it work." Git-pinned per-consumer
  (`github:OpenAgentsInc/three-effect#<sha>`), dual-entry (`./core` pure
  Three+Effect primitives returning `Effect<Handle>`; `./foldkit` wraps them as
  custom elements with `*View` helpers).
- **Foldkit** — our Elm-architecture (model/update/view) virtual-DOM runtime.
  Critically, its `html<Message>()` factory already exposes the **entire SVG tag
  surface** (`svg`, `g`, `path`, `polyline`, `line`, `rect`, `circle`, `marker`,
  `defs`, `linearGradient`, `clipPath`, `foreignObject`, `Xmlns`, …) **and** a
  retained 2D canvas DSL `foldkit/canvas` (`Path`/`MoveTo`/`LineTo`/`BezierTo`/
  `QuadTo`/`Circle`/`Rect`/`Text`/`Group`). So 2D graph rendering needs **no
  escape hatch** — it's native Foldkit.
- **`packages/ui` (`@openagentsinc/ui`)** — Foldkit component library; the
  authoring pattern is plain functions returning `Html` built with the html
  factory (`src/primitives.ts`, `src/class-foldkit.ts`). 24 packages in the Bun
  workspace, named `@openagentsinc/*`, referenced `workspace:*`, `effect` pinned
  via the root `catalog`.

A 2D dataflow graph is **DOM/SVG-native and interactive**, which is squarely
Foldkit territory, not Three.js territory. That single fact drives the
recommendation below.

## What we already have to build on (and what's missing)

Existing, reusable:
- **`apps/openagents.com/apps/web/src/scene/pylonBezierNetworkElement.ts`** — a
  hand-rolled **2D SVG** node/edge/bezier graph as a Foldkit custom element
  (`oa-pylon-bezier-network`): golden-angle ring layout, quadratic-bezier edge
  paths (`M x y Q cx cy CX CY`), `.edge.lit` dash-flow animation. This is the
  closest existing renderer and its path math is directly portable.
- **`three-effect/packages/core/src/bezierNodes.ts`** — a node-graph *model* in
  3D: `BezierNodeDefinition { id, label, color, position, connectedTo[] }`,
  `BezierNodeConnection`, dash-animated bezier links. Conceptual ancestor;
  WebGL, not 2D.
- **`apps/autopilot-desktop/src/ui/pylon-network-visualization.ts`** (+
  `src/shared/pylon-network-scene.ts`) — the domain→visual mapping pattern
  (`PylonNetworkNode`, working/online/offline tones) we'd reuse for node status.

Net-new (does not exist anywhere today):
- **No interactive graph editor.** No drag, no pin hit-testing, no link-creation
  gesture, no typed-port model, no selection. Every current asset is a
  *read-only* visualization. The interactive primitives are the real work.

## Option A — new `unit-effect` sibling package (recommended)

Build `@openagentsinc/unit-effect`, mirroring `three-effect`'s proven dual-entry
shape but DOM/SVG-first instead of WebGL.

**Package layout** (`packages/unit-effect/`, in the Bun workspace):
- `./core` — framework-agnostic, Effect-based. The model + algorithms, zero
  Foldkit:
  - **Model:** `UnitNode { id, label, inputs: Pin[], outputs: Pin[], status,
    datum }`, `Pin { id, name, type, value }`, `Link { from: PinRef, to: PinRef,
    lit }`, `GraphSpec { nodes, links, metadata: { positions } }` — our own,
    trimmed version of Unit's `GraphSpec`, defined with Effect Schema so it
    serializes/validates like the rest of our contracts.
  - **Layout:** a small force/spring simulation (port the *idea* of
    `src/client/simulation.ts`, ~a few hundred lines, no `d3`), exposed as an
    `Effect` that produces settled positions / a tickable handle.
  - **Geometry/hit-testing:** bezier edge path generation, node/pin bounding,
    pointer hit-testing for nodes/pins/links (the net-new interactive core).
  - **Mount handle:** `mountUnitGraph(el, spec, opts): Effect<UnitGraphHandle>`
    where the handle exposes `{ element, dispose, update(spec), setPinValue,
    onNodeSelected, onLinkCreated, … }` — same `Effect<Handle>` ergonomics as
    three-effect's core.
- `./foldkit` — thin Foldkit adapter:
  - `unitGraphView<Message>(attributes, spec, { onNodeSelected, onPinPush, … }):
    Html` rendered with the html factory's SVG tags, **or** wrapped as a custom
    element via `foldkit/customElement` (the same bridge three-effect uses) if we
    want the imperative layout loop to own its own RAF.
  - Event handlers are ordinary `Attribute<Message>` values, so drag/select/
    link-create dispatch into the host app's Elm `update` — no parallel runtime.

**Rendering target:** **SVG via Foldkit** as the default (crisp, accessible,
hit-testable per element, trivial theming via CSS / `design-tokens`), with
`foldkit/canvas` as the fallback for very large graphs where per-element SVG gets
heavy. Reuse `pylonBezierNetworkElement.ts`'s path math.

**Why this is the right call:**
- Clean separation of concerns: 2D control surface ≠ 3D spectacle. Doesn't
  pollute three-effect's WebGL boundary (which `INVARIANTS.md`/`AGENTS.md` keep
  as the sole Three.js home).
- Workspace package (`workspace:*`) instead of three-effect's per-consumer git
  SHA pins → no SHA-drift friction across desktop/web/khala-code.
- Reusable across **all** surfaces (Khala Code, autopilot-desktop, web) from day
  one.

**Costs:** new package wiring — the root `package.json` hand-wires every package
into `test:*`/`typecheck:*` chains, so add `test:unit-effect` +
`typecheck:unit-effect` entries (workspace glob `packages/*` already matches).
The interactive core (hit-testing, drag, link-create, force layout) is genuine
net-new engineering, ~the bulk of the effort.

## Option B — fold 2D graph primitives into `three-effect`

Extend `three-effect` with 2D dataflow nodes, seeded by its existing
`bezierNodes.ts`.

**Viable only if** we want the graph to live *inside* the 3D scene (e.g. a
schematic floating in the Verse, sharing camera/lighting). For that case it's
natural — `bezierNodes` already models nodes+bezier links and the curve/edge
primitives (`curvePrimitives.ts`, `conditionalLinePrimitives.ts`) are reusable.

**Against it for a real editor:**
- three-effect is **WebGL-first**. Crisp 2D text, DOM accessibility, precise
  per-element pointer hit-testing, CSS theming, and form-like inline datum
  editing are all things SVG/DOM gives for free and WebGL makes hard.
- Consumed via **fragile per-app git SHA pins**; every primitive change means
  landing in the sibling repo and re-pinning three places. Bad fit for a
  fast-iterating interactive surface.
- Mixes "2D control" concerns into the "3D spectacle" package, eroding the clean
  boundary.

**Verdict:** do this **only** for an in-3D-scene schematic node, *in addition to*
Option A — not as the home for the interactive editor.

## Option C — port SVG node/pin/link rendering straight into Foldkit / `packages/ui`

Skip a dedicated package; build the graph as components in `@openagentsinc/ui`
(or inline per app), using Foldkit's SVG factory directly.

**Fully supported by the stack:** Foldkit has the whole SVG tag/attribute surface
+ `Xmlns` + `foreignObject` (for inline HTML datum editors inside nodes), and the
`pylonBezierNetworkElement.ts` SVG/bezier logic is the working template.
Interactivity rides the normal Foldkit `Attribute<Message>` event handlers in the
Elm update loop. `class-foldkit.ts`/`primitives.ts` show the authoring pattern.

**Trade-offs:**
- Fastest to a first pixel; lowest ceremony.
- But the **model + layout + hit-testing core is framework-agnostic logic** that
  doesn't belong in a Foldkit view module. If we build it inline, we'll want to
  extract it into a package anyway the moment a second surface needs it — which
  is Option A.
- `packages/ui` is a general component lib; a stateful force-simulated graph
  editor is a big, specialized citizen there.

**Verdict:** Option C is really "Option A without the package boundary." Use its
SVG approach *as the rendering layer of* Option A: author the *renderer* exactly
this way, but put the model/layout/hit-testing in `unit-effect/core` so it's
reusable and testable headless.

## Recommendation

**Option A, with Option C's SVG-in-Foldkit as its rendering layer, and Option B
reserved for a later in-Verse schematic node.**

Concretely: ship `@openagentsinc/unit-effect` with a pure Effect `./core`
(model, force layout, geometry, hit-testing) and a Foldkit `./foldkit` view that
renders SVG the way `pylonBezierNetworkElement.ts` already does. This gives us a
streamlined, owned, purpose-built take on Unit's good primitives — no fork, no
840 units we don't want, native to Effect+Foldkit, reusable everywhere, and free
of three-effect's git-pin friction.

## Phased plan

**Phase 0 — model + headless core (≈2–3 days).** `unit-effect/core`: the
`GraphSpec`/`UnitNode`/`Pin`/`Link` Effect-Schema types, the force-layout
`Effect`, bezier geometry, and hit-testing — all unit-tested headless (no DOM).
Add the root `test:`/`typecheck:` wiring.

**Phase 1 — read-only Foldkit renderer.** `unit-effect/foldkit`
`unitGraphView<Message>` rendering SVG nodes/pins/lit bezier links, ported from
`pylonBezierNetworkElement.ts`. First consumer: Khala Code's fleet board
(`clients/khala-code-desktop`) — Khala → Pylons → Codex/Claude workers, links lit
by real receipts/events from the Bun host. Flag-gated, fixture-backed smoke test
(à la `proof:verse-arc`).

**Phase 2 — interactivity / direct manipulation.** Node drag, selection,
pin-level pointer events, link-create gesture, inline datum editing
(`foreignObject`). Writable pins → operator actions (pause worker, approve,
reroute) dispatched through the Elm `update` loop back into the host.

**Phase 3 — fan-out.** Reuse the same package on `apps/openagents.com/apps/web`
(replacing the bespoke `pylonBezierNetworkElement.ts`) and, if desired, add the
Option-B in-Verse 3D schematic node in `three-effect` for click-through from the
3D world.

## Reference paths

Our stack:
- Workspace/catalog/test wiring: `package.json` (root)
- UI lib + authoring pattern: `packages/ui/package.json`, `packages/ui/src/{index,primitives,class-foldkit}.ts`
- Existing 2D SVG graph (port from): `apps/openagents.com/apps/web/src/scene/pylonBezierNetworkElement.ts`
- Domain→visual mapping (reuse): `apps/autopilot-desktop/src/ui/pylon-network-visualization.ts`, `src/shared/pylon-network-scene.ts`
- three-effect shape/seed: its `packages/{core,foldkit}/src/index.ts`, `packages/core/src/bezierNodes.ts`; consumer types `apps/autopilot-desktop/src/types/three-effect-{core,foldkit}.d.ts`; mount usage `apps/autopilot-desktop/src/ui/view.ts`
- Foldkit SVG/canvas/customElement surface: `foldkit/{html,canvas,customElement}` (typed via the installed `foldkit@0.102.1` dist)

Unit (reference only, `projects/repos/unit/`):
- Node/pin model: `src/Class/Unit/index.ts`, `src/Pin.ts`
- Graph/merges + move ops: `src/Class/Graph/index.ts`, `src/Class/Merge.ts`
- Spec/serialization: `src/types/GraphSpec.ts`, `src/bundle.ts`
- Force layout (port the idea): `src/client/simulation.ts`
- Editor canvas (study, don't copy): `src/system/platform/component/app/Editor/Component.ts`

## Bottom line

Build our own. A small `@openagentsinc/unit-effect` — Unit's good primitives
(typed-pin MIMO nodes, first-class links, JSON spec with embedded layout, force
auto-layout, direct manipulation, live datum, evidence-bound lighting), rebuilt
on Effect + Foldkit and rendered as SVG — gives us a 2D dataflow **control**
surface that complements the 3D `three-effect` Verse, ships first into the Khala
Code fleet board, and stays a clean workspace package rather than a fork or a
WebGL afterthought.
