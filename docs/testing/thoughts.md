> Building this. Help me flesh out the testing framework. I want headless or headed E2E testing. Effect native no external bullshit

Cool, this is a *really* nice shape already. Let’s lock in a concrete, Effect-native testing stack that gives you:

* **Headless E2E** (Effuse widget + real DOM + fake socket)
* **Headed E2E** (same test API, but backed by Playwright/electrobun/whatever)
* **No jest/vitest bullshit** — just `bun test` + Effect.

I’ll avoid re-explaining what you already wrote and focus on **fleshing out missing pieces + making it executable**.

---

## 0. Testing Tiers (what we’re actually building)

You’re basically building 3 layers on the same Effect API:

1. **Widget-level tests (no DOM)**

   * Directly exercise `Widget<S,E,R>` with mock DomService that just records HTML strings.
   * Super fast, pure logic tests.

2. **Headless DOM E2E (Happy-DOM)**

   * Real DOM tree in memory, real events, real `DomService`.
   * Effect-wrapped `TestBrowser` API for “click, type, expectText, waitFor…”.

3. **Headed E2E (Playwright / Electrobun)**

   * Real browser window & your actual mainview.
   * Same `TestBrowser` + `TestHarness` shape, just a different layer.

The key: **Test code imports only `TestBrowserTag` + `TestHarnessTag`** and doesn’t care whether it’s mock/happy-dom/playwright.

---

## 1. Small but important addition: WidgetRuntime & Registry

You want to steer widgets via Effect (state + events) **from the outside**.

Right now `mountWidget` creates `state` and `ctx` internally and then disappears. Let’s add a *very small* registry service that `mountWidget` uses, and the test harness can read from.

### 1.1 `WidgetRuntime` + `WidgetRegistry`

```ts
// src/effuse/widget/runtime.ts
import { Context, Effect } from "effect"
import type { StateCell } from "../services/state.js"

export interface WidgetRuntime<S = any, E = any> {
  id: string
  container: Element
  state: StateCell<S>
  emit: (event: E) => Effect.Effect<void, never>
}

export interface WidgetRegistry {
  register: (runtime: WidgetRuntime) => Effect.Effect<void, never>
  findById: (id: string) => Effect.Effect<WidgetRuntime | null, never>
  findByContainer: (container: Element) => Effect.Effect<WidgetRuntime | null, never>
}

export class WidgetRegistryTag extends Context.Tag("effuse/WidgetRegistry")<
  WidgetRegistryTag,
  WidgetRegistry
>() {}
```

**Production impl** is basically a no-op (so you don’t couple runtime to tests):

```ts
// src/effuse/widget/runtime-live.ts
import { Layer, Effect, Ref } from "effect"
import { WidgetRegistryTag, WidgetRuntime, WidgetRegistry } from "./runtime.js"

const makeWidgetRegistry = Effect.gen(function* () {
  const store = yield* Ref.make(new Map<Element, WidgetRuntime>())

  const registry: WidgetRegistry = {
    register: (runtime) =>
      Ref.update(store, (m) => {
        m.set(runtime.container, runtime)
        return m
      }),

    findById: (id) =>
      Ref.get(store).pipe(
        Effect.map((m) => {
          for (const rt of m.values()) if (rt.id === id) return rt
          return null
        })
      ),

    findByContainer: (container) =>
      Ref.get(store).pipe(
        Effect.map((m) => m.get(container) ?? null)
      )
  }

  return registry
})

export const WidgetRegistryLive = Layer.effect(
  WidgetRegistryTag,
  makeWidgetRegistry
)
```

In production, nobody uses it; in tests, the harness does.

### 1.2 Wire it into `mountWidget`

Just after you build `ctx`, register the runtime:

```ts
// src/effuse/widget/mount.ts
import { Effect, Stream, Queue, Scope } from "effect"
import { DomServiceTag } from "../services/dom.js"
import { StateServiceTag } from "../services/state.js"
import { WidgetRegistryTag } from "./runtime.js"
import type { Widget, WidgetContext } from "./types.js"

export const mountWidget = <S, E, R>(
  widget: Widget<S, E, R>,
  container: Element
): Effect.Effect<void, never, R | DomServiceTag | StateServiceTag | Scope.Scope | WidgetRegistryTag> =>
  Effect.gen(function* () {
    const dom = yield* DomServiceTag
    const stateService = yield* StateServiceTag
    const registry = yield* WidgetRegistryTag

    const state = yield* stateService.cell(widget.initialState())
    const eventQueue = yield* Queue.unbounded<E>()

    const ctx: WidgetContext<S, E> = {
      state,
      emit: (event) => Queue.offer(eventQueue, event),
      dom
    }

    // Register runtime (best-effort; ignore failure)
    yield* registry.register({
      id: widget.id,
      container,
      state,
      emit: ctx.emit
    })

    // Initial render
    const content = yield* widget.render(ctx)
    yield* dom.render(container, content)

    // State changes → re-render
    yield* Stream.fromReadable(state.changes).pipe(
      Stream.tap(() =>
        Effect.gen(function* () {
          const newContent = yield* widget.render(ctx)
          yield* dom.render(container, newContent)
        })
      ),
      Stream.runDrain,
      Effect.forkScoped
    )

    // Event loop
    if (widget.handleEvent) {
      yield* Stream.fromQueue(eventQueue).pipe(
        Stream.tap((event) => widget.handleEvent!(event, ctx)),
        Stream.runDrain,
        Effect.forkScoped
      )
    }

    // Subscriptions
    if (widget.subscriptions) {
      for (const sub of widget.subscriptions(ctx)) {
        yield* sub.pipe(Stream.runDrain, Effect.forkScoped)
      }
    }
  })
```

Now tests can **discover the runtime** after mount and poke state/events through the registry.

---

## 2. Test Core: Errors, Wait, TestBrowser, TestHarness

### 2.1 Shared TestError + WaitOptions

```ts
// src/effuse/testing/errors.ts
import { Data } from "effect"

export class TestError extends Data.TaggedError("TestError")<{
  reason: "element_not_found" | "timeout" | "assertion_failed" | "action_failed"
  message: string
}> {}

export interface WaitOptions {
  timeout?: number // ms
  pollInterval?: number // ms
}
```

### 2.2 `TestBrowser` interface (your DOM API)

```ts
// src/effuse/testing/browser.ts
import { Context, Effect } from "effect"
import { TestError, WaitOptions } from "./errors.js"

export interface TestBrowser {
  // Queries
  query: <T extends Element = Element>(selector: string) => Effect.Effect<T, TestError>
  queryOption: <T extends Element = Element>(selector: string) => Effect.Effect<T | null>
  queryAll:  <T extends Element = Element>(selector: string) => Effect.Effect<T[]>

  // Actions
  click: (selector: string) => Effect.Effect<void, TestError>
  type:  (selector: string, text: string) => Effect.Effect<void, TestError>
  dispatchEvent: (selector: string, event: Event) => Effect.Effect<void, TestError>

  // Inspection
  getInnerHTML: (selector: string) => Effect.Effect<string, TestError>
  getText:      (selector: string) => Effect.Effect<string, TestError>
  getAttribute: (selector: string, attr: string) => Effect.Effect<string | null>
  isVisible:    (selector: string) => Effect.Effect<boolean>

  // Assertions
  expectText:    (selector: string, text: string) => Effect.Effect<void, TestError>
  expectVisible: (selector: string) => Effect.Effect<void, TestError>
  expectCount:   (selector: string, count: number) => Effect.Effect<void, TestError>

  // Waiting
  waitFor:      (selector: string, opts?: WaitOptions) => Effect.Effect<Element, TestError>
  waitForText:  (selector: string, text: string, opts?: WaitOptions) => Effect.Effect<void, TestError>
}

export class TestBrowserTag extends Context.Tag("effuse/TestBrowser")<
  TestBrowserTag,
  TestBrowser
>() {}
```

### 2.3 `WidgetHandle` & `TestHarness`

```ts
// src/effuse/testing/harness.ts
import { Context, Effect, Stream, Scope } from "effect"
import type { Widget } from "../widget/types.js"
import type { WidgetRuntime } from "../widget/runtime.js"
import { WidgetRegistryTag } from "../widget/runtime.js"
import { TestError, WaitOptions } from "./errors.js"
import type { HudMessage } from "../../mainview/shared-types.js"
import { mountWidget } from "../widget/mount.js"

export interface WidgetHandle<S, E> {
  container: Element
  runtime: WidgetRuntime<S, E>

  getState: Effect.Effect<S>
  setState: (s: S) => Effect.Effect<void>
  updateState: (f: (s: S) => S) => Effect.Effect<void>
  emit: (e: E) => Effect.Effect<void>

  stateChanges: Stream.Stream<S>
  waitForState: (
    predicate: (s: S) => boolean,
    opts?: WaitOptions
  ) => Effect.Effect<S, TestError>
}

export interface TestHarness {
  mount: <S, E, R>(
    widget: Widget<S, E, R>,
    containerId?: string
  ) => Effect.Effect<WidgetHandle<S, E>, TestError, R | Scope.Scope>

  injectMessage: (msg: HudMessage) => Effect.Effect<void>
  injectSequence: (msgs: HudMessage[], delayMs?: number) => Effect.Effect<void>

  cleanup: Effect.Effect<void>
}

export class TestHarnessTag extends Context.Tag("effuse/TestHarness")<
  TestHarnessTag,
  TestHarness
>() {}
```

