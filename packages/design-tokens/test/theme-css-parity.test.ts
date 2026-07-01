import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import { fileURLToPath } from "node:url"

import { colorTokens, fontTokens, themeCss, themeCssVars } from "../src/index"

const themeCssPath = fileURLToPath(new URL("../src/theme.css", import.meta.url))

describe("design-tokens theme.css (#6046 part 2)", () => {
  // The checked-in src/theme.css is the static projection that CSS consumers
  // @import to resolve the --oa-* custom properties. It MUST stay byte-identical
  // to the live themeCss() output (minus the generated banner), so the typed
  // source in theme.ts remains the single source of truth.
  test("theme.css ends with exactly the live themeCss() :root block", () => {
    const file = readFileSync(themeCssPath, "utf8")
    expect(file.endsWith(themeCss())).toBe(true)
  })

  test("theme.css exposes the Khala sci-fi component chrome + font tokens", () => {
    const file = readFileSync(themeCssPath, "utf8")
    const vars = themeCssVars()
    for (const [name, value] of [
      ["--oa-color-khala-energy-cyan", "#4fd0ff"],
      ["--oa-color-khala-energy-text-strong", "#cdeeff"],
      ["--oa-color-khala-surface", "#05080e"],
      ["--oa-color-khala-border", "#1d2a44"],
      ["--oa-color-component-text", "#f1efe8"],
      ["--oa-color-component-border", "#1d2a44"],
      ["--oa-color-component-border-strong", "#34507f"],
      ["--oa-color-component-surface", "#05080e"],
      ["--oa-color-component-surface-deep", "#000"],
      ["--oa-color-component-surface-active", "#0a1b31"],
      ["--oa-color-component-input-bg", "#05080e"],
      ["--oa-color-danger-hover", "#f0727a"],
      ["--oa-color-text-on-dark60", "rgb(183 200 220 / 0.6)"],
      ["--oa-color-text-on-dark55", "rgb(183 200 220 / 0.55)"],
      ["--oa-color-text-on-dark45", "rgb(183 200 220 / 0.45)"],
      ["--oa-color-text-on-dark35", "rgb(183 200 220 / 0.35)"],
      ["--oa-color-text-on-dark30", "rgb(183 200 220 / 0.3)"],
      ["--oa-font-code", fontTokens.code],
      ["--oa-font-sans", fontTokens.sans],
    ] as const) {
      expect(vars[name]).toBe(value)
      expect(file).toContain(`${name}: ${value};`)
    }
  })

  test("component chrome aliases the Khala blue sci-fi palette on the typed source", () => {
    expect(colorTokens.khalaEnergyCyan).toBe("#4fd0ff")
    expect(colorTokens.khalaEnergyTextStrong).toBe("#cdeeff")
    expect(colorTokens.khalaSurface).toBe("#05080e")
    expect(colorTokens.khalaBorder).toBe("#1d2a44")
    expect(colorTokens.componentText).toBe("#f1efe8")
    expect(colorTokens.componentBorder).toBe("#1d2a44")
    expect(colorTokens.componentBorderStrong).toBe("#34507f")
    expect(colorTokens.componentSurface).toBe("#05080e")
    expect(colorTokens.componentSurfaceDeep).toBe("#000")
    expect(colorTokens.componentSurfaceActive).toBe("#0a1b31")
    expect(colorTokens.componentInputBg).toBe("#05080e")
    expect(colorTokens.dangerHover).toBe("#f0727a")
    expect(colorTokens.textOnDark60).toBe("rgb(183 200 220 / 0.6)")
    expect(colorTokens.textOnDark55).toBe("rgb(183 200 220 / 0.55)")
    expect(colorTokens.textOnDark45).toBe("rgb(183 200 220 / 0.45)")
    expect(colorTokens.textOnDark35).toBe("rgb(183 200 220 / 0.35)")
    expect(colorTokens.textOnDark30).toBe("rgb(183 200 220 / 0.3)")
    expect(fontTokens.sans).not.toContain("Berkeley")
    expect(fontTokens.code).toContain("Commit Mono")
  })
})
