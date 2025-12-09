/**
 * Tests for Happy-DOM Test Layer
 *
 * Verifies the Effect-native testing infrastructure works correctly.
 */

import { describe, test, expect } from "bun:test"
import { Effect, Stream, pipe } from "effect"
import { makeHappyDomLayer } from "./layers/happy-dom.js"
import { TestBrowserTag } from "./browser.js"
import { TestHarnessTag } from "./harness.js"
import type { Component } from "../component/types.js"
import { html } from "../template/html.js"
import { SocketServiceTag } from "../services/socket.js"

// Simple test component
interface CounterState {
  count: number
}

type CounterEvent = { type: "increment" } | { type: "decrement" } | { type: "set"; value: number }

const CounterWidget: Component<CounterState, CounterEvent> = {
  id: "counter-widget",
  initialState: () => ({ count: 0 }),

  render: (ctx) =>
    Effect.gen(function* () {
      const { count } = yield* ctx.state.get
      return html`
        <div class="counter">
          <span class="count">${count}</span>
          <button class="increment" data-action="increment">+</button>
          <button class="decrement" data-action="decrement">-</button>
        </div>
      `
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      switch (event.type) {
        case "increment":
          yield* ctx.state.update((s) => ({ count: s.count + 1 }))
          break
        case "decrement":
          yield* ctx.state.update((s) => ({ count: s.count - 1 }))
          break
        case "set":
          yield* ctx.state.set({ count: event.value })
          break
      }
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      yield* ctx.dom.delegate(ctx.container, "[data-action]", "click", (_e, target) => {
        const action = (target as HTMLElement).dataset.action
        if (action === "increment") {
          Effect.runPromise(ctx.emit({ type: "increment" }))
        } else if (action === "decrement") {
          Effect.runPromise(ctx.emit({ type: "decrement" }))
        }
      })
    }),
}

// Widget with socket subscriptions
interface MessageState {
  messages: string[]
}

type MessageEvent = { type: "clear" }

// Factory to create widget with socket dependency properly injected
const makeMessageWidget = (
  socketMessages: Stream.Stream<any, never>
): Component<MessageState, MessageEvent> => ({
  id: "message-widget",
  initialState: () => ({ messages: [] }),

  render: (ctx) =>
    Effect.gen(function* () {
      const { messages } = yield* ctx.state.get
      return html`
        <div class="messages">
          <ul class="message-list">
            ${messages.map((m) => html`<li class="message">${m}</li>`)}
          </ul>
          <button class="clear" data-action="clear">Clear</button>
        </div>
      `
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      if (event.type === "clear") {
        yield* ctx.state.set({ messages: [] })
      }
    }),

  subscriptions: (ctx) => [
    pipe(
      socketMessages,
      Stream.filter((msg): msg is { type: "test_message"; text: string } =>
        msg.type === "test_message"
      ),
      Stream.map((msg) =>
        ctx.state.update((s) => ({
          messages: [...s.messages, msg.text],
        }))
      )
    ),
  ],
})

