import { describe, expect, it } from "vitest"
import { Effect } from "effect"
import {
  DomServiceLive,
  DomServiceTag,
  html,
  makeEzRegistry,
  mountEzRuntimeWith,
} from "../src/index.ts"

describe("conformance: hydration", () => {
  it("strict boot attaches behavior without calling DomService.swap", async () => {
    const root = document.createElement("div")
    root.innerHTML = `
      <div data-effuse-shell>
        <div data-effuse-outlet>SSR outlet</div>
        <button data-ez="do">Run</button>
      </div>
    `
    document.body.appendChild(root)

    let swaps = 0
    const dom = {
      ...DomServiceLive,
      swap: (target: Element, content: any, mode?: any) => {
        swaps++
        return DomServiceLive.swap(target, content, mode)
      },
    }

    const registry = makeEzRegistry([
      [
        "do",
        () => Effect.succeed(html`<span data-done="1">ok</span>`),
      ],
    ])

    await Effect.runPromise(
      mountEzRuntimeWith(root, registry).pipe(
        Effect.provideService(DomServiceTag, dom)
      )
    )

    // Strict boot: mounting delegated listeners must not mutate DOM.
    expect(swaps).toBe(0)
    expect(root.innerHTML).toContain("SSR outlet")

    const btn = root.querySelector("button") as HTMLButtonElement
    btn.dispatchEvent(new MouseEvent("click", { bubbles: true }))
    await new Promise((r) => setTimeout(r, 0))

    expect(swaps).toBe(1)
    expect(root.querySelector('[data-done="1"]')).not.toBeNull()

    root.remove()
  })
})

