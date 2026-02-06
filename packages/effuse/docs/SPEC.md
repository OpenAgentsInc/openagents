# Effuse Specification

Complete API reference and specification for the Effuse framework.

## Component Interface

### Component<S, E, R>

```typescript
interface Component<S, E, R = never> {
  /** Unique component identifier (used for debugging) */
  readonly id: string

  /** Factory function for initial state */
  readonly initialState: () => S

  /**
   * Render the component to a TemplateResult.
   *
   * Called on initial mount and whenever state changes.
   * Should be a pure function of state.
   */
  readonly render: (
    ctx: ComponentContext<S, E>
  ) => Effect.Effect<TemplateResult, never, R>

  /**
   * Handle events emitted by this component.
   *
   * Events are typically triggered by user interactions (clicks, inputs).
   * Handlers can update state, make requests, etc.
   */
  readonly handleEvent?: (
    event: E,
    ctx: ComponentContext<S, E>
  ) => Effect.Effect<void, never, R>

  /**
   * Set up event listeners on the container after render.
   *
   * This is called once after mount. Use event delegation for
   * elements that may be re-rendered.
   *
   * Return a cleanup Effect that removes listeners.
   */
  readonly setupEvents?: (
    ctx: ComponentContext<S, E>
  ) => Effect.Effect<void, never, R>

  /**
   * External streams this component subscribes to.
   *
   * Each stream item is an Effect that updates component state.
   * Subscriptions are automatically cleaned up on unmount.
   */
  readonly subscriptions?: (
    ctx: ComponentContext<S, E>
  ) => Stream.Stream<Effect.Effect<void, never, R>, never, R>[]
}
```

### ComponentContext<S, E>

```typescript
interface ComponentContext<S, E> {
  /** Reactive state cell */
  readonly state: StateCell<S>

  /** Emit an event from this component */
  readonly emit: (event: E) => Effect.Effect<void, never>

  /** DOM service for queries and rendering */
  readonly dom: DomService

  /** The container element this component is mounted to */
  readonly container: Element
}
```

## StateCell<A>

Reactive state primitive.

### Interface

```typescript
interface StateCell<A> {
  /** Read current value */
  get: Effect.Effect<A, never>

  /** Replace value (triggers re-render) */
  set: (value: A) => Effect.Effect<void, never>

  /** Transform value (triggers re-render) */
  update: (f: (current: A) => A) => Effect.Effect<void, never>

  /** Stream of state changes */
  changes: Stream.Stream<A, never>
}
```

### Usage

```typescript
// Read current state
const current = yield* ctx.state.get

// Update state (triggers re-render)
yield* ctx.state.set({ count: 5 })

// Transform state (triggers re-render)
yield* ctx.state.update(s => ({ ...s, count: s.count + 1 }))

// Subscribe to changes
yield* pipe(
  ctx.state.changes,
  Stream.tap(newState => Effect.sync(() => console.log("State changed:", newState))),
  Stream.runDrain,
  Effect.forkScoped
)
```

## Template System

### html`` Tagged Template

XSS-safe HTML template literal with automatic escaping.

```typescript
import { html } from "./effuse/index.js"

// Basic usage
const content = html`<div>Hello, ${name}!</div>`

// Automatic escaping
const userInput = "<script>alert('xss')</script>"
html`<div>${userInput}</div>`
// Output: <div>&lt;script&gt;alert('xss')&lt;/script&gt;</div>

// Nested templates (no double-escaping)
const items = ['a', 'b', 'c']
html`<ul>${items.map(i => html`<li>${i}</li>`)}</ul>`

// Conditionals
html`${state.expanded ? html`<details>...</details>` : ""}`

// Raw HTML (use with caution)
import { rawHtml } from "./effuse/index.js"
html`<div>${rawHtml("<span>Unescaped</span>")}</div>`
```

### TemplateResult

```typescript
type TemplateResult = {
  readonly _tag: "TemplateResult"
  readonly parts: readonly TemplatePart[]
}

type TemplatePart =
  | { readonly _tag: "Text"; readonly value: string }
  | { readonly _tag: "Html"; readonly value: string }
  | { readonly _tag: "Template"; readonly value: TemplateResult }
```

## Services

### DomService

Type-safe DOM operations.

```typescript
type DomSwapMode =
  | "inner"
  | "outer"
  | "beforeend"
  | "afterbegin"
  | "delete"
  | "replace"

**Note:** `"replace"` is currently an alias of `"outer"` (both assign `outerHTML`).

interface DomService {
  /**
   * Query for a single element (throws if not found)
   */
  query: (selector: string) => Effect.Effect<Element, DomError>

  /**
   * Query for a single element (returns null when missing)
   */
  queryOption: (selector: string) => Effect.Effect<Element | null, DomError>

  /**
   * Query for multiple elements
   */
  queryAll: (selector: string) => Effect.Effect<readonly Element[], DomError>

  /**
   * Render template to container (inner swap).
   */
  render: (
    container: Element,
    content: TemplateResult
  ) => Effect.Effect<void, DomError>

  /**
   * Swap rendered content into a target element.
   */
  swap: (
    target: Element,
    content: TemplateResult,
    mode?: DomSwapMode
  ) => Effect.Effect<void, DomError>

  /**
   * Set up event delegation
   */
  delegate: (
    container: Element,
    selector: string,
    event: string,
    handler: (e: Event, target: Element) => void
  ) => Effect.Effect<void, DomError>
}
```

