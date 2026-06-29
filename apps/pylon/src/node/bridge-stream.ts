// Bridge stream sequencing and replay core (CL-10 / issue #4912).
//
// Pure, transport-agnostic module — no I/O, no side-effects, deterministic.
// Wiring into the live control-server happens in CL-14; this file is the
// tested algorithmic heart.
//
// Key invariant: SequencedEvent is structurally identical to
// CursoredEvent & { tier } from the shared protocol package, so
// acceptEvent / needsResnapshot / initialCursor all compose with these
// types without adaptation (verified in bridge-stream.test.ts).

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/**
 * Delivery guarantee for a bridge stream event.
 *  - `lossless`    – must survive backpressure drops (e.g. tool results, deltas)
 *  - `best_effort` – may be shed when the queue is overloaded (e.g. heartbeats)
 */
export type DeliveryTier = "lossless" | "best_effort"

/**
 * A sequenced bridge event.  Structurally identical to
 * `CursoredEvent & { tier: DeliveryTier }` from the shared protocol package:
 * `eventId` is the dedup key, `sequence` is the resume cursor, and `tier`
 * controls backpressure behaviour.
 */
export type SequencedEvent = {
  eventId: string
  sequence: number
  tier: DeliveryTier
}

// ---------------------------------------------------------------------------
// EventSequencer
// ---------------------------------------------------------------------------

export type EventSequencer = {
  /**
   * Stamp the next event with a strictly-increasing sequence number.
   * Sequences start at 1 and never repeat within a sequencer instance.
   */
  next(input: { eventId: string; tier: DeliveryTier }): SequencedEvent
}

/**
 * Creates a fresh `EventSequencer`.  The first call to `next` returns
 * sequence 1; every subsequent call increments by exactly 1.
 */
export function createSequencer(): EventSequencer {
  let counter = 0
  return {
    next(input: { eventId: string; tier: DeliveryTier }): SequencedEvent {
      counter += 1
      return { eventId: input.eventId, sequence: counter, tier: input.tier }
    },
  }
}

// ---------------------------------------------------------------------------
// ReplayBuffer
// ---------------------------------------------------------------------------

export type ReplayBuffer = {
  /**
   * Append an event to the buffer.  When the buffer exceeds its capacity the
   * oldest event is evicted and `oldestRetainedSequence` advances.
   */
  append(event: SequencedEvent): void

  /**
   * Return events with `sequence > cursorSequence`.
   *
   * When `cursorSequence < oldestRetainedSequence` the cursor predates our
   * retention window — we have lost events the caller expected.  In that case
   * `lagged` is set to `true` and **all** currently-retained events are
   * returned so the caller can decide to request a fresh snapshot
   * (`needsResnapshot` from the shared protocol package will agree).
   */
  since(cursorSequence: number): { events: SequencedEvent[]; lagged: boolean }

  /**
   * The sequence number of the oldest event the buffer currently holds.
   * `0` means no eviction has happened yet (the buffer remembers everything
   * from the start).
   */
  readonly oldestRetainedSequence: number
}

/**
 * Creates a bounded `ReplayBuffer` that retains at most `capacity` events.
 */
export function createReplayBuffer(capacity: number): ReplayBuffer {
  const ring: SequencedEvent[] = []
  let _oldestRetainedSequence = 0

  return {
    get oldestRetainedSequence(): number {
      return _oldestRetainedSequence
    },

    append(event: SequencedEvent): void {
      ring.push(event)
      if (ring.length > capacity) {
        ring.shift()
        // The new head is the oldest retained event after eviction.
        _oldestRetainedSequence = ring[0]!.sequence
      }
    },

    since(cursorSequence: number): { events: SequencedEvent[]; lagged: boolean } {
      // Cursor predates our retention window — caller should resnapshot.
      // (needsResnapshot from the shared protocol will agree: when eviction
      // has advanced oldestRetainedSequence past the cursor, both signal lag.)
      if (cursorSequence < _oldestRetainedSequence) {
        return { events: [...ring], lagged: true }
      }
      return {
        events: ring.filter((e) => e.sequence > cursorSequence),
        lagged: false,
      }
    },
  }
}

// ---------------------------------------------------------------------------
// backpressureDrop
// ---------------------------------------------------------------------------

/**
 * Pure backpressure policy: when `pending` exceeds `maxQueue`, shed the
 * oldest `best_effort` events until the queue fits.  `lossless` events are
 * **never** dropped regardless of queue depth.
 *
 * @returns
 *   `kept`              – surviving events in original order
 *   `droppedBestEffort` – how many `best_effort` events were removed
 */
export function backpressureDrop(
  pending: SequencedEvent[],
  maxQueue: number,
): { kept: SequencedEvent[]; droppedBestEffort: number } {
  if (pending.length <= maxQueue) {
    return { kept: pending, droppedBestEffort: 0 }
  }

  const excess = pending.length - maxQueue
  const kept: SequencedEvent[] = []
  let dropped = 0

  for (const event of pending) {
    // Drop from the front (oldest) — but only best_effort events.
    if (dropped < excess && event.tier === "best_effort") {
      dropped++
    } else {
      kept.push(event)
    }
  }

  return { kept, droppedBestEffort: dropped }
}
