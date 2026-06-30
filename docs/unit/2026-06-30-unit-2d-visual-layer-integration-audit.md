# Unit as a 2D Visualization / Control Layer for Khala Code and OpenAgents Products

Date: 2026-06-30
Status: Audit / proposal (no code changes)
Author: Raynor (agent)

## TL;DR

[Unit](https://github.com/samuelmtimbo/unit) (`@_unit/unit`, MIT, v1.0.124) is a
general-purpose **visual dataflow programming language + live editing
environment** by Samuel Timbó. Formally: *units are Multi-Input Multi-Output
(MIMO) finite state machines, and a program is a graph*. It ships its own
homegrown DOM/SVG component framework (no React), a 61k-line spatial graph
editor, a hand-rolled force-simulation layout engine, and a transport-agnostic
async facade that can drive a graph locally, in a Web Worker, or over a remote
port.

The interesting question for us is **not** "should we adopt Unit as a
language." It is: *can Unit's 2D live-graph canvas become a cool, reusable
visualization and control surface across OpenAgents products — sitting next to
the 3D `three-effect` Verse rather than replacing it?* The answer is **yes, with
bounded scope**, and the highest-leverage first target is **Khala Code's fleet /
agent-orchestration view**, where a dataflow graph is a near-perfect mental
model for "Khala → Pylon → Codex/Claude workers piping work."

This audit covers what Unit is, where it fits, three concrete integration
targets, the real costs, and a recommended phased plan.

## Relationship to our existing visual stack

We already have a sanctioned visual language: `@openagentsinc/three-effect`
(Effect-native Three.js) renders the **3D Verse** — Pylons, avatars, Khala
crackling arcs, settlement bursts. That stack is about *spectacle and
inhabiting* the machine-work economy ("where you watch it work").

Unit is the orthogonal axis: a **2D, precise, editable dataflow diagram**. It is
about *structure and control* — wiring, inspecting live values, and direct
manipulation of a running pipeline. The two are complementary:

| Axis | Stack | Question it answers |
| --- | --- | --- |
| 3D world / spectacle | `three-effect` + Foldkit (Verse) | "What is the network *doing* right now?" |
| 2D dataflow / control | Unit (proposed) | "How is this pipeline *wired*, and can I retune it live?" |

Unit does **not** belong inside `three-effect`'s boundary (Three.js scene code).
It is a separate DOM/SVG surface. Our `AGENTS.md`/`INVARIANTS.md` rule that
"three-effect is the only home for Three.js scene code" is unaffected — Unit
renders HTML+SVG into a Shadow DOM, never a WebGL canvas.

## What Unit actually is (the parts that matter to us)

- **Program model.** A program is a `GraphSpec` JSON: `{ units, merges, inputs,
  outputs, component, metadata, data }`. `units` reference other specs by `id`
  (recursive/composable); `merges` are links between unit pins; `metadata`
  carries node **positions**. So *logic and layout are serialized together* —
  exactly the shape you want for "store a wiring diagram and replay it."
- **Runtime.** Live JS classes (`Unit`, `Graph`, `Primitive`) with
  `push`/`pull`/`take`/`play`/`pause`. Reactive/dataflow: pins propagate through
  merges, units fire as FSMs when inputs are satisfied. The node graph you see
  *is* the live program — editing it edits a running system.
- **Canvas.** Real HTML + SVG via `createElementNS`, mounted inside
  `root.attachShadow({ mode: 'open' })` — **style-isolated from the host app by
  construction** (a major embedding win). Layout is a custom D3-force-style
  simulation (`src/client/simulation.ts`); nodes settle physically, which is
  what gives the "live objects" feel.
- **Embedding API (already first-class).**
  - Web: `import { renderBundle } from '@_unit/unit/client/platform/web/render'`
    → `renderBundle(rootEl, bundle) → [system, graph, unlisten]`. Then
    `graph.getUnit('x').push('style', { color: '#ffdd00' })`.
  - Headless/Node: `boot()` → `fromBundle(json, _specs, {})` → `new Class(system)`
    → `.play()` / `.push(pin, v)` / `.take(pin)`.
  - Live structural edits: the `Graph` action surface (`addUnit`, `addMerge`,
    `setUnitPinData`, `exposePinSet`, …) and the spec reducer/action layer
    (`src/spec/actions/G.ts`) — built for programmatic *and* collaborative
    drivers.
- **Async facade.** `AsyncGraph`/`AsyncG` (`src/types/interface/async/`) is a
  uniform async API over a graph whether it is in-process, in a worker, or
  remote. This is the clean seam for "drive the diagram from external data."

## Why this is a good fit for OpenAgents specifically

1. **Our domain is already a dataflow graph.** Khala (inference) → Pylon
   (delegation) → Codex/Claude executors, with receipts and token accounting
   flowing back, is literally a MIMO pipeline. Unit's "2D evolution of the CLI
   where stdin/stdout/stderr pipe into a graph" is the same metaphor we use
   verbally in the fleet docs.
2. **"No animation without a receipt" generalizes.** The Verse already binds
   motion to real Khala receipts. A Unit graph can bind **pin values** to the
   same receipts/events — every link that lights up is a real token flow, every
   datum node a real artifact ref. Same evidence discipline, 2D control idiom.
3. **Direct manipulation = operator control.** Unit isn't just a viewer. Pins
   are writable. An operator could pause a worker, retune a budget, or rewire a
   delegation by dragging a link — the graph *is* the control plane, not a
   read-only chart.
4. **Shadow-DOM isolation** means we can drop it next to Foldkit DOM and
   StyleX without CSS collisions.

## Integration targets (ranked)

### Target 1 — Khala Code: fleet / orchestration graph (recommended first)

**Where:** `clients/khala-code-desktop` (Electrobun). The fleet-management spec
(`docs/khala-code/2026-06-30-khala-code-fleet-management-spec.md`) already calls
for an operator UI: inbox, fleet board, worker cards, supervised orchestration,
trace viewer. A Unit graph is a strong candidate for the **fleet board /
orchestration** pane.

**What it shows:** one node per live entity — the Khala model, each Pylon, each
Codex/Claude worker, the approval queue. Merges = active delegations. Datum
nodes = live token counts, approval state, current tool call. Operator actions
(pause worker, approve, reroute) are pin pushes.

**How it mounts:** Electrobun renders Chromium; `renderBundle(div, bundle)` works
directly in the renderer process — the same path the existing
`src/client/platform/electron/index.ts` proves out (today it just points a
BrowserWindow at a local server; we want the in-renderer embed instead). The Bun
host already owns model transport and tool execution, so it is the natural
producer of the event stream that drives the graph via `AsyncGraph`.

**Relationship to ADR 0013.** ADR 0013 already introduces a ProseMirror-inspired
command composer for Khala Code with an *optional `three-effect` HUD*. Unit is a
**third, distinct** visual register: not the text composer, not the 3D HUD, but
a 2D structural diagram of the running fleet. It should be a separate, toggleable
pane, not a replacement for either.

### Target 2 — Verse companion: 2D "schematic view" of a Pylon/run

**Where:** `apps/autopilot-desktop` (the Verse). When you click a Pylon or the
Tassadar training core in the 3D world, a 2D Unit overlay could show its internal
wiring — the dataflow behind the spectacle. 3D for "where," 2D for "how." This
keeps `three-effect` as the sole 3D renderer and adds Unit purely as a DOM
overlay layer, no boundary violation.

**Cost note:** this is additive UI on an already-busy surface; sequence it after
Target 1 proves the embedding pattern.

### Target 3 — openagents.com web: shareable live diagrams

**Where:** `apps/openagents.com/apps/web` (Cloudflare Workers + web bundle).
Public, embeddable, link-shareable dataflow diagrams of agent runs / promises —
"here's the wiring of this accepted job," rendered live in the browser.

**Hard constraint (read before scoping this):** Unit's editor is a *client-side
browser bundle*. It **cannot run inside a Worker isolate** (no DOM/`window`), and
the Node headless runtime depends on `jsdom`/`express`/`ws`/Node ≥20 and is also
not Workers-compatible. On Cloudflare the split must be:
- Worker / D1 / R2: **store and serve `BundleSpec` JSON only** (and the static
  prebuilt IIFE asset).
- Browser: runs the Unit bundle, fetches the JSON, renders.
- Any server-side *headless graph execution* belongs on a Node service
  (container / DO-adjacent), never the Worker.

## Costs, risks, and gotchas (be honest)

- **You embed the whole organism or nothing.** The canvas, runtime, and spec
  model are one system. You cannot cheaply lift out "just the node renderer."
  Budget for embedding the full System, not a widget.
- **Bundle size.** ~850 standard-library units + a 61k-line Editor → a multi-MB
  editor bundle. Tree-shaking is limited because units resolve dynamically by
  `id` from a registry. Fine for Electrobun/desktop; a real cold-load
  consideration for web. Mitigation: a trimmed bundle that ships only the units
  we actually instantiate (our orchestration nodes + the UI primitives they
  use), and lazy-load the full editor only when an operator enters edit mode.
- **Mandatory codegen.** `_specs.ts`/`_classes.ts`/`_components.ts`/`_ids.ts` are
  **not committed** — generated by `npm run setup` (`src/script/sync.ts`) globbing
  ~850 `spec.json`. A fresh clone won't build until setup runs. **Mitigation:
  consume the published npm package `@_unit/unit` (prebuilt `lib/`)** rather than
  vendoring source; this sidesteps the whole setup step. This also matches our
  "don't vendor large external code by default" rule in the workspace `AGENTS.md`
  — Unit stays a dependency, not a fork.
- **Spec authoring is non-trivial but deterministic.** Generating a valid
  `BundleSpec` from our fleet data requires knowing unit `id`s and pin names.
  Recommend a small typed builder in our codebase (`fleetGraph → BundleSpec`)
  rather than hand-writing JSON, and a fixture-backed test like the Verse's
  `proof:verse-arc` smoke.
- **Stack mismatch.** Unit is plain TS/DOM with its own component framework; our
  surfaces are Effect/Foldkit. There is no Effect integration — Unit runs as an
  imperative island we boot and drive. Keep the seam thin: an Effect service that
  owns the `AsyncGraph` handle and translates our event stream (Khala receipts,
  Pylon state) into pin pushes.
- **Two reactive systems side by side.** Unit has its own runtime/scheduler;
  Foldkit/Effect has another. They don't share a fiber. Treat the Unit pane as an
  external sink fed by our event bus, not as state-of-record. State-of-record
  stays in our services; Unit is a projection (same discipline as Verse:
  `verse-khala-effect.ts` is a *projection* of a receipt, not the source).
- **Maintenance.** Single-author upstream, beta. Pin a version. Don't build a
  hard dependency on edit-mode semantics that could shift; the read+drive path
  (`renderBundle` + `AsyncGraph`) is the stable core.

## Recommended phased plan

**Phase 0 — spike (≈1–2 days).** In a throwaway Electrobun renderer, `npm
install @_unit/unit`, `renderBundle` a hand-authored fleet bundle (Khala → 2
Pylons → 3 workers), and drive pin values from a fake event stream. Goal: prove
the embed + drive loop and measure bundle weight. No product wiring.

**Phase 1 — Khala Code fleet graph (read-only).** A typed `fleetGraph →
BundleSpec` builder + an Effect service that subscribes to the existing fleet
event stream and pushes pins via `AsyncGraph`. Ship behind a feature flag as a
toggleable pane in the Khala Code desktop operator UI. Fixture-backed smoke test.

**Phase 2 — direct manipulation.** Make selected pins writable: pause worker,
approve from the approval queue, reroute a delegation — operator actions become
pin pushes routed back into the Bun host. This is where Unit earns its keep over
a static diagram.

**Phase 3 — Verse schematic overlay + web sharing.** Add the 2D click-through
schematic in `autopilot-desktop`, and the static-JSON + browser-bundle sharing
path on `openagents.com` web (respecting the Workers constraint above).

## Key Unit source references (for implementers)

- Embedding API: `src/client/platform/web/render.ts`, `src/client/platform/web/boot.ts`
- Headless runtime: `src/client/platform/node/boot.ts`, `src/boot.ts`, `src/spec/fromBundle.ts`
- Runtime + action surface: `src/Class/Graph/index.ts`, `src/Class/Unit/index.ts`
- Spec / serialization: `src/types/GraphSpec.ts`, `src/types/index.ts`, `src/bundle.ts`
- Async drive facade: `src/types/interface/async/` (`AsyncGraph.ts`, `AsyncG.ts`)
- The canvas: `src/system/platform/component/app/Editor/Component.ts` (61k lines)
- Layout engine: `src/client/simulation.ts`
- Mandatory codegen: `src/script/sync.ts`
- Electron precedent: `src/client/platform/electron/index.ts`

## Bottom line

Unit gives us a credible, MIT-licensed, embeddable **2D live-dataflow control
surface** that maps almost one-to-one onto how we already describe the Khala →
Pylon → worker fleet. It complements `three-effect`/Verse rather than competing
with it (2D structure vs 3D spectacle), it isolates cleanly via Shadow DOM, and
its embed-and-drive API is exactly the shape we need. The right first bet is the
**Khala Code fleet graph** as a read-only, flag-gated pane, consumed via the
published npm package (no fork, no codegen burden), then graduated to direct
manipulation. The one firm constraint to respect: Unit is browser/Node, not
Workers — keep Cloudflare to JSON storage + static asset hosting.
