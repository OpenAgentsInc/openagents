# Effect + Three.js Fiber Audit

Date: 2026-06-14

## Short Answer

An Effect-native Three.js library is worth exploring, but the winning shape is
not "React Three Fiber with React swapped out for Effect." React Three Fiber
works because React is already a component runtime, tree reconciler, scheduler,
event boundary, context system, Suspense runtime, and community language for UI.
Effect is a stronger substrate for typed effects, scoped resource lifetimes,
fibers, streams, test clocks, services, layers, and observability. It is not, by
itself, a UI tree reconciler.

The strongest opportunity is therefore a parallel library:

- `@effect-three/core`: a resource-safe Three runtime built on Effect v4 scopes,
  layers, resources, fibers, queues, streams, and atom/reactivity primitives.
- `@effect-three/foldkit`: a first-class adapter for Foldkit apps such as
  `openagents.com` web and the Autopilot Desktop webview, built around
  Foldkit `ManagedResource`, `Command`, `Subscription`, `Mount`, and
  `CustomElement` boundaries.
- `@effect-three/react`: an optional React adapter, likely using
  `@effect/atom-react` and React only for UI integration, not for ownership of
  the Three scene graph.
- Later, if the core proves itself, a declarative tree DSL or JSX runtime can be
  added. That should be a second project, not the first milestone.

The MVP should not try to reach React Three Fiber parity. It should try to beat
vanilla Three.js on the problems where Effect has real leverage:

- explicit root, renderer, scene, camera, asset, worker, and input lifetimes;
- structured cancellation for loaders, streams, and background animation tasks;
- typed failure channels for WebGL/WebGPU setup, asset loading, decoding, and
  event pipelines;
- testable render loops and asset pipelines;
- deterministic cleanup of GPU-ish resources through `Scope` and finalizers;
- optional reactive state via Effect v4 `AtomRegistry`, `Atom`, and
  `@effect/atom-react`.

If the ambition is "make Three pleasant in an Effect application," this is a
good idea. If the ambition is "replace React Three Fiber for creative-coding
React users," it is a much bigger bet and should only happen after a small
runtime proves that the Effect model can carry real scenes without awkwardness
or frame-loop overhead.

## Source Inventory

I inspected these local references:

- `projects/repos/three.js`
  - `package.json`
  - `src/core/Object3D.js`
  - `src/core/EventDispatcher.js`
  - `src/renderers/WebGLRenderer.js`
- `projects/repos/react-three-fiber`
  - `packages/fiber/src/core/renderer.tsx`
  - `packages/fiber/src/core/reconciler.tsx`
  - `packages/fiber/src/core/store.ts`
  - `packages/fiber/src/core/loop.ts`
  - `packages/fiber/src/core/events.ts`
  - `packages/fiber/src/core/hooks.tsx`
  - `packages/fiber/src/core/utils.tsx`
  - `packages/fiber/src/web/Canvas.tsx`
  - `packages/fiber/src/three-types.ts`
  - `packages/test-renderer/src/index.tsx`
  - package metadata for `@react-three/fiber@9.6.1`
- `projects/repos/effect`
  - This checkout is the current `Effect-TS/effect` monorepo line. Its
    `packages/effect/package.json` reports `effect@3.21.3`, so it is not the
    current v4 source line.
- `projects/repos/effect-smol`
  - This is the current v4 source line for `effect`, fetched from
    `https://github.com/Effect-TS/effect-smol.git`.
  - `packages/effect/package.json` reports `effect@4.0.0-beta.83`.
  - I inspected `Effect`, `Layer`, `Scope`, `Runtime`, `Resource`, `FiberMap`,
    `FiberSet`, `LayerMap`, `Queue`, `PubSub`, `Stream`, and the v4
    `unstable/reactivity` modules.
  - I also inspected `@effect/atom-react` under `packages/atom/react`.
- `projects/repos/effect-solutions`
  - Secondary pattern reference. It depended on `effect@4.0.0-beta.59` and
    helped identify the v4 line before `projects/repos/effect-smol` was found.
- `projects/repos/foldkit`
  - `packages/foldkit/package.json` reports `foldkit@0.111.0`, with peers on
    `effect@4.0.0-beta.78` and `@effect/platform-browser@4.0.0-beta.78`.
  - I inspected `Runtime`, `Command`, `Subscription`, `ManagedResource`,
    `Mount`, `CustomElement`, `Canvas`, `Html`, and test/story exports.
- `openagents`
  - `apps/openagents.com/apps/web` is the current `openagents.com` Foldkit web
    app. It uses `Runtime.makeProgram`, Foldkit navigation, `foldkit/html`,
    `foldkit@^0.102.1`, `effect@4.0.0-beta.70`, and
    `@effect/platform-browser@4.0.0-beta.70`.
  - `apps/autopilot-desktop` is the Electrobun desktop companion. Its webview is
    explicitly required to use Foldkit plus shared `@openagentsinc/autopilot-ui`
    components; Bun main-process authority remains outside Foldkit.
  - `packages/autopilot-ui` is a shared Foldkit HTML component package used by
    web and desktop.
  - `clients/khala-ios/AutopilotRemoteControl` is the Expo / React Native mobile
    client. It shares the Autopilot control protocol but, in the local source
    inspected here, does not directly depend on Foldkit.

The important version correction is: Effect v4 is in `effect-smol`, not the
older `effect` monorepo checkout. The audit below is v4-oriented.

## What React Three Fiber Actually Provides

React Three Fiber is not a thin JSX wrapper around Three.js. It is an alternate
React renderer whose host target is a Three.js object graph. The package name is
accurate: it is React Fiber adapted to Three.

Its core responsibilities are:

1. Create and configure a root for a canvas or offscreen canvas.
2. Maintain a root store containing renderer, camera, scene, raycaster, clock,
   viewport, DPR, frameloop mode, events, controls, and internal bookkeeping.
3. Translate React elements into Three instances.
4. Apply props into Three objects with Three-specific conversion rules.
5. Attach child objects either through `Object3D.add` or through special
   attachment paths such as `material`, `geometry`, or nested material uniforms.
6. Diff updates and reconstruct instances when constructor args or primitive
   object identity changes.
7. Dispose objects, geometries, and materials without stalling the render path.
8. Run a shared render loop with `always`, `demand`, and `never` modes.
9. Expose frame subscriptions through `useFrame`.
10. Implement a pointer event system on top of raycasting.
11. Support asset loading through React Suspense.
12. Provide a test renderer that can mount, update, inspect, fire events, and
    manually advance frames.

The useful lesson is that R3F's value is not JSX alone. The value is the whole
ownership and scheduling contract around Three's mutable graph.

### R3F Root And Store

`packages/fiber/src/core/renderer.tsx` maps each canvas to a root in `_roots`.
The root owns a Zustand store created by `createStore` and a React reconciler
container. `configure` builds or accepts a renderer, scene, camera, raycaster,
event manager, DPR, viewport, and performance options. Then `render` sends the
React node tree into the reconciler.

