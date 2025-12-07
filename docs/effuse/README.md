# Effuse: Effect-Native UI Framework

Effuse is OpenAgents' custom, lightweight UI framework built natively on Effect TypeScript. It powers the mainview desktop HUD with type-safe, reactive, testable widgets.

**TL;DR for coding agents:**
- Widgets are Effect-native components with typed state, events, and services
- Use `html` tagged templates with automatic XSS escaping
- State is reactive via `StateCell<A>` (built on Effect.Ref + Queue)
- Test with `makeTestLayer()` or `makeHappyDomLayer()` for real DOM
- Entry point: `src/effuse/index.ts` exports everything

---

## Quick Start

### Creating a Widget

```typescript
import { Effect } from "effect"
import { html, type Widget } from "../effuse/index.js"

// 1. Define state type
interface CounterState {
  count: number
}

// 2. Define event type (discriminated union)
type CounterEvent =
  | { type: "increment" }
  | { type: "decrement" }

// 3. Create the widget
const CounterWidget: Widget<CounterState, CounterEvent> = {
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

### Mounting a Widget

```typescript
import { Effect } from "effect"
import { mountWidget, EffuseLive } from "../effuse/index.js"

const program = Effect.gen(function* () {
  const container = document.getElementById("my-widget")!
  yield* mountWidget(MyWidget, container)
})

Effect.runPromise(
  program.pipe(
    Effect.provide(EffuseLive),
    Effect.scoped
  )
)
```

### Testing a Widget

```typescript
import { describe, test, expect } from "bun:test"
import { Effect } from "effect"
import { makeTestLayer } from "../effuse/layers/test.js"
import { mountWidget } from "../effuse/widget/mount.js"
import { MyWidget } from "./my-widget.js"

test("renders initial state", async () => {
  await Effect.runPromise(
    Effect.scoped(
      Effect.gen(function* () {
        const { layer, getRendered } = yield* makeTestLayer()
        const container = { id: "test" } as Element

        yield* mountWidget(MyWidget, container).pipe(Effect.provide(layer))

        const html = yield* getRendered(container)
        expect(html).toContain("expected content")
      })
    )
  )
})
```

---

## Core Concepts

### 1. Widget Interface

A widget has five key properties:

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
interface Widget<S, E, R = never> {
  id: string
  initialState: () => S
  render: (ctx: WidgetContext<S, E>) => Effect.Effect<TemplateResult, never, R>
  handleEvent?: (event: E, ctx: WidgetContext<S, E>) => Effect.Effect<void, never, R>
  setupEvents?: (ctx: WidgetContext<S, E>) => Effect.Effect<void, never, R>
  subscriptions?: (ctx: WidgetContext<S, E>) => Stream.Stream<Effect.Effect<void, never, R>, never, R>[]
}
```

### 2. WidgetContext

Passed to all widget methods:

```typescript
interface WidgetContext<S, E> {
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
├── widget/
│   ├── types.ts             # Widget interface
│   └── mount.ts             # mountWidget helpers
├── layers/
│   ├── live.ts              # EffuseLive, EffuseLiveNoSocket
│   └── test.ts              # makeTestLayer, makeCustomTestLayer
├── widgets/                 # Implemented widgets
│   ├── apm-widget.ts        # APM monitor
│   ├── tb-controls.ts       # TerminalBench controls
│   ├── mc-tasks.ts          # MechaCoder tasks
│   └── ...                  # Other widgets
└── testing/                 # Test infrastructure
    ├── harness.ts           # TestHarness, WidgetHandle
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

const MyWidget: Widget<State, Event, SocketServiceTag> = {
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

## Implemented Widgets

| Widget | File | Purpose |
|--------|------|---------|
| `APMWidget` | `widgets/apm-widget.ts` | Actions Per Minute monitor |
| `TBControlsWidget` | `widgets/tb-controls.ts` | TerminalBench suite/run controls |
| `TBOutputWidget` | `widgets/tb-output.ts` | Streaming test output |
| `TBResultsWidget` | `widgets/tb-results.ts` | Run results display |
| `MCTasksWidget` | `widgets/mc-tasks.ts` | Ready tasks list |
| `CategoryTreeWidget` | `widgets/category-tree.ts` | Task category tree |
| `TrajectoryPaneWidget` | `widgets/trajectory-pane.ts` | Trajectory list |
| `ContainerPanesWidget` | `widgets/container-panes.ts` | Container output grid |

---

## Further Reading

- **[ARCHITECTURE.md](./ARCHITECTURE.md)** - Deep dive into Effuse architecture
- **[TESTING.md](./TESTING.md)** - Complete testing guide
- **Source code**: `src/effuse/widgets/apm-widget.ts` is the simplest complete example

---

## Common Mistakes

1. **Forgetting `Effect.scoped`** - Widget mounting requires a scope for cleanup
2. **Not using `Effect.runFork` in event handlers** - DOM callbacks are sync, emit returns Effect
3. **Mutating state directly** - Always use `state.update(s => ({ ...s, newValue }))`
4. **Missing socket service** - If widget has subscriptions, use `EffuseLive` not `EffuseLiveNoSocket`
