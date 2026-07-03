import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import { mountKhalaCodeSidebar } from "../src/ui/sidebar"

describe("Khala Code sidebar hotbar", () => {
  test("handles macOS Option-number symbols from focused text inputs", () => {
    const window = new Window()
    const previousWindow = globalThis.window
    const previousDocument = globalThis.document
    const previousNavigator = globalThis.navigator

    Object.defineProperty(globalThis, "window", { configurable: true, value: window })
    Object.defineProperty(globalThis, "document", { configurable: true, value: window.document })
    Object.defineProperty(globalThis, "navigator", { configurable: true, value: window.navigator })

    const activated: string[] = []
    const handle = mountKhalaCodeSidebar(document.createElement("div"), {
      selectedValue: "chat",
      onActivate: value => activated.push(value),
    })

    try {
      const input = document.createElement("textarea")
      document.body.append(input)
      input.focus()

      const event = new window.KeyboardEvent("keydown", {
        altKey: true,
        bubbles: true,
        cancelable: true,
        code: "Digit2",
        key: "™",
      })

      input.dispatchEvent(event as unknown as Event)

      expect(event.defaultPrevented).toBe(true)
      expect(activated).toEqual(["fleet"])
    } finally {
      handle.destroy()
      Object.defineProperty(globalThis, "window", { configurable: true, value: previousWindow })
      Object.defineProperty(globalThis, "document", { configurable: true, value: previousDocument })
      Object.defineProperty(globalThis, "navigator", { configurable: true, value: previousNavigator })
      window.close()
    }
  })
})
