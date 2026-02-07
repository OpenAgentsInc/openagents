import { describe, expect, it } from "vitest"
import { Effect } from "effect"
import {
  DomServiceLive,
  DomServiceTag,
  html,
  makeEzRegistry,
  mountEzRuntimeWith,
  type DomService,
} from "../src/index.ts"

describe("conformance: router-ish cancellation semantics", () => {
  it("switch-latest: repeated triggers interrupt in-flight work for the same action element", async () => {
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
      mountEzRuntimeWith(root, registry).pipe(Effect.provideService(DomServiceTag, dom))
    )

    const btn = root.querySelector("button") as HTMLButtonElement
    btn.dispatchEvent(new MouseEvent("click", { bubbles: true }))
    btn.dispatchEvent(new MouseEvent("click", { bubbles: true }))

    // Wait long enough for only the latest fiber to complete.
    await Effect.runPromise(Effect.sleep("100 millis"))

    expect(started).toBe(2)
    expect(finished).toBe(1)
    expect(swaps).toBe(1)
    expect(root.querySelector('[data-finished="1"]')).not.toBeNull()

    root.remove()
  })
})