### 2.4 Implementation: `makeTestHarnessLayer` (headless)

Headless harness uses:

* DOM: `document` from happy-dom
* `WidgetRegistry` to grab runtime after mounting
* A test socket layer that exposes `injectMessage`

```ts
// src/effuse/testing/layers/harness-happy-dom.ts
import { Effect, Layer, Ref, Scope, Stream } from "effect"
import { TestHarnessTag, TestHarness, WidgetHandle } from "../harness.js"
import { TestError, WaitOptions } from "../errors.js"
import { WidgetRegistryTag } from "../../effuse/widget/runtime.js"
import { SocketServiceTag } from "../../effuse/services/socket.js"
import { mountWidget } from "../../effuse/widget/mount.js"

export const makeTestHarnessLayer = (
  root: Element
): Layer.Layer<TestHarnessTag | WidgetRegistryTag | SocketServiceTag, never> =>
  Layer.effect(
    TestHarnessTag,
    Effect.gen(function* () {
      const registry = yield* WidgetRegistryTag
      const socket = yield* SocketServiceTag

      const injectQueue = yield* Ref.make<((msg: any) => void) | null>(null)

      // SocketService live impl should be wired such that messages Stream is fed from here.
      // For now assume SocketServiceLiveForTests wraps a Queue we can push into.

      const mountWidgetHandle = <S, E, R>(
        widget: any,
        containerId?: string
      ): Effect.Effect<WidgetHandle<S, E>, TestError, R | Scope.Scope> =>
        Effect.scoped(
          Effect.gen(function* () {
            const container =
              (containerId
                ? (root.querySelector(containerId) as Element | null)
                : null) ?? root.appendChild(root.ownerDocument!.createElement("div"))

            container.setAttribute("data-widget-id", widget.id)

            // mountWidget will register the runtime in WidgetRegistry
            yield* mountWidget<S, E, R>(widget, container)

            const runtime = (yield* registry.findByContainer(container)) as any as
              | import("../../effuse/widget/runtime.js").WidgetRuntime<S, E>
              | null

            if (!runtime) {
              return yield* Effect.fail(
                new TestError({
                  reason: "action_failed",
                  message: `Widget runtime not registered for ${widget.id}`
                })
              )
            }

            const handle: WidgetHandle<S, E> = {
              container,
              runtime,
              getState: runtime.state.get,
              setState: (s) => runtime.state.set(s),
              updateState: (f) =>
                runtime.state.update((current) => f(current as S) as any),
              emit: (e) => runtime.emit(e),
              stateChanges: runtime.state.changes as any,
              waitForState: (pred, opts) =>
                waitForState(runtime.state, pred, opts)
            }

            return handle
          })
        )

      const harness: TestHarness = {
        mount: mountWidgetHandle,

        injectMessage: (msg) =>
          // you’ll hook this to your test SocketService (Queue.offer, etc)
          socket.messages // just to force dependency
            .pipe(Effect.unit) as any, // placeholder; see below

        injectSequence: (msgs, delayMs = 0) =>
          Effect.forEach(msgs, (msg) =>
            harness.injectMessage(msg).pipe(
              delayMs > 0
                ? Effect.delay(delayMs)
                : Effect.unit
            )
          ),

        cleanup: Effect.sync(() => {
          root.innerHTML = ""
        })
      }

      return harness
    })
  )

// Local helper
const waitForState = <S>(
  state: { get: Effect.Effect<S>; changes: Stream.Stream<S> },
  predicate: (s: S) => boolean,
  opts?: WaitOptions
): Effect.Effect<S, TestError> =>
  Effect.gen(function* () {
    const initial = yield* state.get
    if (predicate(initial)) return initial

    const timeout = opts?.timeout ?? 5000
    const poll = opts?.pollInterval ?? 16

    const result = yield* state.changes.pipe(
      Stream.filter(predicate),
      Stream.take(1),
      Stream.runCollect,
      Effect.timeoutFail(
        () =>
          new TestError({
            reason: "timeout",
            message: `waitForState timed out after ${timeout}ms`
          }),
        timeout
      )
    )

    return result[0]!
  })
```

You’ll tie `injectMessage` to whatever mock `SocketService` you already sketched (Queue-backed stream).

---

## 3. Happy-DOM Layer (headless DOM)

This is where TestBrowser actually does things.

