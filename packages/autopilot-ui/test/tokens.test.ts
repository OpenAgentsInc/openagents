import { describe, expect, test } from "bun:test"

import { cssVars, darkTokens, nativeTheme } from "../src/index"

describe("Autopilot UI dark tokens", () => {
  test("match the openagents.com dark palette", () => {
    expect(darkTokens).toEqual({
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

  test("maps tokens to Foldkit CSS variables", () => {
    expect(cssVars(darkTokens)).toEqual({
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

  test("maps tokens to a plain React Native theme object", () => {
    expect(nativeTheme(darkTokens)).toEqual({
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
})
