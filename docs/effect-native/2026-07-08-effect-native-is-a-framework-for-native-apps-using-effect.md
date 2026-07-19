# Effect Native — A Framework for Building Native Applications Using Effect

Date: 2026-07-08
Status: framing decision (owner-directed). This is the definitive statement
of what Effect Native *is*, correcting the earlier, too-shallow "UI library /
UI substrate" language. Grounded in a deep read of Effect v4
(`projects/repos/effect-smol`, `effect@4.0.0-beta.94`) and React Native
(`projects/repos/react-native`) on 2026-07-08. It sits above the other docs
in this folder and recontextualizes them.

## 1. The one-line definition

React Native describes itself as **"a framework for building native
applications using React."** Effect Native is the exact same sentence with
one word changed:

> **Effect Native is a framework for building native applications using
> Effect.**

That is not wordplay. It is a precise architectural claim: where React
Native takes React — a model for describing UI and state — and runs it
against native platforms, Effect Native takes **Effect** — a model for
describing *entire programs* (state, effects, concurrency, data, services,
resource lifetimes, and UI) — and runs it against native platforms. The UI
is one part of an Effect Native application. The rest of the app *is Effect*.

The base is **Effect v4** — the `effect-smol` rewrite (`4.0.0-beta.94`): a
re-architected fiber runtime (~6.3 KB core, ~15 KB with Schema), everything
consolidated into a single `effect` package, and an `unstable/*` module
system. We build on v4, not v3.

## 2. The parallel, precisely

React Native is "React + a native host." React supplies the app-authoring
model (components, hooks, state, context, the reconciler). A per-platform
host (Fabric on iOS/Android) turns that into native views. Effect Native is
"Effect + a native host," and Effect supplies a *far larger* app-authoring
model. Role for role:

| Role | React Native | Effect Native |
|---|---|---|
| **App-authoring model** | React (components, hooks, JSX) | **Effect** (effects as values, `Effect.gen`) |
| **Execution engine** | React Fiber reconciler + scheduler | **Effect fiber runtime** (`Fiber`, `Scheduler`) — inert descriptions the runtime commits |
| **Concurrency** | React concurrent mode (cooperative) | **Structured concurrency** — fibers, supervision, interruption, `Exit` |
| **State** | `useState` / component-local | **`Ref` / `SubscriptionRef` / `Atom`** — fiber-safe, observable |
| **Events / reactive data** | callbacks, event props | **`Stream`** (+ `PubSub`, `Queue`) |
| **Dependency wiring** | React Context / prop-drilling | **`Layer` + `Context.Service`** — a real DI graph, memoized, scoped |
| **Typed data / boundaries** | PropTypes/TS (erasable) | **`Schema`** in core — decode/encode/validate every boundary |
| **Errors** | throw / try-catch / error boundaries | **typed errors in the type** (`Schema.TaggedError`), never thrown |
| **Resource lifetime** | `useEffect` cleanup | **`Scope` / `acquireRelease`** — guaranteed cleanup on `Exit` |
| **Platform host** | Fabric (iOS/Android native) | **`platform-native`** adapter (`NativeRuntime.runMain` + native service Layers) — mirrors `platform-browser`/`-node`/`-bun` |
| **Native painting** | Fabric mounting → UIView/View | **renderer adapters** (RN/Fabric now, Swift/Compose later, DOM on web, canvas via three-effect) |
| **UI description** | JSX component tree | **typed component set** (Schema-typed catalog + typed intents) — the view layer of the Effect app |

Read the table top to bottom and the point lands: React gives React Native a
UI model and a reconciler. Effect gives Effect Native a *whole application
runtime* — concurrency, DI, typed errors, resource safety, streams, schema —
and the UI sits on top of it as one concern among several.

## 3. Why this is a *stronger* foundation than React Native's

