/**
 * Unit tests for the offset codec (PROTOCOL.md §8 properties): lexicographic ==
 * numeric ordering, strict monotonicity, sentinel handling, URL-safety.
 */
import { describe, expect, test } from "bun:test"
import {
  isSentinel,
  OFFSET_BEGINNING,
  OFFSET_NOW,
  offsetForPosition,
  parseOffset,
  tailOffset,
} from "./offset.ts"

describe("offset codec", () => {
  test("offsets are strictly increasing and lexicographically sortable", () => {
    const offs = [0, 1, 9, 10, 100, 999, 1000, 123456789].map(offsetForPosition)
    const sorted = [...offs].sort()
    expect(sorted).toEqual(offs) // lexicographic order == numeric order
    for (let i = 1; i < offs.length; i++) {
      expect(offs[i]! > offs[i - 1]!).toBe(true)
    }
  })

  test("never mints a reserved sentinel", () => {
    for (const n of [0, 1, 1000000]) {
      const o = offsetForPosition(n)
      expect(o).not.toBe(OFFSET_BEGINNING)
      expect(o).not.toBe(OFFSET_NOW)
      expect(isSentinel(o)).toBe(false)
    }
  })

  test("offsets are URL-query safe (no , & = ? /)", () => {
    const o = offsetForPosition(42)
    expect(/[,&=?/]/.test(o)).toBe(false)
  })

  test("tailOffset == offsetForPosition(byteLength)", () => {
    expect(tailOffset(7)).toBe(offsetForPosition(7))
  })

  test("parseOffset: sentinels", () => {
    expect(parseOffset(undefined)).toEqual({ kind: "beginning" })
    expect(parseOffset("-1")).toEqual({ kind: "beginning" })
    expect(parseOffset("now")).toEqual({ kind: "now" })
  })

  test("parseOffset: valid position round-trips", () => {
    const o = offsetForPosition(123)
    const parsed = parseOffset(o)
    expect(parsed?.kind).toBe("position")
    if (parsed?.kind === "position") {
      expect(parsed.position).toBe(123)
      expect(parsed.offset).toBe(o)
    }
  })

  test("parseOffset: malformed → null", () => {
    expect(parseOffset("abc")).toBeNull()
    expect(parseOffset("12,34")).toBeNull()
    expect(parseOffset("")).toBeNull()
  })
})
