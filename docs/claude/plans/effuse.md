# Effuse: Effect-Native UI Framework

## Overview

Create "Effuse", an Effect-native UI framework inspired by Typed.

Refactor the mainview's 2,734-line monolithic `index.ts` into a maintainable, type-safe, testable architecture.

**Decision**: Full Effect integration with services, layers, and proper dependency injection.

## Current Problems

| Problem | Severity | Details |
|---------|----------|---------|
| Monolithic index.ts | Critical | 2,734 lines, 57% of mainview code |
| Code duplication | Critical | 600-700 lines duplicated between modules and index.ts |
| Ad-hoc state | Critical | 30+ module-level variables (`mcTasks`, `tbState`, etc.) |
| Giant message handler | High | 500+ line `handleHudMessage()` with 15+ message types |
| Type-unsafe DOM | High | 58 `getElementById` calls with `!` assertions |
| innerHTML templates | Medium | 25 usages with no type safety or XSS protection |
| No testability | High | Can't mock DOM or socket in tests |

---

## Architecture

### Phase 1: Effuse Core Services (Week 1)

```
src/effuse/
  index.ts                    # Public API exports

  # Services (Effect.Tag pattern)
  services/
    dom.ts                    # DomService - type-safe DOM operations
    dom-live.ts               # Browser implementation
    dom-test.ts               # Mock implementation for tests
    state.ts                  # StateService - reactive state cells
    state-live.ts             # Effect.Ref + Stream implementation
    events.ts                 # EventService - event streams
    socket.ts                 # SocketService - wrap existing socket-client
    socket-live.ts            # Production implementation
    socket-test.ts            # Mock for testing

  # Core primitives
  template/
    html.ts                   # html`` tagged template
    types.ts                  # TemplateResult, TemplateValue
    escape.ts                 # HTML escaping utilities

  state/
    cell.ts                   # StateCell<A> - mutable reactive ref
    computed.ts               # Derived state
    store.ts                  # Redux-like store pattern (optional)

  # Widget system
  widget/
    types.ts                  # Widget<S, E, R> interface
    mount.ts                  # Mount widget to DOM
    context.ts                # WidgetContext with services

  # Layers
  layers/
    live.ts                   # Production: EffuseLive
    test.ts                   # Testing: EffuseTest with mocks

Total: ~800 lines
```

---

### 1.1 Core Services

**DomService** - Type-safe DOM operations:
```typescript
// services/dom.ts
export class DomError extends Error {
  readonly _tag = "DomError"
  constructor(
    readonly reason: "element_not_found" | "render_failed",
    message: string
  ) { super(message) }
}

export interface DomService {
  query: <T extends Element>(selector: string) => Effect.Effect<T, DomError>
  queryOption: <T extends Element>(selector: string) => Effect.Effect<T | null, never>
  render: (element: Element, content: TemplateResult) => Effect.Effect<void, DomError>
  listen: <K extends keyof HTMLElementEventMap>(
    element: Element,
    event: K,
    handler: (e: HTMLElementEventMap[K]) => void
  ) => Effect.Effect<() => void, never>
}

export class DomServiceTag extends Context.Tag("effuse/DomService")<
  DomServiceTag,
  DomService
>() {}
```

**StateService** - Reactive state management:
```typescript
// services/state.ts
export interface StateCell<A> {
  get: Effect.Effect<A, never>
  set: (value: A) => Effect.Effect<void, never>
  update: (f: (current: A) => A) => Effect.Effect<void, never>
  changes: Stream.Stream<A, never>
}

export interface StateService {
  cell: <A>(initial: A) => Effect.Effect<StateCell<A>, never, Scope.Scope>
  computed: <A>(sources: StateCell<unknown>[], compute: () => A) => Effect.Effect<StateCell<A>, never, Scope.Scope>
}

export class StateServiceTag extends Context.Tag("effuse/StateService")<
  StateServiceTag,
  StateService
>() {}
```

**SocketService** - Wraps existing socket-client.ts:
```typescript
// services/socket.ts
export class SocketError extends Error {
  readonly _tag = "SocketError"
  constructor(
    readonly reason: "connection_failed" | "timeout" | "disconnected",
    message: string
  ) { super(message) }
}

export interface SocketService {
  connect: () => Effect.Effect<void, SocketError>
  disconnect: () => Effect.Effect<void, never>
  request: <Req, Res>(type: string, params: Req) => Effect.Effect<Res, SocketError>
  messages: Stream.Stream<HudMessage, SocketError>
}

export class SocketServiceTag extends Context.Tag("effuse/SocketService")<
  SocketServiceTag,
  SocketService
>() {}
```

