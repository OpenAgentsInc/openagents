import { describe, expect, test } from "bun:test"

import { assertThemeParity, CANONICAL_DARK } from "./theme-parity.js"

describe("theme parity", () => {
  test("accepts the canonical dark palette", () => {
    expect(assertThemeParity({ ...CANONICAL_DARK })).toEqual({
      ok: true,
      mismatches: [],
    })
  })

  test("reports a changed background color", () => {
    expect(assertThemeParity({
      ...CANONICAL_DARK,
      bg: "#111",
    })).toEqual({
      ok: false,
      mismatches: ["bg: expected #000, received #111"],
    })
  })

  test("reports a missing key", () => {
    const actual: Record<string, string> = { ...CANONICAL_DARK }
    delete actual.danger

    expect(assertThemeParity(actual)).toEqual({
      ok: false,
      mismatches: ["danger: missing, expected #d32f2f"],
    })
  })

  test("reports multiple mismatches in canonical key order", () => {
    expect(assertThemeParity({
      ...CANONICAL_DARK,
      bgSecondary: "#202020",
      warning: "#ffaa00",
    })).toEqual({
      ok: false,
      mismatches: [
        "bgSecondary: expected #151515, received #202020",
        "warning: expected #ffb400, received #ffaa00",
      ],
    })
  })

  test("ignores additional client palette keys", () => {
    expect(assertThemeParity({
      ...CANONICAL_DARK,
      overlay: "rgba(0, 0, 0, 0.5)",
    })).toEqual({
      ok: true,
      mismatches: [],
    })
  })

  test("exports flattened canonical dark values from autopilot-ui tokens", () => {
    expect(CANONICAL_DARK).toEqual({
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
