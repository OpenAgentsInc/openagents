import { describe, expect, test } from "bun:test"

import {
  autopilotCoreDarkCssVars,
  autopilotCoreDarkTokens,
  autopilotCoreNativeTheme,
  autopilotCoreProtocolDarkTokens,
  colorTokens,
  colorVar,
  oaTokens,
  themeCss,
  themeCssVars,
} from "../src/index"

describe("Autopilot design tokens", () => {
  test("own the canonical dark palette", () => {
    expect(autopilotCoreDarkTokens).toEqual({
      background: "#000",
      backgroundSecondary: "#151515",
      text: "#d7d8e5",
      textSecondary: "#8a8c93",
      outline: "#525458",
      primary: "#fff",
      tones: {
        success: "#00c853",
        warning: "#ffb400",
        danger: "#d32f2f",
        info: "#2979ff",
      },
    })
  })

  test("emit Foldkit CSS variables", () => {
    expect(autopilotCoreDarkCssVars(autopilotCoreDarkTokens)).toEqual({
      "--bg": "#000",
      "--bg-secondary": "#151515",
      "--text": "#d7d8e5",
      "--text-secondary": "#8a8c93",
      "--outline": "#525458",
      "--primary": "#fff",
      "--success": "#00c853",
      "--warning": "#ffb400",
      "--danger": "#d32f2f",
      "--info": "#2979ff",
    })
  })

  test("emit a native-shaped theme", () => {
    expect(autopilotCoreNativeTheme(autopilotCoreDarkTokens)).toEqual({
      colors: {
        background: "#000",
        backgroundSecondary: "#151515",
        text: "#d7d8e5",
        textSecondary: "#8a8c93",
        outline: "#525458",
        primary: "#fff",
        success: "#00c853",
        warning: "#ffb400",
        danger: "#d32f2f",
        info: "#2979ff",
      },
    })
  })

  test("emit flattened protocol parity values", () => {
    expect(autopilotCoreProtocolDarkTokens()).toEqual({
      bg: "#000",
      bgSecondary: "#151515",
      text: "#d7d8e5",
      textSecondary: "#8a8c93",
      outline: "#525458",
      primary: "#fff",
      success: "#00c853",
      warning: "#ffb400",
      danger: "#d32f2f",
      info: "#2979ff",
    })
  })
})

describe("oaTokens typed theme source (#6046)", () => {
  test("core palette parity: the typed color tokens match the canonical dark tokens", () => {
    // The single source of truth must agree with the legacy core dark tokens
    // so migrating to it is theme-preserving.
    expect(colorTokens.bg).toBe(autopilotCoreDarkTokens.background)
    expect(colorTokens.bgSecondary).toBe(autopilotCoreDarkTokens.backgroundSecondary)
    expect(colorTokens.text).toBe(autopilotCoreDarkTokens.text)
    expect(colorTokens.textSecondary).toBe(autopilotCoreDarkTokens.textSecondary)
    expect(colorTokens.outline).toBe(autopilotCoreDarkTokens.outline)
    expect(colorTokens.primary).toBe(autopilotCoreDarkTokens.primary)
    expect(colorTokens.success).toBe(autopilotCoreDarkTokens.tones.success)
    expect(colorTokens.warning).toBe(autopilotCoreDarkTokens.tones.warning)
    expect(colorTokens.danger).toBe(autopilotCoreDarkTokens.tones.danger)
  })

  test("themeCssVars projects every color token to an --oa-color-* property", () => {
    const vars = themeCssVars()
    expect(vars["--oa-color-text"]).toBe("#d7d8e5")
    expect(vars["--oa-color-text-secondary"]).toBe("#8a8c93")
    expect(vars["--oa-color-bg-secondary"]).toBe("#151515")
    expect(vars["--oa-color-hud-primary"]).toBe("#ffffff")
    expect(vars["--oa-radius-3xl"]).toBe("12px")
    expect(vars["--oa-space-4"]).toBe("1rem")
    expect(vars["--oa-z-return-button"]).toBe("9999")
    expect(vars["--oa-shadow-pane"]).toBe("0 24px 64px rgb(0 0 0 / 0.6)")
  })

  test("themeCss emits a :root block with the projected vars", () => {
    const css = themeCss()
    expect(css.startsWith(":root {")).toBe(true)
    expect(css).toContain("--oa-color-text: #d7d8e5;")
    expect(css).toContain("--oa-color-bg: #000;")
  })

  test("themeCss accepts a custom selector", () => {
    expect(themeCss("[data-theme]").startsWith("[data-theme] {")).toBe(true)
  })

  test("colorVar returns a var() reference with a literal fallback", () => {
    expect(colorVar("text")).toBe("var(--oa-color-text, #d7d8e5)")
    expect(colorVar("accent")).toBe("var(--oa-color-accent, #f5b73a)")
  })

  test("oaTokens is the aggregate over every token group", () => {
    expect(Object.keys(oaTokens).sort()).toEqual(
      [
        "color",
        "font",
        "fontSize",
        "letterSpacing",
        "lineHeight",
        "motion",
        "radius",
        "shadow",
        "space",
        "zIndex",
      ].sort(),
    )
  })

  test("reading tokens never touches the DOM (no window dependency)", () => {
    // Regression guard for the StyleX `window is not defined` throw that this
    // package replaces: token access must work in a headless/server context.
    const globalAny = globalThis as { window?: unknown }
    const originalWindow = globalAny.window
    delete globalAny.window
    try {
      expect(() => themeCssVars()).not.toThrow()
      expect(() => colorVar("text")).not.toThrow()
      expect(oaTokens.color.bg).toBe("#000")
    } finally {
      if (originalWindow !== undefined) {
        ;(globalThis as { window?: unknown }).window = originalWindow
      }
    }
  })
})
