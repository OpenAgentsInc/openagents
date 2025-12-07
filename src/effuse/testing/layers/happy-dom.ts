/**
 * Effuse Happy-DOM Test Layer
 *
 * Fast in-process DOM testing using Happy-DOM.
 * Provides real DOM behavior without browser overhead (~10ms per test).
 */

import { Window } from "happy-dom"
import { Effect, Layer, Ref, Queue, Stream, Scope, pipe } from "effect"
import type { DomService } from "../../services/dom.js"
import { DomServiceTag, DomError } from "../../services/dom.js"
import { StateServiceTag } from "../../services/state.js"
import { StateServiceLive } from "../../services/state-live.js"
import { SocketServiceTag, SocketError, type SocketService } from "../../services/socket.js"
import type { HudMessage } from "../../../hud/protocol.js"
import type { TemplateResult } from "../../template/types.js"
import type { Widget, WidgetContext } from "../../widget/types.js"
import type { StateCell } from "../../state/cell.js"
import { TestError, type WaitOptions } from "../errors.js"
import type { TestBrowser } from "../browser.js"
import { TestBrowserTag } from "../browser.js"
import type { TestHarness, WidgetHandle } from "../harness.js"
import { TestHarnessTag } from "../harness.js"

// Type alias for happy-dom's Document type
type HappyDocument = InstanceType<typeof Window>["document"]

/**
 * Result from creating a Happy-DOM test layer.
 */
export interface HappyDomLayerResult {
  /** Composed Effect layer with all services */
  readonly layer: Layer.Layer<
    TestBrowserTag | TestHarnessTag | DomServiceTag | StateServiceTag | SocketServiceTag
  >
  /** The Happy-DOM Window instance */
  readonly window: InstanceType<typeof Window>
  /** Shortcut to inject HUD messages */
  readonly injectMessage: (msg: HudMessage) => Effect.Effect<void, never>
  /** Clean up resources */
  readonly cleanup: Effect.Effect<void, never>
}

/**
 * Internal state for tracking mounted widgets.
 */
interface MountedWidgetInternal<S, E> {
  container: Element
  state: StateCell<S>
  eventQueue: Queue.Queue<E>
}

/**
 * Create DomService backed by Happy-DOM.
 * Uses type assertions to bridge happy-dom types to native DOM types.
 */
const makeDomService = (document: HappyDocument): DomService => ({
  query: <T extends Element = Element>(selector: string) =>
    Effect.gen(function* () {
      const el = document.querySelector(selector)
      if (!el) {
        return yield* Effect.fail(
          new DomError("element_not_found", `Element not found: ${selector}`)
        )
      }
      return el as unknown as T
    }),

  queryOption: <T extends Element = Element>(selector: string) =>
    Effect.succeed(document.querySelector(selector) as unknown as T | null),

  queryId: <T extends Element = Element>(id: string) =>
    Effect.gen(function* () {
      const el = document.getElementById(id)
      if (!el) {
        return yield* Effect.fail(
          new DomError("element_not_found", `Element not found: #${id}`)
        )
      }
      return el as unknown as T
    }),

  render: (element: Element, content: TemplateResult) =>
    Effect.sync(() => {
      element.innerHTML = content.toString()
    }),

  listen: <K extends keyof HTMLElementEventMap>(
    element: Element,
    event: K,
    handler: (e: HTMLElementEventMap[K]) => void,
    options?: AddEventListenerOptions
  ) =>
    Effect.sync(() => {
      element.addEventListener(event, handler as EventListener, options)
      return () => element.removeEventListener(event, handler as EventListener)
    }),

  delegate: <K extends keyof HTMLElementEventMap>(
    container: Element,
    selector: string,
    event: K,
    handler: (e: HTMLElementEventMap[K], target: Element) => void
  ) =>
    Effect.sync(() => {
      const delegatedHandler = (e: Event) => {
        const target = (e.target as Element)?.closest?.(selector)
        if (target && container.contains(target)) {
          handler(e as HTMLElementEventMap[K], target)
        }
      }
      container.addEventListener(event, delegatedHandler)
      return () => container.removeEventListener(event, delegatedHandler)
    }),

  createFragment: (content: TemplateResult) =>
    Effect.sync(() => {
      const template = document.createElement("template")
      template.innerHTML = content.toString()
      return template.content as unknown as DocumentFragment
    }),
})

