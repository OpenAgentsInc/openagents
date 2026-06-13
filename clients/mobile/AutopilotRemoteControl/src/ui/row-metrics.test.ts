import { describe, expect, test } from "bun:test"

import { fixedRowLabelHeight } from "./row-metrics"

describe("row metrics", () => {
  test("defaults to two lines", () => {
    expect(fixedRowLabelHeight(18)).toBe(36)
  })

  test("uses a custom line count", () => {
    expect(fixedRowLabelHeight(20, 3)).toBe(60)
  })

  test("allows zero line height", () => {
    expect(fixedRowLabelHeight(0)).toBe(0)
  })

  test("returns zero for negative line height", () => {
    expect(fixedRowLabelHeight(-18)).toBe(0)
  })

  test("returns zero for non-finite line height", () => {
    expect(fixedRowLabelHeight(Number.NaN)).toBe(0)
    expect(fixedRowLabelHeight(Number.POSITIVE_INFINITY)).toBe(0)
  })

  test("returns zero for invalid line count", () => {
    expect(fixedRowLabelHeight(18, -1)).toBe(0)
    expect(fixedRowLabelHeight(18, Number.NaN)).toBe(0)
  })
})