`packages/fiber/src/core/store.ts` defines the runtime state. Important fields:

- `gl`
- `camera`
- `scene`
- `raycaster`
- `clock`
- `events`
- `xr`
- `controls`
- `pointer`
- `frameloop`
- `performance`
- `size`
- `viewport`
- `invalidate`
- `advance`
- `internal.subscribers`
- `internal.interaction`
- `internal.hovered`
- `internal.capturedMap`

That shape maps naturally to an Effect service. An Effect version should have a
`ThreeRoot` or `ThreeRuntime` service with roughly these capabilities, but not
necessarily the same implementation.

### R3F Render Loop

`packages/fiber/src/core/loop.ts` has a global RAF loop over all roots. Each
root can render continuously, on demand, or only when manually advanced.
Subscribers run before `gl.render(scene, camera)`. The loop also has global
before/after/tail effects.

This is one of the places where Effect can help:

- a root render loop can be a scoped fiber;
- frame callbacks can be fibers or a managed subscription registry;
- `frameloop="never"` maps to a testable manual driver;
- demand rendering maps to a queue/pubsub invalidation signal;
- cancellation can be structural instead of manually unwinding callbacks.

But Effect should not blindly run every per-frame update as a full high-level
Effect program. Per-frame overhead matters. The inner render hot path should
stay lean, with Effect owning lifecycle, cancellation, and control planes around
the hot loop.

### R3F Reconciler

`packages/fiber/src/core/reconciler.tsx` implements React host config for Three:

- a catalogue maps element names to Three constructors;
- `extend` adds more constructors;
- `createInstance` validates type/props and prepares an instance descriptor;
- `appendChild`, `insertBefore`, and container variants link children;
- `handleContainerEffects` creates Three objects, applies props, attaches
  children, and invalidates the root;
- `removeChild` detaches, recursively removes, updates interaction state, and
  disposes;
- `commitUpdate` diffs props and reconstructs when constructor identity changes;
- `hideInstance` / `unhideInstance` support React offscreen semantics.

This is the expensive part to recreate if the goal is an Effect-native
declarative tree. It is not impossible. But React gives R3F a battle-tested tree
runtime for free. Effect does not. If we want declarative scene descriptions
without React, we must build or adopt a tree runtime.

The realistic options are:

- do not build a reconciler initially;
- build a small keyed scene-operation runtime;
- later add JSX or a declarative DSL that compiles into keyed operations.

### R3F Props And Attach Semantics

`packages/fiber/src/core/utils.tsx` is the core compatibility layer with Three.
It handles:

- dash-pierced props such as `material-color`;
- colors;
- vectors, matrices, eulers, layers, and scalar setters;
- arrays into `fromArray` / `set`;
- shader uniforms while preserving uniform object identity;
- texture color space rules;
- event handler registration;
- auto-attaching geometries and materials;
- resetting removed props by comparing against memoized prototypes;
- invalidation on update.

This is not incidental. Any serious Effect version needs a similar `applyProps`
layer. Without it, the library will be less ergonomic than R3F and only
marginally nicer than vanilla Three.

The key question is not "Can Effect replace this?" It cannot. The question is
"Can Effect make these mutations safer, scoped, typed, and testable?" Yes.

### R3F Events

`packages/fiber/src/core/events.ts` turns host pointer events into raycasted
Three events. It tracks interaction objects, intersections, hover state,
pointer capture, propagation, missed clicks, and bubbling through ancestors.

This is a major parity cliff. A small Effect Three runtime can start with
simpler event streams:

- root DOM event stream;
- raycast stream;
- object hit stream;
- explicit subscriptions per object;
- scoped unsubscription.

Full R3F parity needs pointer capture and hover semantics. That should be a
later milestone, because it is easy to get almost right and still break real
interaction code.

### R3F Hooks And Assets

`packages/fiber/src/core/hooks.tsx` exposes:

- `useThree`
- `useStore`
- `useFrame`
- `useGraph`
- `useLoader`

`useLoader` relies on `suspend-react`, memoized loaders, and Suspense. An
Effect version should not copy this directly. Effect v4 has better primitives
for asset lifetimes:

- `Resource` for refreshable acquired values;
- `LayerMap` for keyed loader families or asset caches;
- `FiberMap` for cancellable loads keyed by URL;
- `Scope` finalizers for disposal;
- `Stream` for progress events;
- typed failures for decode/load errors.

React integration can still expose Suspense through `@effect/atom-react` or a
React-specific bridge, but core asset loading should be framework-neutral.

## What Three.js Imposes

Three itself is an imperative engine. The base object is `Object3D`, with
mutable `position`, `rotation`, `quaternion`, `scale`, `matrix`, `matrixWorld`,
`children`, and `parent` state. Adding/removing children mutates the graph and
dispatches events. `EventDispatcher` is a simple mutable listener registry.
`WebGLRenderer` owns a canvas/context and a large internal WebGL resource graph.

This matters because an Effect library should not pretend Three becomes pure.
The honest architecture is:

- pure or persistent values may describe desired changes;
- Effect controls when and why the changes happen;
- Three objects remain mutable resources at the boundary;
- resource cleanup is explicit and scoped;
- the render loop remains imperative at the final edge.

Three's package currently exports both classic WebGL paths and a `./webgpu`
entry. A modern Effect wrapper should avoid hardcoding "WebGL only" into its
public model. It can ship WebGL first, but the renderer service should be an
interface:

```ts
interface ThreeRenderer {
  readonly canvas: HTMLCanvasElement | OffscreenCanvas
  readonly render: (scene: THREE.Scene, camera: THREE.Camera) => void
  readonly dispose: Effect.Effect<void>
}
```

That leaves room for `WebGLRenderer`, `WebGPURenderer`, test renderers,
headless adapters, and worker/offscreen renderers.

## What Effect v4 Adds

Effect v4, as represented by `projects/repos/effect-smol`, changes the analysis
materially. The older `projects/repos/effect` checkout is valuable background,
but v4 has the primitives that make this idea more interesting.

### Core Runtime

`packages/effect/src/Effect.ts` describes lazy workflows with success, error,
and service requirements. Effects compose, run concurrently, acquire resources,
retry, schedule, log, and run through a runtime.

For Three, this means:

- WebGL/WebGPU setup can fail with typed errors.
- Loader errors can be part of the type signature.
- Event handlers can be `Effect`s, not just callbacks.
- Background tasks can be interrupted.
- Tests can run the same program with fake services.

### Scope And Resource Lifetimes

`Scope` finalizers and `Effect.acquireRelease` are a direct match for Three
objects that must be disposed. v4's `Resource` is especially relevant:

