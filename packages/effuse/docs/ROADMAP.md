# Effuse Roadmap

Effuse stays minimal and Effect-native, with two primitives: components and
hypermedia actions. This roadmap focuses on quality and ergonomics without
introducing a VDOM.

## Phase 0: Baseline (complete)

- Template system (`html`, `rawHtml`, escaping)
- StateCell (`Ref` + `Queue` + `Stream`)
- DomService with `swap` modes and focus restoration
- Component lifecycle and mounting
- Hypermedia actions runtime (`data-ez`)

Current behavior:

- Ez concurrency: switch-latest per element (previous fiber interrupted)
- Supported triggers: `click | submit | change | input`
- Supported swaps: `inner | outer | replace | beforeend | afterbegin | delete`
- Implemented attributes: `data-ez`, `data-ez-trigger`, `data-ez-target`,
  `data-ez-swap`, `data-ez-vals`, `data-ez-confirm`, `data-ez-disable`

## Phase 1: State ergonomics and render quality

- `StateCell.computed` / `StateCell.filtered` helpers
- Equivalence-based deduping (`eq`) to avoid redundant re-renders
- `StateCell.batch` to coalesce multiple updates into a single render
- Collection helpers for arrays/maps/sets (append/remove/has/size)
- Tests for StateCell semantics and render coalescing

Proposed API surface (draft):

```ts
interface StateCell<A> {
  computed: <B>(f: (a: A) => B, eq?: (x: B, y: B) => boolean) => StateCell<B>
  filtered: (pred: (a: A) => boolean) => Stream.Stream<A>
  withEq: (eq: (x: A, y: A) => boolean) => StateCell<A>
  batch: <R, E>(effect: Effect.Effect<R, E>) => Effect.Effect<R, E>
}
```

Render loop integration plan:

- `StateCell` emits only when `eq` says the value changed.
- `computed` reuses upstream `eq` to avoid redundant component renders.
- `batch` defers change notifications until the wrapped effect completes.

## Phase 2: Hypermedia runtime v1

- `data-ez-indicator` for loading state
  - Definition of done:
    - Resolves target via selector or `closest(...)`/`find(...)`
    - Adds and removes a `[data-ez-loading]` attribute while action runs
    - Works with cancel/interrupt
- `data-ez-error-target` + `data-ez-error-swap` for error rendering
  - Definition of done:
    - On action failure, renders an error template into target
    - `data-ez-error-swap` matches swap modes (`inner`, `outer`, etc.)
- `data-ez-include` for cross-form params
  - Definition of done:
    - Accepts selectors or `closest(...)`/`find(...)`
    - Merges params ahead of `data-ez-vals`
- `data-ez-trigger` extensions: `load`, `visible`, `idle`
  - Definition of done:
    - `load` fires once on mount
    - `visible` fires when element intersects viewport
    - `idle` fires via `requestIdleCallback` fallback to `setTimeout`
- Concurrency policies per action (`switch`, `exhaust`, `queue`)
  - Definition of done:
    - `switch`: interrupt previous (current behavior)
    - `exhaust`: ignore triggers while running
    - `queue`: serialize triggers in order
    - Tests for each policy
- Tests for action parsing and swap behavior

## Phase 3: DOM services and refs

- Split DomService into granular services (Document, Window, History)
- `ElementRef` helper for stable DOM references
- `settle` behaviors: focus restore (done), optional scroll restore

## Phase 4: Template parts (no VDOM, partial updates)

- Prototype "Part" system for text/attribute/property updates
- Optional hydration path for server-rendered templates
- Keyed list helpers for targeted DOM patches

## Phase 5: Testing and tooling

- Test harness for components and hypermedia actions
- Happy-dom layer and mock services
- Golden snapshot support for TemplateResult output

## Phase 6: UI migration

- Convert small panels first (status, notifications, feeds)
- Replace React flows with Effuse components or actions
- Document migration patterns and pitfalls
