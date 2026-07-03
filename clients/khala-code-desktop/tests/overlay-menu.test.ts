import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import {
  mountOverlayMenu,
  type OverlayMenuEntry,
} from "../src/ui/overlay-menu"

const withDom = async (
  run: (window: Window) => Promise<void> | void,
): Promise<void> => {
  const window = new Window()
  const previousDocument = globalThis.document
  Object.defineProperty(globalThis, "document", {
    configurable: true,
    value: window.document,
  })
  try {
    await run(window)
  } finally {
    Object.defineProperty(globalThis, "document", {
      configurable: true,
      value: previousDocument,
    })
    window.close()
  }
}

describe("overlay menu (generalized dialog-menu primitive)", () => {
  test("hold-to-reveal shows entries, release hides, click selects", async () => {
    await withDom(async () => {
      const entries: OverlayMenuEntry[] = [
        { active: false, id: "alpha", key: "1", label: "Alpha" },
        { active: true, id: "beta", key: "2", label: "Beta" },
      ]
      const selected: string[] = []
      const menu = mountOverlayMenu({
        entries: () => entries,
        hint: "hold to keep open",
        holdDelayMs: 0,
        onSelect: id => selected.push(id),
        title: "Test menu",
      })

      expect(menu.isVisible()).toBe(false)
      menu.notifyHoldKeyDown()
      expect(menu.isVisible()).toBe(true)

      const root = document.querySelector<HTMLElement>(".khala-overlay-menu")
      expect(root?.hidden).toBe(false)
      const items = [...document.querySelectorAll<HTMLButtonElement>(".khala-overlay-menu-item")]
      expect(items.map(item => item.dataset.entryKey)).toEqual(["1", "2"])
      expect(items.map(item => item.textContent)).toEqual(["1Alpha", "2Beta"])
      expect(items[1]?.dataset.active).toBe("true")
      expect(root?.textContent).toContain("hold to keep open")

      menu.notifyHoldKeyUp()
      expect(menu.isVisible()).toBe(false)

      menu.show()
      items[0]?.click()
      const clicked = document.querySelector<HTMLButtonElement>('[data-entry-id="alpha"]')
      clicked?.click()
      expect(selected).toContain("alpha")
      expect(menu.isVisible()).toBe(false)

      menu.destroy()
      expect(document.querySelector(".khala-overlay-menu")).toBeNull()
    })
  })

  test("renders the empty label when there are no entries", async () => {
    await withDom(async () => {
      const menu = mountOverlayMenu({
        emptyLabel: "No items",
        entries: () => [],
        holdDelayMs: 0,
        onSelect: () => undefined,
        title: "Empty menu",
      })
      menu.show()
      expect(document.querySelector(".khala-overlay-menu-empty")?.textContent).toBe("No items")
      menu.destroy()
    })
  })
})
