import { describe, expect, test } from "bun:test"
import { mkdtemp } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { appendSessionEvent, loadSessionRecord } from "../src/session-record-store"

describe("session record store", () => {
  test("append preserves fresh session event order", async () => {
    const dir = await mkdtemp(join(tmpdir(), "pylon-session-records-"))
    const sessionRef = "session.pylon.control.test"
    const first = { phase: "queued", observedAt: new Date(0).toISOString() }
    const second = { phase: "running", observedAt: new Date(1).toISOString() }

    await appendSessionEvent(dir, sessionRef, first)
    await appendSessionEvent(dir, sessionRef, second)

    const record = await loadSessionRecord(dir, sessionRef)
    expect(record?.events).toHaveLength(2)
    expect(record?.events).toEqual([first, second])
  })

  test("load returns null for an unknown session ref", async () => {
    const dir = await mkdtemp(join(tmpdir(), "pylon-session-records-"))

    await expect(loadSessionRecord(dir, "session.pylon.control.unknown")).resolves.toBeNull()
  })
})
