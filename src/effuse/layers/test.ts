/**
 * Effuse Test Layer
 *
 * Mock implementations for testing widgets without a browser or socket.
 */

import { Effect, Layer, Ref, Queue, Stream } from "effect"
import { DomServiceTag, DomError, type DomService } from "../services/dom.js"
import { StateServiceTag } from "../services/state.js"
import { StateServiceLive } from "../services/state-live.js"
import { SocketServiceTag, SocketError, type SocketService } from "../services/socket.js"
import type { HudMessage } from "../../hud/protocol.js"
import type { TemplateResult } from "../template/types.js"

/**
 * Result of creating a test layer.
 *
 * Includes the layer plus helpers for inspecting rendered content
 * and injecting messages.
 */
export interface TestLayerResult {
  /** The Effect layer to provide to tests */
  readonly layer: Layer.Layer<DomServiceTag | StateServiceTag | SocketServiceTag>

  /** Ref holding all rendered content by element */
  readonly renderedContent: Ref.Ref<Map<Element, string>>

  /** Inject a HUD message into the socket stream */
  readonly injectMessage: (msg: HudMessage) => Effect.Effect<void, never>

  /** Get rendered content for an element */
  readonly getRendered: (el: Element) => Effect.Effect<string | undefined, never>

  /** Clear all rendered content */
  readonly clearRendered: () => Effect.Effect<void, never>
}

/**
 * Create a mock DomService for testing.
 */
const makeMockDomService = (
  renderedContent: Ref.Ref<Map<Element, string>>
): DomService => ({
  query: <T extends Element>(_selector: string) =>
    Effect.fail(new DomError("element_not_found", "Mock DOM: use queryOption")),

  queryOption: <T extends Element>(_selector: string) => Effect.succeed(null),

  queryId: <T extends Element>(_id: string) =>
    Effect.fail(new DomError("element_not_found", "Mock DOM: elements not available")),

  render: (element: Element, content: TemplateResult) =>
    Ref.update(renderedContent, (map) => {
      const newMap = new Map(map)
      newMap.set(element, content.toString())
      return newMap
    }),

  listen: <K extends keyof HTMLElementEventMap>(
    _element: Element,
    _event: K,
    _handler: (e: HTMLElementEventMap[K]) => void,
    _options?: AddEventListenerOptions
  ) => Effect.succeed(() => {}),

  delegate: <K extends keyof HTMLElementEventMap>(
    _container: Element,
    _selector: string,
    _event: K,
    _handler: (e: HTMLElementEventMap[K], target: Element) => void
  ) => Effect.succeed(() => {}),

  createFragment: (_content: TemplateResult) =>
    Effect.fail(new DomError("render_failed", "Mock DOM: fragments not supported")),
})

/**
 * Create a mock SocketService for testing.
 */
const makeMockSocketService = (
  messageQueue: Queue.Queue<HudMessage>
): SocketService => ({
  connect: () => Effect.void,
  disconnect: () => Effect.void,
  isConnected: () => Effect.succeed(true),
  messages: Stream.fromQueue(messageQueue),

  // All requests fail in test mode by default
  loadTBSuite: (_suitePath) =>
    Effect.fail(new SocketError("request_failed", "Mock: loadTBSuite not implemented")),
  startTBRun: (_options) =>
    Effect.fail(new SocketError("request_failed", "Mock: startTBRun not implemented")),
  stopTBRun: () =>
    Effect.fail(new SocketError("request_failed", "Mock: stopTBRun not implemented")),
  loadRecentTBRuns: (_count) =>
    Effect.fail(new SocketError("request_failed", "Mock: loadRecentTBRuns not implemented")),
  loadTBRunDetails: (_runId) =>
    Effect.fail(new SocketError("request_failed", "Mock: loadTBRunDetails not implemented")),
  loadReadyTasks: (_limit) =>
    Effect.fail(new SocketError("request_failed", "Mock: loadReadyTasks not implemented")),
  assignTaskToMC: (_taskId, _options) =>
    Effect.fail(new SocketError("request_failed", "Mock: assignTaskToMC not implemented")),
  loadUnifiedTrajectories: (_limit) =>
    Effect.fail(new SocketError("request_failed", "Mock: loadUnifiedTrajectories not implemented")),
})

/**
 * Create a test layer with mock services.
 *
 * @example
 * ```typescript
 * import { describe, test, expect } from "bun:test"
 * import { Effect } from "effect"
 * import { makeTestLayer } from "./layers/test.js"
 * import { mountWidget } from "./widget/mount.js"
 * import { MyWidget } from "./widgets/my-widget.js"
 *
 * describe("MyWidget", () => {
 *   test("renders initial state", async () => {
 *     await Effect.runPromise(
 *       Effect.scoped(
 *         Effect.gen(function* () {
 *           const { layer, getRendered } = yield* makeTestLayer()
 *           const container = { id: "test" } as Element
 *
 *           yield* mountWidget(MyWidget, container).pipe(
 *             Effect.provide(layer)
 *           )
 *
 *           const html = yield* getRendered(container)
 *           expect(html).toContain("expected content")
 *         })
 *       )
 *     )
 *   })
 * })
 * ```
 */
export const makeTestLayer = (): Effect.Effect<TestLayerResult, never> =>
  Effect.gen(function* () {
    const renderedContent = yield* Ref.make(new Map<Element, string>())
    const messageQueue = yield* Queue.unbounded<HudMessage>()

    const domService = makeMockDomService(renderedContent)
    const socketService = makeMockSocketService(messageQueue)

    const layer = Layer.mergeAll(
      Layer.succeed(DomServiceTag, domService),
      Layer.succeed(SocketServiceTag, socketService),
      StateServiceLive
    )

    return {
      layer,
      renderedContent,
      injectMessage: (msg: HudMessage) => Queue.offer(messageQueue, msg),
      getRendered: (el: Element) =>
        Ref.get(renderedContent).pipe(Effect.map((map) => map.get(el))),
      clearRendered: () => Ref.set(renderedContent, new Map()),
    }
  })

/**
 * Create a test layer with custom mock implementations.
 */
export const makeCustomTestLayer = (options: {
  domService?: Partial<DomService>
  socketService?: Partial<SocketService>
}): Effect.Effect<TestLayerResult, never> =>
  Effect.gen(function* () {
    const renderedContent = yield* Ref.make(new Map<Element, string>())
    const messageQueue = yield* Queue.unbounded<HudMessage>()

    const baseDom = makeMockDomService(renderedContent)
    const baseSocket = makeMockSocketService(messageQueue)

    const domService = { ...baseDom, ...options.domService }
    const socketService = { ...baseSocket, ...options.socketService }

    const layer = Layer.mergeAll(
      Layer.succeed(DomServiceTag, domService),
      Layer.succeed(SocketServiceTag, socketService),
      StateServiceLive
    )

    return {
      layer,
      renderedContent,
      injectMessage: (msg: HudMessage) => Queue.offer(messageQueue, msg),
      getRendered: (el: Element) =>
        Ref.get(renderedContent).pipe(Effect.map((map) => map.get(el))),
      clearRendered: () => Ref.set(renderedContent, new Map()),
    }
  })
