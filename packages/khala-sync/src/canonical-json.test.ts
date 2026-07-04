import { describe, expect, test } from "bun:test"
import * as fc from "fast-check"
import { canonicalJson, CanonicalJsonError } from "./index.js"

describe("canonicalJson", () => {
  test("sorts object keys recursively", () => {
    expect(canonicalJson({ b: 1, a: { d: 2, c: 3 } })).toBe(
      '{"a":{"c":3,"d":2},"b":1}',
    )
  })

  test("emits no whitespace", () => {
    expect(canonicalJson({ a: [1, "x", null], b: true })).toBe(
      '{"a":[1,"x",null],"b":true}',
    )
  })

  test("preserves array order (arrays are not sorted)", () => {
    expect(canonicalJson([3, 1, 2])).toBe("[3,1,2]")
  })

  test("drops undefined object members", () => {
    expect(canonicalJson({ a: 1, gone: undefined, z: 2 })).toBe('{"a":1,"z":2}')
  })

  test("rejects undefined array elements with a typed error", () => {
    expect(() => canonicalJson({ xs: [1, undefined, 3] })).toThrow(
      CanonicalJsonError,
    )
    try {
      canonicalJson({ xs: [1, undefined, 3] })
      throw new Error("expected throw")
    } catch (err) {
      expect(err).toBeInstanceOf(CanonicalJsonError)
      expect((err as CanonicalJsonError)._tag).toBe("CanonicalJsonError")
      expect((err as CanonicalJsonError).path).toEqual(["xs", 1])
    }
  })

  test("rejects non-finite numbers with a typed error", () => {
    for (const bad of [Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY]) {
      expect(() => canonicalJson({ n: bad })).toThrow(CanonicalJsonError)
    }
    try {
      canonicalJson({ n: Number.NaN })
      throw new Error("expected throw")
    } catch (err) {
      expect(err).toBeInstanceOf(CanonicalJsonError)
      expect((err as CanonicalJsonError).path).toEqual(["n"])
    }
  })

  test("rejects unsupported value types with a typed error", () => {
    expect(() => canonicalJson({ f: () => 1 })).toThrow(CanonicalJsonError)
    expect(() => canonicalJson({ b: 1n })).toThrow(CanonicalJsonError)
    expect(() => canonicalJson(Symbol("s"))).toThrow(CanonicalJsonError)
  })

  test("normalizes -0 to 0", () => {
    expect(canonicalJson(-0)).toBe("0")
    expect(canonicalJson({ n: -0 })).toBe('{"n":0}')
  })

  test("escapes strings like JSON.stringify", () => {
    expect(canonicalJson('he said "hi"\n')).toBe('"he said \\"hi\\"\\n"')
  })

  test("primitives and empty containers", () => {
    expect(canonicalJson(null)).toBe("null")
    expect(canonicalJson(true)).toBe("true")
    expect(canonicalJson(false)).toBe("false")
    expect(canonicalJson(0.1)).toBe("0.1")
    expect(canonicalJson({})).toBe("{}")
    expect(canonicalJson([])).toBe("[]")
  })

  test("output is insertion-order independent (hash-stable)", () => {
    const a = canonicalJson({ x: 1, y: { b: 2, a: 3 }, z: [1, 2] })
    const b = canonicalJson({ z: [1, 2], y: { a: 3, b: 2 }, x: 1 })
    expect(a).toBe(b)
  })

  test("property: parses back to a deep-equal value and is idempotent", () => {
    const jsonValue = fc.jsonValue()
    fc.assert(
      fc.property(jsonValue, (value) => {
        const s = canonicalJson(value)
        const parsed: unknown = JSON.parse(s)
        // Compare against the JSON.stringify/parse normalization of the
        // input (which, for fc.jsonValue(), is the value itself).
        expect(parsed).toEqual(value as never)
        // Idempotent: canonicalizing the parsed value yields the same bytes.
        expect(canonicalJson(parsed)).toBe(s)
      }),
      { seed: 20260704, numRuns: 256 },
    )
  })

  test("property: key insertion order never changes the output", () => {
    const record = fc.dictionary(fc.string(), fc.jsonValue(), { maxKeys: 8 })
    fc.assert(
      fc.property(record, fc.infiniteStream(fc.nat()), (obj, randoms) => {
        const keys = Object.keys(obj)
        // Rebuild the object with a shuffled insertion order.
        const it = randoms[Symbol.iterator]()
        const shuffled = [...keys]
        for (let i = shuffled.length - 1; i > 0; i--) {
          const r = it.next().value as number
          const j = r % (i + 1)
          const tmp = shuffled[i]!
          shuffled[i] = shuffled[j]!
          shuffled[j] = tmp
        }
        const reordered: Record<string, unknown> = {}
        for (const k of shuffled) reordered[k] = obj[k]
        expect(canonicalJson(reordered)).toBe(canonicalJson(obj))
      }),
      { seed: 20260704, numRuns: 128 },
    )
  })
})