```ts
// src/effuse/testing/layers/happy-dom.ts
import { Window } from "happy-dom"
import { Effect, Layer, Scope } from "effect"

import { DomServiceTag, DomService, DomError } from "../../effuse/services/dom.js"
import { StateServiceLive } from "../../effuse/services/state-live.js"
import { WidgetRegistryLive } from "../../effuse/widget/runtime-live.js"
import { TestBrowserTag, TestBrowser } from "../browser.js"
import { TestError } from "../errors.js"
import { makeTestHarnessLayer } from "./harness-happy-dom.js"

export const makeHappyDomTestEnv = (): Effect.Effect<{
  layer: Layer.Layer<
    | DomServiceTag
    | TestBrowserTag
    | WidgetRegistryLive
    | StateServiceLive
    | import("../harness.js").TestHarnessTag
  >
  window: Window
}, never, Scope.Scope> =>
  Effect.gen(function* () {
    const window = new Window({ url: "http://localhost/" })
    const document = window.document

    const domService: DomService = {
      query: (selector) =>
        Effect.try({
          try: () => {
            const el = document.querySelector(selector)
            if (!el)
              throw new DomError("element_not_found", `Selector: ${selector}`)
            return el as Element
          },
          catch: (e) =>
            e instanceof DomError
              ? e
              : new DomError("element_not_found", String(e))
        }),
      queryOption: (selector) =>
        Effect.sync(() => document.querySelector(selector) as Element | null),
      render: (el, content) =>
        Effect.sync(() => {
          el.innerHTML = content.toString()
        }),
      listen: (el, event, handler) =>
        Effect.sync(() => {
          el.addEventListener(event, handler as any)
          return () => el.removeEventListener(event, handler as any)
        })
    }

    const browser: TestBrowser = {
      query: (selector) =>
        domService.query(selector).pipe(
          Effect.catchAll(() =>
            Effect.fail(
              new TestError({
                reason: "element_not_found",
                message: selector
              })
            )
          )
        ),

      queryOption: (selector) => domService.queryOption(selector) as any,
      queryAll: (selector) =>
        Effect.sync(
          () => Array.from(document.querySelectorAll(selector)) as Element[]
        ),

      click: (selector) =>
        Effect.gen(function* () {
          const el = yield* browser.query(selector)
          ;(el as HTMLElement).click()
        }),

      type: (selector, text) =>
        Effect.gen(function* () {
          const el = (yield* browser.query(selector)) as HTMLInputElement
          el.value = ""
          el.dispatchEvent(new window.Event("input"))
          el.value = text
          el.dispatchEvent(new window.Event("input"))
        }),

      dispatchEvent: (selector, event) =>
        Effect.gen(function* () {
          const el = yield* browser.query(selector)
          el.dispatchEvent(event)
        }),

      getInnerHTML: (selector) =>
        Effect.gen(function* () {
          const el = yield* browser.query(selector)
          return el.innerHTML
        }),

      getText: (selector) =>
        Effect.gen(function* () {
          const el = yield* browser.query(selector)
          return el.textContent ?? ""
        }),

      getAttribute: (selector, attr) =>
        Effect.gen(function* () {
          const el = yield* browser.query(selector)
          return el.getAttribute(attr)
        }),

      isVisible: (selector) =>
        Effect.gen(function* () {
          const el = yield* browser.query(selector)
          const style = window.getComputedStyle(el as any)
          return style.display !== "none" && style.visibility !== "hidden"
        }),

      expectText: (selector, text) =>
        Effect.gen(function* () {
          const actual = yield* browser.getText(selector)
          if (!actual.includes(text)) {
            return yield* Effect.fail(
              new TestError({
                reason: "assertion_failed",
                message: `Expected "${selector}" to contain "${text}", got "${actual}"`
              })
            )
          }
        }),

      expectVisible: (selector) =>
        Effect.gen(function* () {
          const visible = yield* browser.isVisible(selector)
          if (!visible) {
            return yield* Effect.fail(
              new TestError({
                reason: "assertion_failed",
                message: `Expected "${selector}" to be visible`
              })
            )
          }
        }),

      expectCount: (selector, count) =>
        Effect.gen(function* () {
          const els = yield* browser.queryAll(selector)
          if (els.length !== count) {
            return yield* Effect.fail(
              new TestError({
                reason: "assertion_failed",
                message: `Expected ${count} "${selector}" elements, got ${els.length}`
              })
            )
          }
        }),

      waitFor: (selector, opts) =>
        Effect.async< Element, TestError>((resume) => {
          const timeout = opts?.timeout ?? 5000
          const start = Date.now()

          const tick = () => {
            const el = document.querySelector(selector)
            if (el) {
              resume(Effect.succeed(el))
            } else if (Date.now() - start > timeout) {
              resume(
                Effect.fail(
                  new TestError({
                    reason: "timeout",
                    message: `waitFor(${selector}) timed out`
                  })
                )
              )
            } else {
              setTimeout(tick, opts?.pollInterval ?? 16)
            }
          }

          tick()
        }),

      waitForText: (selector, text, opts) =>
        Effect.gen(function* () {
          yield* browser.waitFor(selector, opts)
          const timeout = opts?.timeout ?? 5000
          const start = Date.now()

          const loop = (): Effect.Effect<void, TestError> =>
            Effect.gen(function* () {
              const current = yield* browser.getText(selector)
              if (current.includes(text)) return

              if (Date.now() - start > timeout) {
                return yield* Effect.fail(
                  new TestError({
                    reason: "timeout",
                    message: `waitForText("${text}") on "${selector}" timed out`
                  })
                )
              }

              yield* Effect.sleep(opts?.pollInterval ?? 16)
              return yield* loop()
            })

          yield* loop()
        })
    }

    const root = document.body

    const harnessLayer = makeTestHarnessLayer(root)

    const layer = Layer.mergeAll(
      Layer.succeed(DomServiceTag, domService),
      Layer.succeed(TestBrowserTag, browser),
      StateServiceLive,
      WidgetRegistryLive,
      harnessLayer
    )

    return { layer, window }
  })
```

