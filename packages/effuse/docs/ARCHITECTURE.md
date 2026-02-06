# Effuse Architecture

Deep dive into Effuse's internals for understanding or extending the framework.

Effuse is an Effect-native UI runtime with two primitives: components and
hypermedia actions. Effuse intentionally avoids a Virtual DOM. Rendering is done
via string templates and DOM swap modes.

## Design Principles

1. **Effect-Native** - All UI behavior is expressed as Effects and Streams.
2. **Type-Safe** - Strong typing for state, events, actions, and service dependencies.
3. **Reactive** - State changes and action completions drive DOM updates.
4. **Testable** - Services are mockable; DOM is optional; runtimes are layer-provided.
5. **Minimal** - No virtual DOM, no diffing. Updates are explicit DOM swaps (default `inner`).
6. **Locality** - Prefer targeted updates to specific elements over re-rendering entire screens.
7. **Progressive Enhancement** - HTML carries intent via attributes; runtime interprets it.

## The Two Primitives

Effuse intentionally stays small by providing two complementary UI primitives:

### 1) Components

Components own local state (`StateCell`) and define a `render()` function. On
state change, Effuse re-renders the component and swaps the result into the
component container.

Use components when:
- You need durable local state and a clear state machine.
- You need subscriptions (Streams) driving UI updates.
- You want a testable unit with explicit `handleEvent`.

### 2) Hypermedia Actions (HTMX-inspired)

Hypermedia actions let HTML declare intent using `data-ez-*` attributes. The
runtime intercepts DOM events, runs a registered Effect action, and swaps the
result into a target.

Use hypermedia actions when:
- You want small, localized updates (forms, toggles, list row updates).
- You want minimal wiring (no event union / no component queue).
- You want progressive enhancement: HTML expresses the interaction directly.

## High-Level Data Flow

```
Component path:
StateCell -> Stream -> render() -> TemplateResult -> DomService.swap()

Action path:
DOM event -> EzRuntime -> EzRegistry(action) -> Effect -> TemplateResult -> DomService.swap()
```

## Component Lifecycle

```
1. MOUNT
   └─> Create StateCell with initialState
   └─> Create event Queue
   └─> Build ComponentContext
   └─> Initial render()
   └─> dom.swap(container, content, "inner")  // focus-safe swap
   └─> setupEvents() if defined (delegated listeners)
   └─> Fork state.changes watcher (re-renders)
   └─> Fork eventQueue handler (handleEvent)
   └─> Fork subscriptions (external streams)

2. RUNTIME (until scope closes)
   └─> User action → Event dispatched to queue
   └─> handleEvent processes event
   └─> handleEvent calls state.update()
   └─> StateCell publishes to changes stream
   └─> Re-render fiber calls render()
   └─> dom.swap(container, content, "inner")

3. UNMOUNT (scope closes)
   └─> All forked fibers interrupted
   └─> Event queue shutdown
   └─> StateCell queue shutdown
   └─> Cleanup effects run (finalizers)
```

## Hypermedia Actions Lifecycle

Hypermedia actions are a small runtime that interprets `data-ez-*` attributes
and performs targeted swaps.

```
1. MOUNT RUNTIME (once per root)
   └─> Register delegated listeners for supported triggers (click/submit/change/input)
   └─> Cancel previous in-flight action per element (latest wins)

2. TRIGGER
   └─> DOM event occurs
   └─> Find closest element with `data-ez`
   └─> Parse attributes:
       - trigger (default based on element type)
       - target (this | closest(...) | find(...) | selector)
       - swap mode (inner/outer/beforeend/afterbegin/delete/replace)
   └─> Collect params (form serialization + data-ez-vals)
   └─> Lookup action handler in EzRegistry
   └─> Run action Effect (forked fiber)

3. SWAP
   └─> If action returns TemplateResult:
       dom.swap(target, content, mode)  // focus-safe swap
   └─> If action returns void:
       no DOM update (side effects only)
   └─> (optional) render error to error target (future / phase 2)

4. UNMOUNT
   └─> Interrupt fibers
   └─> Remove delegated listeners
```

## StateCell Semantics

StateCell provides:
- current value (`Ref`)
- update API (`set` / `update`)
- change notifications (`Queue` -> `Stream`)

StateCell is used by components directly, and may also be used by actions or
services that need reactive state.

Under the hood, StateCell uses Effect.Ref for the value and Queue for change
notifications:

```typescript
export const makeCell = <A>(initial: A): Effect.Effect<StateCell<A>, never, Scope.Scope> =>
  Effect.gen(function* () {
    const ref = yield* Ref.make(initial)
    const queue = yield* Queue.unbounded<A>()

    // Shutdown queue when scope closes
    yield* Effect.addFinalizer(() => Queue.shutdown(queue))

    const cell: StateCell<A> = {
      get: Ref.get(ref),

      set: (value: A) =>
        Effect.gen(function* () {
          yield* Ref.set(ref, value)
          yield* Queue.offer(queue, value)
        }),

      update: (f: (current: A) => A) =>
        Effect.gen(function* () {
          const newValue = yield* Ref.updateAndGet(ref, f)
          yield* Queue.offer(queue, newValue)
        }),

      changes: Stream.fromQueue(queue),
    }

    return cell
  })
```

**Key insight:** `changes` is a Stream created from the Queue. When `set` or `update` is called, the new value is offered to the queue, which emits on the stream, triggering re-render.

## DOM Rendering Model

Effuse does not implement a VDOM or diff algorithm. Instead, rendering produces
HTML templates that are swapped into the DOM using `DomService.swap`. Swap modes
enable targeted updates (for example, replacing a list row instead of an entire
screen), and the browser implementation preserves focus/selection where
possible.

Effuse uses string templates and **DOM swaps**. There are two paths:

1. **Components**: `render()` returns a complete HTML string, and
   `dom.render(container, content)` swaps the container **inner** HTML (same as
   `dom.swap(container, content, "inner")`).
2. **Hypermedia actions**: return a `TemplateResult` and the runtime applies
   `dom.swap(target, content, mode)` with a targeted swap mode.

**Implication:** Component re-renders replace all child elements in the mounted
container. Use hypermedia actions (targeted swaps) or structural patterns below
to avoid wiping child component DOM.

**UX note:** `DomServiceLive.swap` restores focus/selection after `inner`, `outer`,
or `replace` swaps when possible to avoid cursor jumps in forms.

**Swap note:** `replace` is currently an alias of `outer` (both assign
`outerHTML`).

## Hypermedia Actions Runtime

Effuse includes an HTMX-inspired runtime for **targeted DOM updates** without a
virtual DOM. Instead of re-rendering entire component containers, actions run
Effects and swap the resulting HTML into a specific target element.

**Core flow:**

1. `mountEzRuntime(root)` installs delegated listeners for `[data-ez]`
2. On trigger, the runtime resolves:
   - **Action** (`data-ez`)
   - **Trigger** (`data-ez-trigger`)
   - **Target** (`data-ez-target`)
   - **Swap mode** (`data-ez-swap`)
   - **Params** (`data-ez-vals`, form serialization)
3. The action runs as an Effect, returning a `TemplateResult` or `void`
4. If a template is returned, the runtime calls `dom.swap(...)` with the mode

This enables partial updates inside a component without the overhead or
complexity of a virtual DOM, while keeping a declarative HTML surface.

**Concurrency:** Current policy is switch-latest per element (previous action
fiber is interrupted when a new trigger fires on the same element).

## Mount Process

The `mountComponent` function orchestrates the component lifecycle:

```typescript
export const mountComponent = <S, E, R>(
  component: Component<S, E, R>,
  container: Element
): Effect.Effect<MountedComponent, never, R | DomServiceTag | StateServiceTag | Scope.Scope>
```

**Step-by-step:**

1. **Get services** from Effect context:
   ```typescript
   const dom = yield* DomServiceTag
   const stateService = yield* StateServiceTag
   ```

2. **Create state cell** (scoped to current Effect scope):
   ```typescript
   const state = yield* stateService.cell(component.initialState())
   ```

3. **Create event queue** with cleanup:
   ```typescript
   const eventQueue = yield* Effect.acquireRelease(
     Queue.unbounded<E>(),
     (queue) => Queue.shutdown(queue)
   )
   ```

4. **Build context**:
   ```typescript
   const ctx: ComponentContext<S, E> = {
     state,
     emit: (event) => Queue.offer(eventQueue, event),
     dom,
     container,
   }
   ```

5. **Initial render**:
   ```typescript
   const initialContent = yield* component.render(ctx)
   yield* dom.render(container, initialContent)
   ```

6. **Set up events** (delegated listeners):
   ```typescript
   if (component.setupEvents) {
     yield* component.setupEvents(ctx)
   }
   ```

7. **Fork re-render fiber** (runs until scope closes):
   ```typescript
   yield* pipe(
     state.changes,
     Stream.tap(() =>
       Effect.gen(function* () {
         const content = yield* component.render(ctx)
         yield* dom.render(container, content)
       })
     ),
     Stream.runDrain,
     Effect.forkScoped
   )
   ```

8. **Fork event handler** (runs until scope closes):
   ```typescript
   if (component.handleEvent) {
     yield* pipe(
       Stream.fromQueue(eventQueue),
       Stream.tap((event) => component.handleEvent!(event, ctx)),
       Stream.runDrain,
       Effect.forkScoped
     )
   }
   ```

