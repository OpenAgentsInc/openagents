import { describe, expect, test } from "bun:test"
import {
  decodePublicCounterEntity,
  encodePublicCounterEntity,
  PUBLIC_COUNTER_ENTITY_TYPE,
  TOKENS_SERVED_COUNTER_ID,
} from "./public-counter.js"

const nowIso = "2026-07-04T15:20:11.412Z"

describe("public counter entity contract (KS-6.3)", () => {
  test("round-trips the tokens-served post-image", () => {
    const entity = decodePublicCounterEntity({
      counterId: TOKENS_SERVED_COUNTER_ID,
      lastEventAt: nowIso,
      total: 8_555_123,
    })
    expect(entity.counterId).toBe("tokens-served")
    expect(entity.total).toBe(8_555_123)
    expect(entity.lastEventAt).toBe(nowIso)
    expect(encodePublicCounterEntity(entity)).toEqual({
      counterId: "tokens-served",
      lastEventAt: nowIso,
      total: 8_555_123,
    })
    expect(PUBLIC_COUNTER_ENTITY_TYPE).toBe("public_counter")
  })

  test("lastEventAt is null before any event", () => {
    const entity = decodePublicCounterEntity({
      counterId: TOKENS_SERVED_COUNTER_ID,
      lastEventAt: null,
      total: 0,
    })
    expect(entity.lastEventAt).toBeNull()
  })

  test("structurally refuses non-counter material (SPEC §7 invariant 9)", () => {
    // Paths, emails, and free text cannot decode into the bounded counter id.
    for (const counterId of [
      "/Users/alice/secret",
      "alice@example.com",
      "Tokens Served",
      "",
    ]) {
      expect(() =>
        decodePublicCounterEntity({ counterId, lastEventAt: null, total: 1 }),
      ).toThrow()
    }
    // Totals must be non-negative safe integers — never strings or floats.
    for (const total of [-1, 1.5, "sk-live-secret", Number.NaN]) {
      expect(() =>
        decodePublicCounterEntity({
          counterId: TOKENS_SERVED_COUNTER_ID,
          lastEventAt: null,
          total,
        }),
      ).toThrow()
    }
    // Timestamps must be ISO-8601 UTC — arbitrary strings refuse.
    expect(() =>
      decodePublicCounterEntity({
        counterId: TOKENS_SERVED_COUNTER_ID,
        lastEventAt: "Bearer abc123",
        total: 1,
      }),
    ).toThrow()
  })
})
