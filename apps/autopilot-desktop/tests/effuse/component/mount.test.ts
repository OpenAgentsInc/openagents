import { describe, expect, it } from "@effect/vitest"
import { Effect, Stream } from "effect"
import type { Component, ComponentContext } from "../../../src/effuse/index.js"
import {
  EffuseLive,
  html,
  mountComponent,
} from "../../../src/effuse/index.js"

const waitFor = (ms: number) =>
  Effect.promise<void>(
    () => new Promise((resolve) => setTimeout(resolve, ms))
  )

const makeContainer = () =>
  Effect.gen(function* () {
    const container = document.createElement("div")
    document.body.appendChild(container)
    yield* Effect.addFinalizer(() => Effect.sync(() => container.remove()))
    return container
  })

describe("mountComponent", () => {
  it.live("renders once on mount", () =>
    Effect.scoped(
      Effect.gen(function* () {
        let renders = 0
        const component: Component<{ count: number }, never> = {
          id: "render-once",
          initialState: () => ({ count: 0 }),
          render: () =>
            Effect.sync(() => {
              renders += 1
              return html`<div>${renders}</div>`
            }),
        }

        const container = yield* makeContainer()
        yield* mountComponent(component, container)

        yield* Effect.sync(() => {
          expect(renders).toBe(1)
        })
      }).pipe(Effect.provide(EffuseLive))
    )
  )

  it.live("re-renders on state updates", () =>
    Effect.scoped(
      Effect.gen(function* () {
        let renders = 0
        let ctxRef: ComponentContext<{ count: number }, never> | null = null

        const component: Component<{ count: number }, never> = {
          id: "rerender",
          initialState: () => ({ count: 0 }),
          render: (ctx) =>
            Effect.sync(() => {
              ctxRef = ctx
              renders += 1
              return html`<div>${renders}</div>`
            }),
        }

        const container = yield* makeContainer()
        yield* mountComponent(component, container)

        if (!ctxRef) {
          throw new Error("Context not captured")
        }

        yield* ctxRef.state.set({ count: 1 })
        yield* waitFor(0)

        yield* Effect.sync(() => {
          expect(renders).toBe(2)
        })
      }).pipe(Effect.provide(EffuseLive))
    )
  )

  it.live("handles emitted events", () =>
    Effect.scoped(
      Effect.gen(function* () {
        let handled = false

        type Event = { type: "ping" }
        const component: Component<{ count: number }, Event> = {
          id: "events",
          initialState: () => ({ count: 0 }),
          render: () => Effect.succeed(html`<div>ok</div>`),
          handleEvent: (event) =>
            Effect.sync(() => {
              if (event.type === "ping") {
                handled = true
              }
            }),
        }

        const container = yield* makeContainer()
        const mounted = yield* mountComponent(component, container)

        yield* mounted.emit({ type: "ping" })
        yield* waitFor(0)

        yield* Effect.sync(() => {
          expect(handled).toBe(true)
        })
      }).pipe(Effect.provide(EffuseLive))
    )
  )

  it.live("runs subscriptions and stops them on scope close", () =>
    Effect.gen(function* () {
      let started = false
      let interrupted = false

      yield* Effect.scoped(
        Effect.gen(function* () {
          const longEffect = Effect.sync(() => {
            started = true
          }).pipe(
            Effect.zipRight(Effect.never),
            Effect.onInterrupt(() =>
              Effect.sync(() => {
                interrupted = true
              })
            )
          )

          const component: Component<{ count: number }, never> = {
            id: "subs",
            initialState: () => ({ count: 0 }),
            render: () => Effect.succeed(html`<div>ok</div>`),
            subscriptions: () => [Stream.fromIterable([longEffect])],
          }

          const container = yield* makeContainer()
          yield* mountComponent(component, container)
          yield* waitFor(0)

          yield* Effect.sync(() => {
            expect(started).toBe(true)
          })
        }).pipe(Effect.provide(EffuseLive))
      )

      yield* Effect.sync(() => {
        expect(interrupted).toBe(true)
      })
    })
  )
})