- it stores refreshable scoped values;
- refresh replaces old resources and releases the previous value's scope;
- `Resource.auto` can refresh on a schedule.

Possible Three uses:

- refreshable environment maps;
- hot-swapped materials;
- GLTF assets that can be invalidated and reacquired;
- render targets rebuilt on DPR/size changes;
- renderer recreation when context options change.

This is a real advantage over ad hoc loader caches.

### Layer And LayerMap

`Layer` models services and their resource acquisition. `LayerMap` is the v4
primitive that stands out for this project. It caches scoped services selected
by key and releases idle entries.

For Effect Three:

- one root per canvas can be a `LayerMap<HTMLCanvasElement, ThreeRoot>`;
- one asset per URL can be a `LayerMap<AssetKey, AssetContext>`;
- one material/texture decoder family can be keyed by type;
- one offscreen worker per canvas or scene can be keyed and released after idle;
- test roots can swap renderer/event/clock services through layers.

R3F has `_roots: Map<canvas, Root>`. An Effect version can generalize this into
a scoped, typed, idle-releasing `LayerMap`.

### FiberMap And FiberSet

`FiberMap` and `FiberSet` manage background fibers within a scope. They
interrupt children on scope close and remove completed fibers automatically.

For Three:

- keyed asset loads can be canceled/replaced;
- animations can be grouped under scene/object scopes;
- pointer drag tasks can be interrupted on pointer-up/unmount;
- controls can own background tasks;
- workers can be joined or interrupted cleanly;
- per-object behavior tasks can die when the object is removed.

This is where an Effect library can do something R3F does not natively model.
R3F users often write lifecycle-sensitive behavior in hooks. Effect can make
that behavior an owned runtime primitive.

### Queue, PubSub, Stream

`Queue` is for one-consumer async handoff. `PubSub` is for broadcasting to many
subscribers. `Stream` is pull-based and has resource-safe constructors such as
event-listener streams.

For Three:

- invalidation can be a queue or pubsub signal;
- pointer events can be streams;
- frame ticks can be a stream;
- loader progress can be streamed;
- physics/simulation updates can be decoupled from render ticks;
- debug/telemetry events can be broadcast without coupling to render code.

Again, the hot path must be careful. But for orchestration, these primitives are
well matched.

### Browser Runtime And Browser Streams

`@effect/platform-browser` provides `BrowserRuntime.runMain`, which interrupts
the main fiber on `beforeunload`, and `BrowserStream` helpers for window and
document event streams. This is useful infrastructure, but it does not solve
canvas-specific events, pointer capture, raycasting, or renderer lifecycle.
Those remain library responsibilities.

### Atom And AtomRegistry

The v4 `unstable/reactivity` module is the largest new finding. `AtomRegistry`
evaluates atoms, caches values, tracks dependencies, manages subscriptions,
refreshes atoms, disposes unused nodes, and can convert atom values to streams.
`Atom` supports effect-backed values, streams, writable state, refresh, and
idle TTL.

`@effect/atom-react` then adapts this to React through `useSyncExternalStore`,
with hooks such as:

- `useAtomValue`
- `useAtomSet`
- `useAtomRefresh`
- `useAtom`
- `useAtomMount`
- scoped atom providers.

This suggests a strong adapter architecture:

- core Three runtime owns roots, scenes, render loop, events, and resources;
- atom/reactivity exposes state and subscriptions;
- React adapter reads/writes atoms, but React does not own Three object
  lifetimes unless explicitly requested;
- Solid/Vue adapters may be possible because v4 has atom packages for those
  ecosystems too.

This is a different idea than "React Three Fiber but Effect." It is more like
"Effect Three core with optional UI framework adapters."

## Foldkit And OpenAgents Fit

The OpenAgents codebase changes the recommendation in a useful way. We are not
only evaluating a hypothetical Effect user base. We already have real Effect v4
and Foldkit surfaces in the workspace:

- `openagents.com` web is a Foldkit browser app.
- Autopilot Desktop's Electrobun webview is required to be Foldkit and to share
  `@openagentsinc/autopilot-ui` components with the web app.
- Autopilot Remote Control mobile is part of the same product family and shares
  the control protocol, but the current local source is Expo / React Native, not
  a direct Foldkit runtime.

That means a useful Three integration should work in two modes:

1. directly with Foldkit for web and desktop webviews;
2. alongside Foldkit, through a framework-neutral Effect core, for mobile and
   other non-Foldkit renderers.

### What Foldkit Provides

Foldkit is already an Effect-based application runtime. It is not React, and it
does not try to be React. The local `openagents.com` notes describe it as an Elm
Architecture runtime:

- `Model` is the serializable application state, described with Effect Schema;
- `Message` values describe facts that happened;
- `init` and `update` return `[Model, Command[]]`;
- `view(model)` returns `foldkit/html` virtual DOM;
- `Command.define` wraps side effects as Effects that eventually emit Messages;
- `Subscription` turns external streams into Messages;
- `ManagedResource` acquires and releases resources based on model state;
- `Mount` attaches element-lifetime scoped effects to DOM nodes;
- `CustomElement.define` creates typed bindings for web components;
- the runtime records enough history for devtools, story tests, and replay-like
  inspection.

Those primitives map better to an Effect Three runtime than React hooks do in
some ways. A Three root is a resource. Asset loaders are resources. Pointer and
frame notifications are streams. Renderer/context setup can fail. Route changes
should release scenes. Foldkit already has names for those categories.

The catch is that Three's object graph is mutable and GPU-backed. It should not
be placed inside the Foldkit `Model`. Foldkit wants model state to be schema
values that can be inspected, decoded, tested, and replayed. Raw
`THREE.WebGLRenderer`, `THREE.Scene`, `THREE.Texture`, `THREE.Mesh`, and
WebGL/WebGPU handles do not belong there.

The correct boundary is:

- Foldkit owns product UI, route state, commands, subscriptions, and public
  serializable projection state.
- Effect Three owns renderer, scene, camera, GPU resources, asset cache,
  animation loop, raycaster, and hot object graph mutation.
- A small adapter translates between Foldkit Messages/Commands/Subscriptions and
  the Effect Three root.

### Web App Fit

The `openagents.com` web app is the best first integration target. Its entrypoint
already boots with `Runtime.makeProgram`, typed `Flags`, routing callbacks, a
Foldkit `Model`, `update`, `view`, and `subscriptions`. It also has a navigation
invariant: production browser navigation belongs to Foldkit commands such as
`pushUrl`, `replaceUrl`, and `load`, not raw History calls.

An Effect Three feature in that app should therefore look like a Foldkit-managed
island:

- `view` renders a stable canvas host, custom element, or mount point with
  `foldkit/html`.
- `ManagedResource` acquires a `ThreeRoot` when the route/model says the scene is
  active and releases it when the route changes.
- Foldkit `Command`s send typed operations to the root: load scene, select
  object, focus camera, apply preview settings, export snapshot.