---

### 1.2 Template System

**html tagged template** - Returns safe HTML:
```typescript
// template/html.ts
export interface TemplateResult {
  strings: TemplateStringsArray
  values: unknown[]
  toString(): string  // Escaped HTML
}

export type TemplateValue =
  | string | number | boolean | null | undefined
  | TemplateResult
  | Effect.Effect<TemplateResult, unknown, unknown>
  | StateCell<unknown>
  | TemplateValue[]

export function html(
  strings: TemplateStringsArray,
  ...values: TemplateValue[]
): TemplateResult {
  return {
    strings,
    values,
    toString() {
      let result = ""
      for (let i = 0; i < strings.length; i++) {
        result += strings[i]
        if (i < values.length) {
          result += renderValue(values[i])  // Auto-escapes
        }
      }
      return result
    }
  }
}
```

---

### 1.3 Widget System

**Widget interface** - Effect-native components:
```typescript
// widget/types.ts
export interface WidgetContext<S, E> {
  state: StateCell<S>
  emit: (event: E) => Effect.Effect<void, never>
  dom: DomService
}

export interface Widget<S, E, R> {
  readonly id: string
  initialState: () => S
  render: (ctx: WidgetContext<S, E>) => Effect.Effect<TemplateResult, never, R>
  handleEvent?: (event: E, ctx: WidgetContext<S, E>) => Effect.Effect<void, never, R>
  subscriptions?: (ctx: WidgetContext<S, E>) => Stream.Stream<Effect.Effect<void, never, R>, never, R>[]
}
```

**mountWidget** - Connects widget to DOM:
```typescript
// widget/mount.ts
export const mountWidget = <S, E, R>(
  widget: Widget<S, E, R>,
  container: Element
): Effect.Effect<void, never, R | DomServiceTag | StateServiceTag | Scope.Scope> =>
  Effect.gen(function* () {
    const dom = yield* DomServiceTag
    const stateService = yield* StateServiceTag

    const state = yield* stateService.cell(widget.initialState())
    const eventQueue = yield* Queue.unbounded<E>()

    const ctx: WidgetContext<S, E> = {
      state,
      emit: (event) => Queue.offer(eventQueue, event),
      dom
    }

    // Initial render
    const content = yield* widget.render(ctx)
    yield* dom.render(container, content)

    // Re-render on state changes (forked fiber)
    yield* pipe(
      state.changes,
      Stream.tap(() => Effect.gen(function* () {
        const newContent = yield* widget.render(ctx)
        yield* dom.render(container, newContent)
      })),
      Stream.runDrain,
      Effect.forkScoped
    )

    // Handle events
    if (widget.handleEvent) {
      yield* pipe(
        Stream.fromQueue(eventQueue),
        Stream.tap((event) => widget.handleEvent!(event, ctx)),
        Stream.runDrain,
        Effect.forkScoped
      )
    }

    // Start subscriptions (e.g., HUD message streams)
    if (widget.subscriptions) {
      for (const sub of widget.subscriptions(ctx)) {
        yield* pipe(sub, Stream.tap(Effect.unit), Stream.runDrain, Effect.forkScoped)
      }
    }
  })
```

---

### 1.4 Layers

**Production Layer**:
```typescript
// layers/live.ts
export const EffuseLive = Layer.mergeAll(
  DomServiceLive,
  StateServiceLive,
  EventServiceLive,
  SocketServiceLive
)
```

**Test Layer** with mocks:
```typescript
// layers/test.ts
export const makeTestLayer = () =>
  Effect.gen(function* () {
    const renderedContent = yield* Ref.make(new Map<Element, string>())
    const messageQueue = yield* Queue.unbounded<HudMessage>()

    const domMock: DomService = {
      query: () => Effect.fail(new DomError("element_not_found", "test")),
      queryOption: () => Effect.succeed(null),
      render: (el, content) => Ref.update(renderedContent, m => m.set(el, content.toString())),
      listen: () => Effect.succeed(() => {})
    }

    const socketMock: SocketService = {
      connect: () => Effect.void,
      disconnect: () => Effect.void,
      request: () => Effect.fail(new SocketError("timeout", "test")),
      messages: Stream.fromQueue(messageQueue)
    }

    return {
      layer: Layer.mergeAll(
        Layer.succeed(DomServiceTag, domMock),
        Layer.succeed(SocketServiceTag, socketMock),
        StateServiceLive
      ),
      renderedContent,
      injectMessage: (msg: HudMessage) => Queue.offer(messageQueue, msg)
    }
  })
```

