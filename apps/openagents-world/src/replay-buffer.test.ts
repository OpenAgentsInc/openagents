import { describe, expect, test } from "bun:test"

import {
  WORLD_DELTA_SCHEMA_VERSION,
  decodeWorldDelta,
  type WorldDelta,
} from "@openagentsinc/world-contract"

import { cursorForSequence, makeHeartbeatFrame } from "./protocol"
import {
  type DeltaReplayBufferRow,
  type ReplayBufferConfig,
  makeReplayBufferRow,
  planGapReplay,
  planReplayBufferReconnect,
  replayBufferConfigFromEnv,
  replayBufferEvictionPlan,
} from "./replay-buffer"

const REGION = "region.run.1"
const AT = "2026-06-22T00:00:00.000Z"

/** A real `update` delta carrying one avatar row at the given sequence. */
const deltaAtSequence = (sequence: number): WorldDelta =>
  decodeWorldDelta({
    schemaVersion: WORLD_DELTA_SCHEMA_VERSION,
    deltaRef: `delta.world.command.seq.${sequence}`,
    kind: "update",
    regionRef: REGION,
    cursor: cursorForSequence(REGION, sequence),
    generatedAt: AT,
    rows: [
      {
        kind: "agent_avatar",
        avatarRef: `avatar.seq.${sequence}`,
        characterId: "char.alpha",
        regionRef: REGION,
        label: `Avatar ${sequence}`,
        avatarKind: "human",
        updatedAt: AT,
        safety: {
          publicProjectionAllowed: true,
          sourceRefs: ["source.public.test"],
          blockerRefs: [],
          caveatRefs: [],
        },
      },
    ],
  })

const rowAtSequence = (sequence: number): DeltaReplayBufferRow =>
  makeReplayBufferRow({ regionRef: REGION, sequence, delta: deltaAtSequence(sequence), generatedAt: AT })

/** Build a contiguous buffer covering `[from, to]` ascending. */
const bufferRange = (from: number, to: number): Array<DeltaReplayBufferRow> => {
  const rows: Array<DeltaReplayBufferRow> = []
  for (let seq = from; seq <= to; seq += 1) rows.push(rowAtSequence(seq))
  return rows
}

describe("replay-buffer config", () => {
  test("defaults to enabled with the 256-delta / 1 MiB caps", () => {
    const config = replayBufferConfigFromEnv({})
    expect(config.enabled).toBe(true)
    expect(config.maxDeltas).toBe(256)
    expect(config.maxBytes).toBe(1024 * 1024)
  })

  test("flag is fail-safe: off/false/0/disabled all disable replay", () => {
    for (const flag of ["off", "false", "0", "disabled", "no"]) {
      expect(replayBufferConfigFromEnv({ OPENAGENTS_WORLD_DELTA_REPLAY: flag }).enabled).toBe(false)
    }
    expect(replayBufferConfigFromEnv({ OPENAGENTS_WORLD_DELTA_REPLAY: "on" }).enabled).toBe(true)
  })

  test("clamps cap overrides into a bounded range", () => {
    const config = replayBufferConfigFromEnv({
      OPENAGENTS_WORLD_DELTA_REPLAY_MAX_DELTAS: "10",
      OPENAGENTS_WORLD_DELTA_REPLAY_MAX_BYTES: "2048",
    })
    expect(config.maxDeltas).toBe(10)
    expect(config.maxBytes).toBe(2048)
    // Garbage/negative falls back to the default, never unbounded.
    expect(replayBufferConfigFromEnv({ OPENAGENTS_WORLD_DELTA_REPLAY_MAX_DELTAS: "-5" }).maxDeltas).toBe(256)
  })
})

describe("planGapReplay (offset-resumption oracle)", () => {
  // Oracle: a read from a stored offset returns the EXACT suffix after it.
  test("within-window cursor yields the exact delta suffix (cursorSeq, currentSeq]", () => {
    const decision = planGapReplay({
      cursorSeq: 2,
      currentSeq: 5,
      bufferedSequences: [1, 2, 3, 4, 5],
    })
    expect(decision.kind).toBe("within_window")
    if (decision.kind !== "within_window") throw new Error("expected within_window")
    expect(decision.replaySequences).toEqual([3, 4, 5])
  })

  test("cursor at the tail replays nothing (heartbeat-only)", () => {
    const decision = planGapReplay({ cursorSeq: 5, currentSeq: 5, bufferedSequences: [3, 4, 5] })
    expect(decision.kind).toBe("at_tail")
  })

  test("cursor older than the earliest buffered sequence falls back (evicted)", () => {
    const decision = planGapReplay({ cursorSeq: 1, currentSeq: 9, bufferedSequences: [5, 6, 7, 8, 9] })
    expect(decision.kind).toBe("out_of_window")
    if (decision.kind !== "out_of_window") throw new Error("expected out_of_window")
    expect(decision.reason).toBe("evicted")
  })

  test("empty buffer / null cursor / future cursor all fall back", () => {
    expect(planGapReplay({ cursorSeq: 2, currentSeq: 5, bufferedSequences: [] }).kind).toBe("out_of_window")
    expect(planGapReplay({ cursorSeq: null, currentSeq: 5, bufferedSequences: [3, 4, 5] }).kind).toBe(
      "out_of_window",
    )
    expect(planGapReplay({ cursorSeq: 9, currentSeq: 5, bufferedSequences: [3, 4, 5] }).kind).toBe(
      "out_of_window",
    )
  })

  test("a hole inside the window fails safe rather than replaying a non-contiguous suffix", () => {
    const decision = planGapReplay({ cursorSeq: 2, currentSeq: 6, bufferedSequences: [3, 5, 6] })
    expect(decision.kind).toBe("out_of_window")
    if (decision.kind !== "out_of_window") throw new Error("expected out_of_window")
    expect(decision.reason).toBe("missing_payload")
  })

  test("earliest == cursorSeq + 1 is exactly within window (boundary)", () => {
    const decision = planGapReplay({ cursorSeq: 2, currentSeq: 5, bufferedSequences: [3, 4, 5] })
    expect(decision.kind).toBe("within_window")
  })
})

