# Effuse First-Class E2E Testing Plan

## Core Insight

Since Effuse is Effect-native, tests should **steer the UI through Effect** - not just poke at the DOM. Widgets have:
- `StateCell<S>` - directly settable/observable state
- `Queue<E>` - event emission via `ctx.emit()`
- `Stream<HudMessage>` - message subscriptions

This means Effect-native tests can drive widgets at the Effect layer, with DOM verification as validation.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    Test Code (Effect.gen)                       │
│                                                                 │
│   const handle = yield* harness.mount(TBControlsWidget)         │
│   yield* handle.emit({ type: "loadSuite", path: "/suite.json"}) │
│   yield* handle.waitForState(s => s.suites.length > 0)          │
│   yield* browser.expectText(".task-list", "Task 1")             │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
                              │
         ┌────────────────────┼────────────────────┐
         v                    v                    v
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│   Mock Layer    │  │ Happy-DOM Layer │  │ Playwright Layer│
│   (existing)    │  │    (new)        │  │    (new)        │
│                 │  │                 │  │                 │
│ - String render │  │ - Real DOM      │  │ - Real browser  │
│ - No events     │  │ - Real events   │  │ - Screenshots   │
│ - ~1ms          │  │ - ~10ms         │  │ - ~500ms        │
└─────────────────┘  └─────────────────┘  └─────────────────┘
```

## New Dependencies

```bash
bun add -d happy-dom
```

## File Structure

```
src/effuse/
  testing/
    index.ts                 # Public barrel export
    errors.ts                # TestError types

    # Core Services
    browser.ts               # TestBrowser service interface
    harness.ts               # TestHarness for widget mounting

    # Layer Implementations
    layers/
      mock.ts                # Enhanced mock (existing pattern)
      happy-dom.ts           # Happy-DOM backed real DOM
      playwright.ts          # Playwright integration

    # Utilities
    assertions.ts            # Effect-native expect helpers
    wait.ts                  # Stream-based waiting utilities

e2e/
  tests/effuse/
    widgets/                 # Widget-specific E2E tests
    flows/                   # Integration flow tests
```

## Service Interfaces

### TestBrowser - DOM Interaction

```typescript
// src/effuse/testing/browser.ts
export class TestError extends Data.TaggedError("TestError")<{
  reason: "element_not_found" | "timeout" | "assertion_failed"
  message: string
}> {}

export interface TestBrowser {
  // Queries
  query: <T extends Element>(selector: string) => Effect<T, TestError>
  queryOption: <T extends Element>(selector: string) => Effect<T | null>
  queryAll: <T extends Element>(selector: string) => Effect<T[]>

  // Actions
  click: (selector: string) => Effect<void, TestError>
  type: (selector: string, text: string) => Effect<void, TestError>
  dispatchEvent: (selector: string, event: Event) => Effect<void, TestError>

  // Inspection
  getInnerHTML: (selector: string) => Effect<string, TestError>
  getText: (selector: string) => Effect<string, TestError>
  getAttribute: (selector: string, attr: string) => Effect<string | null>
  isVisible: (selector: string) => Effect<boolean>

  // Assertions (Effect-native)
  expectText: (selector: string, text: string) => Effect<void, TestError>
  expectVisible: (selector: string) => Effect<void, TestError>
  expectCount: (selector: string, count: number) => Effect<void, TestError>

  // Waiting (Stream-based)
  waitFor: (selector: string, opts?: WaitOptions) => Effect<Element, TestError>
  waitForText: (selector: string, text: string, opts?: WaitOptions) => Effect<void, TestError>
}

export class TestBrowserTag extends Context.Tag("effuse/TestBrowser")<
  TestBrowserTag, TestBrowser
>() {}
```

### TestHarness - Widget Steering

```typescript
// src/effuse/testing/harness.ts
export interface WidgetHandle<S, E> {
  container: Element

  // Direct Effect steering
  getState: Effect<S>
  setState: (s: S) => Effect<void>
  updateState: (f: (s: S) => S) => Effect<void>
  emit: (event: E) => Effect<void>

  // Observable state (Stream-based)
  stateChanges: Stream<S>
  waitForState: (predicate: (s: S) => boolean, opts?: WaitOptions) => Effect<S, TestError>
}

export interface TestHarness {
  mount: <S, E, R>(
    widget: Widget<S, E, R>,
    containerId?: string
  ) => Effect<WidgetHandle<S, E>, TestError, R | Scope>

  // Socket message injection (existing pattern)
  injectMessage: (msg: HudMessage) => Effect<void>
  injectSequence: (msgs: HudMessage[], delayMs?: number) => Effect<void>

  cleanup: Effect<void>
}

export class TestHarnessTag extends Context.Tag("effuse/TestHarness")<
  TestHarnessTag, TestHarness