/**
 * Create mock SocketService with message injection.
 */
const makeSocketService = (
  messageQueue: Queue.Queue<HudMessage>
): SocketService => ({
  connect: () => Effect.void,
  disconnect: () => Effect.void,
  isConnected: () => Effect.succeed(true),
  getMessages: () => Stream.fromQueue(messageQueue),
  loadTBSuite: (_) =>
    Effect.fail(new SocketError("request_failed", "Mock: loadTBSuite")),
  startTBRun: (_) =>
    Effect.fail(new SocketError("request_failed", "Mock: startTBRun")),
  stopTBRun: () =>
    Effect.fail(new SocketError("request_failed", "Mock: stopTBRun")),
  loadRecentTBRuns: (_) =>
    Effect.fail(new SocketError("request_failed", "Mock: loadRecentTBRuns")),
  loadTBRunDetails: (_) =>
    Effect.fail(new SocketError("request_failed", "Mock: loadTBRunDetails")),
  loadReadyTasks: (_) =>
    Effect.fail(new SocketError("request_failed", "Mock: loadReadyTasks")),
  assignTaskToMC: (_, __) =>
    Effect.fail(new SocketError("request_failed", "Mock: assignTaskToMC")),
  loadUnifiedTrajectories: (_) =>
    Effect.fail(new SocketError("request_failed", "Mock: loadUnifiedTrajectories")),
  getHFTrajectoryCount: () => Effect.succeed(0),
  getHFTrajectories: (_, __) => Effect.succeed([]),
})

/**
 * Create TestBrowser backed by Happy-DOM document.
 */
