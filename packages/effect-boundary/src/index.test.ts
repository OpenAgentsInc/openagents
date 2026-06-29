import { describe, expect, test } from "bun:test"
import { Config, Effect, Random, Redacted, Schema as S } from "effect"

import {
  EffectBoundaryError,
  configOverridesLayer,
  decodeRowEffect,
  deterministicNowEffect,
  effectFailure,
  effectFailureTag,
  parseJsonEffect,
  readConfigEffect,
  readRedactedStringConfigEffect,
  readRequestJsonEffect,
  redactedFixture,
  withDeterministicRandom,
} from "./index.js"

const BoundaryFixture = S.Struct({
  id: S.String,
})

describe("@openagentsinc/effect-boundary", () => {
  test("parseJsonEffect returns a typed redacted error for malformed JSON", async () => {
    const error = await effectFailure(parseJsonEffect(BoundaryFixture, "{bad", "fixture.parse"))
    expect(error).toBeInstanceOf(EffectBoundaryError)
    expect(error).toMatchObject({
      boundary: "json",
      contentRedacted: true,
      operation: "fixture.parse",
      reasonKind: "malformed_json",
    })
    expect(error.reasonRef).toStartWith("boundary.json.fixture.parse.")
  })

  test("readRequestJsonEffect preserves operation names for missing fields", async () => {
    const request = new Request("https://openagents.test/fixture", {
      body: JSON.stringify({}),
      method: "POST",
    })
    const error = await effectFailure(
      readRequestJsonEffect(BoundaryFixture, request, "fixture.request"),
    )
    expect(error.boundary).toBe("request_json")
    expect(error.operation).toBe("fixture.request")
    expect(error.contentRedacted).toBe(true)
    expect(error.reasonKind).toBe("schema_decode")
  })

  test("decodeRowEffect reports wrong row shape without returning undefined", async () => {
    const error = await effectFailure(
      decodeRowEffect(BoundaryFixture, { id: 123 }, "fixture.row"),
    )
    expect(error.boundary).toBe("row")
    expect(error.reasonRef).toStartWith("boundary.row.fixture.row.")
  })

  test("readConfigEffect and redacted config helpers keep config failures typed", async () => {
    const missing = await effectFailure(
      readRedactedStringConfigEffect("SECRET_TOKEN", "fixture.secret").pipe(
        Effect.provide(configOverridesLayer({})),
      ),
    )
    expect(missing).toMatchObject({
      boundary: "config",
      contentRedacted: true,
      operation: "fixture.secret",
      reasonKind: "config_failed",
    })

    const value = await Effect.runPromise(
      readConfigEffect(Config.string("MODE"), "fixture.mode").pipe(
        Effect.provide(configOverridesLayer({ MODE: "test" })),
      ),
    )
    expect(value).toBe("test")
  })

  test("redactedFixture and typed failure assertions support Effect tests", async () => {
    const secret = redactedFixture("fixture-secret")
    expect(Redacted.value(secret)).toBe("fixture-secret")
    await expect(effectFailureTag(Effect.fail(new EffectBoundaryError({
      boundary: "json",
      contentRedacted: true,
      operation: "fixture.failure",
      reasonKind: "malformed_json",
      reasonRef: "boundary.json.fixture.failure.expected",
    })))).resolves.toBe("EffectBoundaryError")
  })

  test("deterministic random and clock helpers support repeatable Effect tests", async () => {
    const sample = Random.next
    const first = await Effect.runPromise(withDeterministicRandom(sample, "seed-fixture"))
    const second = await Effect.runPromise(withDeterministicRandom(sample, "seed-fixture"))
    expect(first).toBe(second)

    const now = await Effect.runPromise(deterministicNowEffect("2026-06-29T00:00:00.000Z"))
    expect(now.toISOString()).toBe("2026-06-29T00:00:00.000Z")
  })
})