- Foldkit `Subscription`s bridge root events back into Messages: object hovered,
  object selected, load progress changed, render fault observed, frame stats
  sampled.
- The Foldkit `Model` stores only serializable state such as selected object ID,
  scene spec ID, camera preset, loading status, error text, and inspector UI
  state.

This gives `openagents.com` a 3D surface without violating the existing Foldkit
architecture. It also preserves devtools usefulness: the Foldkit timeline records
semantic scene messages instead of millions of raw pointer/frame/object updates.

Foldkit's existing `Canvas.view` is not the right implementation vehicle for
Three. It is a pure 2D canvas helper that repaints from declarative shapes in
snabbdom hooks. It is valuable precedent for canvas lifecycle and pointer
coordinate normalization, but Three needs a long-lived renderer and object graph.
The stronger Foldkit hooks are `ManagedResource`, `Mount`, and possibly
`CustomElement`.

### Desktop Fit

Autopilot Desktop is also a strong fit, but the authority boundary matters. The
desktop AGENTS guidance is explicit: the webview UI is Foldkit; the Bun main
process owns the control token, loopback control client, node-home discovery,
polling, and typed RPC bridge. Existing desktop commands wrap `rpc.request.*`
verbs in `Command.define`, and inbound node-state notifications enter Foldkit
through a persistent `Subscription`.

That is almost the same shape an Effect Three adapter should use:

- the webview can host the canvas and inspector through Foldkit;
- Three scene operations can be `Command`s, just like the current RPC verbs;
- inbound Three events can use the same queue-backed subscription pattern as
  Electrobun inbound node-state messages;
- a `ManagedResource` can keep renderer lifetime tied to a view route or panel;
- any file-system, secret, deployment, or node-control authority stays in Bun
  main or Pylon, not in the Three scene.

For desktop, the most plausible use cases are operational visualization rather
than general creative coding: topology views, workroom timelines, agent/session
spatial maps, artifact previews, or local-node status environments. Effect Three
would be useful because those scenes can be driven by typed protocol updates and
released when the operator switches views.

The shared `@openagentsinc/autopilot-ui` package also matters. It already exports
Foldkit HTML components for web and desktop. A Three integration should not
fork the control surface. Inspector panels, toolbars, scene controls, and status
chips should live in the shared UI package if both web and desktop need them.
The actual `ThreeRoot` should live behind the app or a shared runtime package,
because GPU resources and canvas lifetime are not ordinary UI components.

### Mobile Fit

The user's note mentions the mobile app too. The local code needs a precise
statement: Autopilot Remote Control mobile is an Expo / React Native app using
React 19, React Navigation, and `react-native-web`. Its `package.json` does not
depend on Foldkit, and the source imports React Native screens and providers,
not `foldkit/html`.

So the proposed library should not assume Foldkit is the only consumer. Mobile
can still benefit from the same project if the core is framework-neutral:

- shared scene specifications, object IDs, protocol messages, and typed loader
  errors can come from `@effect-three/core` or OpenAgents shared packages;
- the mobile app can consume the same scene/control protocol through React
  Native state and screens;
- a separate mobile graphics adapter could target Expo GL, React Native WebGL
  support, or another native rendering bridge;
- a React adapter can expose atoms/hooks for React Native where appropriate;
- Foldkit-specific helpers should stay in `@effect-three/foldkit`, not in core.

This is exactly why the audit should avoid naming the whole project
`effect-three-fiber`. Foldkit and mobile need the same Effect-owned runtime
model, but not the same UI adapter. Web and desktop want Foldkit bindings;
mobile wants shared domain/runtime logic plus a React Native rendering bridge.

### Proposed Foldkit Adapter

The package split should include a small adapter:

- `@effect-three/core`
  - renderer, root, asset cache, scene operations, frame loop, event streams,
    typed errors, test runtime.
- `@effect-three/web`
  - DOM canvas, pointer source, measurement, resize, browser runtime helpers.
- `@effect-three/foldkit`
  - Foldkit `ManagedResource` helpers for root lifecycle;
  - `Command` helpers for typed scene operations;
  - `Subscription` helpers for root events and sampled render stats;
  - `Mount` or `CustomElement` helpers for canvas host elements;
  - recommended `devTools.excludeFromHistory` presets for high-frequency
    events.
- `@effect-three/react`
  - optional atoms/hooks/provider for React web or React Native surfaces.

The Foldkit adapter should be thin. It should not make Foldkit the scene
reconciler. A conceptual integration looks like this:

```ts
const SceneRoot = ManagedResource.tag<EffectThree.Root>()("SceneRoot")

const managedResources = ManagedResource.make<Model, Message>()(entry => ({
  sceneRoot: entry(S.Option(SceneSpec), {
    resource: SceneRoot,
    modelToMaybeRequirements: model =>
      model.route._tag === "Scene"
        ? Option.some(model.route.sceneSpec)
        : Option.none(),
    acquire: sceneSpec =>
      EffectThree.Root.acquire({ canvasId: "scene", sceneSpec }),
    release: root => root.dispose,
    onAcquired: root => SucceededStartedScene({ rootId: root.id }),
    onAcquireError: error =>
      FailedStartedScene({ reason: String(error) }),
    onReleased: () => CompletedStoppedScene(),
  }),
}))
```

And command/subscription wiring:

```ts
const FocusObject = Command.define(
  "FocusObject",
  { objectId: S.String },
  CompletedFocusObject,
  FailedFocusObject,
)(({ objectId }) =>
  EffectThree.Root.current.pipe(
    Effect.flatMap(root => root.focusObject(objectId)),
    Effect.as(CompletedFocusObject({ objectId })),
    Effect.catchAll(error =>
      Effect.succeed(FailedFocusObject({ objectId, reason: error.message })),
    ),
  ),
)
```

This preserves Foldkit's rule that side effects are Commands and state changes
are Messages, while leaving Three's mutable internals under Effect scopes.

### Foldkit-Specific Risks

The biggest Foldkit risk is event volume. A 3D runtime emits pointer moves,
intersection changes, frame ticks, render stats, asset progress, and possibly
simulation events. Feeding all of that into Foldkit history would make devtools
noisy and could slow the app.

Mitigation:

- keep the hot render loop internal to Effect Three;
- send semantic Messages, not per-object/per-frame mutations;
- sample stats at human rates;
- throttle pointer/intersection streams before they become Messages;
- use Foldkit `devTools.excludeFromHistory` for high-frequency messages;
- keep serializable model snapshots small.

The second risk is dependency topology. OpenAgents web currently pins
`effect@4.0.0-beta.70` and `@effect/platform-browser@4.0.0-beta.70`, while the
local Foldkit reference reports `foldkit@0.111.0` against beta.78 and
`effect-smol` reports beta.83. The web app has existing guardrails around
Effect/Foldkit topology. An Effect Three package used by OpenAgents should
either:

