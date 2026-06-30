# Arbiter — Purpose-Built 2D Dataflow-Graph Primitives (`arbiter-effect`)

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

This audit (a) names the thing, (b) distills which Unit primitives are worth
rebuilding, (c) shows how it syncs with **Blueprint**, then (d) fleshes out the
three ways to ship it — with a recommendation.

## Name: `arbiter-effect`

Not `unit-effect`. "Unit" collides with the reference repo we are *not*
adopting, with the literal word "unit" (test units, our own domain "units"), and
carries no brand. Our package convention is `X-effect` (rhyming with
`three-effect`, read as "X + Effect"), and our canon is Protoss: Khala, Pylon,
Tassadar, Artanis, Raynor.

**Arbiter.** In StarCraft the Arbiter is the Protoss capital ship that
*manipulates the battlefield* — Recall, Stasis Field, cloak. That is a **control
plane**, which is exactly what this library is: not a read-only chart but a
surface where you reach in and move things (writable pins → pause a worker,
approve, reroute). It slots into the existing Protoss naming, has no collisions
(unlike `sentry-effect` = the observability SaaS we avoid, or `nexus-effect` =
deprecated `nexus.openagents.com`), and `arbiter-effect` reads cleanly.

Package: `@openagentsinc/arbiter-effect`.

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
7. **Evidence binding (our addition, and our tie to Blueprint).** Same discipline
   as the Verse's "no animation without a receipt": a link only lights when a
   real Khala receipt / Blueprint Trust Receipt dereferences. The graph is a
   *projection* of real state, never state-of-record.

Everything else in Unit — the language, the 850-unit stdlib, the web-OS, the
shadow-DOM app, voice/gesture editing — we deliberately drop.

## How Arbiter syncs with Blueprint

This is the important alignment, and it is strong. **Blueprint already defines a
typed, governed dataflow graph but deliberately deferred its visual editor.
Arbiter is that deferred surface.**

Blueprint (the typed business operating model — Language / Engine / Toolchain) is
deprecated as a standalone repo/service (archived 2026-05-24, specs absorbed into
`autopilot4-deprecated/blueprint/docs/`) but **alive as the shared governance
vocabulary** across Khala (typed Program signatures + GEPA), Probe (Action
Submission proposals), Artanis (release gates), and `docs/promises/` (receipts).
Its canonical specs live at `autopilot4-deprecated/blueprint/docs/`
(`master-spec.md`, `language-engine-toolchain.md`,
`programs-optimization-and-rlm.md`, `receipt-and-evidence-contract.md`).

Blueprint carries two graph-shaped structures that an Arbiter canvas renders
directly:

- **The data graph** — Business Objects (typed nodes) joined by typed
  Relationships (typed edges), queried by Object Set Expressions and walked by a
  Graph Service (`language-engine-toolchain.md` traverse/search_around). Plus a
  Lineage graph from any fact back to its source.
- **The control graph** — Program Types with a **Program Signature** = "the
  DSPy-style contract: typed inputs, typed outputs, instructions, allowed
  evidence, and success criteria" (`programs-optimization-and-rlm.md`). Programs
  **compose** into plan → write → verify pipelines; a Khala request is "a typed
  Blueprint program call." That is precisely our typed-pin / node / link model.

And the visual layer was named and reserved, not built: **Blueprint Map** —
"graph view filtered by domain, object type, source, mission, loop, worker,
evidence, action, and policy" (`master-spec.md`) — with "visual graph editing"
explicitly listed as a **P2 non-goal / do-not-build-first**. So Arbiter is the
missing **Blueprint Toolchain** surface, not a new ontology.

The mapping is one-to-one:

| Arbiter concept | Blueprint artifact |
| --- | --- |
| Node | Business Object (typed instance) / Program Type |
| Typed pin (in/out) | Program Signature `input_fields` / `output_fields` |
| Link (edge) | Relationship (typed edge) / Program composition (plan→write→verify) |
| JSON graph spec | Object Set Expression + Relationships; Program Type / Module Version records |
| Swappable node impl | Module Version (`deterministic` / `model_prompt` / `tool_plan` / `rlm` / `external_worker` / `human_review`) |
| Run record per node fire | Program Run (decision evidence, **no write authority**) |
| Approval gate on an edge | Action Submission → approval → source write lifecycle |
| **Edge "lights" on execution** | **Trust/Failure Receipt** (`evidence_ids`, `executed_changes_json`); promise green-flip |
| The whole canvas | **Blueprint Map** (named, P2-deferred) |

