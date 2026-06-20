import { describe, expect, test } from "bun:test"

import {
  acceptEvent,
  initialCursor,
  needsResnapshot,
  pendingDecision,
  resolveDecision,
} from "./index.js"
import { decisionRequestFixture, sessionEventStreamFixture } from "./fixtures.js"

describe("cross-client conformance", () => {
  test("cursor resume accepts fixture events monotonically and resnapshots after retention gaps", () => {
    let cursor = initialCursor()
    const acceptedSequences: number[] = []

    for (const event of sessionEventStreamFixture) {
      const previousSequence = cursor.lastSequence
      const result = acceptEvent(cursor, event)

      expect(result.accepted).toBe(true)
      expect(result.reason).toBe("accepted")
      expect(result.cursor.lastSequence).toBeGreaterThan(previousSequence)

      cursor = result.cursor
      acceptedSequences.push(cursor.lastSequence)
    }

    expect(acceptedSequences).toEqual(sessionEventStreamFixture.map((event) => event.sequence))
    expect(needsResnapshot(cursor, cursor.lastSequence)).toBe(false)
    expect(needsResnapshot(cursor, cursor.lastSequence + 1)).toBe(true)
  })

  test("dedup ignores a repeated event id and sequence", () => {
    let cursor = initialCursor()
    const firstEvent = sessionEventStreamFixture[0]!
    const first = acceptEvent(cursor, firstEvent)

    expect(first.accepted).toBe(true)
    cursor = first.cursor

    const duplicate = acceptEvent(cursor, firstEvent)

    expect(duplicate.accepted).toBe(false)
    expect(duplicate.reason).toBe("duplicate")
    expect(duplicate.cursor).toEqual(cursor)
  })

  test("decision resolution is exactly once", () => {
    const nowMs = decisionRequestFixture.expiresAtMs - 1
    const pending = pendingDecision(decisionRequestFixture)

    const first = resolveDecision(
      pending,
      { requestId: pending.requestId, verb: "approve" },
      nowMs,
    )

    expect(first.outcome).toBe("accepted")
    expect(first.record.state).toBe("resolved")
    expect(first.record.resolvedVerb).toBe("approve")

    const second = resolveDecision(
      first.record,
      { requestId: pending.requestId, verb: "approve" },
      nowMs,
    )

    expect(second.outcome).toBe("duplicate")
    expect(second.record).toEqual(first.record)
  })
})
