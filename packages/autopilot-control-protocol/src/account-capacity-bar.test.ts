import { describe, expect, test } from "bun:test"

import { capacityBar } from "./account-capacity-bar.js"

describe("account capacity bar", () => {
  test("shows unknown usage as an empty ok bar", () => {
    expect(capacityBar({ usedPct: null, exhausted: false })).toEqual({
      pct: 0,
      tone: "ok",
      label: "unknown",
    })
  })

  test("shows low usage as ok", () => {
    expect(capacityBar({ usedPct: 42, exhausted: false })).toEqual({
      pct: 42,
      tone: "ok",
      label: "42%",
    })
  })

  test("preserves fractional percentages in the label", () => {
    expect(capacityBar({ usedPct: 84.5, exhausted: false })).toEqual({
      pct: 84.5,
      tone: "ok",
      label: "84.5%",
    })
  })

  test("warns at the threshold", () => {
    expect(capacityBar({ usedPct: 85, exhausted: false })).toEqual({
      pct: 85,
      tone: "warn",
      label: "85%",
    })
  })

  test("clamps negative usage to zero", () => {
    expect(capacityBar({ usedPct: -12, exhausted: false })).toEqual({
      pct: 0,
      tone: "ok",
      label: "0%",
    })
  })

  test("clamps usage above one hundred", () => {
    expect(capacityBar({ usedPct: 125, exhausted: false })).toEqual({
      pct: 100,
      tone: "warn",
      label: "100%",
    })
  })

  test("exhausted state overrides warning tone and label", () => {
    expect(capacityBar({ usedPct: 90, exhausted: true })).toEqual({
      pct: 90,
      tone: "exhausted",
      label: "exhausted",
    })
  })

  test("exhausted state overrides unknown label", () => {
    expect(capacityBar({ usedPct: null, exhausted: true })).toEqual({
      pct: 0,
      tone: "exhausted",
      label: "exhausted",
    })
  })
})