- follow the same workspace/catalog Effect beta line as OpenAgents;
- or isolate integration behind package boundaries so one beta mismatch does not
  create two live Effect runtimes in the browser bundle.

The third risk is authority confusion. OpenAgents UI surfaces are projections.
A 3D scene can visualize sessions, plans, spending, agents, or deployment state,
but it must not become the source of authority for approvals, payout, deploy, or
runtime control. Scene interactions should emit typed intent Messages that go
through the same control protocol and command paths as the rest of the app.

### Foldkit Verdict

Foldkit makes the idea more compelling, not less. It gives us an existing
Effect-native UI runtime with Commands, Subscriptions, managed resources, test
stories, schema-first models, and real OpenAgents adoption across web and
desktop. The right move is not to build "React Three Fiber for Foldkit." The
right move is:

1. build a framework-neutral Effect Three core;
2. add a first-class Foldkit adapter for web and desktop;
3. use OpenAgents web or desktop as the first product-quality integration;
4. keep mobile on the shared core/protocol path through a React Native adapter.

That makes the library fit OpenAgents' current architecture while keeping it
usable outside OpenAgents and outside Foldkit.

## Candidate Library Shapes

### Option A: Thin Effect Wrapper Around Vanilla Three

This is the smallest useful package.

It would expose constructors and helpers such as:

```ts
const program = Effect.scoped(Effect.gen(function*() {
  const renderer = yield* WebGLRendererResource.make({ canvas })
  const scene = yield* SceneResource.make()
  const camera = yield* PerspectiveCameraResource.make({ fov: 75 })

  const mesh = yield* MeshResource.box({
    geometry: { width: 1, height: 1, depth: 1 },
    material: { color: "hotpink" }
  })

  yield* SceneGraph.add(scene, mesh)
  yield* RenderLoop.always(renderer, scene, camera)
}))
```

Pros:

- low risk;
- immediately useful for Effect users;
- proves disposal, asset loading, event streams, and render loop services;
- no need to design a full component model;
- can be tested with fake renderer/canvas services.

Cons:

- not a React Three Fiber competitor;
- still feels imperative;
- cannot declaratively reorder/reconcile large scene trees;
- limited appeal to creative-coding users who want JSX.

Verdict: This should be the first milestone.

### Option B: Effect-Owned Runtime With Scene Operations

Instead of a generic reconciler, expose explicit operations:

```ts
yield* ThreeRoot.add("ship", Mesh.make(...))
yield* ThreeRoot.update("ship", Transform.position([0, 1, 0]))
yield* ThreeRoot.remove("ship")
```

Internally:

- each key maps to an object scope;
- children are linked explicitly;
- updates are batched and flushed before render;
- dispose happens through finalizers;
- events are subscriptions on object scopes;
- frame tasks are fibers under object/root scope.

Pros:

- still much simpler than React reconciliation;
- gives deterministic object identity;
- works naturally with Effect services;
- can support commands from streams, atoms, AI agents, or non-React UI.

Cons:

- users must learn a new scene model;
- no JSX tree ergonomics;
- hard to express deeply nested static scene layouts without builder helpers.

Verdict: This is the best "real" core. It can grow into declarative APIs later.

### Option C: Atom-Backed Scene Model

Use Effect v4 atoms for reactive state and derive scene operations from atoms.

Conceptual example:

```ts
const cameraAtom = Atom.make({ position: [0, 2, 5] })
const selectedObjectAtom = Atom.make(Option.none<string>())
const sceneAtom = Atom.readable((get) => {
  const camera = get(cameraAtom)
  const selected = get(selectedObjectAtom)
  return SceneSpec.make({ camera, selected })
})
```

The runtime subscribes to atoms and applies diffs into Three.

Pros:

- uses a v4-native reactive substrate;
- integrates with React/Solid/Vue through existing atom packages;
- can support server/client hydration concepts;
- could expose asset atoms, frame atoms, pointer atoms, and viewport atoms.

Cons:

- `unstable/reactivity` is explicitly unstable;
- atom invalidation semantics are not the same as scene reconciliation;
- care is needed to avoid atom churn on every frame;
- this can become an accidental state management framework.

Verdict: Good adapter layer, not the first core ownership model.

### Option D: JSX Runtime Without React

Build a JSX runtime that compiles TSX into Effect Three descriptors, then
reconcile descriptors into Three objects.

Pros:

- ergonomic for users familiar with R3F;
- can look like a direct parallel to R3F;
- can support declarative scenes without React.

Cons:

- large implementation burden;
- must design component lifecycle, context, keyed children, effects,
  suspension/loading, error boundaries, scheduling, and refs;
- risk of recreating a weaker React;
- TypeScript JSX typing for the whole Three namespace is nontrivial, though
  R3F's `three-types.ts` provides a good conceptual model.

Verdict: Do not start here. Only pursue if the core runtime gets adoption and
the declarative ergonomics become the bottleneck.

### Option E: React Adapter Over Effect Core

Keep React for UI composition but move Three runtime concerns into Effect.

Concept:

```tsx
function Viewer() {
  const root = useThreeRoot()
  const selected = useAtomValue(selectedObjectAtom)

  useEffect(() => {
    return Runtime.run(root.addMesh({ key: "cube", selected }))
  }, [root, selected])

  return <canvas ref={root.canvasRef} />
}
```

Or a more declarative adapter later:

```tsx
<EffectCanvas layer={AppThreeLayer}>
  <Mesh id="cube" geometry={box()} material={standard({ color })} />
</EffectCanvas>
```

Pros:

- can coexist with existing React apps;
- leverages `@effect/atom-react`;
- easier adoption than a totally new UI runtime;
- React owns app UI, Effect owns Three resources.

Cons:

- if pushed too far, it competes awkwardly with R3F;
- users may ask why they should not just use R3F plus Effect in hooks;
- needs a crisp boundary.

Verdict: Build after core. The adapter should be intentionally narrower than
R3F at first.

## Proposed Architecture

The recommended architecture is an Effect-owned Three runtime with optional
framework adapters.

### Package Layout

Potential package split:

- `@effect-three/core`
  - root services;
  - renderer services;
  - scene graph operations;
  - render loop;
  - asset resources;
  - event streams;
  - disposal helpers;
  - testing runtime.
- `@effect-three/web`
  - DOM canvas integration;
  - `ResizeObserver` or measurement service;
  - pointer event source;
  - browser runtime helpers;
  - offscreen canvas/worker adapter.
- `@effect-three/foldkit`
  - `ManagedResource` integration for root/canvas lifetime;
  - `Command` wrappers for typed scene operations;
  - `Subscription` wrappers for pointer hits, load progress, render faults, and
    sampled stats;
  - `Mount` or `CustomElement` helpers for Foldkit-hosted canvases;
  - devtools guidance for high-frequency messages.