Then your **test barrel**:

```ts
// src/effuse/testing/index.ts
export * from "./errors.js"
export * from "./browser.js"
export * from "./harness.js"
export * from "./layers/happy-dom.js"
// later: playwright layer, mock layer, etc
```

---

## 4. Example: Headless E2E for `MCTasksWidget`

You already have the widget; here’s how a Bun test looks using the infra above.

```ts
// src/effuse/widgets/mc-tasks.e2e.test.ts
import { test, expect } from "bun:test"
import { Effect } from "effect"

import { makeHappyDomTestEnv } from "../testing/layers/happy-dom.js"
import { TestBrowserTag } from "../testing/browser.js"
import { TestHarnessTag } from "../testing/harness.js"
import { MCTasksWidget } from "./mc-tasks.js"

test("MC tasks widget loads and assigns a task", async () => {
  await Effect.runPromise(
    Effect.scoped(
      Effect.gen(function* () {
        const { layer } = yield* makeHappyDomTestEnv()

        const browser = yield* TestBrowserTag
        const harness = yield* TestHarnessTag

        const handle = yield* harness.mount(MCTasksWidget)

        // Seed state directly via Effect
        yield* handle.setState({
          tasks: [
            {
              id: "t1",
              title: "Implement tasks CLI",
              priority: 1,
              type: "task",
              labels: ["cli", "task-system"]
            }
          ],
          loading: false,
          error: null,
          collapsed: false
        })

        // Assert DOM
        yield* browser.expectText("[data-task-id='t1'] .title", "Implement tasks CLI")
        yield* browser.expectCount("[data-task-id]", 1)

        // Click Assign
        yield* browser.click("[data-task-id='t1'] [data-action='assign']")

        // State should update
        const state = yield* handle.waitForState((s) => s.tasks.length === 0)
        expect(state.tasks.length).toBe(0)
      }).pipe(Effect.provide(layer))
    )
  )
})
```

Same shape works whether you run:

* **Headless**: `makeHappyDomTestEnv`
* **Headed**: `makePlaywrightLayer(page, baseUrl)` (below) and reuse test logic (or share a common test function).

---

## 5. Headed E2E: Playwright Layer Sketch

You can hook into Playwright (or your Electrobun window) using **the same `TestBrowser` interface**.

