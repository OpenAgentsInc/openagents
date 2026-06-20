import { describe, expect, test } from "bun:test"

import type { SessionSummary } from "./control.js"
import { mergeSessions } from "./multi-origin.js"

const session = (
  sessionRef: string,
  updatedAt: string,
  overrides: Partial<SessionSummary> = {},
): SessionSummary => ({
  sessionRef,
  adapter: "codex",
  state: "running",
  accountRefHash: null,
  updatedAt,
  ...overrides,
})

describe("multi-origin sessions", () => {
  test("tags sessions by origin", () => {
    const merged = mergeSessions({
      local: [session("local.1", "2026-06-13T10:00:00.000Z")],
      bridge: [session("bridge.1", "2026-06-13T09:00:00.000Z")],
      cloud: [session("cloud.1", "2026-06-13T08:00:00.000Z")],
    })

    expect(merged.map(({ sessionRef, origin }) => ({ sessionRef, origin }))).toEqual([
      { sessionRef: "local.1", origin: "local" },
      { sessionRef: "bridge.1", origin: "bridge" },
      { sessionRef: "cloud.1", origin: "cloud" },
    ])
  })

  test("deduplicates by session ref with local before bridge before cloud", () => {
    const merged = mergeSessions({
      local: [session("same", "2026-06-13T08:00:00.000Z", { state: "completed" })],
      bridge: [session("same", "2026-06-13T10:00:00.000Z", { state: "failed" })],
      cloud: [session("same", "2026-06-13T12:00:00.000Z", { state: "running" })],
    })

    expect(merged).toHaveLength(1)
    expect(merged[0]).toMatchObject({
      sessionRef: "same",
      origin: "local",
      state: "completed",
      updatedAt: "2026-06-13T08:00:00.000Z",
    })
  })

  test("sorts by updatedAt descending and preserves order for equal timestamps", () => {
    const merged = mergeSessions({
      local: [
        session("older", "2026-06-13T08:00:00.000Z"),
        session("tie.local", "2026-06-13T09:00:00.000Z"),
      ],
      bridge: [session("newer", "2026-06-13T10:00:00.000Z")],
      cloud: [session("tie.cloud", "2026-06-13T09:00:00.000Z")],
    })

    expect(merged.map((row) => row.sessionRef)).toEqual([
      "newer",
      "tie.local",
      "tie.cloud",
      "older",
    ])
  })
})
