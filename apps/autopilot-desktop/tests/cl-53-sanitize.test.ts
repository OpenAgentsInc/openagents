import { describe, expect, test } from "bun:test"
import { initialModel } from "../src/ui/model"
import { sanitizeTree, view } from "../src/ui/view"

// Regression for the blank-screen crash: Foldkit's element constructors strip
// `null` children but NOT `undefined`/`false`, so such a child reaches
// `dedupeSharedVNodes` which does `child.children` and throws
// "undefined is not an object". `sanitizeTree` drops those before the runtime
// patches, so the view can never blank-screen on a stray falsy child.

const vnode = (children: unknown[]): { sel: string; children: unknown[] } => ({
  sel: "div",
  children,
})

const treeContainsSelector = (node: unknown, selector: string): boolean => {
  if (node === null || typeof node !== "object") return false
  const vnode = node as { sel?: string; children?: unknown[] }
  if (vnode.sel === selector) return true
  return Array.isArray(vnode.children)
    ? vnode.children.some((child) => treeContainsSelector(child, selector))
    : false
}

describe("CL-53 sanitizeTree", () => {
  test("drops undefined / null / false children", () => {
    const out = sanitizeTree(
      vnode(["text", undefined, null, false, vnode(["ok"])]),
    ) as { children: unknown[] }
    expect(out.children).toHaveLength(2)
    expect(out.children[0]).toBe("text")
    expect((out.children[1] as { children: unknown[] }).children).toEqual(["ok"])
  })

  test("recurses into nested children", () => {
    const out = sanitizeTree(
      vnode([vnode(["a", undefined, vnode([false, "b"])])]),
    ) as { children: Array<{ children: unknown[] }> }
    const inner = out.children[0]
    expect(inner.children).toHaveLength(2) // "a" + nested vnode
    const deepest = inner.children[1] as { children: unknown[] }
    expect(deepest.children).toEqual(["b"]) // false dropped
  })

  test("leaves strings and childless nodes untouched", () => {
    expect(sanitizeTree("hello")).toBe("hello")
    expect(sanitizeTree({ sel: "br" })).toEqual({ sel: "br" })
  })

  test("nodes home includes the three-effect cube", () => {
    const document = view(initialModel)
    expect(treeContainsSelector(document.body, "oa-spinning-cube")).toBe(true)
  })
})
