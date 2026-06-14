import { describe, expect, test } from "bun:test"
import { connectionSummary } from "../src/ui/panes/settings"

describe("CL-58 connectionSummary", () => {
  test("returns 'connecting…' when node is null", () => {
    expect(connectionSummary(null)).toBe("connecting…")
  })

  test("returns 'online' when node.ok is true", () => {
    expect(connectionSummary({ ok: true, schema: "autopilot.v1" })).toBe("online")
  })

  test("returns 'offline' when node.ok is false", () => {
    expect(connectionSummary({ ok: false, schema: "autopilot.v1" })).toBe("offline")
  })

  test("works with any schema value", () => {
    expect(connectionSummary({ ok: true, schema: "test.schema.v42" })).toBe("online")
    expect(connectionSummary({ ok: false, schema: "" })).toBe("offline")
  })
})