```ts
// src/effuse/testing/layers/playwright.ts
import type { Page } from "@playwright/test"
import { Effect, Layer, Scope } from "effect"
import { TestBrowserTag, TestBrowser } from "../browser.js"
import { TestHarnessTag, TestHarness } from "../harness.js"
import { TestError } from "../errors.js"

export const makePlaywrightLayer = (
  page: Page,
  baseUrl: string
): Effect.Effect<{ layer: Layer.Layer<TestBrowserTag | TestHarnessTag> }, never, Scope.Scope> =>
  Effect.gen(function* () {
    const browser: TestBrowser = {
      query: (selector) =>
        Effect.tryPromise({
          try: async () => {
            const handle = await page.locator(selector).elementHandle()
            if (!handle) {
              throw new TestError({
                reason: "element_not_found",
                message: selector
              })
            }
            // we can't return DOM element here; tests should treat it opaque.
            // For compatibility, we can just return null and avoid using query() in playwright mode.
            return (null as unknown) as Element
          },
          catch: (e) =>
            e instanceof TestError
              ? e
              : new TestError({ reason: "action_failed", message: String(e) })
        }),
      queryOption: () => Effect.succeed(null),
      queryAll: () => Effect.succeed([]),

      click: (selector) =>
        Effect.tryPromise({
          try: () => page.click(selector),
          catch: (e) =>
            new TestError({ reason: "action_failed", message: String(e) })
        }),

      type: (selector, text) =>
        Effect.tryPromise({
          try: () => page.fill(selector, text),
          catch: (e) =>
            new TestError({ reason: "action_failed", message: String(e) })
        }),

      dispatchEvent: () =>
        Effect.fail(
          new TestError({
            reason: "action_failed",
            message: "dispatchEvent not implemented for Playwright"
          })
        ),

      getInnerHTML: (selector) =>
        Effect.tryPromise({
          try: () => page.locator(selector).innerHTML(),
          catch: (e) =>
            new TestError({ reason: "action_failed", message: String(e) })
        }),

      getText: (selector) =>
        Effect.tryPromise({
          try: () => page.locator(selector).innerText(),
          catch: (e) =>
            new TestError({ reason: "action_failed", message: String(e) })
        }),

      getAttribute: (selector, attr) =>
        Effect.tryPromise({
          try: () => page.locator(selector).getAttribute(attr),
          catch: (e) =>
            new TestError({ reason: "action_failed", message: String(e) })
        }),

      isVisible: (selector) =>
        Effect.tryPromise({
          try: () => page.isVisible(selector),
          catch: (e) =>
            new TestError({ reason: "action_failed", message: String(e) })
        }),

      expectText: (selector, text) =>
        Effect.tryPromise({
          try: async () => {
            const content = await page.locator(selector).innerText()
            if (!content.includes(text)) {
              throw new TestError({
                reason: "assertion_failed",
                message: `Expected "${selector}" to contain "${text}", got "${content}"`
              })
            }
          },
          catch: (e) =>
            e instanceof TestError
              ? e
              : new TestError({ reason: "action_failed", message: String(e) })
        }),

      expectVisible: (selector) =>
        Effect.tryPromise({
          try: async () => {
            const visible = await page.isVisible(selector)
            if (!visible) {
              throw new TestError({
                reason: "assertion_failed",
                message: `Expected "${selector}" to be visible`
              })
            }
          },
          catch: (e) =>
            e instanceof TestError
              ? e
              : new TestError({ reason: "action_failed", message: String(e) })
        }),

      expectCount: (selector, count) =>
        Effect.tryPromise({
          try: async () => {
            const actual = await page.locator(selector).count()
            if (actual !== count) {
              throw new TestError({
                reason: "assertion_failed",
                message: `Expected ${count} "${selector}", got ${actual}`
              })
            }
          },
          catch: (e) =>
            e instanceof TestError
              ? e
              : new TestError({ reason: "action_failed", message: String(e) })
        }),

      waitFor: (selector, opts) =>
        Effect.tryPromise({
          try: () =>
            page.waitForSelector(selector, {
              timeout: opts?.timeout ?? 5000
            }).then((h) => (null as unknown) as Element),
          catch: (e) =>
            new TestError({ reason: "timeout", message: String(e) })
        }),

      waitForText: (selector, text, opts) =>
        Effect.tryPromise({
          try: async () => {
            await page.waitForFunction(
              ([sel, t]) =>
                !!Array.from(document.querySelectorAll(sel as string)).find((el) =>
                  (el.textContent || "").includes(t as string)
                ),
              [selector, text],
              { timeout: opts?.timeout ?? 5000 }
            )
          },
          catch: (e) =>
            new TestError({ reason: "timeout", message: String(e) })
        })
    }

    // For full E2E you probably *don't* mount widgets via harness, you just
    // use harness only for message injection (via window.__effuseInject).
    const harness: TestHarness = {
      mount: () =>
        Effect.fail(
          new TestError({
            reason: "action_failed",
            message: "mount() not supported in Playwright mode; use page.goto()"
          })
        ),

      injectMessage: (msg) =>
        Effect.tryPromise({
          try: () =>
            page.evaluate((message) => {
              // You add this global in your mainview bootstrap for test mode
              ;(window as any).__effuseTestInject?.(message)
            }, msg),
          catch: (e) =>
            new TestError({ reason: "action_failed", message: String(e) })
        }),

      injectSequence: (msgs, delayMs = 0) =>
        Effect.forEach(msgs, (msg) =>
          harness.injectMessage(msg).pipe(
            delayMs > 0 ? Effect.delay(delayMs) : Effect.unit
          )
        ),

      cleanup: Effect.void
    }

    const layer = Layer.mergeAll(
      Layer.succeed(TestBrowserTag, browser),
      Layer.succeed(TestHarnessTag, harness)
    )

    return { layer }
  })
```

