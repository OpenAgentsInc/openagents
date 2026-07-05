import { describe, expect, test } from "bun:test"

import { toggleKnobTargetPosition } from "../src/sync/toggle-position-core"

describe("toggleKnobTargetPosition", () => {
  test("off with no offsets rests flush at the start edge", () => {
    expect(toggleKnobTargetPosition({ knobWidth: 24, on: false })).toEqual({
      marginStart: 0,
      start: "0%"
    })
  })

  test("on with no offsets pulls back exactly one knob-width from the end edge", () => {
    expect(toggleKnobTargetPosition({ knobWidth: 24, on: true })).toEqual({
      marginStart: -24,
      start: "100%"
    })
  })

  test("off honors offsetLeft as the inner rest padding", () => {
    expect(toggleKnobTargetPosition({ knobWidth: 24, offsetLeft: 4, on: false })).toEqual({
      marginStart: 4,
      start: "0%"
    })
  })

  test("on honors offsetRight, subtracted alongside the knob width", () => {
    expect(toggleKnobTargetPosition({ knobWidth: 24, offsetRight: 4, on: true })).toEqual({
      marginStart: -28,
      start: "100%"
    })
  })

  test("off ignores offsetRight and on ignores offsetLeft", () => {
    expect(
      toggleKnobTargetPosition({ knobWidth: 24, offsetLeft: 4, offsetRight: 6, on: false })
    ).toEqual({ marginStart: 4, start: "0%" })
    expect(
      toggleKnobTargetPosition({ knobWidth: 24, offsetLeft: 4, offsetRight: 6, on: true })
    ).toEqual({ marginStart: -30, start: "100%" })
  })

  test("a not-yet-measured knob (width 0) still returns a valid off-state rest position", () => {
    expect(toggleKnobTargetPosition({ knobWidth: 0, on: false })).toEqual({
      marginStart: 0,
      start: "0%"
    })
    expect(toggleKnobTargetPosition({ knobWidth: 0, on: true })).toEqual({
      marginStart: 0,
      start: "100%"
    })
  })

  test("clamps a negative knobWidth to 0 rather than pushing the knob past the track", () => {
    expect(toggleKnobTargetPosition({ knobWidth: -10, on: true })).toEqual({
      marginStart: 0,
      start: "100%"
    })
  })

  test("falls back non-finite knobWidth/offsets to 0 instead of propagating NaN", () => {
    expect(toggleKnobTargetPosition({ knobWidth: Number.NaN, on: false })).toEqual({
      marginStart: 0,
      start: "0%"
    })
    expect(
      toggleKnobTargetPosition({
        knobWidth: 24,
        offsetLeft: Number.POSITIVE_INFINITY,
        on: false
      })
    ).toEqual({ marginStart: 0, start: "0%" })
  })

  test("toggling on then off returns to the exact original off-state position", () => {
    const input = { knobWidth: 28, offsetLeft: 2, offsetRight: 2 } as const
    const off = toggleKnobTargetPosition({ ...input, on: false })
    const on = toggleKnobTargetPosition({ ...input, on: true })
    const offAgain = toggleKnobTargetPosition({ ...input, on: false })
    expect(off).toEqual(offAgain)
    expect(on).not.toEqual(off)
  })

  test("real-world knob/track sizing (32pt knob, 4pt inner padding both sides)", () => {
    expect(toggleKnobTargetPosition({ knobWidth: 24, offsetLeft: 4, offsetRight: 4, on: false })).toEqual({
      marginStart: 4,
      start: "0%"
    })
    expect(toggleKnobTargetPosition({ knobWidth: 24, offsetLeft: 4, offsetRight: 4, on: true })).toEqual({
      marginStart: -28,
      start: "100%"
    })
  })
})
