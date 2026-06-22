import { describe, expect, test } from "bun:test"

import {
  autopilotCoreDarkCssVars,
  autopilotCoreDarkTokens,
  autopilotCoreNativeTheme,
  autopilotCoreProtocolDarkTokens,
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