const makeTestBrowser = (document: HappyDocument): TestBrowser => {
  const DEFAULT_TIMEOUT = 5000
  const DEFAULT_INTERVAL = 50

  const poll = <T>(
    check: () => T | null,
    options?: WaitOptions
  ): Effect.Effect<T, TestError> => {
    const timeout = options?.timeout ?? DEFAULT_TIMEOUT
    const interval = options?.interval ?? DEFAULT_INTERVAL
    const startTime = Date.now()

    const loop: Effect.Effect<T, TestError> = Effect.gen(function* () {
      const result = check()
      if (result !== null) {
        return result
      }
      if (Date.now() - startTime > timeout) {
        return yield* Effect.fail(
          new TestError({ reason: "timeout", message: `Timed out after ${timeout}ms` })
        )
      }
      yield* Effect.sleep(interval)
      return yield* loop
    })

    return loop
  }

  return {
    // Queries
    query: <T extends Element = Element>(selector: string) =>
      Effect.gen(function* () {
        const el = document.querySelector(selector)
        if (!el) {
          return yield* Effect.fail(
            new TestError({ reason: "element_not_found", message: `Not found: ${selector}` })
          )
        }
        return el as unknown as T
      }),

    queryOption: <T extends Element = Element>(selector: string) =>
      Effect.succeed(document.querySelector(selector) as unknown as T | null),

    queryAll: <T extends Element = Element>(selector: string) =>
      Effect.succeed(Array.from(document.querySelectorAll(selector)) as unknown as T[]),

    // Actions
    click: (selector: string) =>
      Effect.gen(function* () {
        const el = document.querySelector(selector)
        if (!el) {
          return yield* Effect.fail(
            new TestError({ reason: "element_not_found", message: `Click target not found: ${selector}` })
          )
        }
        ; (el as unknown as HTMLElement).click()
      }),

    type: (selector: string, text: string) =>
      Effect.gen(function* () {
        const el = document.querySelector(selector)
        if (!el) {
          return yield* Effect.fail(
            new TestError({ reason: "element_not_found", message: `Input not found: ${selector}` })
          )
        }
        const input = el as unknown as HTMLInputElement
        input.value = text
        input.dispatchEvent(new Event("input", { bubbles: true }))
        input.dispatchEvent(new Event("change", { bubbles: true }))
      }),

    clear: (selector: string) =>
      Effect.gen(function* () {
        const el = document.querySelector(selector)
        if (!el) {
          return yield* Effect.fail(
            new TestError({ reason: "element_not_found", message: `Input not found: ${selector}` })
          )
        }
        const input = el as unknown as HTMLInputElement
        input.value = ""
        input.dispatchEvent(new Event("input", { bubbles: true }))
      }),

    check: (selector: string, checked = true) =>
      Effect.gen(function* () {
        const el = document.querySelector(selector)
        if (!el) {
          return yield* Effect.fail(
            new TestError({ reason: "element_not_found", message: `Checkbox not found: ${selector}` })
          )
        }
        const input = el as unknown as HTMLInputElement
        input.checked = checked
        input.dispatchEvent(new Event("change", { bubbles: true }))
      }),

    dispatchEvent: (selector: string, event: Event | string) =>
      Effect.gen(function* () {
        const el = document.querySelector(selector)
        if (!el) {
          return yield* Effect.fail(
            new TestError({ reason: "element_not_found", message: `Event target not found: ${selector}` })
          )
        }
        // Create event using the document's native Event constructor for happy-dom compatibility
        if (typeof event === "string") {
          const nativeEvent = document.createEvent("Event")
          nativeEvent.initEvent(event, true, true)
          el.dispatchEvent(nativeEvent)
        } else {
          el.dispatchEvent(event as unknown as Parameters<typeof el.dispatchEvent>[0])
        }
      }),

    // Inspection
    getInnerHTML: (selector: string) =>
      Effect.gen(function* () {
        const el = document.querySelector(selector)
        if (!el) {
          return yield* Effect.fail(
            new TestError({ reason: "element_not_found", message: `Not found: ${selector}` })
          )
        }
        return el.innerHTML
      }),

    getText: (selector: string) =>
      Effect.gen(function* () {
        const el = document.querySelector(selector)
        if (!el) {
          return yield* Effect.fail(
            new TestError({ reason: "element_not_found", message: `Not found: ${selector}` })
          )
        }
        return el.textContent ?? ""
      }),

    getAttribute: (selector: string, attribute: string) =>
      Effect.gen(function* () {
        const el = document.querySelector(selector)
        if (!el) {
          return yield* Effect.fail(
            new TestError({ reason: "element_not_found", message: `Not found: ${selector}` })
          )
        }
        return el.getAttribute(attribute)
      }),

    isVisible: (selector: string) =>
      Effect.succeed(document.querySelector(selector) !== null),

    exists: (selector: string) =>
      Effect.succeed(document.querySelector(selector) !== null),

    // Assertions
    expectText: (selector: string, text: string) =>
      Effect.gen(function* () {
        const el = document.querySelector(selector)
        if (!el) {
          return yield* Effect.fail(
            new TestError({
              reason: "assertion_failed",
              message: `Expected "${selector}" to exist`,
            })
          )
        }
        if (!el.textContent?.includes(text)) {
          return yield* Effect.fail(
            new TestError({
              reason: "assertion_failed",
              message: `Expected "${selector}" to contain "${text}", got "${el.textContent}"`,
            })
          )
        }
      }),

    expectVisible: (selector: string) =>
      Effect.gen(function* () {
        const el = document.querySelector(selector)
        if (!el) {
          return yield* Effect.fail(
            new TestError({
              reason: "assertion_failed",
              message: `Expected "${selector}" to be visible, but not found`,
            })
          )
        }
      }),

    expectHidden: (selector: string) =>
      Effect.gen(function* () {
        const el = document.querySelector(selector)
        if (el) {
          return yield* Effect.fail(
            new TestError({
              reason: "assertion_failed",
              message: `Expected "${selector}" to be hidden, but found`,
            })
          )
        }
      }),

    expectCount: (selector: string, count: number) =>
      Effect.gen(function* () {
        const els = document.querySelectorAll(selector)
        if (els.length !== count) {
          return yield* Effect.fail(
            new TestError({
              reason: "assertion_failed",
              message: `Expected "${selector}" count to be ${count}, got ${els.length}`,
            })
          )
        }
      }),

    expectAttribute: (selector: string, attribute: string, value: string) =>
      Effect.gen(function* () {
        const el = document.querySelector(selector)
        if (!el) {
          return yield* Effect.fail(
            new TestError({
              reason: "assertion_failed",
              message: `Expected "${selector}" to exist`,
            })
          )
        }
        const actual = el.getAttribute(attribute)
        if (actual !== value) {
          return yield* Effect.fail(
            new TestError({
              reason: "assertion_failed",
              message: `Expected "${selector}" ${attribute}="${value}", got "${actual}"`,
            })
          )
        }
      }),

    // Waiting
    waitFor: (selector: string, options?: WaitOptions) =>
      poll(() => document.querySelector(selector) as unknown as Element | null, options) as Effect.Effect<Element, TestError>,

    waitForHidden: (selector: string, options?: WaitOptions) =>
      poll(
        () => (document.querySelector(selector) === null ? true : null),
        options
      ).pipe(Effect.asVoid),

    waitForText: (selector: string, text: string, options?: WaitOptions) =>
      poll(() => {
        const el = document.querySelector(selector)
        return el?.textContent?.includes(text) ? true : null
      }, options).pipe(Effect.asVoid),
  }
}

