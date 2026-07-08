# three-effect and Effect Native — Split, Don't Choose

Date: 2026-07-08
Status: analysis / decision input. Companion to the Effect Native decision
(`2026-07-08-effect-native-one-ui-substrate-analysis.md`), README, and the
Foldkit + React Native comparisons. Grounded in a deep read of our own
`three-effect` codebase (`~/work/three-effect`) on 2026-07-08.

The short version: **the standalone-vs-fold question has a false binary in
it.** three-effect is really two things wearing one name — a **large domain
library** of Three.js building blocks (the Verse world, VFX, HUD, Drei
ports) and a **small renderer kernel** (a scene reconciler + a frame clock +
a resource scope, ~600 LOC). The domain library should **stay standalone**;
the renderer kernel should **fold into the Effect Native canvas renderer**.
That split is the answer, and it also happens to realize the "Effect" the
name currently only claims.

## 1. What three-effect actually is (honestly)

three-effect is an **Effect-owned, React-free Three.js runtime**: a curated,
React-free port of the highest-volume Drei / react-three-fiber primitives,
plus a Foldkit custom-element adapter, giving our web/desktop surfaces
3D/VFX without React or `@react-three/fiber` at runtime. It's real and
load-bearing (the autopilot-desktop **Verse world**, several openagents.com
web scenes, khala-code-desktop).

But the architecture is not what the name suggests. Two honest findings:

- **The dominant pattern is imperative Three.js, not Effect and not data.**
  Every primitive is a plain synchronous factory returning a disposable
  mutable handle — `create*(options) → { object3D, update(dt), dispose() }`
  — driven by the host's frame loop. A scene is built by imperatively
  calling factories and `scene.add(...)`, not by describing a typed graph.
- **The "Effect" is a thin skin at the seams.** Only `Effect.try` /
  `Effect.sync` / `Data.TaggedError` appear, and only at the outer `mount*`
  boundary. `Scope`, `acquireRelease`, `Stream`, `Layer`, `Fiber`, `Ref` are
  **not used at all** — the two runtime services a renderer needs (the frame
  loop and lifecycle/disposal) are **hand-rolled plain objects,
  deliberately outside Effect**. The name "three-**effect**" oversells the
  Effect content today.

This isn't a criticism of its usefulness — it's the fact that decides how it
should relate to Effect Native.

## 2. The two things inside it (the architectural cut)

The deep read found a clean seam three-effect can be cut along:

**A. The renderer kernel (~10%, ~600 LOC) — genuinely renderer-shaped.**
Three modules form a small, coherent renderer core:
- `sceneNodeReconcilerPrimitives.ts` — diffs a **declarative descriptor
  tree** (`{ id, kind, props, children }`) against live Three objects via a
  factory catalogue, creating/updating/disposing per-node — an
  R3F-style reconciler, and the *only* place a scene is data.
- `frameClockPrimitives.ts` — a from-scratch rAF scheduler
  (`always | demand | manual`, priority callbacks, `invalidate`).
- `resourceScopePrimitives.ts` — a bespoke LIFO finalizer stack (the
  hand-rolled `Scope`).

This triad is exactly the shape of a canvas renderer — descriptor tree →
catalogue → scoped disposal → frame ticks — and it is exactly what
reimplements cleanly on Effect `Scope` / `Stream` / `Layer`.

**B. The domain library (~90%, ~24k LOC) — renderer-independent content.**
The ~50 primitive modules — Drei ports (cameras, controls, fat lines,
text/labels, postprocessing/bloom), the sci-fi **HUD** kit, the **Verse**
VFX (crackling arcs, spark bursts, pylon-network glow), and the MMO-ish
entity/controller/spatial systems — are reusable Three.js building blocks
that don't care *how* they're mounted or scheduled. They're valuable
unchanged whether three-effect is standalone or folded.

## 3. The recommendation: split along that seam

**Fold the kernel (A) into the Effect Native canvas renderer; keep the
domain library (B) standalone as a package the renderer draws from.** This
refines the roadmap's EN-6 ("canvas unification") from "fold three-effect
in" to something precise:

- **Effect Native gains a `canvas` renderer** whose core is the reconciler
  + frame-clock + scope triad, **reimplemented on real Effect** (`Scope`
  for disposal, `Stream`/`RuntimeScheduler`-style ticks for the frame loop,
  `Layer` for the renderer service). Same shape three-effect already has,
  built on the substrate instead of hand-rolled parallels.
- **The canvas renderer's catalogue is populated by three-effect's domain
  primitives** as leaf factories — a beam, a spark burst, a HUD meter
  become `kind`s the reconciler knows. Nothing in the VFX/Verse/HUD library
  is rewritten; it's *consumed* by the renderer.
- **A 3D surface becomes an Effect Native component** whose canvas subtree
  is a typed descriptor tree, rendered by this adapter — so the Verse world,
  proof-replay, and pylon scenes sit under the same component contract as
  the rest of the UI, exactly like DOM and RN surfaces do.

## 4. Why fold the kernel (three converging forces)

