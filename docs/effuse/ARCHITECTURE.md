# Effuse Architecture

Deep dive into Effuse's internals for agents needing to understand or extend the framework.

---

## Design Principles

1. **Effect-Native** - Everything is an Effect, enabling composition and testability
2. **Type-Safe** - Strong typing for state, events, and service dependencies
3. **Reactive** - State changes automatically trigger re-renders via Stream
4. **Testable** - Services are mockable, DOM is optional
5. **Minimal** - No virtual DOM, no diffing - just innerHTML updates

---

## Widget Lifecycle

```
1. MOUNT
   └─> Create StateCell with initialState
   └─> Create event Queue
   └─> Build WidgetContext
   └─> Initial render()
   └─> setupEvents() if defined
   └─> Fork state.changes watcher (re-renders)
   └─> Fork eventQueue handler (handleEvent)
   └─> Fork subscriptions (external streams)

2. RUNTIME (until scope closes)
   └─> User action → Event dispatched to queue
   └─> handleEvent processes event
   └─> handleEvent calls state.update()
   └─> StateCell publishes to changes stream
   └─> Re-render fiber calls render()
   └─> DOM updated via innerHTML

3. UNMOUNT (scope closes)
   └─> All forked fibers interrupted
   └─> Event queue shutdown
   └─> StateCell queue shutdown
   └─> Cleanup effects run (finalizers)
```

---

## StateCell Implementation

`src/effuse/state/cell.ts`

StateCell uses Effect.Ref for the value and Queue for change notifications:

```typescript
export const makeCell = <A>(
  initial: A
): Effect.Effect<StateCell<A>, never, Scope.Scope> =>
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

---

## Mount Process

`src/effuse/widget/mount.ts`

The `mountWidget` function orchestrates the widget lifecycle:

```typescript
export const mountWidget = <S, E, R>(
  widget: Widget<S, E, R>,
  container: Element
): Effect.Effect<MountedWidget, never, R | DomServiceTag | StateServiceTag | Scope.Scope>
```

**Step-by-step:**

1. **Get services** from Effect context:
   ```typescript
   const dom = yield* DomServiceTag
   const stateService = yield* StateServiceTag
   ```

2. **Create state cell** (scoped to current Effect scope):
   ```typescript
   const state = yield* stateService.cell(widget.initialState())
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
   const ctx: WidgetContext<S, E> = {
     state,
     emit: (event) => Queue.offer(eventQueue, event),
     dom,
     container,
   }
   ```

5. **Initial render**:
   ```typescript
   const initialContent = yield* widget.render(ctx)
   yield* dom.render(container, initialContent)
   ```

6. **Set up events** (delegated listeners):
   ```typescript
   if (widget.setupEvents) {
     yield* widget.setupEvents(ctx)
   }
   ```

7. **Fork re-render fiber** (runs until scope closes):
   ```typescript
   yield* pipe(
     state.changes,
     Stream.tap(() =>
       Effect.gen(function* () {
         const content = yield* widget.render(ctx)
         yield* dom.render(container, content)
       })
     ),
     Stream.runDrain,
     Effect.forkScoped
   )
   ```

8. **Fork event handler** (runs until scope closes):
   ```typescript
   if (widget.handleEvent) {
     yield* pipe(
       Stream.fromQueue(eventQueue),
       Stream.tap((event) => widget.handleEvent!(event, ctx)),
       Stream.runDrain,
       Effect.forkScoped
     )
   }
   ```

9. **Fork subscriptions** (external streams):
   ```typescript
   if (widget.subscriptions) {
     for (const sub of widget.subscriptions(ctx)) {
       yield* pipe(
         sub,
         Stream.tap((effect) => effect),
         Stream.runDrain,
         Effect.forkScoped
       )
     }
   }
   ```

---

## Service Architecture

### DomService Interface

`src/effuse/services/dom.ts`

```typescript
export interface DomService {
  query: <T extends Element>(selector: string) => Effect<T, DomError>
  queryOption: <T extends Element>(selector: string) => Effect<T | null, never>
  queryId: <T extends Element>(id: string) => Effect<T, DomError>
  render: (element: Element, content: TemplateResult) => Effect<void, DomError>
  listen: <K extends keyof HTMLElementEventMap>(
    element: Element,
    event: K,
    handler: (e: HTMLElementEventMap[K]) => void,
    options?: AddEventListenerOptions
  ) => Effect<() => void, never>
  delegate: <K extends keyof HTMLElementEventMap>(
    container: Element,
    selector: string,
    event: K,
    handler: (e: HTMLElementEventMap[K], target: Element) => void
  ) => Effect<() => void, never>
  createFragment: (content: TemplateResult) => Effect<DocumentFragment, DomError>
}
```

**Implementation choices:**
- `query` fails on not found - use for required elements
- `queryOption` returns null - use for optional elements
- `render` uses innerHTML - simple but effective
- `delegate` uses event bubbling - handles re-rendered elements

### StateService Interface

`src/effuse/services/state.ts`

```typescript
export interface StateService {
  readonly cell: <A>(initial: A) => Effect<StateCell<A>, never, Scope.Scope>
}
```

Just a factory for StateCells. The implementation (`state-live.ts`) simply wraps `makeCell`.

### SocketService Interface

`src/effuse/services/socket.ts`

```typescript
export interface SocketService {
  connect: () => Effect<void, SocketError>
  disconnect: () => Effect<void, never>
  isConnected: () => Effect<boolean, never>
  getMessages: () => Stream<HudMessage, never>

