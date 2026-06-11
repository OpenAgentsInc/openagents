import { describe, expect, test } from "bun:test"
import {
  openagentsThemeJson,
  resolveTheme,
  resolveThemeHex,
  theme,
  type PylonThemeColorSlot,
} from "../src/tui/theme"

describe("tui theme tokens", () => {
  test("every semantic slot resolves to a def", () => {
    for (const slot of Object.keys(openagentsThemeJson.theme) as PylonThemeColorSlot[]) {
      expect(resolveThemeHex(openagentsThemeJson, slot)).toMatch(/^#[0-9A-Fa-f]{6}$/)
    }
  })

  test("resolved theme exposes parsed colors and a syntax style", () => {
    const resolved = resolveTheme(openagentsThemeJson)
    expect(resolved.name).toBe("openagents")
    expect(resolved.colors.border).toBeDefined()
    expect(resolved.colors.error).toBeDefined()
    expect(resolved.syntaxStyle).toBeDefined()
  })

  test("unknown color ref fails loudly", () => {
    const broken = {
      ...openagentsThemeJson,
      theme: { ...openagentsThemeJson.theme, border: "nope" },
    }
    expect(() => resolveTheme(broken)).toThrow(/unknown def "nope"/)
  })

  test("unknown syntax ref fails loudly", () => {
    const broken = {
      ...openagentsThemeJson,
      syntax: { ...openagentsThemeJson.syntax, keyword: { ref: "missing" } },
    }
    expect(() => resolveTheme(broken)).toThrow(/unknown def "missing"/)
  })

  test("the module-level theme matches the openagents json", () => {
    expect(theme.name).toBe("openagents")
  })

  test("no inline color literals in the Solid view layer", async () => {
    // All colors must come through theme tokens; parseColor stays in theme.ts.
    for (const file of ["app.tsx", "store.ts", "bridge.ts"]) {
      const source = await Bun.file(new URL(`../src/tui/${file}`, import.meta.url)).text()
      expect(source.includes("parseColor(")).toBe(false)
      expect(/#[0-9A-Fa-f]{6}/.test(source)).toBe(false)
    }
  })
})
