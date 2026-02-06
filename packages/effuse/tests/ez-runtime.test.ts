import { describe, expect, it } from "vitest"
import { Effect } from "effect"
import {
  DomServiceLive,
  DomServiceTag,
  makeEzRegistry,
  mountEzRuntimeWith,
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
})

