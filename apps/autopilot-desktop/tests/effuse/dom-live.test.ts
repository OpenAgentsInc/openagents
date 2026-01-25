import { describe, expect, it } from "@effect/vitest"
import { Effect } from "effect"
import { DomServiceLive, html } from "../../src/effuse/index.js"

const makeContainer = () =>
  Effect.gen(function* () {
    const container = document.createElement("div")
    document.body.appendChild(container)
    yield* Effect.addFinalizer(() => Effect.sync(() => container.remove()))
    return container
  })

describe("DomServiceLive.swap", () => {
  it.live("updates DOM based on swap mode", () =>
    Effect.scoped(
      Effect.gen(function* () {
        const container = yield* makeContainer()

        const innerTarget = document.createElement("div")
        innerTarget.id = "inner"
        innerTarget.innerHTML = "<span>old</span>"
        container.appendChild(innerTarget)

        yield* DomServiceLive.swap(innerTarget, html`<span>new</span>`, "inner")
        yield* Effect.sync(() => {
          expect(innerTarget.innerHTML).toContain("new")
        })

        const outerTarget = document.createElement("div")
        outerTarget.id = "outer"
        outerTarget.textContent = "old"
        container.appendChild(outerTarget)

        yield* DomServiceLive.swap(
          outerTarget,
          html`<div id="outer">new</div>`,
          "outer"
        )

        const outerNow = container.querySelector("#outer") as HTMLElement
        yield* Effect.sync(() => {
          expect(outerTarget.isConnected).toBe(false)
          expect(outerNow.textContent).toBe("new")
        })

        const replaceTarget = document.createElement("div")
        replaceTarget.id = "replace"
        replaceTarget.textContent = "old"
        container.appendChild(replaceTarget)

        yield* DomServiceLive.swap(
          replaceTarget,
          html`<div id="replace">new</div>`,
          "replace"
        )

        const replaceNow = container.querySelector("#replace") as HTMLElement
        yield* Effect.sync(() => {
          expect(replaceTarget.isConnected).toBe(false)
          expect(replaceNow.textContent).toBe("new")
        })

        const beforeAfter = document.createElement("div")
        beforeAfter.id = "insert"
        beforeAfter.innerHTML = "<span>base</span>"
        container.appendChild(beforeAfter)

        yield* DomServiceLive.swap(beforeAfter, html`<span>tail</span>`, "beforeend")
        yield* DomServiceLive.swap(beforeAfter, html`<span>head</span>`, "afterbegin")

        yield* Effect.sync(() => {
          expect(beforeAfter.innerHTML.startsWith("<span>head")).toBe(true)
          expect(beforeAfter.innerHTML).toContain("tail")
        })

        const deleteTarget = document.createElement("div")
        deleteTarget.id = "delete"
        container.appendChild(deleteTarget)
        yield* DomServiceLive.swap(deleteTarget, html``, "delete")

        yield* Effect.sync(() => {
          expect(deleteTarget.isConnected).toBe(false)
        })
      })
    )
  )

  it.live("restores focus and selection on inner swap", () =>
    Effect.scoped(
      Effect.gen(function* () {
        const container = yield* makeContainer()
        container.innerHTML = `<input id="focus-input" value="hello" />`
        const input = container.querySelector("#focus-input") as HTMLInputElement

        input.focus()
        input.setSelectionRange(1, 3)

        yield* DomServiceLive.swap(
          container,
          html`<input id="focus-input" value="hello" />`,
          "inner"
        )

        const restored = document.querySelector("#focus-input") as HTMLInputElement

        yield* Effect.sync(() => {
          expect(document.activeElement).toBe(restored)
          expect(restored.selectionStart).toBe(1)
          expect(restored.selectionEnd).toBe(3)
        })
      })
    )
  )

  it.live("restores focus on outer swap", () =>
    Effect.scoped(
      Effect.gen(function* () {
        const container = yield* makeContainer()
        const input = document.createElement("input")
        input.id = "outer-input"
        input.value = "value"
        container.appendChild(input)

        input.focus()

        yield* DomServiceLive.swap(
          input,
          html`<input id="outer-input" value="value" />`,
          "outer"
        )

        const restored = document.querySelector("#outer-input") as HTMLInputElement

        yield* Effect.sync(() => {
          expect(document.activeElement).toBe(restored)
        })
      })
    )
  )
})
