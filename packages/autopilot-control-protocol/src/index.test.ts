import { describe, expect, test } from "bun:test"

import {
  acceptEvent,
  applyExternalResolution,
  decodeControlCommand,
  decodeSessionEvent,
  decodeSessionSummary,
  initialCursor,
  isReadOnlyCapabilitySet,
  needsResnapshot,
  pendingDecision,
  resolveDecision,
  verbAllowedByCapabilities,
  type Capability,
} from "./index"
import {
  decisionRequestFixture,
  sessionEventStreamFixture,
  sessionListFixture,
} from "./fixtures"

describe("control schema", () => {
  test("decodes the session list + event stream fixtures", () => {
    for (const row of sessionListFixture) expect(decodeSessionSummary(row).sessionRef).toBe(row.sessionRef)
    for (const ev of sessionEventStreamFixture) expect(decodeSessionEvent(ev).sequence).toBeGreaterThan(0)
  })

  test("decodes a bounded spawn command and rejects an unknown command", () => {
    const spawn = decodeControlCommand({
      type: "session.spawn",
      adapter: "codex",
      objective: "do the thing",
      verify: ["bun", "test"],
    })
    expect(spawn.type).toBe("session.spawn")
    expect(() => decodeControlCommand({ type: "session.nope" })).toThrow()
  })
})

describe("cursor / dedup / resume", () => {
  test("accepts advancing events and rejects duplicate + out-of-order", () => {
    let cursor = initialCursor()
    const results = sessionEventStreamFixture.map((e) => {
      const r = acceptEvent(cursor, e)
      cursor = r.cursor
      return r
    })
    expect(results.every((r) => r.accepted)).toBe(true)
    expect(cursor.lastSequence).toBe(5)

    // replay the same last event → duplicate, cursor unchanged
    const dup = acceptEvent(cursor, sessionEventStreamFixture[4]!)
    expect(dup.accepted).toBe(false)
    expect(dup.reason).toBe("duplicate")

    // an older sequence → out_of_order
    const old = acceptEvent(cursor, { eventId: "evt.older", sequence: 2 })
    expect(old.accepted).toBe(false)
    expect(old.reason).toBe("out_of_order")
  })

  test("needsResnapshot is true for a fresh cursor and when retention passed it", () => {
    expect(needsResnapshot(initialCursor(), 1)).toBe(true)
    expect(needsResnapshot({ lastSequence: 5, lastEventId: "evt.0005" }, 3)).toBe(false)
    expect(needsResnapshot({ lastSequence: 5, lastEventId: "evt.0005" }, 9)).toBe(true)
  })
})

describe("decision exactly-once", () => {
  const now = 1_000
  const rec = pendingDecision({ ...decisionRequestFixture, expiresAtMs: now + 10_000 })

  test("first resolve accepted; same answer duplicate; different answer already_resolved", () => {
    const first = resolveDecision(rec, { requestId: rec.requestId, verb: "approve" }, now)
    expect(first.outcome).toBe("accepted")
    expect(first.record.state).toBe("resolved")

    const dup = resolveDecision(first.record, { requestId: rec.requestId, verb: "approve" }, now)
    expect(dup.outcome).toBe("duplicate")

    const other = resolveDecision(first.record, { requestId: rec.requestId, verb: "deny" }, now)
    expect(other.outcome).toBe("already_resolved")
  })

  test("expired and unknown-request and cancelled outcomes", () => {
    const expired = resolveDecision(rec, { requestId: rec.requestId, verb: "approve" }, rec.expiresAtMs + 1)
    expect(expired.outcome).toBe("expired")

    const unknown = resolveDecision(rec, { requestId: "other", verb: "approve" }, now)
    expect(unknown.outcome).toBe("unknown_request")

    const cancelled = applyExternalResolution(rec, { state: "cancelled" })
    expect(resolveDecision(cancelled, { requestId: rec.requestId, verb: "approve" }, now).outcome).toBe("cancelled")
  })

  test("external resolution disables a pending card", () => {
    const ext = applyExternalResolution(rec, { state: "resolved", verb: "deny" })
    expect(ext.state).toBe("resolved")
    expect(ext.resolvedVerb).toBe("deny")
  })
})

describe("capability gating", () => {
  test("read-only set cannot do effectful verbs", () => {
    const ro: Capability[] = ["observe_public", "read_artifact"]
    expect(isReadOnlyCapabilitySet(ro)).toBe(true)
    expect(verbAllowedByCapabilities("decision.resolve", ro)).toBe(false)
    expect(verbAllowedByCapabilities("turn.interrupt", ro)).toBe(false)
    expect(verbAllowedByCapabilities("session.list", ro)).toBe(true)
  })

  test("granted capabilities allow their verbs", () => {
    const full: Capability[] = ["observe_public", "answer_decision", "cancel", "send_instruction"]
    expect(isReadOnlyCapabilitySet(full)).toBe(false)
    expect(verbAllowedByCapabilities("decision.resolve", full)).toBe(true)
    expect(verbAllowedByCapabilities("turn.steer", full)).toBe(true)
    expect(verbAllowedByCapabilities("session.cancel", full)).toBe(true)
  })
})
