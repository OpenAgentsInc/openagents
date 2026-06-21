# Verse Scene Graph vs. react-three-fiber — Construction Audit

Date: 2026-06-21
Scope: how the Autopilot desktop "Verse" 3D world builds its Three.js scene
graph today (via `@openagentsinc/three-effect` + Foldkit), how
react-three-fiber (r3f) solves the same problem, what we should learn from r3f,
what to add to `three-effect`, and how Effect best practices apply.

This is an architecture/direction audit, not an implementation plan. It pairs
with `2026-06-21-spacetimedb-verse-multiplayer-audit.md` (data/transport) and
`2026-06-21-autopilot-auto-forum-loop-and-verse-reflection-audit.md`.

## TL;DR

- Our Verse renderer is a **two-layer imperative pipeline**: pure data mappers
  in `apps/autopilot-desktop/src/shared/*` produce one big
  `TrainingRunVisualizationOptions` record, and `three-effect`'s
  `mountTrainingRunVisualization` imperatively builds the entire `THREE.Scene`
  with manual `.add()` calls and a raw `requestAnimationFrame` loop.
- **The seam has no reconciler.** The Foldkit custom element does a
  `JSON.stringify` equality check on the whole options object and, on *any*
  change, tears down the entire WebGL scene and rebuilds it from scratch. There
  is no diffing, keying, or pooling at the app↔library boundary. (One exception:
  a single bespoke keyed-diff for remote avatars inside the trainingRun scene,
  which the desktop app cannot currently reach through the pinned package.)
- r3f solves exactly this problem with a **catalogue + custom reconciler +
  prop-diffing + attach + on-demand frameloop + automatic disposal**. None of
  those concepts is React-specific; they are the portable ideas.
- `three-effect` uses Effect's *types* (`Data.TaggedError`,
  `Effect.Effect` signatures) but almost none of Effect's *runtime* — **no
  `Scope`, no `acquireRelease`, no `Layer`/service, no `Ref`/`SubscriptionRef`,
  no `Stream`, no `Effect.gen`**. That is the single biggest lever: the
  resource-ownership and reactive-state model r3f hand-rolls is exactly what
  Effect gives us for free.

## 1. How the Verse scene graph is constructed today

### 1.1 Two layers, one record

The Verse world is built on a strict split:

1. **App layer** — `apps/autopilot-desktop/src/shared/*` and `src/ui/view.ts`.
   Pure, Three.js-free data projection. Each "scene" file is a pure function
   that appends descriptors onto a single plain record,
   `TrainingRunVisualizationOptions` (arrays of node / entity / beam / burst /
   world-item / remote-avatar descriptors defined in
   `src/types/three-effect-core.d.ts`). No Three.js, no `.add()`, no render
   loop, almost no Effect. These are unit-testable.

2. **Library layer** — `@openagentsinc/three-effect`. All imperative Three.js.
   `mountTrainingRunVisualization` receives the record and builds the real
   `THREE.Scene`, runs the RAF loop, and disposes resources.

The canonical composition is `verseSceneVisualization` in
`src/ui/view.ts:5849`, an append-only fold of layer functions:

```ts
const base       = pylonNetworkVisualizationOptions(liveScene ?? CHAT_SCENE)
const withTraining = withVerseTrainingLayer(base, {...})
const withBulletin = withVerseBulletinBoardLayer(withTraining, ...)
const withBase     = withPylonBaseLayer(withBulletin, pylonBase)
const withWorld    = withChatWorldMultiplayerLayer(withBase, multiplayer, {...})
// ...then pose, camera, payment layers
```

Each `with*Layer` returns `{ ...base, nodes: [...(base.nodes ?? []), node] }`
(e.g. `verse-bulletin-board.ts:116`, `pylon-base-scene.ts:328`,
`chat-world-visualization.ts:589`). This part is actually clean — it is a
declarative, data-driven description of *what the scene should contain*.

### 1.2 The imperative side

