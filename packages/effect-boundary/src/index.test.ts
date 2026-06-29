import { describe, expect, test } from "bun:test"
import { Config, Effect, Redacted, Schema as S } from "effect"

import {
  assertEffectFailsWithBoundaryError,
  boundaryConfigLayer,
  decodeRowEffect,
  deterministicBoundaryTestContext,
  parseJsonEffect,
  parseLocalStateJsonEffect,
  readRedactedConfigEffect,
  readRequestJsonEffect,
  redactedFixture,
} from "./index.js"

const Example = S.Struct({
  id: S.String,
  count: S.Number,
})

describe("Effect boundary helpers", () => {
  test("returns typed errors for malformed JSON", async () => {
    const error = await assertEffectFailsWithBoundaryError(
      parseJsonEffect(Example, "{bad", "example.parse"),
    )

    expect(error.boundary).toBe("json")
    expect(error.operation).toBe("example.parse")
    expect(error.reason).toBe("malformed_json")
    expect(error.reasonRef).toBe("boundary.json.example.parse.malformed_json")
  })

  test("returns typed errors for missing fields", async () => {
    const error = await assertEffectFailsWithBoundaryError(
      parseJsonEffect(Example, '{"id":"ok"}', "example.missing"),
    )

    expect(error.reason).toBe("schema_mismatch")
  })

  test("decodes request JSON through Effect Schema", async () => {
    const request = new Request("https://example.test", {
      method: "POST",
      body: '{"id":"row_1","count":1}',
    })

    const decoded = await Effect.runPromise(
      readRequestJsonEffect(Example, request, "route.example"),
    )

    expect(decoded).toEqual({ id: "row_1", count: 1 })
  })

  test("decodes local-state JSON and preserves source refs on failure", async () => {
    const error = await assertEffectFailsWithBoundaryError(
      parseLocalStateJsonEffect(
        Example,
        '{"id":1,"count":1}',
        "pylon.local_state",
        "file.pylon.local_state.active_run",
      ),
    )

    expect(error.boundary).toBe("file_json")
    expect(error.sourceRef).toBe("file.pylon.local_state.active_run")
  })

  test("returns typed errors for wrong row shape", async () => {
    const Row = S.Struct({
      id: S.String,
      payload_json: S.String,
    })
    const error = await assertEffectFailsWithBoundaryError(
      decodeRowEffect(Row, { id: 1, payload_json: "{}" }, "d1.row"),
    )

    expect(error.boundary).toBe("row")
    expect(error.reasonRef).toBe("boundary.row.d1.row.schema_mismatch")
  })

  test("maps Effect config failures without exposing secret names or values", async () => {
    const config = Config.redacted("API_KEY")
    const error = await assertEffectFailsWithBoundaryError(
      readRedactedConfigEffect(config, "config.api_key").pipe(
        Effect.provide(boundaryConfigLayer({})),
      ),
    )

    expect(error.boundary).toBe("config")
    expect(error.reason).toBe("config_unavailable")
    expect(String(error)).not.toContain("API_KEY")
  })

  test("reads redacted config through Effect config providers", async () => {
    const value = await Effect.runPromise(
      readRedactedConfigEffect(Config.redacted("API_KEY"), "config.api_key").pipe(
        Effect.provide(boundaryConfigLayer({ API_KEY: "sk-test" })),
      ),
    )

    expect(Redacted.value(value)).toBe("sk-test")
  })

  test("provides redacted fixtures and deterministic test context", () => {
    expect(redactedFixture("API_KEY")).toEqual({
      key: "API_KEY",
      value: "<redacted>",
      reasonRef: "boundary.test.api_key.redacted_fixture",
    })

    const left = deterministicBoundaryTestContext("seed")
    const right = deterministicBoundaryTestContext("seed")
    expect(left.now.toISOString()).toBe("2026-01-01T00:00:00.000Z")
    expect([left.random(), left.random()]).toEqual([right.random(), right.random()])
  })
})