Crucially, our **"a link only lights on a real receipt"** rule is a faithful UI
rendering of Blueprint's hardest invariants — "Program Runs are decision
evidence, they do not authorize writes," "Evidence by default," and (live in this
repo) promises flip to green **only on a dereferenceable receipt with real
movement** (`docs/promises/`), echoed by the Khala→Pylon→Codex runbook's "counter
movement alone is never completion evidence." An Arbiter edge stays inert until a
receipt dereferences; an approval gate on an edge is the Action Submission
lifecycle made visible. So Arbiter does not just *coexist* with Blueprint — it is
the direct visual/control expression of the Blueprint governance model.

**Practical consequence for the design:** Arbiter's `GraphSpec` should be a
*projection target* for Blueprint artifacts, not a parallel format. The
`./core` model types (below) should map cleanly from Program Signatures
(→ pins), Relationships (→ links), Module Versions (→ swappable node impls), and
Trust Receipts (→ lit edges), so the same library renders a fleet board *and* a
Blueprint Program/Map without a second schema.

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
- **No Blueprint Map / visual program builder** — named in the Blueprint spec,
  never built.

## Option A — new `arbiter-effect` sibling package (recommended)

Build `@openagentsinc/arbiter-effect`, mirroring `three-effect`'s proven
dual-entry shape but DOM/SVG-first instead of WebGL.

**Package layout** (`packages/arbiter-effect/`, in the Bun workspace):
- `./core` — framework-agnostic, Effect-based. The model + algorithms, zero
  Foldkit:
  - **Model:** `UnitNode { id, label, inputs: Pin[], outputs: Pin[], status,
    datum }`, `Pin { id, name, type, value }`, `Link { from: PinRef, to: PinRef,
    lit }`, `GraphSpec { nodes, links, metadata: { positions } }` — defined with
    Effect Schema, designed as a **projection target for Blueprint artifacts**
    (Program Signatures → pins, Relationships → links, Module Versions →
    node impls, Trust Receipts → lit edges) so one library renders both a fleet
    board and a Blueprint Program/Map.
  - **Layout:** a small force/spring simulation (port the *idea* of
    `src/client/simulation.ts`, ~a few hundred lines, no `d3`), exposed as an
    `Effect` that produces settled positions / a tickable handle.
  - **Geometry/hit-testing:** bezier edge path generation, node/pin bounding,
    pointer hit-testing for nodes/pins/links (the net-new interactive core).
  - **Mount handle:** `mountArbiterGraph(el, spec, opts): Effect<ArbiterHandle>`
    where the handle exposes `{ element, dispose, update(spec), setPinValue,
    onNodeSelected, onLinkCreated, … }` — same `Effect<Handle>` ergonomics as
    three-effect's core.
- `./foldkit` — thin Foldkit adapter:
  - `arbiterGraphView<Message>(attributes, spec, { onNodeSelected, onPinPush, …
    }): Html` rendered with the html factory's SVG tags, **or** wrapped as a
    custom element via `foldkit/customElement` (the same bridge three-effect
    uses) if we want the imperative layout loop to own its own RAF.
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
- Reusable across **all** surfaces (Khala Code, autopilot-desktop, web, and a
  future Blueprint Map) from day one.

**Costs:** new package wiring — the root `package.json` hand-wires every package
into `test:*`/`typecheck:*` chains, so add `test:arbiter-effect` +
`typecheck:arbiter-effect` entries (workspace glob `packages/*` already matches).
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
this way, but put the model/layout/hit-testing in `arbiter-effect/core` so it's
reusable and testable headless.

## Recommendation

**Option A (`arbiter-effect`), with Option C's SVG-in-Foldkit as its rendering
layer, and Option B reserved for a later in-Verse schematic node.**

Concretely: ship `@openagentsinc/arbiter-effect` with a pure Effect `./core`
(model, force layout, geometry, hit-testing) and a Foldkit `./foldkit` view that
renders SVG the way `pylonBezierNetworkElement.ts` already does. This gives us a
streamlined, owned, purpose-built take on Unit's good primitives — no fork, no
840 units we don't want, native to Effect+Foldkit, reusable everywhere, free of
three-effect's git-pin friction, and aligned one-to-one with the Blueprint
governance model it visualizes.

## Phased plan

**Phase 0 — model + headless core (≈2–3 days).** `arbiter-effect/core`: the
`GraphSpec`/`UnitNode`/`Pin`/`Link` Effect-Schema types (designed as a Blueprint
projection target), the force-layout `Effect`, bezier geometry, and hit-testing —
all unit-tested headless (no DOM). Add the root `test:`/`typecheck:` wiring.

