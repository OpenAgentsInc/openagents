# Effuse: Effect-Native UI Framework

Effuse is a lightweight, type-safe UI framework built natively on Effect TypeScript. It provides reactive components with automatic re-rendering, hypermedia actions, and a simple template system.

Effuse has two primitives:

1. Components - stateful render loops driven by StateCell.
2. Hypermedia actions - HTMX-inspired, attribute-driven Effects that swap HTML into targets.

## Quick Start

### Creating a Component

```typescript
import { Effect } from "effect"
import { html, mountComponent, EffuseLive, type Component } from "./effuse/index.js"

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
import { mountComponent, EffuseLive } from "./effuse/index.js"

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

## Core Concepts

### Component Interface

A component has five key properties:

| Property | Required | Description |
|----------|----------|-------------|
| `id` | Yes | Unique identifier for debugging |
| `initialState` | Yes | Factory returning initial state |
| `render` | Yes | Effect producing TemplateResult from state |
| `handleEvent` | No | Effect handling emitted events |
| `setupEvents` | No | Set up DOM event listeners after mount |
| `subscriptions` | No | Streams for external data (timers, feeds, services) |

### ComponentContext

Passed to all component methods:

```typescript
interface ComponentContext<S, E> {
  readonly state: StateCell<S>      // Reactive state
  readonly emit: (event: E) => Effect<void>  // Emit events
  readonly dom: DomService          // DOM operations
  readonly container: Element       // Mounted container
}
```

### StateCell

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

### Template System

The `html` tagged template literal provides XSS-safe HTML:

```typescript
import { html } from "./effuse/index.js"

// Automatic escaping
const userInput = "<script>alert('xss')</script>"
html`<div>${userInput}</div>`
// Output: <div>&lt;script&gt;alert('xss')&lt;/script&gt;</div>

// Nested templates (no double-escaping)
const items = ['a', 'b', 'c']
html`<ul>${items.map(i => html`<li>${i}</li>`)}</ul>`
```

### Hypermedia Actions (HTMX-Inspired)

Effuse supports declarative, targeted DOM updates without a VDOM. Add `data-ez`
attributes to HTML, register an action, and the runtime handles event wiring
and swapping.

```typescript
import { Effect } from "effect"
import {
  EffuseLive,
  html,
  makeEzRegistry,
  mountComponent,
  mountEzRuntimeWith,
} from "./effuse/index.js"

const actions = makeEzRegistry([
  ["counter.inc", ({ el }) =>
    Effect.gen(function* () {
      const count = Number(el.getAttribute("data-count") ?? "0") + 1
      el.setAttribute("data-count", String(count))
      return html`${count}`
    })
  ],
])

const Counter = {
  id: "counter",
  initialState: () => ({}),
  render: () =>
    Effect.succeed(
      html`<button data-ez="counter.inc" data-count="0">0</button>`
    ),
}

Effect.runPromise(
  Effect.gen(function* () {
    const root = document.getElementById("root")!
    yield* mountComponent(Counter, root)
    yield* mountEzRuntimeWith(root, actions)
  }).pipe(Effect.provide(EffuseLive), Effect.scoped)
)
```

**Common attributes:**

- `data-ez`: action name
- `data-ez-trigger`: `click | submit | change | input`
- `data-ez-target`: `this`, `closest(...)`, `find(...)`, or a selector
- `data-ez-swap`: `inner | outer | beforeend | afterbegin | delete | replace`
- `data-ez-vals`: JSON to merge into params

### Services

Core Effect services and registries:

| Service | Tag | Purpose |
|---------|-----|---------|
| `DomService` | `DomServiceTag` | Type-safe DOM queries, rendering, events, swaps |
| `StateService` | `StateServiceTag` | Creates StateCell instances |
| `EzRegistry` | `EzRegistryTag` | Hypermedia action registry |

## Design Principles

1. **Effect-Native** - All UI behavior is expressed as Effects and Streams.
2. **Type-Safe** - Strong typing for state, events, actions, and service dependencies.
3. **Reactive** - State changes and action completions drive DOM updates.
4. **Testable** - Services are mockable; DOM is optional; runtimes are layer-provided.
5. **Minimal** - No virtual DOM, no diffing. Updates are explicit DOM swaps (default `inner`).
6. **Locality** - Prefer targeted updates to specific elements over re-rendering entire screens.
7. **Progressive Enhancement** - HTML carries intent via attributes; runtime interprets it.

## Key Features

- ✅ **Type-Safe Components**: Full TypeScript inference for state and events
- ✅ **Reactive State**: Automatic re-rendering on state changes
- ✅ **XSS-Safe Templates**: Automatic HTML escaping
- ✅ **Hypermedia Actions**: Declarative DOM updates with `data-ez` attributes
- ✅ **Service Abstraction**: Mockable services for testing
- ✅ **Stream Subscriptions**: Subscribe to external data sources (timers, feeds, services)

## Documentation

- **[ARCHITECTURE.md](./ARCHITECTURE.md)** - Deep dive into Effuse internals
- **[SPEC.md](./SPEC.md)** - Complete specification and API reference
- **[EZ.md](./EZ.md)** - Hypermedia actions guide
- **[DOM.md](./DOM.md)** - Swap semantics and focus behavior
- **[TESTING.md](./TESTING.md)** - Testing status and plans
- **[ORIGIN.md](./ORIGIN.md)** - Historical context from previous implementation

## Next Steps

1. Read [ARCHITECTURE.md](./ARCHITECTURE.md) to understand how Effuse works internally
2. Check [SPEC.md](./SPEC.md) for complete API reference
3. Start building components using the patterns above