---

### Phase 2: Message Router (Week 2, Days 1-2)

Split the 500-line `handleHudMessage()` into domain-specific handlers:

```typescript
// src/effuse/message-router.ts
export interface MessageHandler<R> {
  types: string[]
  handle: (message: HudMessage) => Effect.Effect<void, never, R>
}

export const createMessageRouter = <R>(
  handlers: MessageHandler<R>[]
): (message: HudMessage) => Effect.Effect<void, never, R> => {
  const handlerMap = new Map<string, MessageHandler<R>[]>()

  for (const handler of handlers) {
    for (const type of handler.types) {
      const existing = handlerMap.get(type) || []
      handlerMap.set(type, [...existing, handler])
    }
  }

  return (message) => Effect.gen(function* () {
    const handlers = handlerMap.get(message.type) || []
    for (const handler of handlers) {
      yield* handler.handle(message)
    }
  })
}

// Usage:
const tbHandler: MessageHandler<TBWidgetDeps> = {
  types: ['tb_run_start', 'tb_task_start', 'tb_task_complete', 'tb_run_complete'],
  handle: (msg) => Effect.gen(function* () {
    // Update TB state based on message type
  })
}

const router = createMessageRouter([tbHandler, apmHandler, containerHandler])
```

---

### Phase 3: Convert Widgets (Week 2-3)

Convert widgets one at a time, starting with simplest:

| Order | Widget | Lines | Complexity | Reason |
|-------|--------|-------|------------|--------|
| 1 | APM Widget | ~80 | Low | Currently commented out, safe to experiment |
| 2 | Trajectory Pane | 127 | Low | Clear state model (list + selection) |
| 3 | Container Panes | 121 | Medium | Tests list rendering with streams |
| 4 | TB Output | 122 | Medium | Tests text streaming |
| 5 | MC Tasks | 324 | High | Complex table, many actions |
| 6 | TB Controls | 343 | High | Many buttons, form inputs |
| 7 | Category Tree | 212 | Medium | Recursive tree rendering |

#### Example: MC Tasks Widget (Full Effect)

```typescript
// src/effuse/widgets/mc-tasks.ts
import { Effect, Stream, pipe } from "effect"
import { html } from "../template/html.js"
import type { Widget, WidgetContext } from "../widget/types.js"
import { SocketServiceTag } from "../services/socket.js"

interface MCTask {
  id: string
  title: string
  priority: number
  type: string
  labels: string[]
}

interface MCTasksState {
  tasks: MCTask[]
  loading: boolean
  error: string | null
  collapsed: boolean
}

type MCTasksEvent =
  | { type: "refresh" }
  | { type: "assign"; taskId: string }
  | { type: "toggleCollapse" }

export const MCTasksWidget: Widget<MCTasksState, MCTasksEvent, SocketServiceTag> = {
  id: "mc-tasks",

  initialState: () => ({
    tasks: [],
    loading: false,
    error: null,
    collapsed: false,
  }),

  render: (ctx) =>
    Effect.gen(function* () {
      const { tasks, loading, error, collapsed } = yield* ctx.state.get

      if (loading) {
        return html`<div class="mc-tasks loading">Loading tasks...</div>`
      }

      if (error) {
        return html`<div class="mc-tasks error">Error: ${error}</div>`
      }

      return html`
        <div class="mc-tasks-widget ${collapsed ? 'collapsed' : ''}">
          <header data-action="toggleCollapse">
            <h2>Ready Tasks (${tasks.length})</h2>
            <span>${collapsed ? '+' : '-'}</span>
          </header>
          ${!collapsed ? html`
            <table>
              <tbody>
                ${tasks.map(task => html`
                  <tr data-task-id="${task.id}">
                    <td class="priority">P${task.priority}</td>
                    <td class="title">${task.title}</td>
                    <td class="type">${task.type}</td>
                    <td><button data-action="assign">Assign</button></td>
                  </tr>
                `)}
              </tbody>
            </table>
          ` : ''}
        </div>
      `
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      const socket = yield* SocketServiceTag

      switch (event.type) {
        case "refresh":
          yield* ctx.state.update(s => ({ ...s, loading: true, error: null }))
          const result = yield* socket.request<{limit: number}, MCTask[]>(
            "loadReadyTasks", { limit: 20 }
          ).pipe(
            Effect.map(tasks => ({ tasks, error: null })),
            Effect.catchAll(e => Effect.succeed({ tasks: [], error: e.message }))
          )
          yield* ctx.state.update(s => ({ ...s, loading: false, ...result }))
          break

        case "assign":
          yield* socket.request("assignTaskToMC", { taskId: event.taskId, options: { sandbox: true } })
          yield* ctx.state.update(s => ({ ...s, tasks: s.tasks.filter(t => t.id !== event.taskId) }))
          break

        case "toggleCollapse":
          yield* ctx.state.update(s => ({ ...s, collapsed: !s.collapsed }))
          break
      }
    }),
}
```