>() {}
```

## Layer Implementations

### 1. Happy-DOM Layer (Primary for Unit + Integration)

```typescript
// src/effuse/testing/layers/happy-dom.ts
import { Window } from "happy-dom"

export const makeHappyDomLayer = (): Effect<{
  layer: Layer<TestBrowserTag | TestHarnessTag | DomServiceTag | StateServiceTag>
  window: Window
  cleanup: Effect<void>
}, never, Scope> =>
  Effect.gen(function* () {
    const window = new Window({ url: "http://localhost/" })
    const document = window.document

    // Create real DomService backed by happy-dom
    const domService: DomService = {
      query: (selector) => Effect.try({
        try: () => document.querySelector(selector),
        catch: () => new DomError("query_failed", selector)
      }),
      render: (el, content) => Effect.sync(() => {
        el.innerHTML = content.toString()
      }),
      listen: (el, event, handler) => Effect.sync(() => {
        el.addEventListener(event, handler)
        return () => el.removeEventListener(event, handler)
      }),
      // ... other methods
    }

    // TestBrowser wraps document queries + assertions
    const browser: TestBrowser = {
      click: (selector) => Effect.gen(function* () {
        const el = yield* domService.query(selector)
        el.click()
      }),
      expectText: (selector, text) => Effect.gen(function* () {
        const el = yield* domService.query(selector)
        if (!el.textContent?.includes(text)) {
          yield* Effect.fail(new TestError({
            reason: "assertion_failed",
            message: `Expected "${selector}" to contain "${text}"`
          }))
        }
      }),
      // ... other methods
    }

    return {
      layer: Layer.mergeAll(
        Layer.succeed(TestBrowserTag, browser),
        Layer.succeed(DomServiceTag, domService),
        StateServiceLive,  // Real state service
        makeTestHarnessLayer(browser, domService)
      ),
      window,
      cleanup: Effect.sync(() => window.close())
    }
  })
```

### 2. Playwright Layer (For Real Browser E2E)

```typescript
// src/effuse/testing/layers/playwright.ts
import type { Page } from "@playwright/test"

export const makePlaywrightLayer = (
  page: Page,
  baseUrl: string
): Effect<{
  layer: Layer<TestBrowserTag | TestHarnessTag>
}, never, Scope> =>
  Effect.gen(function* () {
    const browser: TestBrowser = {
      query: (selector) => Effect.tryPromise({
        try: () => page.locator(selector).elementHandle(),
        catch: () => new TestError({ reason: "element_not_found", message: selector })
      }),
      click: (selector) => Effect.tryPromise({
        try: () => page.click(selector),
        catch: (e) => new TestError({ reason: "action_failed", message: String(e) })
      }),
      expectText: (selector, text) => Effect.tryPromise({
        try: async () => {
          await expect(page.locator(selector)).toContainText(text)
        },
        catch: (e) => new TestError({ reason: "assertion_failed", message: String(e) })
      }),
      waitFor: (selector, opts) => Effect.tryPromise({
        try: () => page.waitForSelector(selector, { timeout: opts?.timeout ?? 5000 }),
        catch: () => new TestError({ reason: "timeout", message: selector })
      }),
      // ... wrap all Playwright methods in Effect
    }

    // HUD injection via WebSocket (existing pattern)
    const harness: TestHarness = {
      injectMessage: (msg) => Effect.tryPromise({
        try: () => page.evaluate((m) => {
          window.__effuseTestInject?.(m)
        }, msg),
        catch: (e) => new TestError({ reason: "action_failed", message: String(e) })
      }),
      // ... other methods
    }

    return {
      layer: Layer.mergeAll(
        Layer.succeed(TestBrowserTag, browser),
        Layer.succeed(TestHarnessTag, harness)
      )
    }
  })
```

## Example Tests

### Unit Test (Happy-DOM) - Widget Steering

```typescript
// src/effuse/widgets/tb-controls.test.ts
import { test, expect } from "bun:test"
import { Effect } from "effect"
import { makeHappyDomLayer } from "../testing/layers/happy-dom.js"
import { TBControlsWidget } from "./tb-controls.js"

