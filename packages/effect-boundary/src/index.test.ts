import { describe, expect, test } from "bun:test"
import { Effect, Redacted, Schema as S } from "effect"

import {
  boundaryTestConfigLayer,
  decodeRowEffect,
  expectBoundaryFailure,
  parseJsonEffect,
  readJsonFileEffect,
  readRedactedConfigEffect,
  readRequestJsonEffect,
} from "./index.js"

const Example = S.Struct({
  id: S.String,
})

describe("@openagentsinc/effect-boundary", () => {
  test("returns typed failures for malformed JSON", async () => {
    const error = await Effect.runPromise(
      expectBoundaryFailure(parseJsonEffect(Example, "{bad", "test.json"), {
        boundary: "json",
        operation: "test.json",
        reasonRef: "boundary.json.malformed",
      }),
    )

    expect(error._tag).toBe("OpenAgentsBoundaryError")
  })

  test("returns typed failures for missing fields", async () => {
    const error = await Effect.runPromise(
      expectBoundaryFailure(parseJsonEffect(Example, "{}", "test.missing"), {
        reasonRef: "boundary.schema.invalid",
      }),
    )

    expect(error.operation).toBe("test.missing")
  })

  test("returns typed failures for wrong row shape", async () => {
    const Row = S.Struct({
      id: S.String,
      status: S.Literals(["ok"]),
    })

    const error = await Effect.runPromise(
      expectBoundaryFailure(decodeRowEffect(Row, { id: "row_1", status: "bad" }, "db.row"), {
        boundary: "row",
        reasonRef: "boundary.row.invalid",
      }),
    )

    expect(error.message).toContain("schema")
  })

  test("reads request JSON through schema", async () => {
    const decoded = await Effect.runPromise(
      readRequestJsonEffect(Example, new Request("https://example.test", {
        body: JSON.stringify({ id: "req_1" }),
        method: "POST",
      }), "worker.request"),
    )

    expect(decoded).toEqual({ id: "req_1" })
  })

  test("reads local-state JSON through schema", async () => {
    const decoded = await Effect.runPromise(
      readJsonFileEffect(Example, async () => JSON.stringify({ id: "file_1" }), "pylon.file"),
    )

    expect(decoded).toEqual({ id: "file_1" })
  })

  test("keeps config failures typed and redacts successful secrets", async () => {
    const missing = await Effect.runPromise(
      expectBoundaryFailure(readRedactedConfigEffect("OPENAGENTS_SECRET", "config.secret"), {
        boundary: "config",
        reasonRef: "boundary.config.missing_or_invalid",
      }),
    )
    expect(missing.operation).toBe("config.secret")

    const secret = await Effect.runPromise(
      readRedactedConfigEffect("OPENAGENTS_SECRET", "config.secret").pipe(
        Effect.provide(boundaryTestConfigLayer({ OPENAGENTS_SECRET: "shh" })),
      ),
    )
    expect(String(secret)).toBe("<redacted>")
    expect(Redacted.value(secret)).toBe("shh")
  })

  test("asserts typed boundary failures in Effect tests", async () => {
    const error = await Effect.runPromise(
      expectBoundaryFailure(
        parseJsonEffect(Example, '{"id": 123}', "typed.assertion"),
        { operation: "typed.assertion" },
      ),
    )

    expect(error.reasonRef).toBe("boundary.schema.invalid")
  })
})