1. **three-effect's only integration layer is a dead end.** The
   `@openagentsinc/three-effect-foldkit` adapter is built on **Foldkit,
   which the 2026-07-04 decision marked migration-era legacy** ("do not
   start new Foldkit surfaces"). Its canvas surfaces need a *new* host seam
   regardless of Effect Native — and the Effect Native canvas adapter is
   precisely that replacement. There is no "leave it alone" option here; the
   integration layer has to move.
2. **Folding realizes the thesis the name already claims.** three-effect's
   "Effect" is nominal (§1). An Effect Native canvas renderer would *give it*
   the real `Scope`/`Stream`/`Layer` backbone it currently fakes by hand —
   deterministic, resource-safe, testable — turning "three-effect" from
   aspirational to accurate.
3. **The canvas renderer is already on the roadmap (EN-6).** We're building
   a canvas adapter anyway; the kernel triad is the ~600 LOC head start.
   Reimplementing it on Effect is a bounded, well-understood task, not a
   research project.

## 5. Why keep the domain library standalone

- **It's renderer-independent by nature.** VFX, HUD, Verse entities, and
  Drei ports are Three.js content; they don't need — and shouldn't be
  coupled to — the Effect Native contract. Coupling them would bloat the
  substrate and violate the "keep v0 ruthlessly small" discipline.
- **It's production-load-bearing and must not break.** The Verse world in
  autopilot-desktop (the heavyweight consumer — `trainingRun.ts` alone is
  ~190k) and the openagents.com scenes depend on these primitives *as they
  are*. A standalone package with a stable factory API lets the renderer
  evolve underneath without a risky rewrite of 24k LOC of working VFX.
- **Standalone keeps the boundary honest.** The domain library is a
  Three.js library; the canvas renderer is a UI renderer; the seam between
  them (the factory catalogue) is the same clean boundary React uses between
  host components and the reconciler.

So: **package `three-effect-core` stays a standalone Three.js primitive
library; its renderer kernel migrates into `@effect-native/render-canvas`;
its Foldkit adapter is retired in favor of the Effect Native canvas
adapter.**

## 6. Risks and caveats (so this is decided with eyes open)

- **Don't break the Verse.** autopilot-desktop and the web scenes are live.
  The migration is: build the Effect Native canvas renderer *beside* the
  existing mounts, move surfaces one scene at a time under capture-smoke
  parity (three-effect already has headless Playwright capture smokes over
  9 scenes — reuse them as the migration oracle), and retire the Foldkit
  adapter last.
- **Version skew is real.** three-effect pins `effect@4.0.0-beta.70`;
  Foldkit is on `beta.88`. Any shared-Effect story (Effect Native runtime +
  three-effect) needs the versions reconciled — a pre-req, not a surprise.
- **Reimplementing the kernel on Effect has a cost** (rebuilding frame-clock
  + scope on `Stream`/`Scope`). It's bounded (~600 LOC of known behavior +
  its tests), but it is real work; sequence it inside EN-6, not v0.
- **Resist over-folding.** The temptation will be to "make everything
  Effect." The domain primitives should stay plain, fast, imperative
  Three.js under the hood — the Effect discipline belongs in the
  *renderer/lifecycle*, not in every particle update running through the
  Effect runtime (that would be a performance and complexity mistake).

## 7. Recommendation

**Split, don't choose.** three-effect is not a monolith to fold or preserve
whole — it's a domain library with a small renderer kernel bolted on.

1. **Keep the domain primitive library standalone** (`three-effect-core`):
   stable factory API, the Verse/VFX/HUD/Drei-port content, unchanged and
   safe.
2. **Fold the renderer kernel into `@effect-native/render-canvas`** as EN-6:
   reconciler + frame clock + scope, reimplemented on real Effect
   `Scope`/`Stream`/`Layer`, drawing the standalone library's primitives as
   its catalogue.
3. **Retire the Foldkit adapter** (`three-effect-foldkit`) — it's on the
   deprecated path — replacing it with the Effect Native canvas adapter so
   3D surfaces become Effect Native components.
4. **Migrate live scenes one at a time** under the existing capture-smoke
   parity; never rewrite the working VFX library to move it.

This gives us the canvas renderer Effect Native needs, finally puts a real
Effect backbone under three-effect, retires the last Foldkit dependency in
the 3D stack, and does it all without a risky rewrite of the 24k LOC of Verse
and VFX work that's already carrying products.

## 8. Open questions

1. Packaging: does `three-effect-core` publish independently (stable factory
   API) while `@effect-native/render-canvas` depends on it, or does the
   renderer vendor the kernel and leave `three-effect` purely as primitives?
2. Effect version reconciliation (beta.70 → the Effect Native line) —
   sequence this before EN-6 starts.
3. The descriptor-tree contract: does the canvas renderer's node descriptor
   unify with the DOM/RN component descriptor (one Effect Native tree with a
   `canvas` region), or stay a distinct 3D descriptor the canvas component
   embeds? (Leaning: distinct 3D descriptor inside a canvas component — 3D
   scene graphs and 2D component trees are genuinely different shapes.)
4. arbiter-effect (the typed graph renderer, already pluggable) folds under
   the same EN-6 canvas umbrella — one renderer with two catalogues (3D +
   graph), or two sibling canvas renderers?
5. Do the Verse's MMO systems (entity registry, controllers, spatial hash)
   belong in the primitive library at all, or split further into a
   `verse-world` domain package the canvas renderer is agnostic to?