**Phase 1 — read-only Foldkit renderer.** `arbiter-effect/foldkit`
`arbiterGraphView<Message>` rendering SVG nodes/pins/lit bezier links, ported
from `pylonBezierNetworkElement.ts`. First consumer: Khala Code's fleet board
(`clients/khala-code-desktop`) — Khala → Pylons → Codex/Claude workers, links lit
by real receipts/events from the Bun host. Flag-gated, fixture-backed smoke test
(à la `proof:verse-arc`).

**Phase 2 — interactivity / direct manipulation.** Node drag, selection,
pin-level pointer events, link-create gesture, inline datum editing
(`foreignObject`). Writable pins → operator actions (pause worker, approve,
reroute) dispatched through the Elm `update` loop back into the host. This is
where the "Arbiter" name earns out — the approval gate on an edge is the
Blueprint Action Submission lifecycle made manipulable.

**Phase 3 — fan-out + Blueprint Map.** Reuse the same package on
`apps/openagents.com/apps/web` (replacing the bespoke
`pylonBezierNetworkElement.ts`); render a Blueprint Program (plan→write→verify
composition) and the named-but-unbuilt **Blueprint Map** from the same
`GraphSpec`; and, if desired, add the Option-B in-Verse 3D schematic node in
`three-effect` for click-through from the 3D world.

## Reference paths

Our stack:
- Workspace/catalog/test wiring: `package.json` (root)
- UI lib + authoring pattern: `packages/ui/package.json`, `packages/ui/src/{index,primitives,class-foldkit}.ts`
- Existing 2D SVG graph (port from): `apps/openagents.com/apps/web/src/scene/pylonBezierNetworkElement.ts`
- Domain→visual mapping (reuse): `apps/autopilot-desktop/src/ui/pylon-network-visualization.ts`, `src/shared/pylon-network-scene.ts`
- three-effect shape/seed: its `packages/{core,foldkit}/src/index.ts`, `packages/core/src/bezierNodes.ts`; consumer types `apps/autopilot-desktop/src/types/three-effect-{core,foldkit}.d.ts`; mount usage `apps/autopilot-desktop/src/ui/view.ts`
- Foldkit SVG/canvas/customElement surface: `foldkit/{html,canvas,customElement}` (typed via the installed `foldkit@0.102.1` dist)

Blueprint (governance model Arbiter visualizes; specs absorbed, repo deprecated):
- `autopilot4-deprecated/blueprint/docs/master-spec.md` (Language/Engine/Toolchain; Source Authority; Blueprint Map + P2 deferral)
- `autopilot4-deprecated/blueprint/docs/language-engine-toolchain.md` (Object/Relationship graph; Graph Service traverse/search)
- `autopilot4-deprecated/blueprint/docs/programs-optimization-and-rlm.md` (Program Signatures = typed pins; Module Versions; Program Runs as evidence)
- `autopilot4-deprecated/blueprint/docs/receipt-and-evidence-contract.md` (Trust/Failure Receipt fields a lit edge binds to)
- Live usage: `docs/khala/2026-06-23-khala-blueprint-program-and-plugin-extensibility.md`, `apps/openagents.com/INVARIANTS.md`, `docs/promises/`

Unit (reference only, `projects/repos/unit/`):
- Node/pin model: `src/Class/Unit/index.ts`, `src/Pin.ts`
- Graph/merges + move ops: `src/Class/Graph/index.ts`, `src/Class/Merge.ts`
- Spec/serialization: `src/types/GraphSpec.ts`, `src/bundle.ts`
- Force layout (port the idea): `src/client/simulation.ts`
- Editor canvas (study, don't copy): `src/system/platform/component/app/Editor/Component.ts`

## Bottom line

Build our own, and call it **Arbiter**. A small `@openagentsinc/arbiter-effect` —
Unit's good primitives (typed-pin MIMO nodes, first-class links, JSON spec with
embedded layout, force auto-layout, direct manipulation, live datum,
evidence-bound lighting), rebuilt on Effect + Foldkit and rendered as SVG —
gives us a 2D dataflow **control** surface that complements the 3D `three-effect`
Verse, ships first into the Khala Code fleet board, and lands as the visual
**Blueprint Map / program builder** that Blueprint's own spec named and deferred.
It is a clean workspace package, not a fork or a WebGL afterthought, and its
"a link only lights on a real receipt" rule is the Blueprint governance model
rendered directly in the UI.