React was a UI library that later grew an application model (hooks, context,
state) as an afterthought — and it shows: no typed errors, no dependency
injection, no structured concurrency, no resource-lifetime guarantees, no
built-in typed-data boundary. React Native inherits exactly those gaps at the
app layer, which is why its ecosystem bolts on state libraries, data-fetching
libraries, DI conventions, and error handling per project — the "React slop on
React slop" the Effect Native decision exists to escape.

Effect is the opposite: it was *built* as the application foundation. So
Effect Native inherits, for free, the things RN apps assemble by hand:

- **The runtime is a reconciler-grade scheduler already.** Effects are inert
  descriptions. The fiber runtime commits them, schedules cooperatively
  (`Scheduler.shouldYield`), and enforces structured concurrency and
  interruption. This is the same "declare a description, the runtime commits
  it" mental model as React — we already own the engine.
- **The app is *wired*, not prop-drilled.** `Layer` + `Context.Service` is a
  memoized, scoped dependency graph. An Effect Native app is assembled by
  composing Layers into one root and handing it to the native `runMain` — the
  direct analog of mounting a React root, but for the *whole* app.
- **Every boundary is typed and validated with Schema.** Component props,
  style tokens, the native bridge, persisted state, network payloads — one
  tool (`Schema`, now in core) decodes/encodes/validates them all. This is
  the typed spine that makes 1000 agent edits/day safe.
- **Errors, resources, and state are handled by construction** — typed
  errors, guaranteed cleanup via `Scope`, fiber-safe observable state via
  `SubscriptionRef`/`Atom`. RN leaves all three to the app author.

So "Effect Native" is not "React Native but with Effect sprinkled on." It is a
native application framework whose foundation is categorically more capable
than React Native's — which is the entire reason to build it.

## 4. The technical seam: Effect Native is a platform host adapter

The reframe is not aspirational — Effect already has the exact extension point.
Effect v4's platform packages follow one pattern: **core defines abstract
platform services** (`FileSystem`, `Path`, `Socket`, `Terminal`, HTTP, workers
— as `Layer` requirements), and **a host package provides concrete Layers plus
a `runMain`**:

- `@effect/platform-node` → `NodeRuntime.runMain` + `NodeFileSystem`,
  `NodeHttpServer`, `NodeWorker`, … (SIGINT/SIGTERM interruption).
- `@effect/platform-browser` → `BrowserRuntime.runMain` (interrupts on
  `beforeunload`) + `BrowserHttpClient`, `Geolocation`, `Clipboard`,
  `Permissions`, `IndexedDb`, …
- `@effect/platform-bun` → the Bun equivalents.

**Effect Native is simply the next member of that family: a `platform-native`
adapter** — `NativeRuntime.runMain` for the mobile/native process, native
`FileSystem`/`Path`/`Socket`/secure-storage/notification Layers, and the UI
bridge — plus the renderer(s) that paint the view. That is *exactly* how
React Native relates to React (a host under a shared model), except the shared
model is Effect and the host is ours to write. The pattern is proven three
times over in the repo. We are extending it, not inventing it.

Two pieces already exist that a native renderer consumes:
- **Reactive state → view binding** lives in `effect/unstable/reactivity`
  (`Atom`, `AtomRegistry`, `Reactivity`) and ships as framework bindings today
  (`@effect/atom-react` React hooks, plus Solid and Vue). The pattern for
  "observe Effect state, drive a view" is established. Effect Native adds the
  *native* binding.
- **The runtime entry** (`Effect.runFork`, `makeRunMain`, `ManagedRuntime`
  for foreign call sites) is the seam a native host wraps — the same seam
  `BrowserRuntime`/`NodeRuntime` wrap.

## 5. What an "Effect Native app" is, end to end

One Effect program, run by a native host, painted by renderers:

- **Services** (`Layer` + `Context.Service`) wire the app — the sync client,
  the auth service, the credits client, the CRM client — provided once,
  memoized, scoped.