Then a Playwright test uses it like:

```ts
// e2e/tests/effuse/tb-run.spec.ts
import { test } from "@playwright/test"
import { Effect } from "effect"
import { makePlaywrightLayer } from "../../../src/effuse/testing/layers/playwright.js"
import { TestBrowserTag } from "../../../src/effuse/testing/browser.js"
import { TestHarnessTag } from "../../../src/effuse/testing/harness.js"

test("TB run end-to-end", async ({ page }) => {
  await Effect.runPromise(
    Effect.scoped(
      Effect.gen(function* () {
        const { layer } = yield* makePlaywrightLayer(page, "http://localhost:8080")
        const browser = yield* TestBrowserTag
        const harness = yield* TestHarnessTag

        await page.goto("http://localhost:8080/")

        yield* browser.waitFor("#tb-controls-widget")

        yield* harness.injectMessage({
          type: "tb_run_start",
          runId: "r1",
          // ...
        })

        yield* browser.expectText(".status", "Running")
      }).pipe(Effect.provide(layer))
    )
  )
})
```

---

## 6. Running it: “no external bullshit”

* **Headless tests**: just `bun test` picking up `*.test.ts` that use `makeHappyDomTestEnv`.
  No jest, no vitest, everything Effect-native + Bun.

* **Headed tests**: `npx playwright test` (or `bunx playwright test`) using the Playwright layer.
  You’re just using Playwright as a **driver**, not as a framework: all logic still lives in Effect services.

---

If you want, next step I can do a **tiny PoC repo layout**: 1 widget, 1 happy-dom test, 1 Playwright test, and all the exact import paths wired so you can paste it into `src/effuse/`.


> We replaced Electrobun with simpler bun-webview. Confirm all above works same with that?

Yep, everything still works conceptually the same with bun-webview instead of Electrobun. Nothing in the testing architecture I gave is actually tied to Electrobun.

Let me break it down concretely:

---

## 1. Headless tests (Happy-DOM) ✅

These don’t care *at all* what you use for the desktop shell.

* They run in **Bun**.
* They use **happy-dom** to simulate the browser DOM.
* Your Effuse stack is just:

  * `DomServiceLive` (happy-dom backed)
  * `StateServiceLive`
  * `SocketServiceTest`
  * `WidgetRegistryLive`
  * `TestBrowser` / `TestHarness` layers

Whether you bundle the app with Electrobun, bun-webview, Tauri, or just ship it as a website — headless tests are unchanged.

So: all the `makeHappyDomTestEnv()`, `TestBrowserTag`, `TestHarnessTag`, `WidgetRegistry`, etc. are 100% compatible and don’t need edits.

---

## 2. Message router, widgets, services ✅

Also completely unaffected:

* `Widget<S, E, R>` types
* `mountWidget`
* `WidgetRegistry`
* `DomService`, `StateService`, `SocketService`
* `createMessageRouter` and all your domain handlers

All of that is *pure TS + Effect + DOM*. The desktop container (Electrobun vs bun-webview) is just “how you host the browser”.

---

## 3. Headed E2E: what changes (or not)

There are two possible “headed” stories:

### Option A — Keep Playwright as driver (easiest) ✅

In the design I wrote, the **headed** layer (`makePlaywrightLayer`) was:

* Just a thin wrapper that translates `TestBrowser` operations → Playwright calls.
* Assumes your UI is reachable at `http://localhost:PORT`.

In that model:

* Your **bun-webview binary is irrelevant** to tests.
* You simply:

  * Run your dev server (Bun + Vite/whatever) on a port.
  * Point Playwright at `http://localhost:PORT`.
  * Use `TestBrowserTag` / `TestHarnessTag` to drive the UI.

Switching Electrobun → bun-webview does **not change this at all**, because tests are against the dev server, not the packaged window.

So if you’re ok with Playwright as the “one external thing”, you don’t need to change a single line in the architecture — just mentally replace “Electrobun HUD” with “bun-webview HUD” in your docs.

### Option B — No Playwright, pure Bun + bun-webview 🔧

If you want *zero* external test runner and everything done inside Bun, bun-webview actually fits nicely:

* You run tests with `bun test`.
* Each test:

  * Spins up a bun-webview `WebUI`/`Window` instance.
  * Loads your app (either via `show('<html…>')` or pointing it at a local HTTP server). ([Reddit][1])
  * Installs a small JS bridge inside the webview that:

    * Exposes `window.__effuseTestBridge` with methods like `click(selector)`, `getText(selector)`, etc.
    * Communicates back to Bun via bun-webview’s messaging API.