/**
 * Create a Happy-DOM backed test layer.
 *
 * Provides all Effuse services plus testing utilities.
 *
 * @example
 * ```typescript
 * test("widget responds to events", async () => {
 *   await Effect.runPromise(
 *     Effect.scoped(
 *       Effect.gen(function* () {
 *         const { layer } = yield* makeHappyDomLayer()
 *         const harness = yield* TestHarnessTag
 *         const browser = yield* TestBrowserTag
 *
 *         const handle = yield* harness.mount(MyWidget)
 *         yield* handle.emit({ type: "click" })
 *         yield* browser.expectText(".status", "clicked")
 *       }).pipe(Effect.provide(layer))
 *     )
 *   )
 * })
 * ```
 */
export const makeHappyDomLayer = (): Effect.Effect<
  HappyDomLayerResult,
  never,
  Scope.Scope
> =>
  Effect.gen(function* () {
    // Create Happy-DOM window
    const window = new Window({ url: "http://localhost/" })
    const document = window.document

    // Create a container element for widgets
    const rootContainer = document.createElement("div")
    rootContainer.id = "test-root"
    document.body.appendChild(rootContainer)

    // Message queue for socket injection
    const messageQueue = yield* Queue.unbounded<HudMessage>()
    yield* Effect.addFinalizer(() => Queue.shutdown(messageQueue))

    // Track mounted widgets for cleanup
    const mountedWidgets = yield* Ref.make<MountedWidgetInternal<unknown, unknown>[]>([])

    // Create services
    const domService = makeDomService(document)
    const socketService = makeSocketService(messageQueue)
    const browser = makeTestBrowser(document)

    // Widget counter for unique IDs
    let widgetCounter = 0

    // Create test harness
    const harness: TestHarness = {
      mount: <S, E, R>(
        widget: Widget<S, E, R>,
        options?: { containerId?: string; initialState?: S }
      ): Effect.Effect<WidgetHandle<S, E>, TestError, R | StateServiceTag | Scope.Scope> =>
        Effect.gen(function* () {
          const stateService = yield* StateServiceTag

          // Create container
          const containerId = options?.containerId ?? `widget-${++widgetCounter}`
          const container = document.createElement("div")
          container.id = containerId
          rootContainer.appendChild(container)

          // Create state cell
          const initialState = options?.initialState ?? widget.initialState()
          const state = yield* stateService.cell(initialState)

          // Create event queue
          const eventQueue = yield* Effect.acquireRelease(
            Queue.unbounded<E>(),
            (queue) => Queue.shutdown(queue)
          )

          // Build widget context
          const ctx: WidgetContext<S, E> = {
            state,
            emit: (event) => Queue.offer(eventQueue, event),
            dom: domService,
            container: container as unknown as Element,
          }

          // Initial render - map DomError to TestError
          const initialContent = yield* widget.render(ctx)
          yield* domService.render(container as unknown as Element, initialContent).pipe(
            Effect.mapError((e) => new TestError({ reason: "mount_failed", message: e.message }))
          )

          // Set up events
          if (widget.setupEvents) {
            yield* widget.setupEvents(ctx)
          }

          // Re-render on state changes
          yield* pipe(
            state.changes,
            Stream.tap(() =>
              Effect.gen(function* () {
                const content = yield* widget.render(ctx)
                yield* domService.render(container as unknown as Element, content).pipe(
                  Effect.catchAll(() => Effect.void) // Ignore render errors in background
                )
              })
            ),
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

          // Start subscriptions
          if (widget.subscriptions) {
            const subs = widget.subscriptions(ctx)
            for (const sub of subs) {
              yield* pipe(
                sub,
                Stream.tap((effect) => effect),
                Stream.runDrain,
                Effect.forkScoped
              )
            }
          }

          // Track for cleanup
          yield* Ref.update(mountedWidgets, (widgets) => [
            ...widgets,
            { container: container as unknown as Element, state, eventQueue } as MountedWidgetInternal<unknown, unknown>,
          ])

          // Return widget handle
          const handle: WidgetHandle<S, E> = {
            container: container as unknown as Element,

            getState: state.get,
            setState: state.set,
            updateState: state.update,
            emit: (event) => Queue.offer(eventQueue, event),

            stateChanges: state.changes,

            waitForState: (predicate, opts) => {
              const timeout = opts?.timeout ?? 5000
              const interval = opts?.interval ?? 50
              const startTime = Date.now()

              const loop: Effect.Effect<S, TestError> = Effect.gen(function* () {
                const current = yield* state.get
                if (predicate(current)) {
                  return current
                }
                if (Date.now() - startTime > timeout) {
                  return yield* Effect.fail(
                    new TestError({
                      reason: "timeout",
                      message: `State predicate not satisfied after ${timeout}ms`,
                    })
                  )
                }
                yield* Effect.sleep(interval)
                return yield* loop
              })

              return loop
            },

            getHTML: Effect.sync(() => container.innerHTML),
            waitForRender: Effect.sleep(10), // Allow re-render fiber to run
          }

          return handle
        }),

      injectMessage: (msg) => Queue.offer(messageQueue, msg),

      injectSequence: (messages, delayMs = 10) =>
        Effect.gen(function* () {
          for (const msg of messages) {
            yield* Queue.offer(messageQueue, msg)
            yield* Effect.sleep(delayMs)
          }
        }),

      cleanup: Effect.gen(function* () {
        const widgets = yield* Ref.get(mountedWidgets)
        for (const w of widgets) {
          w.container.remove()
        }
        yield* Ref.set(mountedWidgets, [])
      }),
    }

    // Compose layer
    const layer = Layer.mergeAll(
      Layer.succeed(DomServiceTag, domService),
      Layer.succeed(SocketServiceTag, socketService),
      Layer.succeed(TestBrowserTag, browser),
      Layer.succeed(TestHarnessTag, harness),
      StateServiceLive
    )

    return {
      layer,
      window,
      injectMessage: (msg) => Queue.offer(messageQueue, msg),
      cleanup: Effect.sync(() => window.close()),
    }
  })
