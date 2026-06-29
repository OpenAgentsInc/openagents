// Tests for the bridge stream sequencing/replay core (CL-10 / #4912).
// Pure unit tests — no I/O, no network, fully deterministic.

import { describe, expect, test } from "bun:test"
import {
  initialCursor,
  needsResnapshot,
  type StreamCursor,
} from "@openagentsinc/autopilot-control-protocol"
import {
  backpressureDrop,
  createReplayBuffer,
  createSequencer,
  type SequencedEvent,
} from "../src/node/bridge-stream"

// ---------------------------------------------------------------------------
// EventSequencer
// ---------------------------------------------------------------------------

describe("EventSequencer", () => {
  test("assigns strictly-increasing sequence numbers starting at 1", () => {
    const seq = createSequencer()

    const e1 = seq.next({ eventId: "a", tier: "lossless" })
    const e2 = seq.next({ eventId: "b", tier: "best_effort" })
    const e3 = seq.next({ eventId: "c", tier: "lossless" })

    expect(e1.sequence).toBe(1)
    expect(e2.sequence).toBe(2)
    expect(e3.sequence).toBe(3)
  })

  test("preserves eventId and tier on the output event", () => {
    const seq = createSequencer()

    const ev = seq.next({ eventId: "evt-xyz", tier: "best_effort" })

    expect(ev.eventId).toBe("evt-xyz")
    expect(ev.tier).toBe("best_effort")
    expect(ev.sequence).toBe(1)
  })

  test("each sequencer instance has its own independent counter", () => {
    const seqA = createSequencer()
    const seqB = createSequencer()

    seqA.next({ eventId: "a1", tier: "lossless" })
    seqA.next({ eventId: "a2", tier: "lossless" })

    const b1 = seqB.next({ eventId: "b1", tier: "lossless" })

    // seqB starts fresh at 1 regardless of seqA's state
    expect(b1.sequence).toBe(1)
  })
})

// ---------------------------------------------------------------------------
// ReplayBuffer — basic windowing
// ---------------------------------------------------------------------------

describe("ReplayBuffer — since()", () => {
  test("returns only events with sequence > cursorSequence", () => {
    const buf = createReplayBuffer(10)
    const seq = createSequencer()

    const e1 = seq.next({ eventId: "e1", tier: "lossless" })
    const e2 = seq.next({ eventId: "e2", tier: "lossless" })
    const e3 = seq.next({ eventId: "e3", tier: "lossless" })
    buf.append(e1)
    buf.append(e2)
    buf.append(e3)

    const { events, lagged } = buf.since(1)

    expect(lagged).toBe(false)
    expect(events.map((e) => e.sequence)).toEqual([2, 3])
  })

  test("since(0) returns all buffered events when no eviction has occurred", () => {
    const buf = createReplayBuffer(5)
    const seq = createSequencer()

    buf.append(seq.next({ eventId: "e1", tier: "lossless" }))
    buf.append(seq.next({ eventId: "e2", tier: "best_effort" }))

    const { events, lagged } = buf.since(0)

    expect(lagged).toBe(false)
    expect(events.length).toBe(2)
  })

  test("returns empty slice when cursor is already at the latest sequence", () => {
    const buf = createReplayBuffer(5)
    const seq = createSequencer()

    buf.append(seq.next({ eventId: "e1", tier: "lossless" }))
    buf.append(seq.next({ eventId: "e2", tier: "lossless" }))

    const { events, lagged } = buf.since(2)

    expect(lagged).toBe(false)
    expect(events).toHaveLength(0)
  })
})

// ---------------------------------------------------------------------------
// ReplayBuffer — eviction / lag
// ---------------------------------------------------------------------------

