import { describe, expect, test } from "bun:test"

import { drawerButtonBars } from "./drawer-interp"

describe("drawerButtonBars", () => {
  test("maps closed progress to the hamburger state", () => {
    expect(drawerButtonBars(0)).toEqual({
      topBar: {
        translateX: 0,
        rotateDeg: 0,
        width: 18,
        colorStop: 0,
        marginBottom: 0,
      },
      middleBar: {
        width: 18,
        colorStop: 0,
      },
      bottomBar: {
        translateX: 0,
        rotateDeg: 0,
        width: 18,
        colorStop: 0,
        marginTop: 4,
      },
      container: {
        translateX: 0,
      },
    })
  })

  test("maps open progress to the back-arrow state", () => {
    expect(drawerButtonBars(1)).toEqual({
      topBar: {
        translateX: -11.5,
        rotateDeg: -45,
        width: 12,
        colorStop: 1,
        marginBottom: -2,
      },
      middleBar: {
        width: 16,
        colorStop: 1,
      },
      bottomBar: {
        translateX: -11.5,
        rotateDeg: 45,
        width: 12,
        colorStop: 1,
        marginTop: 2,
      },
      container: {
        translateX: -60,
      },
    })
  })

  test("interpolates halfway progress linearly", () => {
    expect(drawerButtonBars(0.5)).toEqual({
      topBar: {
        translateX: -5.75,
        rotateDeg: -22.5,
        width: 15,
        colorStop: 0.5,
        marginBottom: -1,
      },
      middleBar: {
        width: 17,
        colorStop: 0.5,
      },
      bottomBar: {
        translateX: -5.75,
        rotateDeg: 22.5,
        width: 15,
        colorStop: 0.5,
        marginTop: 3,
      },
      container: {
        translateX: -30,
      },
    })
  })

  test("clamps progress below zero", () => {
    expect(drawerButtonBars(-0.25)).toEqual(drawerButtonBars(0))
  })

  test("clamps progress above one", () => {
    expect(drawerButtonBars(1.25)).toEqual(drawerButtonBars(1))
  })

  test("uses progress as each color stop", () => {
    const bars = drawerButtonBars(0.75)

    expect(bars.topBar.colorStop).toBe(0.75)
    expect(bars.middleBar.colorStop).toBe(0.75)
    expect(bars.bottomBar.colorStop).toBe(0.75)
  })

  test("clamps NaN progress to zero", () => {
    expect(drawerButtonBars(Number.NaN)).toEqual(drawerButtonBars(0))
  })
})
