import { describe, expect, it } from "vitest"
import { Effect } from "effect"
import { DomServiceLive, html } from "../src/index.ts"

describe("DomService.swap (contract)", () => {
  it("restores focus and input selection on inner swaps", async () => {
    const root = document.createElement("div")
    root.innerHTML = `<div id="target"><input id="field" name="field" value="hello world" /></div>`
    document.body.appendChild(root)

    const target = root.querySelector("#target") as HTMLElement
    const input = root.querySelector("#field") as HTMLInputElement

    input.focus()
    input.setSelectionRange(2, 5, "forward")
    expect(document.activeElement).toBe(input)

    await Effect.runPromise(
      DomServiceLive.swap(
        target,
        html`<input id="field" name="field" value="hello world" />`,
        "inner"
      )
    )

    const next = root.querySelector("#field") as HTMLInputElement
    expect(next).not.toBe(input)
    expect(document.activeElement).toBe(next)
    expect(next.selectionStart).toBe(2)
    expect(next.selectionEnd).toBe(5)

    root.remove()
  })

  it("restores focus and textarea selection on inner swaps", async () => {
    const root = document.createElement("div")
    root.innerHTML = `<div id="target"><textarea id="field" name="field">hello world</textarea></div>`
    document.body.appendChild(root)

    const target = root.querySelector("#target") as HTMLElement
    const textarea = root.querySelector("#field") as HTMLTextAreaElement

    textarea.focus()
    textarea.setSelectionRange(2, 5, "forward")
    expect(document.activeElement).toBe(textarea)

    await Effect.runPromise(
      DomServiceLive.swap(
        target,
        html`<textarea id="field" name="field">hello world</textarea>`,
        "inner"
      )
    )

    const next = root.querySelector("#field") as HTMLTextAreaElement
    expect(next).not.toBe(textarea)
    expect(document.activeElement).toBe(next)
    expect(next.selectionStart).toBe(2)
    expect(next.selectionEnd).toBe(5)

    root.remove()
  })

  it("restores focus and input selection on outer swaps", async () => {
    const root = document.createElement("div")
    root.innerHTML = `<div id="target"><input id="field" name="field" value="hello world" /></div>`
    document.body.appendChild(root)

    const target = root.querySelector("#target") as HTMLElement
    const input = root.querySelector("#field") as HTMLInputElement

    input.focus()
    input.setSelectionRange(1, 4, "forward")
    expect(document.activeElement).toBe(input)

    await Effect.runPromise(
      DomServiceLive.swap(
        target,
        html`<div id="target"><input id="field" name="field" value="hello world" /></div>`,
        "outer"
      )
    )

    const next = root.querySelector("#field") as HTMLInputElement
    expect(next).not.toBe(input)
    expect(document.activeElement).toBe(next)
    expect(next.selectionStart).toBe(1)
    expect(next.selectionEnd).toBe(4)

    root.remove()
  })

  it("restores scroll positions for nodes with data-scroll-id", async () => {
    const root = document.createElement("div")
    root.innerHTML = `<div id="target"><div data-scroll-id="swap-scroll-1"></div></div>`
    document.body.appendChild(root)

    const target = root.querySelector("#target") as HTMLElement
    const scrollEl = target.querySelector(
      '[data-scroll-id="swap-scroll-1"]'
    ) as HTMLElement
    scrollEl.scrollTop = 123
    scrollEl.scrollLeft = 45

    await Effect.runPromise(
      DomServiceLive.swap(
        target,
        html`<div data-scroll-id="swap-scroll-1"></div><div>more</div>`,
        "inner"
      )
    )

    const nextScrollEl = target.querySelector(
      '[data-scroll-id="swap-scroll-1"]'
    ) as HTMLElement
    expect(nextScrollEl.scrollTop).toBe(123)
    expect(nextScrollEl.scrollLeft).toBe(45)

    root.remove()
  })

  it("restores scroll positions for nodes with data-scroll-id on outer swaps", async () => {
    const root = document.createElement("div")
    root.innerHTML = `<div id="target"><div data-scroll-id="swap-scroll-outer"></div></div>`
    document.body.appendChild(root)

    const target = root.querySelector("#target") as HTMLElement
    const scrollEl = target.querySelector(
      '[data-scroll-id="swap-scroll-outer"]'
    ) as HTMLElement
    scrollEl.scrollTop = 123
    scrollEl.scrollLeft = 45

    await Effect.runPromise(
      DomServiceLive.swap(
        target,
        html`<div id="target"><div data-scroll-id="swap-scroll-outer"></div><div>more</div></div>`,
        "outer"
      )
    )

    const nextTarget = root.querySelector("#target") as HTMLElement
    const nextScrollEl = nextTarget.querySelector(
      '[data-scroll-id="swap-scroll-outer"]'
    ) as HTMLElement
    expect(nextScrollEl.scrollTop).toBe(123)
    expect(nextScrollEl.scrollLeft).toBe(45)

    root.remove()
  })

  it("supports swap modes: beforeend / afterbegin / delete", async () => {
    const root = document.createElement("div")
    root.innerHTML = `<div id="target"><span>a</span></div>`
    document.body.appendChild(root)

    const target = root.querySelector("#target") as HTMLElement

    await Effect.runPromise(
      DomServiceLive.swap(target, html`<span>b</span>`, "beforeend")
    )
    expect(target.innerHTML).toBe("<span>a</span><span>b</span>")

    await Effect.runPromise(
      DomServiceLive.swap(target, html`<span>z</span>`, "afterbegin")
    )
    expect(target.innerHTML).toBe("<span>z</span><span>a</span><span>b</span>")

    await Effect.runPromise(
      DomServiceLive.swap(target, html`<span>ignored</span>`, "delete")
    )
    expect(root.querySelector("#target")).toBeNull()

    root.remove()
  })

  it("outer/replace swaps replace the target element", async () => {
    const root = document.createElement("div")
    root.innerHTML = `<div id="target"><span>a</span></div>`
    document.body.appendChild(root)

    const target = root.querySelector("#target") as HTMLElement
    expect(target.isConnected).toBe(true)

    await Effect.runPromise(
      DomServiceLive.swap(
        target,
        html`<div id="target"><span>b</span></div>`,
        "outer"
      )
    )

    expect(target.isConnected).toBe(false)
    expect(root.querySelector("#target")?.innerHTML).toBe("<span>b</span>")

    // replace is an alias for outer replacement in v1
    const target2 = root.querySelector("#target") as HTMLElement
    await Effect.runPromise(
      DomServiceLive.swap(
        target2,
        html`<div id="target"><span>c</span></div>`,
        "replace"
      )
    )
    expect(root.querySelector("#target")?.innerHTML).toBe("<span>c</span>")

    root.remove()
  })
})