---

### Phase 4: Full Migration & Cleanup (Week 4)

1. **Create MainviewApp** - Root widget composing all sub-widgets
2. **Replace index.ts** - Slim bootstrap that mounts Effuse
3. **Delete legacy code** - Remove all old render functions, global state
4. **Update tests** - Use test layer for all widget tests

**Target index.ts structure after migration:**
```typescript
// src/mainview/index.ts (~200 lines)
import { Effect, Layer } from "effect"
import { EffuseLive } from "./effuse/layers/live.js"
import { MainviewApp } from "./effuse/widgets/mainview-app.js"
import { mountWidget } from "./effuse/widget/mount.js"

// Bootstrap
const program = Effect.gen(function* () {
  const container = document.getElementById("app")!
  yield* mountWidget(MainviewApp, container)
})

// Run with production layer
Effect.runPromise(
  program.pipe(
    Effect.provide(EffuseLive),
    Effect.scoped
  )
)

// Keyboard shortcuts (global)
document.addEventListener("keydown", handleKeyboardShortcut)
```

---

## Migration Safety

1. **Parallel DOM containers**: New widgets render to `#effuse-*` containers alongside legacy
2. **Feature flags**: `localStorage.getItem('effuse.enabled')` - global switch
3. **Widget flags**: `localStorage.getItem('effuse.mc-tasks')` - per-widget
4. **Ctrl+E toggle**: Runtime switch between legacy and Effuse
5. **Rollback**: `window.DISABLE_EFFUSE = true` kills all Effuse rendering

---

## Timeline

| Week | Phase | Deliverables |
|------|-------|--------------|
| 1 | Core Services | DomService, StateService, SocketService, Layers |
| 1 | Template System | html``, TemplateResult, escaping |
| 1 | Widget System | Widget interface, mountWidget, context |
| 2 | Message Router | Split handleHudMessage into handlers |
| 2 | First Widgets | APM, Trajectory Pane, Container Panes |
| 3 | More Widgets | TB Output, MC Tasks, TB Controls |
| 3 | Category Tree | Tree widget with recursion |
| 4 | MainviewApp | Root widget composing all |
| 4 | Cleanup | Remove legacy code, finalize tests |

**Total: 3-4 weeks**

---

## Critical Files

### To Create
- `src/effuse/` - Entire new directory
- `src/effuse/services/*.ts` - Effect services
- `src/effuse/template/*.ts` - Template system
- `src/effuse/widget/*.ts` - Widget system
- `src/effuse/layers/*.ts` - Production + Test layers
- `src/effuse/widgets/*.ts` - Converted widgets

### To Modify
- `src/mainview/index.ts` - Eventually replace with thin bootstrap
- `src/mainview/index.html` - Add Effuse container elements

### To Reference
- `src/mainview/socket-client.ts` - Wrap with SocketService
- `src/mainview/shared-types.ts` - Reuse types
- `~/code/typed/packages/template/` - html`` implementation reference
- `~/code/typed/packages/fx/src/RefSubject.ts` - StateCell reference

### To Delete (Phase 4)
- `src/mainview/mc-tasks.ts` - Replaced by Effuse widget
- `src/mainview/tb-controls.ts` - Replaced
- `src/mainview/trajectory-pane.ts` - Replaced
- `src/mainview/category-tree.ts` - Replaced
- `src/mainview/tb-output.ts` - Replaced
- `src/mainview/container-panes.ts` - Replaced
