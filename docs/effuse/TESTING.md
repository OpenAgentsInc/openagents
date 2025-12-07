# Effuse Testing Guide

Complete guide to testing Effuse widgets at all levels.

---

## Testing Pyramid

Effuse provides three testing layers with increasing fidelity and cost:

| Layer | Speed | DOM | Events | Use Case |
|-------|-------|-----|--------|----------|
| **Mock** | ~1ms | No | No | State-only logic, render output |
| **Happy-DOM** | ~10ms | Yes | Yes | Widget behavior, event handling |
| **Playwright** | ~500ms | Browser | Full | E2E, visual regression |

---

## Quick Start

### Basic Mock Layer Test

```typescript
import { describe, test, expect } from "bun:test"
import { Effect } from "effect"
import { makeTestLayer } from "../effuse/layers/test.js"
import { mountWidget } from "../effuse/widget/mount.js"
import { MyWidget } from "./my-widget.js"

describe("MyWidget", () => {
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
})
```

### Happy-DOM Test with Real DOM

```typescript
import { describe, test, expect } from "bun:test"
import { Effect } from "effect"
import { makeHappyDomLayer } from "../effuse/testing/layers/happy-dom.js"
import { TestHarnessTag, TestBrowserTag } from "../effuse/testing/index.js"
import { MyWidget } from "./my-widget.js"

describe("MyWidget with real DOM", () => {
  test("responds to click events", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeHappyDomLayer()

          yield* Effect.gen(function* () {
            const harness = yield* TestHarnessTag
            const browser = yield* TestBrowserTag

            const handle = yield* harness.mount(MyWidget)

            // Click a button
            yield* browser.click("[data-action='submit']")

            // Wait for state change
            yield* handle.waitForState(s => s.submitted === true)

            // Assert DOM updated
            yield* browser.expectText(".status", "Submitted!")
          }).pipe(Effect.provide(layer))
        })
      )
    )
  })
})
```

---

## Test Layers in Detail

### makeTestLayer()

`src/effuse/layers/test.ts`

The simplest test layer with mock services:

```typescript
const { layer, getRendered, injectMessage, clearRendered } = yield* makeTestLayer()
```

**Returns:**
- `layer` - Effect layer to provide to tests
- `getRendered(element)` - Get rendered HTML for element
- `injectMessage(msg)` - Inject HudMessage into socket stream
- `clearRendered()` - Reset all rendered content

**Mock Behavior:**
- DOM: Stores renders in Map, no real elements
- Socket: All requests fail, use `injectMessage` for messages
- State: Real implementation (works without DOM)

**Best for:**
- Render output verification
- State management logic
- Socket message handling

### makeHappyDomLayer()

`src/effuse/testing/layers/happy-dom.ts`

Real DOM via Happy-DOM plus testing utilities:

```typescript
const { layer, window, injectMessage, cleanup } = yield* makeHappyDomLayer()
```

**Returns:**
- `layer` - Composite layer with all services + TestBrowser + TestHarness
- `window` - Happy-DOM Window instance
- `injectMessage(msg)` - Inject HudMessage into socket stream
- `cleanup` - Dispose resources

**Provides via layer:**
- `DomServiceTag` - Happy-DOM backed
- `StateServiceTag` - Real implementation
- `SocketServiceTag` - Mock with message injection
- `TestBrowserTag` - DOM query/assertion utilities
- `TestHarnessTag` - Widget mounting utilities

**Best for:**
- Event handling tests
- DOM interaction tests
- Integration tests

---

## TestHarness API

`src/effuse/testing/harness.ts`

Mount widgets and access internals:

```typescript
const harness = yield* TestHarnessTag

// Mount widget
const handle = yield* harness.mount(MyWidget, {
  containerId: "custom-id",     // Optional
  initialState: { count: 5 },   // Optional override
})

// Access widget handle
yield* handle.getState          // Effect<State>
yield* handle.setState(newState)    // Effect<void>
yield* handle.updateState(s => ({ ...s, updated: true }))

// Emit events directly (bypasses DOM)
yield* handle.emit({ type: "increment" })

// Wait for state conditions
const state = yield* handle.waitForState(
  s => s.count > 10,
  { timeout: 1000, interval: 50 }
)

// DOM access
const html = yield* handle.getHTML  // Get innerHTML
yield* handle.waitForRender         // Allow re-render fiber to run
```

### WidgetHandle<S, E>