Then you implement a **`WebviewTestBrowser`** that satisfies the existing `TestBrowser` interface by:

* For each method (click, type, getText, expectText, waitFor…), calling `webview.eval('window.__effuseTestBridge.click("...")')` or equivalent and returning results wrapped in `Effect`.

The nice thing: you **don’t have to change any test code**:

* Tests still depend only on `TestBrowserTag` + `TestHarnessTag`.
* You just provide a different layer: `makeWebviewLayer()` instead of `makePlaywrightLayer()`.

So the API shape stays:

```ts
const { layer } = yield* makeWebviewLayer() // or makePlaywrightLayer()
const browser = yield* TestBrowserTag
const harness = yield* TestHarnessTag
```

Only the *implementation* behind `TestBrowser` changes.

---

## 4. TL;DR

* **Headless Effuse tests** (happy-dom + Effect) → completely unchanged.
* **Effuse core architecture** (widgets, services, router, registry) → unchanged.
* **Headed tests**:

  * If you stick with Playwright as a driver → unchanged.
  * If you want to go full “no external bullshit”, you swap the Playwright-specific layer for a bun-webview-backed `TestBrowser`/`TestHarness`, but the public test API stays identical.

If you tell me which flavor you prefer for headed (keep Playwright vs build a `makeWebviewLayer`), I can sketch the exact `bun-webview` bridge code next.

[1]: https://www.reddit.com/r/bun/comments/1ins1u2/bunwebui_use_any_web_browser_as_your_gui/?utm_source=chatgpt.com "Bun-WebUI - Use Any Web Browser as Your GUI"

> Playwright can’t steer webkit- based browsers I thought or that’s wrong?

Playwright *can* steer WebKit-based browsers — but there are a few nuances.

### 1. What Playwright actually supports

Playwright ships with **three browser engines** out of the box:

* Chromium
* Firefox
* WebKit

You can run tests explicitly against WebKit:

```bash
npx playwright test --project=webkit
```

Under the hood that’s not “your installed Safari”, but a **bundled WebKit build** that Playwright maintains. It’s close to Safari’s engine, and is *designed* to be a cross-platform WebKit target (runs on macOS, Windows, Linux).

So: **yes, Playwright absolutely can steer a WebKit engine**.

### 2. WebKit vs “real Safari” vs bun-webview

Important distinctions:

* **Playwright WebKit**:

  * A Playwright-maintained WebKit binary.
  * Fully scriptable by Playwright (click, type, etc.).
  * Great for cross-engine compatibility testing.

* **Safari app on macOS**:

  * Controlled via WebDriver/SafariDriver, not Playwright.
  * Playwright does *not* automate Safari.app directly; it uses its own WebKit.

* **bun-webview**:

  * On macOS it’s also WebKit under the hood (via WebKit’s C APIs), but it’s not the same binary Playwright uses and not exposed to Playwright at all.
  * Playwright has **no idea** bun-webview exists – it’s just a native window that happens to embed WebKit.

So:

* You **can** run Effuse tests against a WebKit engine via Playwright (good for “how does this behave on WebKit?”).
* You **cannot** use Playwright to directly remote-drive your **bun-webview window**. For that you’d either:

  * Test the *web app* via Playwright (hit `http://localhost:PORT`), or
  * Build your own steering layer on top of bun-webview (our `makeWebviewLayer()` idea).

### 3. How this intersects with Effuse testing

Given your goals (“Effect native, no external bullshit”):

* **Headless / CI**: use Happy-DOM + Effuse `TestBrowser` / `TestHarness`. No browser engines involved.
* **Cross-engine correctness** (Chromium vs WebKit):

  * If you care about “does this render correctly on a WebKit-like engine?”, using Playwright’s WebKit project is totally valid and works.
* **True desktop binary testing (bun-webview HUD)**:

  * Playwright can’t see that window.
  * You’d build a small JS bridge in the webview and drive it from Bun with your own `WebviewTestBrowser`.

So: your earlier testing architecture is still good, you just decide:

* Use **Playwright WebKit** to simulate “a WebKit browser”, OR
* Skip Playwright entirely and drive **bun-webview** directly from Bun via a custom test layer.

If you want, I can sketch a minimal `WebviewTestBrowser` that:

* Spins up a bun-webview window,
* Injects `window.__effuseTestBridge` for `click`, `type`, `querySelector` etc.,
* Implements the existing `TestBrowser` interface so your tests don’t change.
