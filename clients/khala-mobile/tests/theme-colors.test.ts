import { describe, expect, test } from "bun:test"

import { colors } from "../src/ignite/theme/colorsDark"

const hexToRgb = (hex: string): { b: number; g: number; r: number } => {
  const value = hex.replace("#", "")
  return {
    b: parseInt(value.slice(4, 6), 16),
    g: parseInt(value.slice(2, 4), 16),
    r: parseInt(value.slice(0, 2), 16),
  }
}

// Oracle for khala_mobile.theme.base_background_dark_navy.v1
describe("contract khala_mobile.theme.base_background_dark_navy.v1", () => {
  test("base_background_is_dark_navy.unit — the app-wide base background is a very dark navy blue", () => {
    // `background` is the token every Ignite `Screen` reads; it must resolve to
    // the palette's `neutral200` slot.
    expect(colors.background).toBe(colors.palette.neutral200)

    const { b, g, r } = hexToRgb(colors.background)

    // Very dark: every channel low (clearly not a bright surface).
    expect(r).toBeLessThanOrEqual(0x20)
    expect(g).toBeLessThanOrEqual(0x20)
    expect(b).toBeLessThanOrEqual(0x30)

    // Clearly navy: blue is the dominant channel, and it's not pure black.
    expect(b).toBeGreaterThan(r)
    expect(b).toBeGreaterThan(g)
    expect(b).toBeGreaterThanOrEqual(0x10)

    // Not the retired warm brown (#191015 = rgb(25,16,21), where red > blue).
    expect(colors.background.toLowerCase()).not.toBe("#191015")
  })
})