`mountTrainingRunVisualization` (three-effect `packages/core/src/trainingRun.ts`)
does the real construction inside one big `Effect.try` closure: `new
THREE.Scene()`, lights `.add()`ed individually, a `PerspectiveCamera`, a `root`
`Group`, floor grid + street district, and per-descriptor meshes built with
inline `new THREE.XGeometry(...)` / `new THREE.YMaterial(...)`. The render loop
is the classic recursive `requestAnimationFrame` (`trainingRun.ts:~3288`); the
per-frame closure runs controller updates via `Effect.runSync(...)` 60×/s, then
`renderer.render(scene, camera)`.

So: **declarative data on top, classic imperative `.add()` underneath, with a
custom element as the seam.**

### 1.3 The reconciliation seam — full teardown + rebuild

This is the central finding. The Foldkit element
(`packages/foldkit/src/index.ts`):

```ts
set visualization(value: unknown) {
  const signature = stableOptionsSignature(value) // JSON.stringify
  if (signature === this.#visualizationSignature) return
  this.#visualizationSignature = signature
  if (this.isConnected && this.#mount !== null) this.#remount()
}
#remount() {
  this.#unmount()                                  // dispose entire scene
  Effect.runSync(mountTrainingRunVisualization(...)) // rebuild from scratch
}
```

Every time *any* field of the options changes (a pylon comes online, an agent
moves, a payment beam appears, a node flips status), the entire WebGL context,
scene graph, geometries, materials, and RAF loop are destroyed and recreated.
There is:

- **No diffing** of node/entity/beam/avatar arrays.
- **No object pooling/keying** across the app↔library boundary.
- Only a `JSON.stringify` change-detector, which is order- and float-sensitive
  — so the pure mappers must defensively `.toFixed(3)` positions and use stable
  ring layouts purely to avoid spurious remounts. That defensive rounding is a
  smell that points directly at the wrong reconciliation primitive.

For a live agent MMORPG with moving avatars and flying payment particles this
is a **remount-per-tick risk**: repeated `WebGLRenderer` create/dispose (browsers
cap live contexts ~16), GC pressure from reallocating every geometry/material,
lost camera/controller momentum, and visual hitches.

There is exactly one in-place update path in the whole stack:
`updateRemoteAvatars` (`trainingRun.ts:~3359`), a single-purpose
`Map<string, RemoteAvatarRuntime>` diff keyed by `id` with create / update
(interpolate in place) / remove logic. It is the only place that hints at what a
real reconciler would look like — and the desktop app cannot currently use it,
because the app's local `three-effect-foldkit.d.ts` advertises a richer handle
(`updateRemoteAvatars`, presence callbacks) than the *installed* package
forwards. That type/runtime skew should be reconciled regardless of the larger
direction.

### 1.4 The counter-example we already have

`src/ui/hud-status-scene.ts` is a hand-rolled imperative Three.js mini-renderer
(its own `WebGLRenderer`, `Scene`, RAF loop, explicit `disposables[]`). Crucially
it updates **in place** via `setProjection` — `meter.setValue`,
`label.setText`, `material.color.set` — never tearing down. This is the pattern
the world scene *should* adopt, and it proves the team already knows how; it is
just not generalized.

### 1.5 Resource lifecycle today

- **Creation**: inline `new` per object; no shared geometry/material cache, so
  each (re)mount reallocates everything.
- **Disposal**: three-effect has a correct recursive `disposeObject` and a
  thorough `dispose` handle (cancel RAF, disconnect observers, remove listeners,
  dispose geometries/materials/textures, `renderer.dispose()`). It is correct
  but **entirely manual** — a leak is one forgotten `dispose` away, and the
  `Effect.try` mount has a partial-construction leak window (a global `keydown`
  listener is added before later code that can throw; the `catch` path does not
  run partial cleanup).

### 1.6 How Effect actually shows up

Minimally, and only at the boundary:
- App scene files: no Effect.
- `mountTrainingRunVisualization`: returns `Effect.Effect<Handle, MountError>`
  but is one synchronous `Effect.try`; `dispose`/`resize` are `Effect.sync`
  thunks.
- The Foldkit element runs everything with `Effect.runSync` at lifecycle
  callbacks, which also swallows the typed mount errors the code carefully
  builds.