describe("replayBufferEvictionPlan (bounded growth)", () => {
  test("count cap evicts the oldest rows so the retained set is a contiguous suffix", () => {
    const config: ReplayBufferConfig = { enabled: true, maxDeltas: 3, maxBytes: 10 * 1024 * 1024 }
    const existing = bufferRange(1, 3)
    const plan = replayBufferEvictionPlan(existing, rowAtSequence(4), config)
    expect(plan.retainedCount).toBe(3)
    expect(plan.evictedSequences).toEqual([1])
    expect(plan.minRetainedSequence).toBe(2)
  })

  test("byte cap evicts oldest rows independently of the count cap", () => {
    const sample = rowAtSequence(1).byteLen
    // Cap admits exactly two rows by bytes; count cap is generous.
    const config: ReplayBufferConfig = { enabled: true, maxDeltas: 1000, maxBytes: sample * 2 + 1 }
    const existing = bufferRange(1, 4)
    const plan = replayBufferEvictionPlan(existing, rowAtSequence(5), config)
    expect(plan.retainedCount).toBe(2)
    expect(plan.retainedBytes).toBeLessThanOrEqual(config.maxBytes)
    expect(plan.evictedSequences).toEqual([1, 2, 3])
    expect(plan.minRetainedSequence).toBe(4)
  })

  test("the newest row is never evicted even when it alone exceeds the byte cap", () => {
    const config: ReplayBufferConfig = { enabled: true, maxDeltas: 1000, maxBytes: 1 }
    const plan = replayBufferEvictionPlan(bufferRange(1, 2), rowAtSequence(3), config)
    expect(plan.retainedCount).toBe(1)
    expect(plan.minRetainedSequence).toBe(3)
  })

  test("repeated appends with the count cap keep the buffer at exactly the cap", () => {
    const config: ReplayBufferConfig = { enabled: true, maxDeltas: 4, maxBytes: 10 * 1024 * 1024 }
    let buffer: Array<DeltaReplayBufferRow> = []
    let lastMin = 0
    for (let seq = 1; seq <= 20; seq += 1) {
      const plan = replayBufferEvictionPlan(buffer, rowAtSequence(seq), config)
      const evicted = new Set(plan.evictedSequences)
      buffer = [...buffer, rowAtSequence(seq)].filter(row => !evicted.has(row.sequence))
      lastMin = plan.minRetainedSequence
      expect(buffer.length).toBeLessThanOrEqual(config.maxDeltas)
    }
    expect(buffer.length).toBe(4)
    expect(buffer.map(r => r.sequence)).toEqual([17, 18, 19, 20])
    expect(lastMin).toBe(17)
  })
})

describe("planReplayBufferReconnect (DO-facing plan)", () => {
  const make = (reconnectCursor: string | null, currentSeq: number, bufferedRows: Array<DeltaReplayBufferRow>) =>
    planReplayBufferReconnect({
      regionRef: REGION,
      reconnectCursor,
      currentSeq,
      bufferedRows,
      generatedAt: AT,
      makeHeartbeatFrame: (cursor, generatedAt) => makeHeartbeatFrame(REGION, cursor, generatedAt),
    })

  test("within-window reconnect produces TRUE gap replay (delta frames, NOT a snapshot)", () => {
    const plan = make(cursorForSequence(REGION, 2), 5, bufferRange(1, 5))
    expect(plan.kind).toBe("gap_replay")
    if (plan.kind !== "gap_replay") throw new Error("expected gap_replay")
    // Exact suffix: sequences 3, 4, 5 — three delta frames, no snapshot frame.
    expect(plan.replayedSequences).toEqual([3, 4, 5])
    expect(plan.frames).toHaveLength(3)
    expect(plan.frames.every(frame => frame.frameKind === "delta")).toBe(true)
    expect(plan.frames.some(frame => frame.frameKind === "snapshot")).toBe(false)
    // The replayed payloads are exactly the buffered deltas, in order.
    expect(plan.frames.map(frame => String(frame.delta.cursor))).toEqual([
      cursorForSequence(REGION, 3),
      cursorForSequence(REGION, 4),
      cursorForSequence(REGION, 5),
    ])
    // Resume cursor is the live tail.
    expect(plan.cursor).toBe(cursorForSequence(REGION, 5))
  })

  test("tail reconnect is a heartbeat-only resume (no replay)", () => {
    const plan = make(cursorForSequence(REGION, 5), 5, bufferRange(1, 5))
    expect(plan.kind).toBe("at_tail")
    if (plan.kind !== "at_tail") throw new Error("expected at_tail")
    expect(plan.frames[0]?.delta.kind).toBe("heartbeat")
  })

  test("out-of-window reconnect signals snapshot fallback with a stale-cursor diagnostic", () => {
    const plan = make(cursorForSequence(REGION, 1), 9, bufferRange(5, 9))
    expect(plan.kind).toBe("snapshot_fallback")
    if (plan.kind !== "snapshot_fallback") throw new Error("expected snapshot_fallback")
    expect(plan.reason).toBe("evicted")
    expect(plan.diagnostic?.tag).toBe("cursor")
  })
})
