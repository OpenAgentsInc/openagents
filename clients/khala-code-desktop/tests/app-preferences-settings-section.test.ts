import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import {
  applyKhalaCodeAppPreferences,
  defaultKhalaCodeAppPreferences,
  type KhalaCodeAppPreferences,
} from "../src/shared/app-preferences"
import { mountKhalaCodeAppPreferencesSettingsSection } from "../src/ui/app-preferences-settings-section"

const setGlobal = (key: string, value: unknown): void => {
  Object.defineProperty(globalThis, key, {
    configurable: true,
    value,
    writable: true,
  })
}

const installDom = (): {
  readonly container: HTMLElement
  readonly cleanup: () => void
  readonly window: Window
} => {
  const window = new Window()
  const previousWindow = globalThis.window
  const previousDocument = globalThis.document
  const previousHTMLElement = globalThis.HTMLElement

  setGlobal("window", window)
  setGlobal("document", window.document)
  setGlobal("HTMLElement", window.HTMLElement)

  const container = window.document.createElement("section")
  window.document.body.append(container)

  return {
    container: container as unknown as HTMLElement,
    window,
    cleanup: () => {
      Object.defineProperty(globalThis, "window", { configurable: true, value: previousWindow })
      Object.defineProperty(globalThis, "document", { configurable: true, value: previousDocument })
      Object.defineProperty(globalThis, "HTMLElement", { configurable: true, value: previousHTMLElement })
      window.close()
    },
  }
}

describe("Khala Code app preferences settings section", () => {
  test("writes theme/font/toggle changes and applies root preferences", () => {
    const { cleanup, container, window } = installDom()
    let preferences: KhalaCodeAppPreferences = defaultKhalaCodeAppPreferences()
    const writes: KhalaCodeAppPreferences[] = []

    try {
      const section = mountKhalaCodeAppPreferencesSettingsSection({
        apply: next => applyKhalaCodeAppPreferences(window.document.documentElement as unknown as HTMLElement, next),
        read: () => preferences,
        reset: () => {
          preferences = defaultKhalaCodeAppPreferences()
          return preferences
        },
        write: next => {
          preferences = next
          writes.push(next)
        },
      })
      container.append(section.render())

      const color = container.querySelector('select[name="app-preference-colorScheme"]') as HTMLSelectElement
      color.value = "light"
      color.dispatchEvent(new window.Event("change") as unknown as Event)
      expect(writes.at(-1)?.colorScheme).toBe("light")
      expect(window.document.documentElement.dataset.khalaColorScheme).toBe("light")

      const errors = container.querySelector('input[name="app-preference-notifications.errors"]') as HTMLInputElement
      errors.checked = false
      errors.dispatchEvent(new window.Event("change") as unknown as Event)
      expect(writes.at(-1)?.notifications.errors).toBe(false)
      expect(container.textContent).toContain("Saved notifications.errors.")
    } finally {
      cleanup()
    }
  })

  test("resets preferences to defaults", () => {
    const { cleanup, container } = installDom()
    let preferences: KhalaCodeAppPreferences = {
      ...defaultKhalaCodeAppPreferences(),
      colorScheme: "light",
      uiFont: "mono",
    }

    try {
      const section = mountKhalaCodeAppPreferencesSettingsSection({
        apply: () => {},
        read: () => preferences,
        reset: () => {
          preferences = defaultKhalaCodeAppPreferences()
          return preferences
        },
        write: next => {
          preferences = next
        },
      })
      container.append(section.render())
      const reset = container.querySelector(".khala-preferences-reset") as HTMLButtonElement
      reset.click()
      expect(preferences).toEqual(defaultKhalaCodeAppPreferences())
      expect(container.textContent).toContain("Reset app preferences to defaults.")
    } finally {
      cleanup()
    }
  })
})