- **No `Scope`, no `acquireRelease`, no `Layer`/service, no `Ref`/
  `SubscriptionRef`, no `Stream`, no `Effect.gen`.** Per-frame mutation uses
  plain closure-captured `let` bindings. Effect here is a typed try/catch
  wrapper, not a structured-concurrency/resource model.

### 1.7 Repeated boilerplate worth extracting

Independent of the reconciler question, these are duplicated across scene files
and want a shared `scene-helpers.ts`:
- `with*Layer(base, x) => ({ ...base, field: [...(base.field ?? []), item] })`
- `uniqueStrings` / `uniqueRefs` dedupe helpers (≥4 copies)
- deterministic ring-layout (`ringPosition` vs `endpointRingPosition`)
- status → renderer-status mappers (`baseNodeStatus`, `statusFromRun`,
  `statusFromRunState`, `toneToStatus`)
- `vectorFromXYZ` / `finite` numeric guards and `.toFixed(3)` rounding
- the evidence record `{motionId, motionKind, sourceRefs, ...}` rebuilt by hand

## 2. How react-three-fiber constructs its scene graph

r3f is a **custom React reconciler** (`react-reconciler` host config) whose
"host instances" are THREE objects rather than DOM nodes. The pieces, all in
`packages/repos/react-three-fiber/packages/fiber/src/core`:

1. **Catalogue + `extend`** (`reconciler.tsx`). A plain object mapping JSX tag
   names → THREE constructors. `extend(THREE)` registers the whole namespace at
   `Canvas` mount, which is why `<mesh>`, `<boxGeometry>`,
   `<meshStandardMaterial>` "just work": the tag is PascalCased and looked up.
   Open-ended — third-party classes register the same way.

2. **The parallel `Instance` descriptor with a `__r3f` back-pointer**
   (`prepare`, `utils.tsx`). Every THREE object carries a lightweight metadata
   node holding `parent / children / props / handlers / attach / previousAttach`.
   r3f maintains its *own* child list independent of THREE's `.children`, which
   is what enables ordered insertion, disposal policy, event registration, and
   reconstruction.

3. **Lazy, Suspense-safe construction.** `createInstance` does not call `new`;
   construction is deferred to `handleContainerEffects` once the subtree is
   actually mounted under the scene, so speculative React work never leaks GPU
   objects. `new target(...args)` is driven by an `args` prop.

4. **`applyProps` / `diffProps`** (`utils.tsx`). Props are applied polymorphically
   by target shape: `THREE.Color.set(...)` for colors, `copy()` for matching math
   types (mutate in place, no realloc), `fromArray()` for `position={[x,y,z]}`,
   `setScalar()` for `scale={2}`, else plain assignment. `diffProps` computes a
   minimal change set with a tunable equality, and — critically — **resets
   removed props to constructor defaults** via a cached blank prototype (THREE has
   no "unset").

5. **`attach`** (`utils.tsx`). The single insight that a child is either *added*
   to the parent's display list (`parent.add(child)`) or *assigned* to a parent
   property slot (`mesh.geometry = box`). Auto-attach infers `geometry`/`material`;
   string attach supports dash-piercing (`material-color`) and array indices;
   functional attach returns a teardown. `detach` restores the previous value.

6. **On-demand frameloop** (`loop.ts`). One global RAF across all roots. Modes
   `always` / `demand` / `never`. A frame counter (`internal.frames`) gates
   demand rendering; `invalidate()` bumps the counter and restarts the loop if it
   had stopped. Every store mutation auto-invalidates, so mutations re-render
   exactly once and then the loop goes idle — power-efficient retained-mode
   rendering. `useFrame` subscriptions are priority-ordered; a positive-priority
   subscriber takes over the render call (custom pipelines / postprocessing).

7. **Automatic, policy-aware disposal** (`reconciler.tsx`). `removeChild`
   recurses depth-first, detaches, removes from THREE, deletes `__r3f`, and
   disposes — honoring a `dispose={null}` opt-out, never disposing primitives or
   `Scene`, and scheduling the actual GPU free at idle priority to avoid frame
   stalls. A separate deep sweep runs on full unmount.

