/**
 * Simple Counter Demo Component
 */

import { Effect } from "effect"
import { html, type Component } from "../effuse/index.js"

type CounterState = {
  count: number
}

type CounterEvent = { type: "increment" } | { type: "decrement" }

export const CounterComponent: Component<CounterState, CounterEvent> = {
  id: "counter-demo",

  initialState: () => ({ count: 0 }),

  render: (ctx) =>
    Effect.gen(function* () {
      const { count } = yield* ctx.state.get
      return html`
        <div class="flex flex-col items-center gap-3">
          <div class="text-2xl font-semibold">${count}</div>
          <div class="flex justify-center gap-3">
            <button data-action="decrement">-</button>
            <button data-action="increment">+</button>
          </div>
        </div>
      `
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      yield* ctx.dom.delegate(ctx.container, "[data-action]", "click", (_e, target) => {
        const action = (target as HTMLElement).dataset.action
        if (action === "increment") {
          Effect.runFork(ctx.emit({ type: "increment" }))
        }
        if (action === "decrement") {
          Effect.runFork(ctx.emit({ type: "decrement" }))
        }
      })
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
      }
    }),
}
