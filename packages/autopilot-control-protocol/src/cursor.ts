// Cursor / sequence / dedup logic for resumable event streams. Pure and
// transport-agnostic, so web (DOM), desktop (Bun), and mobile (RN) all resume
// and deduplicate identically. `sequence` is the resume cursor; `eventId` is
// the duplicate-detection key.

export type StreamCursor = {
  lastSequence: number
  lastEventId: string | null
}

export type CursoredEvent = {
  eventId: string
  sequence: number
}

export const initialCursor = (): StreamCursor => ({ lastSequence: 0, lastEventId: null })

export type AcceptResult = {
  cursor: StreamCursor
  accepted: boolean
  reason: "accepted" | "duplicate" | "out_of_order"
}

// Accept an event iff its sequence advances the cursor. Duplicates (same id or
// non-advancing sequence) are rejected without moving the cursor.
export function acceptEvent(cursor: StreamCursor, event: CursoredEvent): AcceptResult {
  if (event.eventId === cursor.lastEventId) {
    return { cursor, accepted: false, reason: "duplicate" }
  }
  if (event.sequence <= cursor.lastSequence) {
    return { cursor, accepted: false, reason: "out_of_order" }
  }
  return {
    cursor: { lastSequence: event.sequence, lastEventId: event.eventId },
    accepted: true,
    reason: "accepted",
  }
}

// On reconnect, decide whether the server can replay from our cursor or whether
// the cursor is older than retention and we must take a fresh snapshot
// (surfaced to the user as a `stream.lagged` caveat).
export function needsResnapshot(cursor: StreamCursor, oldestRetainedSequence: number): boolean {
  // Fresh client (no progress yet) always takes a snapshot.
  if (cursor.lastSequence === 0) return true
  // If retention has advanced past our cursor, we missed events: resnapshot.
  return cursor.lastSequence < oldestRetainedSequence
}