- `@effect-three/react`
  - React provider;
  - atom bindings;
  - optional components;
  - Suspense bridge if needed;
  - React Native or Expo-facing hooks if a mobile graphics adapter is added.
- `@effect-three/loaders`
  - GLTF/texture/audio/font loader resources;
  - progress streams;
  - cache invalidation.
- `@effect-three/test`
  - fake renderer;
  - manual frame driver;
  - event firing;
  - graph snapshots.

Do not start with all of these packages. Start with one package and keep the
boundary clean enough to split later.

### Core Services

The core service set should be small:

```ts
class ThreeRoot extends Context.Service<ThreeRoot>()("@effect-three/ThreeRoot", {
  // conceptual only
}) {}

interface ThreeRoot {
  readonly renderer: ThreeRenderer
  readonly scene: THREE.Scene
  readonly camera: THREE.Camera
  readonly raycaster: THREE.Raycaster
  readonly size: Ref.Ref<Size>
  readonly viewport: Ref.Ref<Viewport>
  readonly invalidate: Effect.Effect<void>
  readonly advance: (timestamp: number) => Effect.Effect<void>
  readonly add: (key: NodeKey, node: ThreeNode) => Effect.Effect<void, ThreeError>
  readonly remove: (key: NodeKey) => Effect.Effect<void>
  readonly events: ThreeEvents
}
```

Do not expose every mutable field as public API. Expose enough to compose:

- root lifecycle;
- renderer lifecycle;
- scene graph changes;
- frame subscriptions;
- event subscriptions;
- asset access;
- test hooks.

### Render Loop

Model the render loop as a scoped service:

- `RenderLoop.always`
- `RenderLoop.demand`
- `RenderLoop.manual`

Internally:

- keep RAF scheduling outside the Effect interpreter when possible;
- use an invalidation flag/queue to avoid repeated scheduling;
- run registered frame callbacks in deterministic priority order;
- let frame callbacks be either raw callbacks for hot paths or `Effect`s for
  managed paths;
- expose manual `advance(timestamp)` for tests.

R3F's `useFrame` is callback based for a reason. An Effect version should offer
both:

```ts
Frame.onFrame((state, delta) => {
  mesh.rotation.y += delta
})

Frame.onFrameEffect(Effect.fn("spin")(function*(state, delta) {
  const config = yield* SpinConfig
  state.mesh.rotation.y += delta * config.speed
}))
```

The second is more powerful but should not be the only path.

### Scene Graph Ownership

Use keyed object scopes:

- each object key owns one closeable scope;
- finalizers dispose object resources unless configured otherwise;
- parent-child links are explicit;
- update operations are batched and flushed;
- reconstruction is explicit when constructor args change.

R3F reconstructs objects when `args` change or primitive object identity
changes. An Effect API can make this less magical:

```ts
yield* root.replace("ship.material", Material.standard({ color: "red" }))
```

or:

```ts
yield* Node.reconstruct("ship", Mesh.box({ args: [2, 2, 2] }))
```

Magic is convenient, but explicit replacement is easier to type, test, and
reason about.

### Props Layer

A real implementation still needs a Three prop application layer. It should be
ported conceptually, not copied wholesale:

- resolve direct and pierced properties;
- support vectors, eulers, colors, matrices, layers, arrays, and scalars;
- preserve shader uniform identity;
- auto-attach geometry/material by default;
- optionally reset removed props;
- set texture color space where appropriate;
- invalidate root after changes;
- register event handlers in the event subsystem.

This can live in `Props.apply(object, patch)`.

The API should avoid ad hoc string intent routing. String paths such as
`"material-color"` are acceptable only as bounded property paths after the user
has explicitly selected the Three prop API. For higher-level scene intent, use
typed operations, not keyword matching.

### Assets

Assets should be one of the main selling points.

Proposed model:

- `AssetKey` encodes loader type, URL(s), extensions, decode options, and
  transform policy.
- `AssetCache` is a `LayerMap<AssetKey, AssetContext>`.
- each asset load is an `Effect.acquireRelease`;
- loaded assets can have disposal finalizers;
- progress is a `Stream`;
- cancellation interrupts the load fiber where the underlying API allows it;
- GLTF results can include a graph of named nodes/materials like R3F's
  `useGraph`.

Possible APIs:

```ts
const gltf = yield* Assets.gltf("/models/ship.glb")
const texture = yield* Assets.texture("/textures/albedo.png")
yield* Assets.invalidate(AssetKey.gltf("/models/ship.glb"))
```

React adapter:

```tsx
const ship = useAtomResult(Assets.gltfAtom("/models/ship.glb"))
```

Core should not depend on React Suspense. React can adapt asset atoms into
Suspense behavior.

### Events

Start with streams:

- `Pointer.raw`
- `Pointer.normalized`
- `Raycaster.intersections`
- `ObjectEvents.pointerDown(key)`
- `ObjectEvents.pointerMove(key)`
- `ObjectEvents.click(key)`

Full event semantics require:

- bubbling through Three parents;
- hover enter/leave;
- stop propagation;
- pointer capture;
- missed clicks;
- layered event priorities;
- portals/multiple roots;
- custom raycast filtering.

R3F already has these details. The Effect version can stage them.

Effect advantage:

- handlers can be effects with typed errors;
- subscriptions are scoped;
- drag gestures can be fibers;
- pointer streams can be throttled/debounced/scheduled;
- tests can push synthetic events without React.

### State And Reactivity

Effect v4 atoms should be treated as a state/reactivity layer, not as the scene
graph itself at first.

Good atom candidates:

- root size;
- DPR;
- viewport;
- selected object;
- hovered object;
- camera state;
- asset result;
- controls state;
- performance regression state;
- render stats;
- user-defined scene state.

Risky atom candidates:

- every transform on every object every frame;
- raw pointer move at device frequency unless carefully buffered;
- per-vertex or per-instance data;
- hot animation state that belongs in a mutable Three buffer.

Rule: use atoms for state that UI, tools, tests, or external systems need to
observe. Use raw Three mutation for hot render data.

### Testing

R3F has a test renderer. Effect Three should have a test runtime from day one.

Capabilities:

- create a fake canvas/root;
- configure fake renderer;
- mount graph operations;
- inspect object tree;
- fire synthetic pointer events;
- manually advance frames;
- assert disposals/finalizers ran;
- test asset cache invalidation;
- test layer substitution;
- use Effect test clock where possible.

This is a major place to outperform vanilla Three and avoid competing directly
with R3F's mature React integration.

## Feasibility Assessment

### Technical Feasibility

Technically feasible.

The core problem is not WebGL. Three already handles that. The problem is
ownership of an imperative object graph. Effect has strong primitives for
ownership, but it does not provide a tree reconciler. If the first target is
resource-safe Three orchestration, feasibility is high. If the first target is
R3F-level declarative ergonomics and ecosystem parity, feasibility is medium to
low and requires a much longer runway.