test("loadSuite event updates state", async () => {
  await Effect.runPromise(
    Effect.scoped(
      Effect.gen(function* () {
        const { layer } = yield* makeHappyDomLayer()
        const harness = yield* TestHarnessTag
        const browser = yield* TestBrowserTag

        // Mount widget - get direct Effect handle
        const handle = yield* harness.mount(TBControlsWidget)

        // Steer via Effect - emit event directly
        yield* handle.emit({
          type: "suiteLoaded",
          suite: { name: "test", tasks: [{ id: "t1", name: "Task 1" }] }
        })

        // Wait for state via Stream
        const state = yield* handle.waitForState(s => s.suites.length > 0)
        expect(state.suites[0].name).toBe("test")

        // Verify DOM reflects state
        yield* browser.expectText("[data-task-id='t1']", "Task 1")
      }).pipe(Effect.provide(layer))
    )
  )
})
```

### Integration Test (Happy-DOM) - Socket Messages

```typescript
test("processes TB run sequence", async () => {
  await Effect.runPromise(
    Effect.scoped(
      Effect.gen(function* () {
        const { layer } = yield* makeHappyDomLayer()
        const harness = yield* TestHarnessTag
        const browser = yield* TestBrowserTag

        yield* harness.mount(TBOutputWidget)

        // Inject message sequence
        yield* harness.injectSequence([
          { type: "tb_run_start", runId: "r1", ... },
          { type: "tb_task_output", runId: "r1", line: "Hello world", ... },
          { type: "tb_run_complete", runId: "r1", ... }
        ], 10)

        yield* browser.expectText(".output-line", "Hello world")
        yield* browser.expectVisible(".status-complete")
      }).pipe(Effect.provide(layer))
    )
  )
})
```

### E2E Test (Playwright) - Full Flow

```typescript
// e2e/tests/effuse/flows/tb-run.spec.ts
import { test } from "@playwright/test"
import { Effect } from "effect"
import { makePlaywrightLayer } from "../../../src/effuse/testing/layers/playwright.js"

test("complete TB run flow", async ({ page }) => {
  await Effect.runPromise(
    Effect.scoped(
      Effect.gen(function* () {
        const { layer } = yield* makePlaywrightLayer(page, "http://localhost:8080")
        const browser = yield* TestBrowserTag
        const harness = yield* TestHarnessTag

        yield* Effect.promise(() => page.goto("/"))
        yield* browser.waitFor("#tb-controls-widget")

        // Inject suite info
        yield* harness.injectMessage({
          type: "tb_suite_info",
          name: "test-suite",
          tasks: [{ id: "t1", name: "Task 1" }]
        })

        yield* browser.expectText(".task-list", "Task 1")
        yield* browser.click("[data-action='startRun']")
        yield* browser.waitForText(".status", "Running", { timeout: 5000 })
      }).pipe(Effect.provide(layer))
    )
  )
})
```

## Implementation Phases

### Phase 1: Core Infrastructure
1. Create `src/effuse/testing/` directory structure
2. Define `TestError` types in `errors.ts`
3. Define `TestBrowser` interface in `browser.ts`
4. Define `TestHarness` interface in `harness.ts`
5. Add `happy-dom` dependency

### Phase 2: Happy-DOM Layer
6. Implement `makeHappyDomLayer()` in `layers/happy-dom.ts`
7. Implement real DomService backed by happy-dom
8. Implement TestBrowser with assertions
9. Implement TestHarness with widget mounting + message injection
10. Add waiting utilities (Stream-based)

### Phase 3: Migrate Existing Tests
11. Update `src/effuse/widgets/*.test.ts` to use new harness
12. Add interaction tests (click handlers, inputs)
13. Verify all 7 widgets have coverage

### Phase 4: Playwright Layer
14. Implement `makePlaywrightLayer()` wrapping Playwright in Effect
15. Add message injection via `page.evaluate()`
16. Create E2E test fixtures

### Phase 5: E2E Test Coverage
17. Create widget-specific E2E tests in `e2e/tests/effuse/widgets/`
18. Create integration flow tests in `e2e/tests/effuse/flows/`
19. Add visual regression tests (screenshots)

## Critical Files to Modify

| File | Changes |
|------|---------|
| `src/effuse/testing/index.ts` | New - barrel export |
| `src/effuse/testing/browser.ts` | New - TestBrowser service |
| `src/effuse/testing/harness.ts` | New - TestHarness service |
| `src/effuse/testing/layers/happy-dom.ts` | New - happy-dom implementation |
| `src/effuse/testing/layers/playwright.ts` | New - Playwright wrapper |
| `src/effuse/layers/test.ts` | Reference for existing patterns |
| `src/effuse/widget/mount.ts` | May need hooks for test access |
| `src/effuse/widgets/*.test.ts` | Migrate to new harness |
| `package.json` | Add happy-dom dependency |
| `e2e/tests/effuse/` | New E2E tests |

## Key Design Decisions

1. **Effect-native first** - All async ops return Effect, not Promise
2. **Widget steering via Effect** - Direct access to StateCell and event Queue
3. **Stream-based waiting** - `waitForState` uses Stream.takeUntil internally
4. **Layered abstraction** - Same test code runs on mock, happy-dom, or Playwright
5. **Happy-DOM as default** - Fast enough for CI, real enough for DOM behavior
6. **Playwright for E2E** - Full browser when needed, wrapped in Effect
