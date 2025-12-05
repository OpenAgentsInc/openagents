# Typed Framework Deep Dive

> Research document for informing Effuse, our lightweight Effect-based UI framework inspired by Typed.

## What is Typed?

Typed is an **Effect-native toolkit** for building full-stack web applications with deep type-safety integration. It's a monorepo containing 24+ specialized packages that work together to provide:

- Type-safe HTML templates with reactive properties
- Push-based reactive programming (Fx)
- Comprehensive routing and navigation
- Full-stack server/client synchronization
- Context-aware dependency injection
- Multi-environment support (browser, server, workers, static rendering)

**Core Philosophy**: Long-term-friendly design patterns and dependency inversion, favoring correctness over extreme performance.

---

## Package Architecture

| Package | Purpose |
|---------|---------|
| **@typed/core** | Prelude/façade combining essential packages |
| **@typed/template** | HTML template system with type-safe rendering |
| **@typed/fx** | Push-based reactive primitives (built on Effect) |
| **@typed/router** | Type-safe routing with guards |
| **@typed/route** | Route AST and pattern matching |
| **@typed/navigation** | Browser navigation abstraction |
| **@typed/dom** | Type-safe DOM services |
| **@typed/context** | Context.Tag extensions |
| **@typed/environment** | Environment tracking (dom, server, test) |
| **@typed/async-data** | Loading/Success/Error state |
| **@typed/server** | HTTP routing, OpenAPI schemas |
| **@typed/compiler** | Template string compiler/parser |
| **@typed/vite-plugin** | Multi-build Vite integration |

---

## Core Concepts

### 1. The `html` Tagged Template Literal

```typescript
// packages/template/src/RenderTemplate.ts
export function html<const Values extends ReadonlyArray<Renderable<any, any>>>(
  template: TemplateStringsArray,
  ...values: Values
): Fx.Fx<
  RenderEvent,
  Placeholder.Error<Values[number]>,
  Placeholder.Context<Values[number]>
>
```

The `html` function:
- Takes static template strings + interpolated values
- Returns an `Fx<RenderEvent>` (push-based reactive stream)
- **Automatically infers error and context types** from interpolated values
- `const Values` captures exact types of all interpolations

### 2. The Placeholder Protocol

All values interpolated in templates must implement `Placeholder`:

```typescript
// packages/template/src/Placeholder.ts
export interface Placeholder<A = unknown, E = never, R = never> {
  readonly [PlaceholderTypeId]: {
    readonly _R: (_: never) => R
    readonly _E: (_: never) => E
    readonly _A: (_: never) => A
  }
}
```

Typed extends JavaScript primitives globally:

```typescript
// packages/template/src/internal/module-augmentation.ts
declare global {
  export interface String extends Placeholder<string> {}
  export interface Number extends Placeholder<number> {}
  export interface Boolean extends Placeholder<boolean> {}
  export interface HTMLElement extends Placeholder<HTMLElement> {}
}

declare module "@typed/fx/Fx" {
  export interface Fx<A, E, R> extends Placeholder<A, E, R> {}
}
```

This allows primitives, Effects, Fx streams, and DOM nodes to all be interpolated with full type inference.

### 3. Fx: Push-Based Reactive Streams

```typescript
// packages/fx/src/Fx.ts
export interface Fx<out A, out E = never, out R = never> {
  run<R2>(sink: Sink.Sink<A, E, R2>): Effect.Effect<unknown, never, R | R2>
}
```

Key operations:
- `Fx.succeed()` - Static value
- `Fx.gen()` - Effect-like generator syntax
- `Fx.map()`, `Fx.filter()` - Transform/filter
- `Fx.merge()` - Combine streams
- `Fx.hold()` - Share/cache values

### 4. RefSubject: Mutable Reactive References

```typescript
// packages/fx/src/RefSubject.ts
interface RefSubject<A, E, R> extends Computed<A, E, R>, Subject<A, E, R> {
  readonly runUpdates: (f: GetSetDelete<A, E, R>) => Effect<B, E2, R2>
}
```

Usage in templates:

```typescript
const Counter = Fx.gen(function*() {
  const count = yield* RefSubject.of(0)

  return html`<div>
    <p>Count: ${count}</p>
    <button onclick=${RefSubject.increment(count)}>+</button>
  </div>`
})
```

### 5. Template Parsing & AST

```typescript
// packages/template/src/Template.ts
export class Template {
  readonly nodes: ReadonlyArray<Node>
  readonly hash: string  // Cache key
  readonly parts: ReadonlyArray<[PartNode, path]>
}

type Node =
  | ElementNode | SelfClosingElementNode | TextOnlyElement
  | TextNode | NodePart | Comment | DocType

type PartNode =
  | AttrPartNode      // attr="${value}"
  | BooleanPartNode   // disabled=${true}
  | ClassNamePartNode // class="${classes}"
  | EventPartNode     // onclick=${handler}
  | PropertyPartNode  // .property=${value}
  | RefPartNode       // ref=${elementRef}
  | NodePart          // ${content}
```

### 6. ElementSource: Type-Safe DOM Queries

```typescript
// packages/template/src/ElementSource.ts
type DefaultEventMap<T> =
  T extends Window ? WindowEventMap
  : T extends HTMLVideoElement ? HTMLVideoElementEventMap
  : T extends HTMLElement ? HTMLElementEventMap
  : T extends SVGElement ? SVGElementEventMap
  : Record<string, Event>

// CSS selector type parsing for element inference
type ParseSelector<T extends string, Fallback> = ...
```

