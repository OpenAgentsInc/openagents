import { describe, expect, it, vi } from "vitest"
import { Effect } from "effect"
import {
  DomServiceLive,
  DomServiceTag,
  html,
  makeEzRegistry,
  mountEzRuntimeWith,
  type DomService,
} from "../src/index.ts"

describe("Effuse Ez runtime", () => {
  it("runs click actions declared with data-ez", async () => {
    const root = document.createElement("div")
    root.innerHTML = `<button data-ez="inc">Inc</button>`
    document.body.appendChild(root)

    let ran = false
    let resolve!: () => void
    const done = new Promise<void>((r) => {
      resolve = r
    })

    const registry = makeEzRegistry([
      [
        "inc",
        () =>
          Effect.sync(() => {
            ran = true
            resolve()
          }),
      ],
    ])

    await Effect.runPromise(
      mountEzRuntimeWith(root, registry).pipe(
        Effect.provideService(DomServiceTag, DomServiceLive),
      ),
    )

    const btn = root.querySelector("button")
    expect(btn).not.toBeNull()
    btn!.dispatchEvent(new MouseEvent("click", { bubbles: true }))

    await done
    expect(ran).toBe(true)

    root.remove()
  })

  it("is mount-once per root: mounting twice does not double-execute actions", async () => {
    const root = document.createElement("div")
    root.innerHTML = `<button data-ez="inc">Inc</button>`
    document.body.appendChild(root)

    let runs = 0

    const registry = makeEzRegistry([
      [
        "inc",
        () =>
          Effect.sync(() => {
            runs++
          }),
      ],
    ])

    await Effect.runPromise(
      mountEzRuntimeWith(root, registry).pipe(
        Effect.zipRight(mountEzRuntimeWith(root, registry)),
        Effect.provideService(DomServiceTag, DomServiceLive),
      ),
    )

    const btn = root.querySelector("button") as HTMLButtonElement | null
    expect(btn).not.toBeNull()
    btn!.dispatchEvent(new MouseEvent("click", { bubbles: true }))

    // Give the delegated handler a tick to run.
    await new Promise((r) => setTimeout(r, 0))

    expect(runs).toBe(1)

    root.remove()
  })

  it("switch-latest: a second trigger interrupts an in-flight action for the same element", async () => {
    const root = document.createElement("div")
    root.innerHTML = `<button data-ez="slow">Go</button>`
    document.body.appendChild(root)

    let started = 0
    let finished = 0
    let swaps = 0

    const dom: DomService = {
      ...DomServiceLive,
      swap: (target, content, mode) => {
        swaps++
        return DomServiceLive.swap(target, content, mode)
      },
    }

    const registry = makeEzRegistry([
      [
        "slow",
        () =>
          Effect.gen(function* () {
            started++
            yield* Effect.sleep("50 millis")
            finished++
            return html`<span data-finished="${String(finished)}">done</span>`
          }),
      ],
    ])

    await Effect.runPromise(
      mountEzRuntimeWith(root, registry).pipe(Effect.provideService(DomServiceTag, dom)),
    )

    const btn = root.querySelector("button") as HTMLButtonElement
    btn.dispatchEvent(new MouseEvent("click", { bubbles: true }))
    btn.dispatchEvent(new MouseEvent("click", { bubbles: true }))

    await Effect.runPromise(Effect.sleep("100 millis"))

    expect(started).toBe(2)
    expect(finished).toBe(1)
    expect(swaps).toBe(1)
    expect(root.querySelector('[data-finished="1"]')).not.toBeNull()

    root.remove()
  })

  it("bounds action failures: errors do not swap and disabled state is restored", async () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {})

    const root = document.createElement("div")
    root.innerHTML = `<button data-ez="boom" data-ez-disable>Go</button>`
    document.body.appendChild(root)

    const btn = root.querySelector("button") as HTMLButtonElement
    expect(btn.disabled).toBe(false)
    expect(btn.getAttribute("aria-disabled")).toBeNull()

    let swaps = 0
    const dom: DomService = {
      ...DomServiceLive,
      swap: (target, content, mode) => {
        swaps++
        return DomServiceLive.swap(target, content, mode)
      },
    }

    const registry = makeEzRegistry([
      ["boom", () => Effect.die(new Error("boom"))],
    ])

    await Effect.runPromise(
      mountEzRuntimeWith(root, registry).pipe(Effect.provideService(DomServiceTag, dom)),
    )

    btn.dispatchEvent(new MouseEvent("click", { bubbles: true }))
    // Allow the action fiber to run its finally clause.
    await Effect.runPromise(Effect.sleep("10 millis"))

    expect(swaps).toBe(0)
    expect(btn.disabled).toBe(false)
    expect(btn.getAttribute("aria-disabled")).toBeNull()

    root.remove()
    errorSpy.mockRestore()
  })

  it("collects submit params via FormData", async () => {
    const root = document.createElement("div")
    root.innerHTML = `
      <form data-ez="submit">
        <input name="email" value="you@example.com" />
        <button type="submit">Submit</button>
      </form>
    `
    document.body.appendChild(root)

    let captured: Record<string, string> | null = null
    let resolve!: () => void
    const done = new Promise<void>((r) => {
      resolve = r
    })

    const registry = makeEzRegistry([
      [
        "submit",
        ({ params }) =>
          Effect.sync(() => {
            captured = { ...params }
            resolve()
          }),
      ],
    ])

    await Effect.runPromise(
      mountEzRuntimeWith(root, registry).pipe(
        Effect.provideService(DomServiceTag, DomServiceLive),
      ),
    )

    const form = root.querySelector("form")
    expect(form).not.toBeNull()
    form!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }))

    await done
    expect(captured?.email).toBe("you@example.com")

    root.remove()
  })

  it("collects input params from name/value when trigger is input", async () => {
    const root = document.createElement("div")
    root.innerHTML = `<input data-ez="q" data-ez-trigger="input" name="q" value="" />`
    document.body.appendChild(root)

    let last: Record<string, string> | null = null
    let resolve!: () => void
    const done = new Promise<void>((r) => {
      resolve = r
    })

    const registry = makeEzRegistry([
      [
        "q",
        ({ params }) =>
          Effect.sync(() => {
            last = { ...params }
            resolve()
          }),
      ],
    ])

    await Effect.runPromise(
      mountEzRuntimeWith(root, registry).pipe(
        Effect.provideService(DomServiceTag, DomServiceLive),
      ),
    )

    const input = root.querySelector("input") as HTMLInputElement | null
    expect(input).not.toBeNull()
    input!.value = "hello"
    input!.dispatchEvent(new Event("input", { bubbles: true }))

    await done
    expect(last?.q).toBe("hello")

    root.remove()
  })

  it("keeps delegated action handling after outer swaps rerender the action element", async () => {
    const root = document.createElement("div")
    root.innerHTML = `<button data-ez="flip" data-ez-swap="outer">Run</button>`
    document.body.appendChild(root)

    let runs = 0
    let resolveSecond!: () => void
    const secondRun = new Promise<void>((resolve) => {
      resolveSecond = resolve
    })

    const registry = makeEzRegistry([
      [
        "flip",
        () =>
          Effect.sync(() => {
            runs++
            if (runs === 2) {
              resolveSecond()
            }
            return html`<button data-ez="flip" data-ez-swap="outer">Run ${runs}</button>`
          }),
      ],
    ])

    await Effect.runPromise(
      mountEzRuntimeWith(root, registry).pipe(
        Effect.provideService(DomServiceTag, DomServiceLive),
      ),
    )

    const first = root.querySelector("button")
    expect(first).not.toBeNull()
    first!.dispatchEvent(new MouseEvent("click", { bubbles: true }))

    await Effect.runPromise(Effect.sleep("10 millis"))

    const second = root.querySelector("button")
    expect(second).not.toBeNull()
    second!.dispatchEvent(new MouseEvent("click", { bubbles: true }))

    await secondRun
    expect(runs).toBe(2)
    expect(root.querySelector("button")?.textContent).toContain("Run 2")

    root.remove()
  })
})