```typescript
interface WidgetHandle<S, E> {
  container: Element

  // State
  getState: Effect<S>
  setState: (state: S) => Effect<void>
  updateState: (f: (s: S) => S) => Effect<void>
  stateChanges: Stream<S>

  // Events
  emit: (event: E) => Effect<void>

  // Waiting
  waitForState: (predicate: (s: S) => boolean, options?: WaitOptions) => Effect<S, TestError>

  // DOM
  getHTML: Effect<string>
  waitForRender: Effect<void>
}
```

---

## TestBrowser API

`src/effuse/testing/browser.ts`

DOM queries and assertions:

```typescript
const browser = yield* TestBrowserTag

// Queries
yield* browser.query(".my-element")          // Effect<Element, TestError>
yield* browser.queryOption(".optional")      // Effect<Element | null>
yield* browser.queryAll(".items")            // Effect<Element[]>

// Actions
yield* browser.click("[data-action='save']")
yield* browser.type("input[name='email']", "test@example.com")
yield* browser.clear("input[name='email']")
yield* browser.check("input[type='checkbox']", true)
yield* browser.dispatchEvent(".element", "focus")

// Inspection
yield* browser.getInnerHTML(".container")
yield* browser.getText(".label")
yield* browser.getAttribute(".link", "href")
yield* browser.isVisible(".modal")
yield* browser.exists(".element")

// Assertions (fail with TestError if not met)
yield* browser.expectText(".status", "Success")
yield* browser.expectVisible(".modal")
yield* browser.expectHidden(".loading")
yield* browser.expectCount(".items li", 5)
yield* browser.expectAttribute(".link", "href", "/home")

// Waiting
yield* browser.waitFor(".loading")           // Wait for element to appear
yield* browser.waitForHidden(".loading")     // Wait for element to disappear
yield* browser.waitForText(".status", "Done") // Wait for text content
```

---

## Testing Patterns

### Testing Render Output

```typescript
test("renders with custom state", async () => {
  await Effect.runPromise(
    Effect.scoped(
      Effect.gen(function* () {
        const { layer, getRendered } = yield* makeTestLayer()

        // Widget with custom initial state
        const widget = {
          ...MyWidget,
          initialState: () => ({ items: ["a", "b", "c"] }),
        }

        const container = { id: "test" } as Element
        yield* mountWidget(widget, container).pipe(Effect.provide(layer))

        const html = yield* getRendered(container)
        expect(html).toContain("<li>a</li>")
        expect(html).toContain("<li>b</li>")
        expect(html).toContain("<li>c</li>")
      })
    )
  )
})
```

### Testing Socket Subscriptions

```typescript
test("updates on socket message", async () => {
  await Effect.runPromise(
    Effect.scoped(
      Effect.gen(function* () {
        const { layer, getRendered, injectMessage } = yield* makeTestLayer()
        const container = { id: "test" } as Element

        yield* mountWidget(APMWidget, container).pipe(Effect.provide(layer))

        // Initial state
        let html = yield* getRendered(container)
        expect(html).toContain("0.0")

        // Inject socket message
        yield* injectMessage({
          type: "apm_update",
          sessionId: "test",
          sessionAPM: 15.5,
          recentAPM: 12.0,
          totalActions: 50,
          durationMinutes: 10,
        })

        // Allow stream to process
        yield* Effect.sleep(50)

        // Check updated render
        html = yield* getRendered(container)
        expect(html).toContain("15.5")
      })
    )
  )
})
```

### Testing Event Handling

```typescript
test("handles click events", async () => {
  await Effect.runPromise(
    Effect.scoped(
      Effect.gen(function* () {
        const { layer } = yield* makeHappyDomLayer()

        yield* Effect.gen(function* () {
          const harness = yield* TestHarnessTag
          const browser = yield* TestBrowserTag

          const handle = yield* harness.mount(CounterWidget)

          // Initial state
          let state = yield* handle.getState
          expect(state.count).toBe(0)

          // Click increment
          yield* browser.click("[data-action='increment']")
          yield* Effect.sleep(10)  // Allow event to process

          // Check state updated
          state = yield* handle.getState
          expect(state.count).toBe(1)
        }).pipe(Effect.provide(layer))
      })
    )
  )
})
```

### Testing Async State Changes