### 7. Event Handlers as Effects

```typescript
// packages/template/src/EventHandler.ts
export interface EventHandler<Ev extends Event, E, R> extends Placeholder<never, E, R> {
  readonly handler: (event: Ev) => Effect.Effect<unknown, E, R>
  readonly options: AddEventListenerOptions | undefined
}
```

### 8. Multi-Environment Rendering

```typescript
// Different layers for different environments
RenderContext.dom     // Browser DOM
RenderContext.server  // Server-side rendering
RenderContext.static  // Pre-rendered static HTML
RenderContext.test    // Testing

// Code checks environment:
const env = yield* CurrentEnvironment
if (env === "dom") { /* browser-specific */ }
```

### 9. Render Queue Scheduling

```typescript
// packages/template/src/RenderQueue.ts
RenderQueue.sync      // Immediate (testing)
RenderQueue.microtask // Microtask queue
RenderQueue.raf       // requestAnimationFrame
RenderQueue.idle      // requestIdleCallback
```

---

## Key Type Safety Mechanisms

1. **Placeholder Protocol**: All interpolated values must be Placeholders
2. **Error Propagation**: Errors bubble via `Placeholder.Error<T>`
3. **Context Merging**: Resources merge via `Placeholder.Context<T>`
4. **Compile-time Parsing**: HTML parsed to extract part locations
5. **Event Map Inference**: Element types determine valid events
6. **Selector Type Parsing**: CSS selectors parsed to infer element types

---

## Example: Counter Application

```typescript
// Shared component
const Counter = Fx.gen(function*() {
  const count = yield* RefSubject.of(0)

  return html`<div>
    <p>Count: ${count}</p>
    <button onclick=${RefSubject.decrement(count)}>-</button>
    <button onclick=${RefSubject.increment(count)}>+</button>
  </div>`
})

// Server rendering
toServerRouter(router, {
  layout: ({ content, script }) => html`
    <!doctype html>
    <html><body>${content}${script}</body></html>
  `,
  clientEntry: "browser"
}).pipe(Node.listen({ port: 3000 }))

// Browser hydration
Browser.run(router)
```

---

## Comparison: Typed vs Our Mainview

| Aspect | Typed | Our Mainview |
|--------|-------|--------------|
| **Template Safety** | Full type inference, Placeholder protocol | Raw innerHTML strings |
| **State Management** | RefSubject + Fx streams | ~30+ ad-hoc global variables |
| **Event Handling** | Effect-based EventHandlers | Scattered addEventListener calls |
| **DOM Access** | ElementSource with type parsing | String selectors, `!` assertions |
| **Reactivity** | Fine-grained Fx updates | Full re-render via innerHTML |
| **Error Handling** | Effect error channels | try/catch with console.log |
| **Code Organization** | 24 focused packages | 2,734 line monolith |
| **Testing** | Multi-environment layers | Manual DOM mocking |

### Our Mainview Pain Points

1. **2,734 line index.ts** - 57% of mainview code in one file
2. **600-700 lines duplicated** - Same code in modules AND index.ts
3. **500+ line handleHudMessage()** - One function handling 15+ message types
4. **No centralized state** - 30+ module-level variables
5. **Type-unsafe DOM** - `getElementById("x")!.textContent = ...`
6. **No virtual DOM** - Full innerHTML rebuilds on every render

---

## Considerations for Effuse

### What We Need

1. **Type-safe templates** - Eliminate string-based HTML construction
2. **Centralized state** - Replace 30+ global variables
3. **Modular message handlers** - Split 500-line handler
4. **Safe DOM access** - No more `!` assertions
5. **Effect integration** - Leverage our existing Effect usage

### What We DON'T Need (Yet)

1. SSR/hydration (desktop app only)
2. Routing (single-page HUD)
3. Vite plugin (using Bun.serve)
4. Full Fx reactive system (overkill for our use case)

### Effuse Minimum Viable Feature Set

1. **`html` tagged template** - Type-safe HTML construction
2. **ElementRef** - Type-safe DOM element references
3. **State container** - Centralized state with subscriptions
4. **Event handlers** - Effect-based event handling
5. **Widget protocol** - Standard interface for UI modules

### Gradual Migration Path

**Phase 1**: Create Effuse core with `html`, `ElementRef`, `State`
**Phase 2**: Convert one widget (e.g., mc-tasks) to Effuse
**Phase 3**: Add message handler registry
**Phase 4**: Migrate remaining widgets
**Phase 5**: Slim down index.ts to bootstrap only

---

## Key Typed Files for Reference

| File | Purpose |
|------|---------|
| `packages/template/src/RenderTemplate.ts` | Main `html` function |
| `packages/template/src/Placeholder.ts` | Type propagation |
| `packages/template/src/ElementSource.ts` | DOM querying |
| `packages/fx/src/Fx.ts` | Reactive streams |
| `packages/fx/src/RefSubject.ts` | Mutable refs |
| `packages/template/src/internal/parser2.ts` | HTML parser |
| `packages/template/src/internal/module-augmentation.ts` | Global types |

---

## Questions to Resolve

1. **Complexity**: How much of Typed's sophistication do we need?
2. **Reactivity**: Full Fx streams or simpler state subscriptions?
3. **Migration**: Gradual conversion or focused rewrite?
4. **Templates**: Compile-time parsing or runtime-only?
5. **Scope**: Just mainview or framework for all OpenAgents UIs?