**Delegate behavior:** the handler receives the closest matching element, not
necessarily the original event target.

### StateService

Creates StateCell instances.

```typescript
interface StateService {
  /**
   * Create a new StateCell with initial value
   */
  cell: <A>(initial: A) => Effect.Effect<StateCell<A>, never, Scope.Scope>
}
```

## Hypermedia Actions

Effuse includes an HTMX-inspired runtime for declarative DOM updates. Actions
are Effects that can return HTML, which the runtime swaps into a target element.

### EzAction

```typescript
type EzAction = (args: {
  readonly event: Event
  readonly el: Element
  readonly params: Record<string, string>
  readonly dom: DomService
}) => Effect.Effect<TemplateResult | void, unknown>
```

### EzRegistry

```typescript
type EzRegistry = Map<string, EzAction>
export const EzRegistryTag = Context.GenericTag<EzRegistry>("EzRegistry")
```

### mountEzRuntime

```typescript
export const mountEzRuntime: (
  root: Element
) => Effect.Effect<void, never, DomServiceTag | EzRegistryTag | Scope.Scope>

export const mountEzRuntimeWith: (
  root: Element,
  registry: EzRegistry
) => Effect.Effect<void, never, DomServiceTag | Scope.Scope>
```

### Attributes

Implemented:

- `data-ez`: action name
- `data-ez-trigger`: `click | submit | change | input` (defaults based on element type)
- `data-ez-target`: `this`, `closest(...)`, `find(...)`, or a selector
- `data-ez-swap`: `inner | outer | beforeend | afterbegin | delete | replace`
- `data-ez-vals`: JSON to merge into params
- `data-ez-confirm`: confirmation text (uses `window.confirm`)
- `data-ez-disable`: presence-based boolean that disables the element while the action runs

Planned (Phase 2):

- `data-ez-indicator`
- `data-ez-error-target`
- `data-ez-error-swap`
- `data-ez-include`
- `data-ez-trigger`: `load | visible | idle`
- `data-ez-concurrency`: `switch | exhaust | queue`

### Runtime Behavior

- **Concurrency:** switch-latest per element (previous fiber interrupted on re-trigger).

### Params Collection

The runtime builds `params` as a flat string map:

- `submit` events serialize the closest form (event target form, or nearest ancestor).
- `change` / `input` events use the element's `name` and `value` when present.
- `data-ez-vals` JSON is merged last; non-string values are `JSON.stringify`'d.

## Mounting

### mountComponent

Mount a component to a DOM container.

```typescript
export const mountComponent = <S, E, R>(
  component: Component<S, E, R>,
  container: Element
): Effect.Effect<MountedComponent<E>, never, R | DomServiceTag | StateServiceTag | Scope.Scope>
```

**Example:**

```typescript
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

## Layers

### EffuseLive

Production layer with browser services.

```typescript
import { EffuseLive } from "./effuse/index.js"

Effect.runPromise(
  program.pipe(
    Effect.provide(EffuseLive),
    Effect.scoped
  )
)
```

Additional layers (testing and development) are planned; see `docs/effuse/ROADMAP.md`.

## Event Patterns

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

### Discriminated Union Events

Use discriminated unions for type-safe event handling:

```typescript
type MyEvent =
  | { type: "click"; target: string }
  | { type: "input"; value: string }
  | { type: "submit" }

handleEvent: (event, ctx) =>
  Effect.gen(function* () {
    switch (event.type) {
      case "click":
        // TypeScript knows event.target exists here
        yield* handleClick(event.target)
        break
      case "input":
        // TypeScript knows event.value exists here
        yield* handleInput(event.value)
        break
      case "submit":
        yield* handleSubmit()
        break
    }
  })
```

## Subscription Patterns

### Multiple Subscriptions

```typescript
subscriptions: (ctx) => [
  // Subscription 1: External stream
  pipe(
    Stream.fromIterable([1, 2, 3]),
    Stream.map((value) => ctx.state.update(s => ({ ...s, lastValue: value })))
  ),
  // Subscription 2: Timer
  pipe(
    Stream.interval("1 second"),
    Stream.map(() => ctx.state.update(s => ({ ...s, tick: s.tick + 1 })))
  )
]
```

## Testing

Contract tests live under `tests/effuse/`; see `docs/effuse/TESTING.md` for
details and usage.

## Type Constraints

### State Type

- Should be immutable (use spread operator for updates)
- Can be any TypeScript type

### Event Type

- Should be a discriminated union
- Must have a `type` field for discrimination
- Can include additional fields per event variant

### Requirements Type (R)

- Use Context.Tag for service dependencies
- Can be combined with `|` operator
- Example: `Component<State, Event, DomServiceTag | StateServiceTag>`

## Error Handling

### DomError

```typescript
class DomError {
  readonly _tag: "DomError"
  readonly message: string
  readonly cause?: unknown
}
```

### Error Handling Pattern

```typescript
yield* ctx.dom.query("#element").pipe(
  Effect.catchAll((error) => {
    console.error("DOM query failed:", error)
    return Effect.succeed(null) // or handle error
  })
)
```