```typescript
test("waits for async state", async () => {
  await Effect.runPromise(
    Effect.scoped(
      Effect.gen(function* () {
        const { layer } = yield* makeHappyDomLayer()

        yield* Effect.gen(function* () {
          const harness = yield* TestHarnessTag

          const handle = yield* harness.mount(AsyncWidget)

          // Trigger async operation
          yield* handle.emit({ type: "loadData" })

          // Wait for loading state
          yield* handle.waitForState(s => s.loading === true)

          // Wait for loaded state (with timeout)
          const finalState = yield* handle.waitForState(
            s => s.loading === false && s.data !== null,
            { timeout: 5000 }
          )

          expect(finalState.data).toBeDefined()
        }).pipe(Effect.provide(layer))
      })
    )
  )
})
```

### Testing with Custom Mock Services

```typescript
import { makeCustomTestLayer } from "../effuse/layers/test.js"

test("with custom socket behavior", async () => {
  await Effect.runPromise(
    Effect.scoped(
      Effect.gen(function* () {
        const { layer, getRendered } = yield* makeCustomTestLayer({
          socketService: {
            loadTBSuite: (path) => Effect.succeed({
              id: "test-suite",
              name: "Test Suite",
              tasks: [{ id: "1", title: "Task 1" }],
            }),
          },
        })

        const container = { id: "test" } as Element
        yield* mountWidget(TBControlsWidget, container).pipe(Effect.provide(layer))

        // Widget can now successfully call loadTBSuite
      })
    )
  )
})
```

---

## Test File Organization

```
src/effuse/widgets/
├── my-widget.ts           # Widget implementation
└── my-widget.test.ts      # Widget tests

src/effuse/testing/
├── index.ts               # Public testing exports
├── harness.ts             # TestHarness interface
├── browser.ts             # TestBrowser interface
├── errors.ts              # TestError types
├── happy-dom.test.ts      # Layer tests
└── layers/
    ├── happy-dom.ts       # Happy-DOM layer
    └── webview.ts         # Playwright layer (E2E)
```

---

## Common Test Patterns

### Test Fixture Pattern

```typescript
const setupWidget = () =>
  Effect.gen(function* () {
    const { layer, injectMessage } = yield* makeHappyDomLayer()
    const harness = yield* TestHarnessTag.pipe(Effect.provide(layer))
    const handle = yield* harness.mount(MyWidget).pipe(Effect.provide(layer))
    return { handle, injectMessage, layer }
  })

test("scenario 1", async () => {
  await Effect.runPromise(
    Effect.scoped(
      Effect.gen(function* () {
        const { handle } = yield* setupWidget()
        // test logic
      })
    )
  )
})
```

### Parameterized Tests

```typescript
const testCases = [
  { input: 0, expected: "zero" },
  { input: 1, expected: "one" },
  { input: 2, expected: "two" },
]

for (const { input, expected } of testCases) {
  test(`displays "${expected}" for ${input}`, async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const widget = {
            ...MyWidget,
            initialState: () => ({ value: input }),
          }
          const container = { id: "test" } as Element
          yield* mountWidget(widget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain(expected)
        })
      )
    )
  })
}
```

---

## Running Tests

```bash
# Run all Effuse tests
bun test src/effuse/

# Run specific widget tests
bun test src/effuse/widgets/apm-widget.test.ts

# Run with watch
bun test --watch src/effuse/

# Run with coverage
bun test --coverage src/effuse/
```

---

## Debugging Tests

### Inspect Rendered HTML

```typescript
const html = yield* getRendered(container)
console.log("Rendered HTML:", html)
```

### Inspect State Changes

```typescript
yield* pipe(
  handle.stateChanges,
  Stream.take(5),
  Stream.tap(state => Effect.sync(() => console.log("State:", state))),
  Stream.runDrain,
  Effect.forkScoped
)
```

### Add Delays to Debug Timing

```typescript
yield* Effect.sleep(1000)  // Pause to inspect state
```

### Use Longer Timeouts

```typescript
yield* handle.waitForState(
  s => s.done,
  { timeout: 10000 }  // 10 seconds for debugging
)
```

---

## Error Types

### TestError

```typescript
export class TestError extends Error {
  readonly _tag = "TestError"
  constructor(readonly info: {
    reason: "timeout" | "element_not_found" | "assertion_failed" | "mount_failed"
    message: string
  }) {
    super(info.message)
  }
}
```

Handle in tests:

```typescript
yield* Effect.catchTag("TestError", (e) => {
  console.error("Test failed:", e.info.reason, e.info.message)
  return Effect.fail(e)
})
```