### Product Feasibility

There are four viable audiences:

1. Effect applications that need Three and want resource safety.
2. Agent/workroom/tooling applications that need programmatic 3D scenes without
   React owning the world.
3. Foldkit applications, including OpenAgents web and desktop, that need a
   managed 3D island without moving raw Three resources into the Foldkit model.
4. React users who already use Effect and want better asset/runtime lifetimes
   than hooks alone.

The less viable audience is the general R3F creative-coding community. They
already have a strong JSX ecosystem, Drei, postprocessing, physics bindings,
examples, and mindshare. An Effect library must offer a different advantage,
not just a different taste.

### Maintenance Feasibility

The maintenance burden is moderate for a core runtime and high for R3F parity.

High-burden areas:

- full Three prop typing;
- pointer event parity;
- asset loader ecosystem;
- React adapter semantics;
- WebXR and WebGPU;
- fast refresh / hot reload;
- mobile/touch edge cases;
- docs and examples.

Lower-burden, high-value areas:

- scoped renderer/resource lifecycle;
- keyed asset cache;
- manual/test render loop;
- basic object graph operations;
- frame callbacks;
- typed loader errors;
- observability hooks.

## Key Risks

### Effect v4 Is Still Beta

`effect-smol` currently reports `effect@4.0.0-beta.83`. The atom/reactivity
exports are under `unstable/reactivity`. Building a public library directly on
unstable atom APIs may create churn.

Mitigation:

- put atom integration behind a small adapter boundary;
- keep core runtime on stable-looking primitives first: `Effect`, `Scope`,
  `Layer`, `Resource`, `FiberMap`, `FiberSet`, `Queue`, `PubSub`, `Stream`;
- version the package as experimental until Effect v4 settles.

### Reconciler Scope Creep

Trying to recreate React host config, Suspense, hooks, context, refs, and
concurrent scheduling is the easiest way to sink the project.

Mitigation:

- first build an imperative Effect runtime;
- add declarative descriptors only after the runtime works;
- keep React adapter optional and honest;
- do not promise R3F parity.

### Frame Loop Overhead

Effect is powerful, but per-frame code can be extremely hot. Running thousands
of tiny Effect programs per frame would be a mistake.

Mitigation:

- keep raw callback APIs for hot frame paths;
- use Effect for lifecycle and orchestration;
- batch scene graph operations;
- benchmark early with realistic animation counts;
- provide guidance: use Effects for IO/control, raw mutation for inner loops.

### Three Prop Compatibility

R3F's `applyProps` exists because Three's API is large and irregular.
Underestimating this will make the library feel bad.

Mitigation:

- start with explicit resource constructors for common objects;
- add a well-tested `Props.apply` layer;
- port concepts from R3F carefully;
- avoid supporting every weird prop in v0;
- document escape hatches.

### Event Semantics

Pointer events over 3D objects are complex. R3F has years of edge cases.

Mitigation:

- start with streams and explicit raycast subscriptions;
- do not initially claim DOM-like event parity;
- add hover/capture/missed semantics incrementally;
- build a test matrix from R3F behavior.

### Ecosystem Gravity

R3F users rely on Drei and related packages. An Effect library will not have
those packages.

Mitigation:

- interoperate with raw Three objects and loaders;
- make it easy to use R3F in React apps and Effect Three elsewhere;
- focus on workroom/tooling/agent/Effect-native use cases first.

## The Important Differentiator

The library should not be marketed as "JSX for Three, but Effect." That invites
comparison on React's home turf.

The differentiator should be:

> Build, run, observe, test, and dispose complex Three.js runtimes as typed
> Effect programs.

That is different enough to matter.

Examples of scenarios where this is better than vanilla Three and not directly
competing with R3F:

- a browser-based simulation where assets, workers, and renderer contexts must
  be torn down deterministically;
- an agent-generated 3D preview where scene changes arrive as typed commands;
- an editor/workroom with separate UI state, object state, selection state, and
  background loaders;
- a Three app that must run both in browser and test/headless modes;
- a multi-root visualization dashboard where inactive roots should release
  assets after idle TTL;
- a WebGPU/WebGL fallback system that exposes typed setup failures.

## Prototype Plan

### Milestone 1: Core Root And Render Loop

Deliverables:

- `ThreeRoot.layer(canvas, options)`;
- WebGL renderer acquisition/release;
- default scene/camera/raycaster;
- `RenderLoop.always`, `RenderLoop.demand`, `RenderLoop.manual`;
- frame subscriptions;
- `invalidate`;
- `advance`;
- fake renderer test runtime.

Acceptance tests:

- creates root and renders once;
- demand mode renders only after invalidation;
- manual mode advances deterministic deltas;
- scope close disposes renderer and stops RAF;
- frame subscriber cleanup runs on scope close.

### Milestone 2: Scene Graph Operations

Deliverables:

- keyed node registry;
- add/remove/reparent;
- attach/detach geometry/material;
- basic `Props.apply` for position, rotation, scale, color, visible, material;
- disposal finalizers;
- object tree inspection.

Acceptance tests:

- add mesh to scene;
- update transform;
- remove mesh disposes geometry/material;
- reparent preserves object identity;
- replacement disposes previous object.

### Milestone 3: Assets

Deliverables:

- `AssetCache` as `LayerMap`;
- texture loader resource;
- GLTF loader resource;
- progress stream;
- cache invalidation;
- graph extraction for named nodes/materials.

Acceptance tests:

- load success/failure typed errors;
- repeated request reuses resource;
- invalidation reloads and disposes previous;
- scope close releases cached assets.

### Milestone 4: Events

Deliverables:

- DOM pointer stream for canvas;
- normalized pointer coordinates;
- raycast intersections;
- object-level pointer down/up/move/click streams;
- scoped subscriptions.

Acceptance tests:

- synthetic pointer event hits mesh;
- unsubscribe removes handler;
- removed object no longer receives events;
- missed click is observable.

### Milestone 5: Foldkit Adapter And OpenAgents Spike

Deliverables:

- Foldkit `ManagedResource` helper for `ThreeRoot` lifecycle;
- Foldkit `Command` helper for scene operations;
- Foldkit `Subscription` helper for root event streams;
- canvas host helper using `foldkit/html` plus `Mount` or `CustomElement`;
- devtools exclusion guidance for high-frequency scene events;
- one OpenAgents web or desktop prototype surface.

Acceptance tests:

- route/model activation acquires a root and route/model exit releases it;
- command failure maps to `Failed*` Message instead of crashing the app;
- root event stream emits semantic Messages and tears down on release;
- Foldkit `Model` remains serializable and does not store raw Three objects;
- web and desktop can share controls through `@openagentsinc/autopilot-ui`.

### Milestone 6: Atom/React Adapter

Deliverables:

- `ThreeRegistryProvider` or root provider;
- atoms for size, viewport, hover, selection, asset results;
- `useThreeRoot`;
- `useFrame` equivalent backed by core runtime;
- optional canvas component.

Acceptance tests:

- React component observes root size atom;
- React cleanup closes root scope;
- asset atom supports waiting/error states;
- frame callback unsubscribes on unmount.

### Milestone 7: Mobile Adapter Spike

Deliverables:

- shared scene/control schema usable from Expo / React Native;
- a minimal React Native or Expo graphics host decision;
- proof that core scene commands and asset/error types can be reused without
  Foldkit;
- clear boundary between mobile UI state and Three runtime state.

Acceptance tests:

- mobile can decode the same scene/control protocol as web/desktop;
- mobile adapter can mount and unmount a root without leaking resources;
- no Foldkit HTML dependency is pulled into the mobile bundle unless a future
  runtime decision explicitly changes the mobile architecture.

## API Sketch

This is intentionally illustrative, not a final design.

```ts
import { BrowserRuntime } from "@effect/platform-browser"
import { Effect, Layer } from "effect"
import * as Three from "@effect-three/core"

const App = Effect.gen(function*() {
  const root = yield* Three.Root

  const cube = yield* Three.Mesh.box({
    key: "cube",
    geometry: { args: [1, 1, 1] },
    material: { color: "hotpink" },
    position: [0, 0, 0]
  })

  yield* root.add(cube)

  yield* Three.Frame.onFrame((state, delta) => {
    cube.object.rotation.y += delta
  })

  yield* Three.Events.onClick("cube", Effect.fn("selectCube")(function*() {
    yield* Effect.logInfo("cube clicked")
  }))
})

const Live = Three.Root.layer({
  canvas: document.querySelector("canvas")!,
  frameloop: "demand"
})

BrowserRuntime.runMain(
  Effect.scoped(App.pipe(Effect.provide(Live)))
)
```

A lower-level version may be clearer:

```ts
const program = Effect.scoped(Effect.gen(function*() {
  const root = yield* Three.createRoot({ canvas })
  const texture = yield* Three.Assets.texture("/albedo.png")
  const mesh = Three.Mesh.create({
    geometry: Three.Geometry.box(1, 1, 1),
    material: Three.Material.standard({ map: texture })
  })

  yield* Three.Root.add(root, "mesh", mesh)
  yield* Three.Root.run(root)
}))
```

React adapter concept:

```tsx
function SceneTools() {
  const selected = useAtomValue(selectionAtom)
  const setSelected = useAtomSet(selectionAtom)
  const stats = useAtomValue(renderStatsAtom)

  return <Inspector selected={selected} stats={stats} onSelect={setSelected} />
}
```

The React adapter should not need to render every Three object as JSX in v0.

## Comparison With Existing Alternatives

### Vanilla Three.js

Vanilla Three is flexible and direct. The cost is lifecycle discipline.
Applications must manually manage:

- renderer/context cleanup;
- asset cancellation and disposal;
- event listeners;
- animation loops;
- resize/DPR updates;
- test doubles;
- background tasks;
- object ownership.

Effect Three can win by making these explicit and scoped.

### React Three Fiber

R3F is ideal when React is the application shell and scene declarativity is the
primary ergonomic need. It already has:

- JSX;
- React scheduling;
- Suspense;
- hooks;
- ecosystem packages;
- mature event semantics;
- broad examples.

Effect Three should not try to displace R3F in React-first creative apps.
Instead it should be useful when Effect is the application runtime, when
resource ownership matters more than JSX, or when React is only one adapter.

### "Just Use Effect In R3F Hooks"

This is a valid competitor to the idea. A React app can already use Effect in
`useEffect`, `useFrame`, loader hooks, and event callbacks.

That approach is enough for many apps.

The separate library becomes justified only if it centralizes lifecycle:

- root scope;
- object scopes;
- asset cache scopes;
- event scopes;
- background fiber scopes;
- test runtime.

If the library is only helper functions for R3F hooks, it should probably be an
R3F utility package rather than a parallel runtime.

## Recommendation

Build a small experimental Effect Three core. Do not build a full R3F parallel
first.

The first public claim should be:

> A resource-safe Effect runtime for Three.js.

Not:

> React Three Fiber without React.

The project should be considered successful if, after the first spike, it can:

- create and dispose a canvas root correctly;
- manage a render loop in always/demand/manual modes;
- add/update/remove keyed objects;
- load and cache a texture or GLTF with scoped cleanup;
- expose pointer intersections as streams;
- run deterministic tests without a browser renderer;
- integrate with one React component through atoms.
- integrate with one Foldkit surface through managed resources, commands, and
  subscriptions.

If that feels good, then it is reasonable to explore declarative scene specs.
If it feels awkward, stop before building a custom reconciler.

## Open Design Questions

1. Should object identity be keyed strings, symbols, branded IDs, or object
   handles?
2. Should scene operations mutate immediately or batch until the next frame?
3. How much of R3F's `applyProps` behavior should be supported in v0?
4. Should materials/geometries be independently scoped resources or owned by
   meshes by default?
5. Should frame callbacks be raw callbacks first, Effect callbacks first, or
   both?
6. How should errors from frame callbacks be reported without killing the
   render loop unexpectedly?
7. Should asset caches be global, per-root, or user-provided layers?
8. Can atom/reactivity remain optional while still being first-class?
9. What is the WebGPU abstraction boundary?
10. Is React adapter support a core goal or a demonstration?
11. Should Foldkit integration live in a public `@effect-three/foldkit` package
    or in an OpenAgents-specific adapter first?
12. Should the first OpenAgents spike target `openagents.com` web or the
    Autopilot Desktop webview?
13. What mobile graphics host is acceptable for Expo / React Native if the core
    proves useful outside Foldkit?

## Suggested Internal Name

Avoid `effect-three-fiber` initially. It overpromises a React Fiber parallel and
invites the wrong comparison.

Better names:

- `effect-three`
- `effect-three-runtime`
- `three-effect`
- `@openagents/effect-three`

If a declarative reconciler is later built, that package can be named more
specifically.

## Bottom Line

The idea is strong if it is reframed.

Effect should not replace React in React Three Fiber. Effect should own the
parts React Three Fiber does not model as strongly: resource lifetimes, typed
errors, structured cancellation, service composition, asset cache invalidation,
test clocks, background work, and cross-framework runtime state.

The path is:

1. Build a small Effect Three runtime.
2. Prove scoped cleanup, render-loop control, asset cache behavior, and tests.
3. Add a Foldkit adapter and validate it in OpenAgents web or desktop.
4. Add atom/reactivity and React/React Native adapter support where it pays for
   itself.
5. Only then consider a declarative tree DSL or JSX runtime.

That gives the project a differentiated reason to exist and avoids spending
the first phase rebuilding a weaker React.
