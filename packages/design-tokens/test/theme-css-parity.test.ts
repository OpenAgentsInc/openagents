import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import { fileURLToPath } from "node:url"

import { colorTokens, themeCss, themeCssVars } from "../src/index"

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

  test("theme.css exposes the new component-chrome + translucent text tokens", () => {
    const file = readFileSync(themeCssPath, "utf8")
    const vars = themeCssVars()
    for (const [name, value] of [
      ["--oa-color-component-text", "#f1efe8"],
      ["--oa-color-component-border", "#222"],
      ["--oa-color-component-border-strong", "#333"],
      ["--oa-color-component-surface", "#080808"],
      ["--oa-color-component-surface-deep", "#010102"],
      ["--oa-color-component-surface-active", "#141414"],
      ["--oa-color-component-input-bg", "#030303"],
      ["--oa-color-danger-hover", "#ff6f00"],
      ["--oa-color-text-on-dark60", "rgba(255, 255, 255, 0.6)"],
      ["--oa-color-text-on-dark55", "rgba(255, 255, 255, 0.55)"],
      ["--oa-color-text-on-dark45", "rgba(255, 255, 255, 0.45)"],
      ["--oa-color-text-on-dark35", "rgba(255, 255, 255, 0.35)"],
      ["--oa-color-text-on-dark30", "rgba(255, 255, 255, 0.3)"],
    ] as const) {
      expect(vars[name]).toBe(value)
      expect(file).toContain(`${name}: ${value};`)
    }
  })

  test("the new tokens carry their exact original values on the typed source", () => {
    expect(colorTokens.componentText).toBe("#f1efe8")
    expect(colorTokens.componentBorder).toBe("#222")
    expect(colorTokens.componentBorderStrong).toBe("#333")
    expect(colorTokens.componentSurface).toBe("#080808")
    expect(colorTokens.componentSurfaceDeep).toBe("#010102")
    expect(colorTokens.componentSurfaceActive).toBe("#141414")
    expect(colorTokens.componentInputBg).toBe("#030303")
    expect(colorTokens.dangerHover).toBe("#ff6f00")
    expect(colorTokens.textOnDark60).toBe("rgba(255, 255, 255, 0.6)")
    expect(colorTokens.textOnDark55).toBe("rgba(255, 255, 255, 0.55)")
    expect(colorTokens.textOnDark45).toBe("rgba(255, 255, 255, 0.45)")
    expect(colorTokens.textOnDark35).toBe("rgba(255, 255, 255, 0.35)")
    expect(colorTokens.textOnDark30).toBe("rgba(255, 255, 255, 0.3)")
  })
})