describe("ReplayBuffer — eviction", () => {
  test("eviction past capacity advances oldestRetainedSequence and sets lagged", () => {
    const buf = createReplayBuffer(3)
    const seq = createSequencer()

    // Fill to capacity — no eviction yet.
    buf.append(seq.next({ eventId: "e1", tier: "lossless" })) // seq 1
    buf.append(seq.next({ eventId: "e2", tier: "lossless" })) // seq 2
    buf.append(seq.next({ eventId: "e3", tier: "lossless" })) // seq 3
    expect(buf.oldestRetainedSequence).toBe(0) // nothing evicted yet

    // One more event causes the first eviction — seq 1 is dropped.
    buf.append(seq.next({ eventId: "e4", tier: "lossless" })) // seq 4
    expect(buf.oldestRetainedSequence).toBe(2)

    // A cursor at seq 1 is now behind the retention window → lagged.
    const { events: laggedEvents, lagged: lagged1 } = buf.since(1)
    expect(lagged1).toBe(true)
    // Returns ALL retained events when lagged.
    expect(laggedEvents.map((e) => e.sequence)).toEqual([2, 3, 4])

    // A cursor at seq 2 (the oldest retained) is NOT lagged.
    const { lagged: lagged2 } = buf.since(2)
    expect(lagged2).toBe(false)

    // Second eviction — seq 2 is dropped.
    buf.append(seq.next({ eventId: "e5", tier: "lossless" })) // seq 5
    expect(buf.oldestRetainedSequence).toBe(3)
  })

  test("since() returns a defensive copy of retained events when lagged", () => {
    const buf = createReplayBuffer(2)
    const seq = createSequencer()

    buf.append(seq.next({ eventId: "e1", tier: "lossless" })) // seq 1
    buf.append(seq.next({ eventId: "e2", tier: "lossless" })) // seq 2
    buf.append(seq.next({ eventId: "e3", tier: "lossless" })) // evicts seq 1; oldest = 2

    const { events } = buf.since(0) // cursor 0 < oldest 2 → lagged
    // Mutating the returned slice must not corrupt the buffer.
    events.splice(0, events.length)

    const { events: again } = buf.since(0)
    expect(again.length).toBe(2)
  })
})

// ---------------------------------------------------------------------------
// needsResnapshot ↔ lagged agreement
// ---------------------------------------------------------------------------

describe("needsResnapshot agrees with the buffer's lagged signal", () => {
  test("lagged flag and needsResnapshot agree after eviction for cursor positions", () => {
    const buf = createReplayBuffer(3)
    const seq = createSequencer()

    // Fill past capacity: seq 1–4, evicts seq 1 → oldest = 2.
    for (let i = 1; i <= 4; i++) {
      buf.append(seq.next({ eventId: `e${i}`, tier: "lossless" }))
    }
    expect(buf.oldestRetainedSequence).toBe(2)

    // Case A: cursor behind retention window.
    const behindCursor: StreamCursor = { lastSequence: 1, lastEventId: "e1" }
    const { lagged: laggedA } = buf.since(behindCursor.lastSequence)
    const resnapshotA = needsResnapshot(behindCursor, buf.oldestRetainedSequence)
    expect(laggedA).toBe(true)
    expect(resnapshotA).toBe(true)
    expect(laggedA).toBe(resnapshotA) // they agree

    // Case B: cursor AT the oldest retained — within window.
    const atOldestCursor: StreamCursor = { lastSequence: 2, lastEventId: "e2" }
    const { lagged: laggedB } = buf.since(atOldestCursor.lastSequence)
    const resnapshotB = needsResnapshot(atOldestCursor, buf.oldestRetainedSequence)
    expect(laggedB).toBe(false)
    expect(resnapshotB).toBe(false)
    expect(laggedB).toBe(resnapshotB) // they agree

    // Case C: cursor well inside the window.
    const currentCursor: StreamCursor = { lastSequence: 3, lastEventId: "e3" }
    const { lagged: laggedC } = buf.since(currentCursor.lastSequence)
    const resnapshotC = needsResnapshot(currentCursor, buf.oldestRetainedSequence)
    expect(laggedC).toBe(false)
    expect(resnapshotC).toBe(false)
    expect(laggedC).toBe(resnapshotC) // they agree
  })

  test("a fresh initialCursor() always needs a resnapshot (protocol rule)", () => {
    // needsResnapshot's fresh-client rule (lastSequence === 0) is independent
    // of the buffer and always returns true.
    const cursor = initialCursor()
    expect(needsResnapshot(cursor, 0)).toBe(true)
    expect(needsResnapshot(cursor, 5)).toBe(true)
  })
})