8. **Reconstruction batching.** When immutable constructor `args` change you must
   rebuild; r3f batches reconstructions and flushes at the sibling tail,
   transferring interactivity/refs and disposing the old object.

9. **Store-as-context + selector subscriptions** (`store.ts`, Zustand). Root
   state (`gl / scene / camera / raycaster / size / viewport / frameloop /
   internal`) lives in one store whose *reference* is stable on React context;
   consumers subscribe to slices, so external mutations drive both THREE config
   and on-demand re-renders without re-rendering the whole tree. The store
   subscribes to itself to resize/recompute-viewport/invalidate.

10. **Flat interaction registry + synthetic bubbling** (`events.ts`). r3f does
    not raycast the whole scene; it keeps a flat array of only handler-bearing
    objects, raycasts those, sorts by layer-priority then distance, dedupes, and
    synthesizes DOM-like bubbling by walking parents. `stopPropagation` is a
    flag; pointer-capture is an out-of-band map.

## 3. Side-by-side

| Concern | Verse (today) | react-three-fiber |
|---|---|---|
| Scene description | Declarative data record (good) | Declarative JSX tree |
| Element → THREE class | Hard-coded in one giant mount fn | Catalogue + `extend` |
| Construction | Eager, inline `new` in one closure | Lazy, Suspense-safe, `args`-driven |
| State → scene update | **Full teardown + rebuild on any change** | Keyed reconciliation + minimal prop diff |
| Prop application | Rebuild the object | `applyProps` mutate-in-place by shape |
| Geometry/material parenting | Manual `.add()` / assignment | Unified `attach` (+ dash-pierce, indices) |
| Frameloop | Always-on raw RAF | `always`/`demand`/`never` + `invalidate` |
| Per-frame hooks | One closure; `Effect.runSync` 60×/s | Priority-ordered `useFrame`, render takeover |
| Disposal | Manual, all-or-nothing on remount | Per-node, recursive, idle-scheduled, opt-out |
| Resource ownership | Manual dispose handle (leak-prone) | Reconciler-owned lifecycle |
| Root state | Plain Foldkit model + closure `let`s | Zustand store + selector subscriptions |
| Interaction | (full hit-target registry rebuilt per mount) | Flat handler registry + synthetic bubbling |
| Change detection | `JSON.stringify` signature | Structural per-prop diff with tunable equality |

The gap is not "r3f is declarative and we are imperative" — our *input* is
already declarative data. The gap is that **we have no retained-mode
reconciler between the declarative data and the imperative scene**, so we
collapse every update to a teardown. r3f's entire value is that middle layer.

## 4. What we should learn / port (concepts, not React)

These are renderer-agnostic and translate directly into a non-React,
Effect-based `three-effect`:

1. **A node/element model.** A serializable description of the desired Object3D
   tree: `{ type, key, props, children }`. We already produce something close
   (the descriptor arrays); formalize it into a generic node tree with stable
   `key`/`id` per node so it can be diffed instead of stringified.

2. **A catalogue.** A typed registry mapping node `type` → THREE constructor
   (+ `args`). Mirrors `extend`; lets the trainingRun-specific builders become
   data instead of bespoke code, and lets new node kinds register without editing
   the mount function.

3. **A general reconciler.** Diff previous vs. next node trees keyed by
   `id`: create / update / remove, plus a prop-differ that decides
   **set-in-place vs. dispose-and-recreate** (args/geometry change → recreate;
   color/position change → mutate). The existing `updateRemoteAvatars` map-diff
   is the seed — generalize it from "avatars" to "any keyed children."

4. **`applyProps` by target shape.** Port the small set of setter strategies
   (Color.set, copy for matching math types, fromArray, setScalar, assign) so
   `position`, `color`, `scale` mutate existing objects with no reallocation, and
   port the **default-reset-on-removal** trick (cached blank prototype).

5. **`attach`.** Unify "add to display list" vs. "assign to property slot" with
   dash-piercing, array indices, and functional attach/detach. In Effect this is
   a natural `acquireRelease` (acquire = assign, release = restore).

6. **On-demand frameloop.** Replace the always-on RAF + `disposed` boolean with
   a dirty-signal-driven loop: render only when state changes, then idle. This
   matters for a desktop app's battery/GPU.

