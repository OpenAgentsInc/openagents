# Effuse: Effect-Native UI Framework

Effuse is OpenAgents' custom, lightweight UI framework built natively on Effect TypeScript. It powers the mainview desktop HUD with type-safe, reactive, testable components.

**TL;DR for coding agents:**
- Components are Effect-native UI elements with typed state, events, and services
- Use `html` tagged templates with automatic XSS escaping
- State is reactive via `StateCell<A>` (built on Effect.Ref + Queue)
- Test with `makeTestLayer()` or `makeHappyDomLayer()` for real DOM
- Entry point: `src/effuse/index.ts` exports everything

---

## Quick Start

### Creating a Component

```typescript
import { Effect } from "effect"
import { html, type Component } from "../effuse/index.js"

// 1. Define state type
interface CounterState {
  count: number
}

// 2. Define event type (discriminated union)
type CounterEvent =
  | { type: "increment" }
  | { type: "decrement" }

// 3. Create the component
const CounterComponent: Component<CounterState, CounterEvent> = {
  id: "counter",

  initialState: () => ({ count: 0 }),

  render: (ctx) =>
    Effect.gen(function* () {
      const { count } = yield* ctx.state.get
      return html`
        <div class="counter">
          <span>Count: ${count}</span>
          <button data-action="decrement">-</button>
          <button data-action="increment">+</button>
        </div>
      `
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      yield* ctx.dom.delegate(ctx.container, "[data-action]", "click", (_e, target) => {
        const action = (target as HTMLElement).dataset.action
        if (action === "increment") Effect.runFork(ctx.emit({ type: "increment" }))
        if (action === "decrement") Effect.runFork(ctx.emit({ type: "decrement" }))
      })
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      switch (event.type) {
        case "increment":
          yield* ctx.state.update(s => ({ count: s.count + 1 }))
          break
        case "decrement":
          yield* ctx.state.update(s => ({ count: s.count - 1 }))
          break
      }
    }),
}
```

### Mounting a Component

```typescript
import { Effect } from "effect"
import { mountComponent, EffuseLive } from "../effuse/index.js"

const program = Effect.gen(function* () {
  const container = document.getElementById("my-component")!
  yield* mountComponent(MyComponent, container)
})

Effect.runPromise(
  program.pipe(
    Effect.provide(EffuseLive),
    Effect.scoped
  )
)
```

### Testing a Component

```typescript
import { describe, test, expect } from "bun:test"
import { Effect } from "effect"
import { makeTestLayer } from "../effuse/layers/test.js"
import { mountComponent } from "../effuse/component/mount.js"
import { MyComponent } from "./my-component.js"

test("renders initial state", async () => {
  await Effect.runPromise(
    Effect.scoped(
      Effect.gen(function* () {
        const { layer, getRendered } = yield* makeTestLayer()
        const container = { id: "test" } as Element

        yield* mountComponent(MyComponent, container).pipe(Effect.provide(layer))

        const html = yield* getRendered(container)
        expect(html).toContain("expected content")
      })
    )
  )
})
```

---

## Core Concepts

### 1. Component Interface

A component has five key properties:

| Property | Required | Description |
|----------|----------|-------------|
| `id` | Yes | Unique identifier for debugging |
| `initialState` | Yes | Factory returning initial state |
| `render` | Yes | Effect producing TemplateResult from state |
| `handleEvent` | No | Effect handling emitted events |
| `setupEvents` | No | Set up DOM event listeners after mount |
| `subscriptions` | No | Streams for external data (socket messages) |

**Type signature:**
```typescript
interface Component<S, E, R = never> {
  id: string
  initialState: () => S
  render: (ctx: ComponentContext<S, E>) => Effect.Effect<TemplateResult, never, R>
  handleEvent?: (event: E, ctx: ComponentContext<S, E>) => Effect.Effect<void, never, R>
  setupEvents?: (ctx: ComponentContext<S, E>) => Effect.Effect<void, never, R>
  subscriptions?: (ctx: ComponentContext<S, E>) => Stream.Stream<Effect.Effect<void, never, R>, never, R>[]
}
```

### 2. ComponentContext

Passed to all component methods:

```typescript
interface ComponentContext<S, E> {
  readonly state: StateCell<S>      // Reactive state
  readonly emit: (event: E) => Effect<void>  // Emit events
  readonly dom: DomService          // DOM operations
  readonly container: Element       // Mounted container
}
```

### 3. StateCell<A>

Reactive state primitive:

```typescript
interface StateCell<A> {
  get: Effect<A>                    // Read current value
  set: (value: A) => Effect<void>   // Replace value
  update: (f: A => A) => Effect<void>  // Transform value
  changes: Stream<A>                // Stream of updates
}
```

State changes automatically trigger re-renders.

### 4. Template System

The `html` tagged template literal provides XSS-safe HTML:

```typescript
import { html } from "../effuse/index.js"

// Automatic escaping
const userInput = "<script>alert('xss')</script>"
html`<div>${userInput}</div>`
// Output: <div>&lt;script&gt;alert('xss')&lt;/script&gt;</div>

// Nested templates (no double-escaping)
const items = ['a', 'b', 'c']
html`<ul>${items.map(i => html`<li>${i}</li>`)}</ul>`
// Output: <ul><li>a</li><li>b</li><li>c</li></ul>

// Conditionals
html`${state.expanded ? html`<details>...</details>` : ""}`
```

### 5. Services

Three core Effect services:

| Service | Tag | Purpose |
|---------|-----|---------|
| `DomService` | `DomServiceTag` | Type-safe DOM queries, rendering, events |
| `StateService` | `StateServiceTag` | Creates StateCell instances |
| `SocketService` | `SocketServiceTag` | Desktop server communication |

### 6. Layers

Pre-configured Effect layers:

```typescript
// Production (browser + socket)
import { EffuseLive } from "../effuse/index.js"

// Production without socket (UI-only testing)
import { EffuseLiveNoSocket } from "../effuse/index.js"

// Testing with mocks
import { makeTestLayer } from "../effuse/layers/test.js"

// Testing with real DOM (Happy-DOM)
import { makeHappyDomLayer } from "../effuse/testing/layers/happy-dom.js"
```

---

## File Structure

```
src/effuse/
├── index.ts                 # Public barrel export
├── services/
│   ├── dom.ts               # DomService interface
│   ├── dom-live.ts          # Browser implementation
│   ├── state.ts             # StateService interface
│   ├── state-live.ts        # Effect.Ref implementation
│   ├── socket.ts            # SocketService interface
│   └── socket-live.ts       # Desktop socket client
├── state/
│   └── cell.ts              # StateCell<A> implementation
├── template/
│   ├── html.ts              # html`` tagged template
│   ├── types.ts             # TemplateResult types
│   └── escape.ts            # HTML escaping
├── component/
│   ├── types.ts             # Component interface
│   └── mount.ts             # mountComponent helpers
├── layers/
│   ├── live.ts              # EffuseLive, EffuseLiveNoSocket
│   └── test.ts              # makeTestLayer, makeCustomTestLayer
├── components/              # Implemented components
│   ├── apm-component.ts      # APM monitor
│   ├── tb-controls.ts       # TerminalBench controls
│   ├── mc-tasks.ts          # MechaCoder tasks
│   └── ...                  # Other components
└── testing/                 # Test infrastructure
    ├── harness.ts           # TestHarness, ComponentHandle
    ├── browser.ts           # TestBrowser service
    └── layers/
        └── happy-dom.ts     # Real DOM testing
```

---

## Key Patterns

### Event Delegation

Use `data-action` attributes and delegated handlers:

```typescript
render: (ctx) =>
  Effect.gen(function* () {
    return html`
      <button data-action="save">Save</button>
      <button data-action="cancel">Cancel</button>
    `
  }),

setupEvents: (ctx) =>
  Effect.gen(function* () {
    yield* ctx.dom.delegate(ctx.container, "[data-action]", "click", (_e, target) => {
      const action = (target as HTMLElement).dataset.action
      if (action === "save") Effect.runFork(ctx.emit({ type: "save" }))
      if (action === "cancel") Effect.runFork(ctx.emit({ type: "cancel" }))
    })
  }),
```