  // TB Operations
  loadTBSuite: (suitePath: string) => Effect<TBSuiteInfo, SocketError>
  startTBRun: (options: StartTBRunOptions) => Effect<{ runId: string }, SocketError>
  stopTBRun: () => Effect<{ stopped: boolean }, SocketError>
  loadRecentTBRuns: (count?: number) => Effect<TBRunHistoryItem[], SocketError>
  loadTBRunDetails: (runId: string) => Effect<TBRunDetails | null, SocketError>

  // Task Operations
  loadReadyTasks: (limit?: number) => Effect<MCTask[], SocketError>
  assignTaskToMC: (taskId: string, options?: AssignTaskOptions) => Effect<{ assigned: boolean }, SocketError>

  // Trajectory Operations
  loadUnifiedTrajectories: (limit?: number) => Effect<UnifiedTrajectory[], SocketError>
}
```

**Key insight:** `getMessages()` returns a Stream that widgets subscribe to. The socket client maintains a WebSocket connection and publishes incoming HUD protocol messages.

---

## Template System

### html Tagged Template

`src/effuse/template/html.ts`

```typescript
export function html(
  strings: TemplateStringsArray,
  ...values: TemplateValue[]
): TemplateResult {
  return {
    _tag: "TemplateResult",
    strings,
    values,
    toString() {
      let result = ""
      for (let i = 0; i < strings.length; i++) {
        result += strings[i]
        if (i < values.length) {
          result += renderValue(values[i])
        }
      }
      return result
    },
  }
}
```

**renderValue logic:**
- `null`/`undefined` → empty string
- `string` → escaped via `escapeHtml()`
- `number`/`boolean` → String()
- `TemplateResult` → `toString()` (already escaped)
- `Array` → map and join

### XSS Escaping

`src/effuse/template/escape.ts`

```typescript
export const escapeHtml = (str: string): string =>
  str
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;")
```

**Critical:** All user input is escaped by default. Only use `rawHtml()` for trusted content like SVG paths.

---

## Layer Composition

### EffuseLive

`src/effuse/layers/live.ts`

```typescript
export const EffuseLive = Layer.mergeAll(
  DomServiceLive,
  StateServiceLive,
  SocketServiceDefault
)
```

Provides all three services for production browser use.

### Test Layers

`src/effuse/layers/test.ts`

```typescript
export const makeTestLayer = (): Effect<TestLayerResult, never>
```

Returns:
- `layer` - Mock services layer
- `renderedContent` - Ref tracking rendered HTML
- `injectMessage` - Push HudMessage to socket stream
- `getRendered` - Read rendered HTML for element
- `clearRendered` - Reset rendered content

**Mock behavior:**
- DomService stores renders in a Map, no real DOM
- SocketService fails all requests (use `injectMessage` to simulate responses)
- StateService uses real implementation (works without DOM)

---

## Error Handling

### DomError

```typescript
export class DomError extends Error {
  readonly _tag = "DomError"
  constructor(
    readonly reason: "element_not_found" | "render_failed" | "invalid_selector",
    message: string
  ) {
    super(message)
  }
}
```

### SocketError

```typescript
export class SocketError extends Error {
  readonly _tag = "SocketError"
  constructor(
    readonly reason: "connection_failed" | "timeout" | "disconnected" | "request_failed",
    message: string
  ) {
    super(message)
  }
}
```

**Pattern:** Services use typed errors. Widgets should catch and handle appropriately, or let errors propagate for debugging.

---

## Mainview Integration

`src/mainview/effuse-main.ts`

Entry point that:
1. Creates the Effuse layer with socket client
2. Mounts all widgets to their containers
3. Keeps the Effect scope alive with `Effect.never`

```typescript
const initEffuse = () => {
  const layer = createEffuseLayer()

  const program = Effect.gen(function* () {
    yield* mountAllWidgets
    yield* Effect.never  // Keep scope alive
  })

  Effect.runFork(
    program.pipe(
      Effect.provide(layer),
      Effect.scoped,
      Effect.catchAllDefect((defect) => {
        console.error("[Effuse] Defect:", defect)
        return Effect.void
      })
    )
  )
}
```

**Key insight:** `Effect.never` prevents the scope from closing, keeping all forked fibers (event handlers, subscriptions) running indefinitely.

---

## Performance Considerations

1. **innerHTML Updates** - Effuse replaces entire container innerHTML on each render. This is simple but can be slow for large widgets. Keep widgets focused.

2. **No Virtual DOM** - No diffing means every render is a full replace. State changes should be batched if possible.

3. **Stream-Based Re-rendering** - Each state change triggers a render. Multiple rapid updates will queue up renders.

4. **Event Delegation** - Using `delegate` instead of individual listeners avoids re-attaching on render.

---

## Extending Effuse

### Adding a New Widget

1. Create `src/effuse/widgets/my-widget.ts`
2. Define state and event types
3. Implement Widget interface
4. Add tests in `my-widget.test.ts`
5. Export from `src/effuse/index.ts`
6. Mount in `src/mainview/effuse-main.ts`

### Adding a New Service

1. Create interface in `src/effuse/services/my-service.ts`
2. Create Context.Tag
3. Create live implementation in `my-service-live.ts`
4. Add to layers in `layers/live.ts`
5. Add mock to `layers/test.ts`
6. Export from `src/effuse/index.ts`

### Modifying StateCell

Be careful - StateCell is core infrastructure. Changes affect all widgets. Key files:
- `src/effuse/state/cell.ts`
- `src/effuse/services/state.ts`
- `src/effuse/services/state-live.ts`