// ---------------------------------------------------------------------------
// backpressureDrop
// ---------------------------------------------------------------------------

describe("backpressureDrop", () => {
  // Helper: build a SequencedEvent without needing a sequencer instance.
  function ev(seq: number, tier: "lossless" | "best_effort"): SequencedEvent {
    return { eventId: `e${seq}`, sequence: seq, tier }
  }

  test("returns pending unchanged when at or under maxQueue", () => {
    const pending = [ev(1, "lossless"), ev(2, "best_effort"), ev(3, "lossless")]
    const { kept, droppedBestEffort } = backpressureDrop(pending, 3)
    expect(kept).toEqual(pending) // same reference is fine; length check covers it
    expect(droppedBestEffort).toBe(0)
  })

  test("drops oldest best_effort events first when over maxQueue", () => {
    // [lossless, be, be, lossless, be]  maxQueue=3 → drop 2
    const pending = [
      ev(1, "lossless"),
      ev(2, "best_effort"),
      ev(3, "best_effort"),
      ev(4, "lossless"),
      ev(5, "best_effort"),
    ]
    const { kept, droppedBestEffort } = backpressureDrop(pending, 3)

    expect(droppedBestEffort).toBe(2)
    expect(kept.length).toBe(3)
    // Oldest best_effort (seq 2 and 3) are dropped; newest (seq 5) survives.
    expect(kept.map((e) => e.sequence)).toEqual([1, 4, 5])
  })

  test("preserves ALL lossless events even when queue is far over budget", () => {
    // All lossless + 1 best_effort — maxQueue=1 means we need to drop 3, but
    // only 1 best_effort is available.
    const pending = [
      ev(1, "lossless"),
      ev(2, "lossless"),
      ev(3, "best_effort"),
      ev(4, "lossless"),
    ]
    const { kept, droppedBestEffort } = backpressureDrop(pending, 1)

    expect(droppedBestEffort).toBe(1) // dropped the only best_effort
    expect(kept.length).toBe(3)
    expect(kept.every((e) => e.tier === "lossless")).toBe(true)
  })

  test("drops nothing when all events are lossless (never drops lossless)", () => {
    const pending = [ev(1, "lossless"), ev(2, "lossless"), ev(3, "lossless")]
    const { kept, droppedBestEffort } = backpressureDrop(pending, 1)

    // Cannot shed lossless events — all survive even though queue is 3× budget.
    expect(droppedBestEffort).toBe(0)
    expect(kept.length).toBe(3)
  })

  test("drops all best_effort events when budget is zero and only they are pending", () => {
    const pending = [ev(1, "best_effort"), ev(2, "best_effort"), ev(3, "best_effort")]
    const { kept, droppedBestEffort } = backpressureDrop(pending, 0)

    expect(droppedBestEffort).toBe(3)
    expect(kept).toHaveLength(0)
  })

  test("preserves original ordering of surviving events", () => {
    // Interleaved: be, lossless, be, lossless, be  — maxQueue=3 → drop 2
    const pending = [
      ev(1, "best_effort"),
      ev(2, "lossless"),
      ev(3, "best_effort"),
      ev(4, "lossless"),
      ev(5, "best_effort"),
    ]
    const { kept } = backpressureDrop(pending, 3)

    // Dropped: seq 1 and seq 3 (oldest be); surviving order: 2, 4, 5
    expect(kept.map((e) => e.sequence)).toEqual([2, 4, 5])
  })
})