7. **Priority-ordered frame subscriptions.** A registry of per-tick callbacks
   ordered by priority, with the option to cede the render call — replaces the
   single hard-coded render closure and makes controllers/animators composable.

8. **Per-node, policy-aware, idle-scheduled disposal.** Recursive, with a
   "don't dispose this" opt-out, never disposing Scenes, freeing at idle.

9. **Shared geometry/material cache.** Once updates stop being full rebuilds,
   cache and reuse primitives keyed by their args to cut allocation/GC.

10. **Flat interaction registry.** Keep hit targets in a registry maintained by
    the reconciler rather than rebuilt wholesale each mount; raycast only
    handler-bearing objects and synthesize bubbling.

What we should **not** do: adopt React, `react-reconciler`, or JSX. The desktop
app is Foldkit/Effect; a React reconciler would be a foreign runtime. We want
r3f's *concepts* implemented on Effect primitives.

## 5. How Effect best practices help

This is the highest-leverage observation: **the machinery r3f hand-rolls in
Zustand + a custom reconciler + manual disposal maps almost one-to-one onto
Effect primitives we are currently not using.** (Per `effect-solutions
services-and-layers` and the repo's Effect 4.x conventions.)

- **`Scope` / `acquireRelease` instead of manual dispose handles.** Each scene
  resource (renderer, geometry, material, texture, RAF loop, event listeners,
  `ResizeObserver`) becomes an `Effect.acquireRelease(create, release)`. Tie each
  reconciled node's GPU resources to a child scope; when the node is removed,
  close its scope and finalizers run automatically. This makes leaks
  *structurally impossible* and closes the partial-construction leak window in
  the current `Effect.try` mount. This is r3f's "automatic disposal," but
  enforced by the type system instead of by reconciler bookkeeping.

- **`Context.Service` + `Layer` for the root state.** Model r3f's store as a
  `SceneContext` service: `renderer`, `camera`, `clock`, `raycaster`, `size`,
  `frameloop`, plus an asset cache. Provide it as a `Layer.scoped` so the
  renderer/cache acquire and release with the scene. This replaces positional
  raw-THREE arguments threaded through builders, and makes scenes testable by
  swapping a headless renderer layer (the team already wants a frame-clock
  abstraction for tests; this is the home for it).

- **`SubscriptionRef` / `Stream` for state → scene.** Hold the declarative node
  tree (or app model) in a `SubscriptionRef`. The reconciler consumes
  `ref.changes` as a `Stream`, diffs old vs. new, and applies the minimal
  mutation. This replaces the `JSON.stringify` signature + full remount with
  r3f's "mutation auto-invalidates and re-renders exactly the delta." The
  multiplayer/SpacetimeDB subscription path (see
  `2026-06-21-spacetimedb-verse-multiplayer-audit.md`) feeds this `Ref`
  naturally — moving avatars become in-place updates, not remounts.

- **A `Stream`/`Schedule`-driven frame clock instead of raw RAF.** Drive frames
  from an Effect fiber so the loop is interruptible by `Scope` (no `disposed`
  boolean), testable (step the clock without real RAF), and gated by a dirty
  signal for on-demand rendering. Per-frame controller updates stop being
  `Effect.runSync` 60×/s inside a JS closure and become part of the fiber.

- **`Effect.gen` + `Effect.fn` for the mount/reconcile path** instead of one
  giant `Effect.try` closure, so construction composes, errors stay typed
  through the boundary (don't `Effect.runSync`-swallow them — surface mount
  failure as a visible degraded state), and call-site tracing works.

- **Keep `Data.TaggedError` / typed errors** — that part is already good; just
  stop discarding them at the Foldkit boundary.

Net: r3f proves the *concepts* (catalogue, instance tree, attach, prop-diff,
on-demand loop, automatic disposal, reactive store). Effect gives us a *better
substrate* for half of them — resource ownership (`Scope`), reactive state
(`SubscriptionRef`/`Stream`), DI/testability (`Layer`/service), and an
interruptible frame loop (fibers) — than r3f's React+Zustand stack.

## 6. Recommendations (priority order)

Direction only; sequencing belongs in an implementation plan.

1. **Fix the type/runtime skew now (cheap, unblocks everything).** Repin or bump
   `three-effect` so the installed package's `TrainingRunVisualizationHandle`
   matches `src/types/three-effect-foldkit.d.ts` (the `updateRemoteAvatars` /
   presence-callback gap). Today the desktop app passes callbacks and expects
   incremental methods that the pinned package silently drops.

2. **Route live updates through the incremental path, not remount.** Once #1
   lands, push moving avatars / beams / status flips through in-place handle
   methods (generalize `updateRemoteAvatars`) so the common per-tick case stops
   triggering `#remount()`. This alone removes the worst WebGL-context churn.

3. **Add a `Scope`-based resource model in `three-effect`.** Convert mount to
   `Effect.gen` + `acquireRelease` for renderer/listeners/observer/RAF and the
   per-node resources. Structural leak safety + closes the partial-mount window.

4. **Introduce a node tree + catalogue + general reconciler in `three-effect`.**
   Promote the descriptor records to a keyed node tree; diff and mutate instead
   of rebuild; port `applyProps`/`attach`/default-reset. Drive it from a
   `SubscriptionRef` fed by the app model.

5. **Replace the raw RAF with a fiber/`Stream` frame clock + on-demand
   rendering** and a priority-ordered frame-subscription registry.

6. **Extract the duplicated pure helpers** (`scene-helpers.ts`: dedupe, ring
   layout, status mapping, evidence record, vector guards) — independent of all
   the above and worth doing immediately.

All of this work belongs in `three-effect` (the workspace's owned home for
spatial primitives, per `docs/game/README.md` "Implementation Homes"), consumed
unchanged by `apps/autopilot-desktop` and `apps/openagents.com`. Do not build an
app-local reconciler in the desktop app.

## Appendix — key files

Verse / desktop:
- `apps/autopilot-desktop/src/ui/view.ts` — `verseSceneVisualization` (~:5849),
  `trainingSceneOptions` (~:1534), chat/Verse background (~:6100)
- `apps/autopilot-desktop/src/shared/*` — pure projection mappers
  (`chat-world-scene.ts`, `chat-world-visualization.ts`, `pylon-base-scene.ts`,
  `verse-training-visualization.ts`, `verse-bulletin-board.ts`, etc.)
- `apps/autopilot-desktop/src/ui/hud-status-scene.ts` — the only in-place
  imperative renderer (the pattern the world scene should adopt)
- `apps/autopilot-desktop/src/types/three-effect-core.d.ts`,
  `three-effect-foldkit.d.ts` — contracts (note the skew vs. installed pkg)

three-effect:
- `packages/core/src/index.ts` — `mountSpinningCube` reference mount shape
- `packages/core/src/trainingRun.ts` — `mountTrainingRunVisualization`, RAF loop,
  `disposeObject`, and the lone keyed diff `updateRemoteAvatars` (~:3359)
- `packages/core/src/sceneGraphPrimitives.ts`, `renderPrimitives.ts`,
  `assetPrimitives.ts`, `controlsPrimitives.ts` — imperative primitive toolkit
- `packages/foldkit/src/index.ts` — custom-element lifecycle, `JSON.stringify`
  signature gate, `#remount`
- `docs/2026-06-14-implementation-audit.md` — maintainers' own gap list
  (resource registry, frame-clock, SceneResource lifecycle) corroborating this

react-three-fiber (reference, `projects/repos/react-three-fiber/packages/fiber/src/core`):
- `reconciler.tsx` — catalogue/`extend`, `createInstance`,
  `handleContainerEffects`, `commitUpdate`, reconstruction, `removeChild`
- `utils.tsx` — `prepare`, `applyProps`, `diffProps`, `attach`/`detach`,
  `dispose`, `invalidateInstance`
- `store.ts` — Zustand root state + self-subscriptions
- `loop.ts` — global RAF, `always`/`demand`/`never`, `invalidate`, `advance`
- `hooks.tsx` — `useThree`/`useFrame`/`useLoader`
- `events.ts` — interaction registry, raycast, synthetic bubbling