9. **Fork subscriptions** (external streams):
   ```typescript
   if (component.subscriptions) {
     for (const sub of component.subscriptions(ctx)) {
       yield* pipe(
         sub,
         Stream.tap((effect) => effect),
         Stream.runDrain,
         Effect.forkScoped
       )
     }
   }
   ```

## Service Architecture

### DomService

Type-safe DOM operations:

```typescript
interface DomService {
  query: (selector: string) => Effect.Effect<Element, DomError>
  queryOption: (selector: string) => Effect.Effect<Element | null, DomError>
  queryAll: (selector: string) => Effect.Effect<readonly Element[], DomError>
  render: (container: Element, content: TemplateResult) => Effect.Effect<void, DomError>
  swap: (
    target: Element,
    content: TemplateResult,
    mode?: DomSwapMode
  ) => Effect.Effect<void, DomError>
  delegate: (
    container: Element,
    selector: string,
    event: string,
    handler: (e: Event, target: Element) => void
  ) => Effect.Effect<void, DomError>
}
```

### StateService

Creates StateCell instances:

```typescript
interface StateService {
  cell: <A>(initial: A) => Effect.Effect<StateCell<A>, never, Scope.Scope>
}
```

### EzRegistry

Hypermedia action registry:

```typescript
type EzRegistry = Map<string, EzAction>
```

## Parent/Child Component Relationships

**Problem:** Re-rendering a parent component wipes out child component DOM because component renders use an `inner` swap.

**Solutions:**

### Option 1: Direct DOM Manipulation

Don't re-render containers that hold child components - update them directly:

```typescript
// In handleEvent for tab switching:
yield* ctx.state.update((s) => ({ ...s, activeTab: event.tab }))

// Directly update tab container visibility (avoids wiping child components)
for (const tabId of TABS) {
  const container = yield* ctx.dom.queryOption(`#tab-${tabId}`)
  if (container) {
    container.classList.toggle("hidden", tabId !== event.tab)
  }
}
```

### Option 2: Restructure Containers

Don't render child containers in parent - create them once in HTML or mount separately:

```typescript
// In HTML (index.html)
<div id="parent-component">
  <div id="child-container-1"></div>
  <div id="child-container-2"></div>
</div>

// Parent component only renders its own UI, not child containers
const ParentComponent: Component<ParentState, ParentEvent> = {
  render: (ctx) =>
    Effect.gen(function* () {
      // Only render parent's UI (sidebar, header, etc.)
      // Child containers exist in HTML, not in render output
      return html`<div>Parent UI only</div>`
    }),
}
```

### Option 3: Conditional Re-rendering

Only re-render parts that don't contain child components:

```typescript
// Split render into parts that can be safely re-rendered
// vs. parts that contain child components (render once, update classes only)
```

## Hot Module Replacement (HMR)

HMR is not implemented in the current Effuse runtime. See `docs/effuse/ROADMAP.md`
for planned HMR support.

## Extending Effuse

### Adding a New Component

1. Create component file (for example in `src/components/`)
2. Define state and event types
3. Implement Component interface
4. Mount in `src/main.ts` or from another component
5. Add tests under `tests/` (see `docs/effuse/TESTING.md`)

### Adding a New Service

1. Create interface in `src/effuse/services/my-service.ts`
2. Create Context.Tag
3. Create live implementation in `my-service-live.ts`
4. Add to layers in `layers/live.ts`
5. Export from `src/effuse/index.ts`
6. Add tests under `tests/` (see `docs/effuse/TESTING.md`)

### Modifying StateCell

Be careful - StateCell is core infrastructure. Changes affect all components. Key files:
- `src/effuse/state/cell.ts`
- `src/effuse/services/state.ts`
- `src/effuse/services/state-live.ts`

## File Structure

```
src/effuse/
├── index.ts                 # Public barrel export
├── component/
│   ├── types.ts              # Component interface
│   └── mount.ts              # mountComponent helpers
├── ez/
│   ├── types.ts              # Hypermedia action types
│   ├── registry.ts           # EzRegistryTag + helpers
│   └── runtime.ts            # mountEzRuntime
├── layers/
│   └── live.ts               # EffuseLive layer
├── services/
│   ├── dom.ts               # DomService interface
│   ├── dom-live.ts          # Browser implementation
│   ├── state.ts             # StateService interface
│   └── state-live.ts        # Effect.Ref implementation
├── state/
│   └── cell.ts              # StateCell<A> implementation
└── template/
    ├── html.ts              # html`` tagged template
    ├── types.ts             # TemplateResult types
    └── escape.ts            # HTML escaping
```

## Documentation Contracts

Docs are enforced by contract tests in `tests/`:

- `tests/ez-runtime.test.ts` (Ez runtime events + params)

More tests (DomService swap behavior, component mount behavior) are planned.
