// CL-33 cross-client conformance matrix. One fixture-driven set of assertions
// over the shared protocol primitives that EVERY client (web, desktop, mobile)
// runs in its own test suite, proving identical protocol behavior regardless of
// runtime (DOM / Bun / Hermes): cursor resume, dedup, exactly-once ordering,
// resnapshot, read-only gating, and projection levels. Pure — no IO, no
// framework — so each client just imports runConformanceMatrix() and asserts
// every case is ok.

import { isReadOnlyCapabilitySet, type Capability, type PairingCredentialClaims } from "./bridge.js"
import { acceptEvent, initialCursor, needsResnapshot } from "./cursor.js"
import { pendingDecision, resolveDecision } from "./decision.js"
import { sessionEventStreamFixture } from "./fixtures.js"
import { projectionLevelOf } from "./pairing-client.js"

export type ConformanceResult = { name: string; ok: boolean; detail?: string }

export const CONFORMANCE_CASE_NAMES = [
  "cursor-resume-advances",
  "dedup-duplicate-id",
  "exactly-once-out-of-order",
  "resnapshot-fresh",
  "resnapshot-lagged",
  "read-only-gating",
  "projection-levels",
  "decision-exactly-once",
  "decision-already-resolved",
  "decision-expired",
] as const

function claimsAt(level: PairingCredentialClaims["projectionLevel"]): PairingCredentialClaims {
  return {
    pairingRef: "pairing.fixture",
    clientId: "client.fixture",
    deviceClass: "test",
    issuer: "issuer.fixture",
    audience: "audience.fixture",
    expiresAt: "2030-01-01T00:00:00.000Z",
    jti: "jti.fixture",
    projectionLevel: level,
    capabilities: ["observe_public"] as Capability[],
  }
}

export function runConformanceMatrix(): ConformanceResult[] {
  const results: ConformanceResult[] = []
  const check = (name: string, fn: () => boolean): void => {
    try {
      results.push({ name, ok: fn() === true })
    } catch (error) {
      results.push({ name, ok: false, detail: error instanceof Error ? error.message : String(error) })
    }
  }

  // Cursor resume: replaying the fixture stream advances monotonically and
  // every event is accepted exactly once.
  check("cursor-resume-advances", () => {
    let cursor = initialCursor()
    for (const event of sessionEventStreamFixture) {
      const result = acceptEvent(cursor, { eventId: event.eventId, sequence: event.sequence })
      if (!result.accepted) return false
      cursor = result.cursor
    }
    const last = sessionEventStreamFixture[sessionEventStreamFixture.length - 1]
    if (!last) return false
    return cursor.lastSequence === last.sequence
  })

  // Dedup: replaying the same eventId is rejected without moving the cursor.
  check("dedup-duplicate-id", () => {
    const first = sessionEventStreamFixture[0]
    if (!first) return false
    const cursor = acceptEvent(initialCursor(), { eventId: first.eventId, sequence: first.sequence }).cursor
    const dup = acceptEvent(cursor, { eventId: first.eventId, sequence: first.sequence })
    return dup.accepted === false && dup.reason === "duplicate" && dup.cursor.lastSequence === first.sequence
  })

  // Exactly-once: a non-advancing (replayed/out-of-order) sequence is rejected.
  check("exactly-once-out-of-order", () => {
    const cursor = acceptEvent(initialCursor(), { eventId: "evt.a", sequence: 5 }).cursor
    const stale = acceptEvent(cursor, { eventId: "evt.b", sequence: 3 })
    return stale.accepted === false && stale.reason === "out_of_order"
  })

  // Resnapshot: a fresh cursor always snapshots.
  check("resnapshot-fresh", () => needsResnapshot(initialCursor(), 1) === true)

  // Resnapshot: a cursor behind retention snapshots; one ahead resumes.
  check(
    "resnapshot-lagged",
    () =>
      needsResnapshot({ lastSequence: 3, lastEventId: "x" }, 10) === true &&
      needsResnapshot({ lastSequence: 10, lastEventId: "x" }, 5) === false,
  )

  // Read-only gating: an observe-only capability set is read-only; any mutating
  // capability makes it not read-only.
  check(
    "read-only-gating",
    () =>
      isReadOnlyCapabilitySet(["observe_public"] as Capability[]) === true &&
      isReadOnlyCapabilitySet(["observe_public", "cancel"] as Capability[]) === false,
  )

  // Projection levels: projectionLevelOf is identity over the credential's level.
  check(
    "projection-levels",
    () =>
      projectionLevelOf(claimsAt("public_safe")) === "public_safe" &&
      projectionLevelOf(claimsAt("team")) === "team" &&
      projectionLevelOf(claimsAt("private")) === "private",
  )

  // Decision exactly-once (CL-29): a pending decision resolves once; an
  // identical repeat is a duplicate (no re-resolution).
  check("decision-exactly-once", () => {
    const pending = pendingDecision({ requestId: "req.1", actionRef: "action.1", expiresAtMs: 10_000 })
    const first = resolveDecision(pending, { requestId: "req.1", verb: "approve" }, 1_000)
    if (first.outcome !== "accepted" || first.record.state !== "resolved") return false
    const repeat = resolveDecision(first.record, { requestId: "req.1", verb: "approve" }, 2_000)
    return repeat.outcome === "duplicate" && repeat.record.resolvedVerb === "approve"
  })

  // A different verb after resolution is rejected as already-resolved.
  check("decision-already-resolved", () => {
    const pending = pendingDecision({ requestId: "req.2", actionRef: "action.2", expiresAtMs: 10_000 })
    const resolved = resolveDecision(pending, { requestId: "req.2", verb: "approve" }, 1_000).record
    const conflicting = resolveDecision(resolved, { requestId: "req.2", verb: "deny" }, 2_000)
    return conflicting.outcome === "already_resolved" && conflicting.record.resolvedVerb === "approve"
  })

  // An expired decision cannot be resolved.
  check("decision-expired", () => {
    const pending = pendingDecision({ requestId: "req.3", actionRef: "action.3", expiresAtMs: 1_000 })
    const late = resolveDecision(pending, { requestId: "req.3", verb: "approve" }, 5_000)
    return late.outcome === "expired" && late.record.state === "expired"
  })

  return results
}
