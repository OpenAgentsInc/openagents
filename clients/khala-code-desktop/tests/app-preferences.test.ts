import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import {
  KHALA_CODE_APP_PREFERENCES_STORAGE_KEY,
  applyKhalaCodeAppPreferences,
  defaultKhalaCodeAppPreferences,
  parseKhalaCodeAppPreferences,
  readKhalaCodeAppPreferences,
  resetKhalaCodeAppPreferences,
  updateKhalaCodeAppPreference,
  writeKhalaCodeAppPreferences,
} from "../src/shared/app-preferences"

describe("Khala Code app preferences", () => {
  test("parses partial preferences while preserving current defaults", () => {
    const preferences = parseKhalaCodeAppPreferences({
      colorScheme: "light",
      uiFont: "mono",
      notifications: { errors: false },
      sounds: { completions: true, volume: 2 },
      features: { denseWorkbench: true },
    })

    expect(preferences).toMatchObject({
      colorScheme: "light",
      uiFont: "mono",
      codeFont: "default",
      notifications: {
        agentEvents: true,
        errors: false,
      },
      sounds: {
        completions: true,
        volume: 1,
      },
      features: {
        denseWorkbench: true,
        compactComposer: false,
      },
    })
  })

  test("reads, writes, resets, and applies preferences without changing defaults", () => {
    const window = new Window()
    const storage = window.localStorage as unknown as Storage
    const root = window.document.documentElement as unknown as HTMLElement

    const updated = updateKhalaCodeAppPreference(
      updateKhalaCodeAppPreference(defaultKhalaCodeAppPreferences(), "uiFont", "mono"),
      "notifications.errors",
      false,
    )
    writeKhalaCodeAppPreferences(storage, updated)
    expect(storage.getItem(KHALA_CODE_APP_PREFERENCES_STORAGE_KEY)).toContain('"uiFont": "mono"')
    expect(readKhalaCodeAppPreferences(storage)).toMatchObject({
      uiFont: "mono",
      notifications: { errors: false },
    })

    applyKhalaCodeAppPreferences(root, updated)
    expect(root.dataset.khalaColorScheme).toBe("khala")
    expect(root.style.getPropertyValue("--khala-ui-font-family")).toContain("SFMono")

    const reset = resetKhalaCodeAppPreferences(storage)
    expect(reset).toEqual(defaultKhalaCodeAppPreferences())
    expect(storage.getItem(KHALA_CODE_APP_PREFERENCES_STORAGE_KEY)).toBeNull()
    window.close()
  })
})
