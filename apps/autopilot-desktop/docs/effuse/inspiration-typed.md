# Inspiration: Lessons from Typed

`Typed` is a comprehensive, production-grade, Effect-native full-stack framework. While Effuse aims to be lightweight and simple (avoiding VDOM), `Typed` offers valuable architectural lessons, particularly in reactive state management and service abstraction.

## Core Philosophy Comparison

| Feature | Effuse | Typed |
|---------|--------|-------|
| **Scope** | Lightweight UI Framework | Comprehensive Full-Stack Toolkit |
| **State** | `StateCell` (Ref + Queue + Stream) | `RefSubject` (Fx + DeferredRef) |
| **Rendering** | `innerHTML` replacement | AST-based Templating with Hydration |
| **Reactivity** | Pull-based `Stream` | Push-based `Fx` (specialized Stream) |
| **DOM** | `DomService` (Unified) | Granular Services (Window, Document, History) |

## Key Lessons for Effuse

### 1. Advanced State Composition (`RefSubject` pattern)

`Typed`'s `RefSubject` is the direct equivalent of Effuse's `StateCell`. However, `RefSubject` supports powerful composition that `StateCell` currently lacks:

*   **`computed`**: Derived read-only state.
*   **`struct` / `tuple`**: Composing multiple state cells into one.
*   **`filtered`**: A view of state that only emits when specific conditions are met.
*   **`eq` / `skipRepeats`**: Equivalence-based deduping to avoid redundant emissions.
*   **`runUpdates`**: Batch multiple updates through a single update boundary.

**Action Item:** Enhance `StateCell` to support `computed` / `filtered` and `eq`-based deduping, plus a simple `batch` or `runUpdates` API. This allows components to subscribe only to *relevant* state changes, preventing unnecessary re-renders without needing manual selectors.

```typescript
// Current Effuse
const { count } = yield* ctx.state.get // Component re-renders on ANY state change

// Inspired by Typed
const count = yield* ctx.state.computed(s => s.count) // Component re-renders ONLY when count changes
```

### 2. Fine-Grained Templating

`@typed/template` parses template literals into an AST with "Parts" (dynamic holes). Each Part has an `update` method (text, attribute, property, ref, event, etc), so it can update *only* the changing node instead of re-rendering the parent container.

**Action Item:** While Effuse intentionally avoids a VDOM, the "Part" concept matches perfectly with our `Ez` runtime goals. Instead of `innerHTML` replacement, future Effuse versions could identify dynamic "holes" during the initial render and bind `StateCell` streams directly to those DOM nodes. This would provide `lit-html` performance with `Effect` safety.

### 3. Granular DOM Services

`Typed` splits DOM access into specific services: `Window`, `Document`, `History`, `Location`, `Navigator`.

**Action Item:** As Effuse grows, `DomService` should be broken down. This is critical for testing. For example, testing a component that only needs `History` shouldn't require mocking the entire `Element` query API.

### 4. Push-Based Reactivity (`Fx`)

`Typed` introduces `Fx`, a specialized "hot" stream optimized for UI events (push-based) versus standard `Effect` streams (pull-based).

**Action Item:** Evaluate if `Stream` overhead is impactful. For 99% of UI interactions, standard `Stream` is fine. However, for high-frequency events (mouse moves, scroll), adopting a lighter-weight push/pub-sub pattern internally (like `Fx`) might be necessary.

### 5. Element References (`ElementRef`)

`@typed/template` has `ElementRef`, a tracked reference to a rendered element that exposes queries, events, and dispatch helpers.

**Action Item:** Consider adding a small Effuse `ElementRef` helper or `DomService.ref()` API so components can hold stable references without manual querying after every render.

### 6. Collection Helpers (RefArray / RefChunk / RefHashMap / RefHashSet)

Typed includes specialized RefSubject helpers for arrays, chunks, and hash maps/sets to avoid manual update logic.

**Action Item:** Add small collection helpers for `StateCell` (append, updateAt, remove, has, size) so component logic stays declarative.

## Conclusion

Effuse should **not** try to be `Typed`. Effuse's value proposition is simplicity: "just render HTML." However, adopting `Typed`'s **state composition patterns** (derived/computed state) is the single highest-value improvement we can make to improve developer experience and performance.

Additional near-term wins we can copy from Typed without adding complexity:

- Equivalence-based deduping (`eq`) to avoid redundant re-renders.
- A tiny batching API for multiple state updates in a single render tick.
- A minimal `ElementRef` concept for stable DOM access.
