import { describe, expect, it } from "vitest"
import { applyPatch, createEmptyTree } from "../../../src/effuse/ui/patch.js"
import type { UIElement, UITree } from "../../../src/effuse/ui/types.js"

describe("applyPatch", () => {
  it("applies add and replace operations to elements", () => {
    const tree = createEmptyTree()
    const rootElement: UIElement = {
      key: "root",
      type: "container",
      props: { label: "Root" },
      children: [],
    }

    const withElement = applyPatch(tree, {
      op: "add",
      path: "/elements/root",
      value: rootElement,
    })

    const withRoot = applyPatch(withElement, {
      op: "set",
      path: "/root",
      value: "root",
    })

    const updated = applyPatch(withRoot, {
      op: "replace",
      path: "/elements/root/props/label",
      value: "Updated",
    })

    expect(updated.root).toBe("root")
    expect(updated.elements.root?.props).toEqual({ label: "Updated" })
  })

  it("removes elements", () => {
    const tree: UITree = {
      root: "root",
      elements: {
        root: {
          key: "root",
          type: "container",
          props: {},
        },
        child: {
          key: "child",
          type: "text",
          props: {},
        },
      },
    }

    const updated = applyPatch(tree, {
      op: "remove",
      path: "/elements/child",
    })

    expect(updated.elements.child).toBeUndefined()
  })
})