- **State** lives in `Ref` / `SubscriptionRef` / `Atom` (fiber-safe,
  observable), bound from Khala Sync.
- **Logic and effects** are `Effect.gen` blocks with typed errors and
  structured concurrency — no throwing, no untyped async.
- **Data** at every boundary is `Schema` — decoded from the wire, validated,
  encoded for the native bridge.
- **Events** flow through `Stream` / `PubSub`.
- **The UI** is the typed component set (this folder's other docs): a
  Schema-typed catalog with typed intents (not callbacks), a deterministic
  typed-object styling model, rendered by swappable adapters.
- **Resource lifetimes** are `Scope`-managed. The native `runMain` interrupts
  the root fiber on process teardown, and everything cleans up on `Exit`.

The UI is the last bullet, not the first — because Effect Native is an
*application* framework, and the UI is one of its concerns.

## 6. How this recontextualizes the other docs

- **The main analysis + EN-0…EN-9 roadmap** describes the *UI/renderer layer*
  of this framework — the typed component set and the renderer adapters. It is
  correct. It is just one layer of the whole, and its "runtime" is Effect's
  runtime, its typed contracts are Schema.
- **Foldkit** is the closest prior art: a *whole-app* Effect framework
  (Elm/MVU) — the "framework, not library" instinct, but web-only and
  all-or-nothing. We take its lessons (interactions-as-data, effect taxonomy,
  Ports) into Effect Native's app + UI layers.
- **React Native** is the *renderer* we borrow (Fabric/Yoga) — and the model
  we replace. The positive half of "use the renderer, leave the model" is:
  **Effect is our React.** The app is authored in Effect. RN only paints.
- **three-effect** is a *renderer* (canvas) folding under this framework, and
  a domain library it draws from.
- **Styling** (Tailwind/StyleX) is one typed boundary among many — styles as
  Schema-typed values, lowered per renderer.

## 7. Honest caveats

> **Implementation status (2026-07-14):** the July 8 caveats are reconciled
> below. The owned framework has since shipped a v39 Schema catalog,
> React-free direct DOM renderer, React-backed React Native renderer,
> `ViewProgram`, host-driver, token/style, mobile, and Electron host paths. A
> fully native Swift/Compose `platform-native` target remains future work. The
> remaining web-specific gap is first-class React DOM/SSR integration under
> the same contract. See the
> [React web renderer harmonization analysis](./2026-07-14-react-web-renderer-harmonization-gap-analysis.md).

- **A fully native `platform-native` does not exist yet.** The platform-adapter
  pattern is proven and the React Native/Expo and Electron/DOM host paths now
  ship, but pure Swift/Compose host Layers and native
  reactive-state→view/process bindings remain net-new work.
- **The Effect→View binding now exists. First-class React DOM does not.**
  `ViewProgram` maps typed state into a renderer-neutral View stream consumed
  by the direct DOM and React Native renderers. A concurrency-safe synchronous
  View source plus React DOM SSR/hydration surface is the remaining web gap,
  not permission to move application state into hooks.
- **v4 is beta.** We pin `effect-smol@4.0.0-beta.94` and track it. The core
  programming model (Effect/Layer/Schema/Stream) is stable per the v4 migration
  notes, but the `unstable/*` surface (including reactivity) can move — expect
  it and pin hard.
- **Ambition vs. shipping.** This framing is the *what it is*, not authority
  to expand closed product scope. Build renderer and host capabilities only
  when a retained Desktop, mobile, or web surface pulls them, and require a
  bounded cross-renderer receipt rather than speculative framework breadth.

## 8. The corrected description (use this)

Wherever Effect Native was called a "UI library," "UI framework," or "UI
substrate," use instead:

> **Effect Native — a framework for building native applications using
> Effect.**

The UI/component substrate is a *part* of Effect Native, not the whole of it.
The whole of it is: build your entire application — services, state, effects,
concurrency, typed data, and UI — as one Effect program, and run it natively
on any platform.