### Socket Subscriptions

Subscribe to HUD messages for real-time updates:

```typescript
import { SocketServiceTag } from "../effuse/index.js"
import { Stream, pipe } from "effect"

const MyComponent: Component<State, Event, SocketServiceTag> = {
  // ...
  subscriptions: (ctx) => {
    const socket = Effect.map(SocketServiceTag, s => s)

    return [
      pipe(
        Stream.unwrap(Effect.map(socket, s => s.getMessages())),
        Stream.filter(msg => msg.type === "my_event"),
        Stream.map(msg =>
          ctx.state.update(s => ({ ...s, data: msg.data }))
        )
      ),
    ]
  },
}
```

### Type Guards for Messages

```typescript
const isMyMessage = (msg: HudMessage): msg is HudMessage & {
  type: "my_type"
  data: string
} => msg.type === "my_type"

// In subscription
Stream.filter((msg): msg is HudMessage => isMyMessage(msg))
```

---

## Implemented Components

| Component | File | Purpose |
|----------|------|---------|
| `APMComponent` | `components/apm-component.ts` | Actions Per Minute monitor |
| `TBControlsComponent` | `components/tb-controls.ts` | TerminalBench suite/run controls |
| `TBOutputComponent` | `components/tb-output.ts` | Streaming test output |
| `TBResultsComponent` | `components/tb-results.ts` | Run results and per-task metrics |
| `TBLearningComponent` | `components/tb-learning.ts` | FM learning metrics display |
| `ATIFDetailsComponent` | `components/atif-details.ts` | ATIF trajectory step viewer |
| `MCTasksComponent` | `components/mc-tasks.ts` | Ready tasks list |
| `CategoryTreeComponent` | `components/category-tree.ts` | Task category tree |
| `TrajectoryPaneComponent` | `components/trajectory-pane.ts` | Trajectory list |
| `ContainerPanesComponent` | `components/container-panes.ts` | Container output grid |

---

## Further Reading

- **[ui-components.md](./ui-components.md)** - Complete UI/layout guide with Tailwind patterns and examples
- **[ARCHITECTURE.md](./ARCHITECTURE.md)** - Deep dive into Effuse architecture
- **[TESTING.md](./TESTING.md)** - Complete testing guide
- **[HMR.md](./HMR.md)** - Hot Module Replacement with state preservation and migration
- **Source code**: `src/effuse/components/apm-component.ts` is the simplest complete example

---

## Common Mistakes

1. **Using raw `addEventListener()` instead of `ctx.dom.delegate()`** - **CRITICAL!** Effuse uses `innerHTML` replacement on re-render. Raw listeners attached to elements inside the container will break because those elements are destroyed and recreated. Always use `yield* ctx.dom.delegate(ctx.container, selector, event, handler)` which attaches to the container and uses event bubbling.

   ```typescript
   // ❌ WRONG - breaks after state change
   setupEvents: (ctx) => Effect.gen(function* () {
     ctx.container.addEventListener("click", handler)  // BROKEN!
   })

   // ✅ CORRECT - survives re-renders
   setupEvents: (ctx) => Effect.gen(function* () {
     yield* ctx.dom.delegate(ctx.container, "[data-action]", "click", handler)
   })
   ```

2. **Forgetting `Effect.scoped`** - Component mounting requires a scope for cleanup
3. **Not using `Effect.runFork` in event handlers** - DOM callbacks are sync, emit returns Effect
4. **Mutating state directly** - Always use `state.update(s => ({ ...s, newValue }))`
5. **Missing socket service** - If component has subscriptions, use `EffuseLive` not `EffuseLiveNoSocket`
6. **Re-rendering parent wipes child components** - If parent renders containers for child components, re-rendering the parent will destroy child DOM. Use direct DOM manipulation (classList) for visibility changes, or restructure so parent doesn't render child containers. See [ARCHITECTURE.md](./ARCHITECTURE.md#parentchild-component-relationships) for details.
7. **State shape changes break HMR** - If you change a component's state interface, old preserved state won't match. Add migration logic in `mount.ts` (see [HMR.md](./HMR.md#state-migration)) or clear state with `clearAllState()`.