describe("Happy-DOM Test Layer", () => {
  describe("makeHappyDomLayer", () => {
    test("creates layer with all services", async () => {
      await Effect.runPromise(
        Effect.scoped(
          Effect.gen(function* () {
            const { layer, window } = yield* makeHappyDomLayer()

            // Layer should provide all services
            expect(layer).toBeDefined()
            expect(window).toBeDefined()
            expect(window.document).toBeDefined()
          })
        )
      )
    })
  })

  describe("TestBrowser", () => {
    test("queries elements", async () => {
      await Effect.runPromise(
        Effect.scoped(
          Effect.gen(function* () {
            const { layer } = yield* makeHappyDomLayer()

            const result = yield* Effect.gen(function* () {
              const browser = yield* TestBrowserTag

              // Element doesn't exist yet
              const exists = yield* browser.exists(".test-el")
              expect(exists).toBe(false)

              // Query returns null for missing
              const maybeEl = yield* browser.queryOption(".test-el")
              expect(maybeEl).toBe(null)

              return "done"
            }).pipe(Effect.provide(layer))

            expect(result).toBe("done")
          })
        )
      )
    })

    test("queries elements after mounting", async () => {
      await Effect.runPromise(
        Effect.scoped(
          Effect.gen(function* () {
            const { layer } = yield* makeHappyDomLayer()

            yield* Effect.gen(function* () {
              const browser = yield* TestBrowserTag
              const harness = yield* TestHarnessTag

              yield* harness.mount(CounterWidget)

              // Now element should exist
              const exists = yield* browser.exists(".counter")
              expect(exists).toBe(true)

              const count = yield* browser.getText(".count")
              expect(count).toBe("0")
            }).pipe(Effect.provide(layer))
          })
        )
      )
    })

    test("expectText assertion", async () => {
      await Effect.runPromise(
        Effect.scoped(
          Effect.gen(function* () {
            const { layer } = yield* makeHappyDomLayer()

            yield* Effect.gen(function* () {
              const browser = yield* TestBrowserTag
              const harness = yield* TestHarnessTag

              yield* harness.mount(CounterWidget)

              // Should pass
              yield* browser.expectText(".count", "0")

              // Should fail
              const failed = yield* browser.expectText(".count", "99").pipe(
                Effect.map(() => false),
                Effect.catchAll(() => Effect.succeed(true))
              )
              expect(failed).toBe(true)
            }).pipe(Effect.provide(layer))
          })
        )
      )
    })
  })

  describe("TestHarness", () => {
    test("mounts widget with initial state", async () => {
      await Effect.runPromise(
        Effect.scoped(
          Effect.gen(function* () {
            const { layer } = yield* makeHappyDomLayer()

            yield* Effect.gen(function* () {
              const harness = yield* TestHarnessTag

              const handle = yield* harness.mount(CounterWidget)

              const state = yield* handle.getState
              expect(state.count).toBe(0)
            }).pipe(Effect.provide(layer))
          })
        )
      )
    })

    test("allows custom initial state", async () => {
      await Effect.runPromise(
        Effect.scoped(
          Effect.gen(function* () {
            const { layer } = yield* makeHappyDomLayer()

            yield* Effect.gen(function* () {
              const harness = yield* TestHarnessTag

              const handle = yield* harness.mount(CounterWidget, {
                initialState: { count: 42 },
              })

              const state = yield* handle.getState
              expect(state.count).toBe(42)
            }).pipe(Effect.provide(layer))
          })
        )
      )
    })

    test("emit triggers handleEvent", async () => {
      await Effect.runPromise(
        Effect.scoped(
          Effect.gen(function* () {
            const { layer } = yield* makeHappyDomLayer()

            yield* Effect.gen(function* () {
              const harness = yield* TestHarnessTag

              const handle = yield* harness.mount(CounterWidget)

              yield* handle.emit({ type: "increment" })
              yield* handle.waitForRender

              const state = yield* handle.getState
              expect(state.count).toBe(1)
            }).pipe(Effect.provide(layer))
          })
        )
      )
    })

    test("setState updates state directly", async () => {
      await Effect.runPromise(
        Effect.scoped(
          Effect.gen(function* () {
            const { layer } = yield* makeHappyDomLayer()

            yield* Effect.gen(function* () {
              const harness = yield* TestHarnessTag

              const handle = yield* harness.mount(CounterWidget)

              yield* handle.setState({ count: 100 })
              yield* handle.waitForRender

              const state = yield* handle.getState
              expect(state.count).toBe(100)
            }).pipe(Effect.provide(layer))
          })
        )
      )
    })

    test("updateState applies function", async () => {
      await Effect.runPromise(
        Effect.scoped(
          Effect.gen(function* () {
            const { layer } = yield* makeHappyDomLayer()

            yield* Effect.gen(function* () {
              const harness = yield* TestHarnessTag

              const handle = yield* harness.mount(CounterWidget, {
                initialState: { count: 10 },
              })

              yield* handle.updateState((s) => ({ count: s.count * 2 }))
              yield* handle.waitForRender

              const state = yield* handle.getState
              expect(state.count).toBe(20)
            }).pipe(Effect.provide(layer))
          })
        )
      )
    })

    test("waitForState waits for predicate", async () => {
      await Effect.runPromise(
        Effect.scoped(
          Effect.gen(function* () {
            const { layer } = yield* makeHappyDomLayer()

            yield* Effect.gen(function* () {
              const harness = yield* TestHarnessTag

              const handle = yield* harness.mount(CounterWidget)

              // Emit multiple increments
              yield* handle.emit({ type: "increment" })
              yield* handle.emit({ type: "increment" })
              yield* handle.emit({ type: "increment" })

              // Wait for state to reach target
              const state = yield* handle.waitForState((s) => s.count >= 3)
              expect(state.count).toBe(3)
            }).pipe(Effect.provide(layer))
          })
        )
      )
    })

    test("getHTML returns rendered content", async () => {
      await Effect.runPromise(
        Effect.scoped(
          Effect.gen(function* () {
            const { layer } = yield* makeHappyDomLayer()

            yield* Effect.gen(function* () {
              const harness = yield* TestHarnessTag

              const handle = yield* harness.mount(CounterWidget)

              const html = yield* handle.getHTML
              expect(html).toContain("counter")
              expect(html).toContain("count")
              expect(html).toContain("increment")
            }).pipe(Effect.provide(layer))
          })
        )
      )
    })

    test("re-renders on state change", async () => {
      await Effect.runPromise(
        Effect.scoped(
          Effect.gen(function* () {
            const { layer } = yield* makeHappyDomLayer()

            yield* Effect.gen(function* () {
              const browser = yield* TestBrowserTag
              const harness = yield* TestHarnessTag

              const handle = yield* harness.mount(CounterWidget)

              // Initial value
              yield* browser.expectText(".count", "0")

              // Update state
              yield* handle.emit({ type: "set", value: 42 })
              yield* handle.waitForRender

              // DOM should be updated
              yield* browser.expectText(".count", "42")
            }).pipe(Effect.provide(layer))
          })
        )
      )
    })
  })

  describe("Message injection", () => {
    test("injectMessage reaches subscriptions", async () => {
      await Effect.runPromise(
        Effect.scoped(
          Effect.gen(function* () {
            const { layer } = yield* makeHappyDomLayer()

            yield* Effect.gen(function* () {
              const harness = yield* TestHarnessTag
              const socket = yield* SocketServiceTag

              // Create widget with socket messages stream
              const MessageWidget = makeMessageWidget(socket.getMessages())
              const handle = yield* harness.mount(MessageWidget)

              // Inject a message
              yield* harness.injectMessage({
                type: "test_message",
                text: "Hello from test",
              } as any)

              // Wait for state update
              const state = yield* handle.waitForState((s) => s.messages.length > 0)
              expect(state.messages).toContain("Hello from test")
            }).pipe(Effect.provide(layer))
          })
        )
      )
    })

    test("injectSequence sends multiple messages", async () => {
      await Effect.runPromise(
        Effect.scoped(
          Effect.gen(function* () {
            const { layer } = yield* makeHappyDomLayer()

            yield* Effect.gen(function* () {
              const harness = yield* TestHarnessTag
              const socket = yield* SocketServiceTag

              // Create widget with socket messages stream
              const MessageWidget = makeMessageWidget(socket.getMessages())
              const handle = yield* harness.mount(MessageWidget)

              // Inject sequence
              yield* harness.injectSequence(
                [
                  { type: "test_message", text: "First" } as any,
                  { type: "test_message", text: "Second" } as any,
                  { type: "test_message", text: "Third" } as any,
                ],
                10
              )

              // Wait for all messages
              const state = yield* handle.waitForState((s) => s.messages.length >= 3)
              expect(state.messages).toEqual(["First", "Second", "Third"])
            }).pipe(Effect.provide(layer))
          })
        )
      )
    })
  })

  describe("DOM interaction", () => {
    test("click triggers events", async () => {
      await Effect.runPromise(
        Effect.scoped(
          Effect.gen(function* () {
            const { layer } = yield* makeHappyDomLayer()

            yield* Effect.gen(function* () {
              const browser = yield* TestBrowserTag
              const harness = yield* TestHarnessTag

              const handle = yield* harness.mount(CounterWidget)

              // Click increment button
              yield* browser.click(".increment")
              yield* handle.waitForRender

              // State should be updated
              const state = yield* handle.getState
              expect(state.count).toBe(1)

              // DOM should reflect
              yield* browser.expectText(".count", "1")
            }).pipe(Effect.provide(layer))
          })
        )
      )
    })
  })
})
