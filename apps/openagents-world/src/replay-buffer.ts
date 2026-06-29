/**
 * Bounded delta-replay buffer for the Verse-world Region Durable Object
 * (durable-stream Rank-2 / EPIC #6056, issue #6059).
 *
 * The Region DO already persists the transport CLOCK (`current_seq` /
 * `min_replay_seq`) but NOT the delta PAYLOADS, so an in-window reconnect could
 * only send a heartbeat + a fresh snapshot — never gap-free replay. This module
 * adds an additive, bounded, offset-addressed buffer of recent delta payloads,
 * keyed on the existing `WorldSequence` (the integer behind every
 * `cursor.<region>.<seq>` token).
 *
 * It ports the offset-log IDEA from `@openagentsinc/durable-stream` (read at a
 * stored offset → the exact suffix; offsets monotonic + opaque) WITHOUT adopting
 * its HTTP wire format. The Durable Streams offset-resumption / streaming-
 * equivalence conformance cases are the test ORACLE for the replay semantics
 * here, not a mandate to reshape `world-contract`.
 *
 * This module is pure and Cloudflare-free so it is unit-testable under Bun. The
 * Region DO wires it to `ctx.storage.sql`.
 */
import type { WorldDelta } from "@openagentsinc/world-contract"

import { cursorForSequence, makeDiagnostic, sequenceFromCursor, type WorldTransportFrame } from "./protocol"
import type { WorldDiagnostic } from "@openagentsinc/world-contract"

/** Default count cap: matches the existing `sequence - 256` replay window. */
export const DEFAULT_REPLAY_BUFFER_MAX_DELTAS = 256

/**
 * Default byte cap (~1 MiB of serialized delta payloads per region). Bounds DO
 * SQLite storage independently of the count cap so a burst of large deltas can
 * never grow the buffer without limit.
 */
export const DEFAULT_REPLAY_BUFFER_MAX_BYTES = 1024 * 1024

export type ReplayBufferConfig = Readonly<{
  enabled: boolean
  maxDeltas: number
  maxBytes: number
}>

export const DEFAULT_REPLAY_BUFFER_CONFIG: ReplayBufferConfig = {
  enabled: true,
  maxDeltas: DEFAULT_REPLAY_BUFFER_MAX_DELTAS,
  maxBytes: DEFAULT_REPLAY_BUFFER_MAX_BYTES,
}

/**
 * A single buffered delta payload. `sequence` is the `WorldSequence` offset; it
 * is the integer behind the delta's `cursor.<region>.<seq>` token and is
 * monotonic + unique per region (the DO's single-threaded command loop advances
 * it by one per accepted command).
 */
export type DeltaReplayBufferRow = Readonly<{
  regionRef: string
  sequence: number
  byteLen: number
  /** Serialized `WorldDelta` JSON (the exact payload the live broadcast sent). */
  deltaJson: string
  generatedAt: string
}>

export type ReplayBufferEntryInput = Readonly<{
  regionRef: string
  sequence: number
  delta: WorldDelta
  generatedAt: string
}>

/** SQLite migration for the per-region bounded delta-replay buffer. */
export const regionDeltaReplayBufferMigrationStatements = [
  `CREATE TABLE IF NOT EXISTS region_delta_replay_buffer (
    region_ref TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    byte_len INTEGER NOT NULL,
    delta_json TEXT NOT NULL,
    generated_at TEXT NOT NULL,
    PRIMARY KEY (region_ref, sequence)
  )`,
  `CREATE INDEX IF NOT EXISTS idx_region_delta_replay_buffer_region_seq
    ON region_delta_replay_buffer(region_ref, sequence)`,
] as const

const clampPositiveInt = (value: number, fallback: number, max: number): number => {
  if (!Number.isFinite(value)) {
    return fallback
  }
  const floored = Math.floor(value)
  if (floored < 1) {
    return fallback
  }
  return Math.min(floored, max)
}

/**
 * Build the replay-buffer config from Cloudflare string env. Fail-safe: any
 * value that disables the feature (`"0"`/`"false"`/`"off"`) degrades the DO to
 * the existing heartbeat + fresh-snapshot reconnect behavior.
 */
export const replayBufferConfigFromEnv = (env: {
  readonly OPENAGENTS_WORLD_DELTA_REPLAY?: string
  readonly OPENAGENTS_WORLD_DELTA_REPLAY_MAX_DELTAS?: string
  readonly OPENAGENTS_WORLD_DELTA_REPLAY_MAX_BYTES?: string
}): ReplayBufferConfig => {
  const flag = (env.OPENAGENTS_WORLD_DELTA_REPLAY ?? "").trim().toLowerCase()
  const enabled = flag === "" ? true : !["0", "false", "off", "no", "disabled"].includes(flag)
  return {
    enabled,
    maxDeltas: clampPositiveInt(
      Number(env.OPENAGENTS_WORLD_DELTA_REPLAY_MAX_DELTAS),
      DEFAULT_REPLAY_BUFFER_MAX_DELTAS,
      4096,
    ),
    maxBytes: clampPositiveInt(
      Number(env.OPENAGENTS_WORLD_DELTA_REPLAY_MAX_BYTES),
      DEFAULT_REPLAY_BUFFER_MAX_BYTES,
      32 * 1024 * 1024,
    ),
  }
}

/** Serialize a delta into a buffer row, measuring its byte length once. */
export const makeReplayBufferRow = (input: ReplayBufferEntryInput): DeltaReplayBufferRow => {
  const deltaJson = JSON.stringify(input.delta)
  return {
    regionRef: input.regionRef,
    sequence: Math.max(0, Math.floor(input.sequence)),
    byteLen: new TextEncoder().encode(deltaJson).length,
    deltaJson,
    generatedAt: input.generatedAt,
  }
}

export type ReplayBufferEvictionPlan = Readonly<{
  /** Sequences to delete (oldest first) so the buffer respects both caps. */
  evictedSequences: ReadonlyArray<number>
  /** The minimum retained sequence after eviction (== the live `min_replay_seq`). */
  minRetainedSequence: number
  retainedCount: number
  retainedBytes: number
}>

/**
 * Given the EXISTING buffered rows (ascending by sequence) plus the row being
 * appended, decide which oldest rows to evict so the buffer stays within BOTH
 * the count cap and the byte cap. Eviction is always from the oldest end, which
 * keeps the retained set a contiguous suffix — exactly what offset-addressed
 * replay needs (`min_replay_seq .. current_seq`).
 *
 * `existing` MUST be ascending and MUST NOT already contain `incoming.sequence`.
 */
export const replayBufferEvictionPlan = (
  existing: ReadonlyArray<DeltaReplayBufferRow>,
  incoming: DeltaReplayBufferRow,
  config: ReplayBufferConfig,
): ReplayBufferEvictionPlan => {
  const ordered = [...existing, incoming].sort((a, b) => a.sequence - b.sequence)
  const maxDeltas = Math.max(1, config.maxDeltas)
  const maxBytes = Math.max(1, config.maxBytes)

  // Drop from the oldest end until both caps are satisfied. The newest row is
  // never evicted (so a single oversized delta still replays as the tail).
  let startIndex = 0
  let totalBytes = ordered.reduce((sum, row) => sum + row.byteLen, 0)
  while (
    startIndex < ordered.length - 1 &&
    (ordered.length - startIndex > maxDeltas || totalBytes > maxBytes)
  ) {
    totalBytes -= ordered[startIndex]!.byteLen
    startIndex += 1
  }

  const evictedSequences = ordered.slice(0, startIndex).map(row => row.sequence)
  const retained = ordered.slice(startIndex)
  return {
    evictedSequences,
    minRetainedSequence: retained[0]?.sequence ?? incoming.sequence,
    retainedCount: retained.length,
    retainedBytes: retained.reduce((sum, row) => sum + row.byteLen, 0),
  }
}

export type GapReplayFallbackReason =
  | "no_cursor"
  | "evicted"
  | "buffer_empty"
  | "missing_payload"
  | "future_cursor"

export type GapReplayDecision =
  | Readonly<{
      kind: "at_tail"
      /** Cursor is already at the live tail; nothing to replay. */
      fromSequence: number
    }>
  | Readonly<{
      kind: "within_window"
      /** Exact suffix of sequences to replay (`cursorSeq + 1 .. currentSeq`). */
      replaySequences: ReadonlyArray<number>
    }>
  | Readonly<{
      kind: "out_of_window"
      reason: GapReplayFallbackReason
    }>

/**
 * Decide how to serve a reconnect from `cursorSeq`, using the SEQUENCES actually
 * present in the buffer (`bufferedSequences`, ascending). This is the oracle-
 * faithful core: a read from a stored offset returns the EXACT suffix after it,
 * provided that suffix is still buffered; otherwise we fall back.
 *
 *  - cursor at the tail (`cursorSeq === currentSeq`) → `at_tail` (heartbeat-only)
 *  - cursor strictly inside the buffered window AND every intervening sequence is
 *    present → `within_window` with the exact suffix (true gap replay)
 *  - cursor older than the earliest buffered sequence (evicted), buffer empty,
 *    or a gap in the buffered payloads → `out_of_window` (snapshot fallback)
 */
export const planGapReplay = (input: {
  readonly cursorSeq: number | null
  readonly currentSeq: number
  readonly bufferedSequences: ReadonlyArray<number>
}): GapReplayDecision => {
  const { cursorSeq, currentSeq } = input
  if (cursorSeq === null) {
    return { kind: "out_of_window", reason: "no_cursor" }
  }
  if (cursorSeq > currentSeq) {
    return { kind: "out_of_window", reason: "future_cursor" }
  }
  if (cursorSeq === currentSeq) {
    return { kind: "at_tail", fromSequence: cursorSeq }
  }

  const buffered = [...input.bufferedSequences].sort((a, b) => a - b)
  if (buffered.length === 0) {
    return { kind: "out_of_window", reason: "buffer_empty" }
  }

  const earliest = buffered[0]!
  // The client needs sequences (cursorSeq, currentSeq]. The earliest buffered
  // payload must cover the first sequence the client is missing.
  if (earliest > cursorSeq + 1) {
    return { kind: "out_of_window", reason: "evicted" }
  }

  const present = new Set(buffered)
  const replaySequences: Array<number> = []
  for (let seq = cursorSeq + 1; seq <= currentSeq; seq += 1) {
    if (!present.has(seq)) {
      // A hole inside the window (e.g. partial eviction) — fail safe to snapshot
      // rather than apply a non-contiguous suffix.
      return { kind: "out_of_window", reason: "missing_payload" }
    }
    replaySequences.push(seq)
  }

  return { kind: "within_window", replaySequences }
}

/** Decode a buffered row's JSON back into a live `delta` transport frame. */
export const replayFrameFromBufferRow = (row: DeltaReplayBufferRow): WorldTransportFrame => ({
  frameKind: "delta",
  delta: JSON.parse(row.deltaJson) as WorldDelta,
})

export type GapReplayPlan =
  | Readonly<{
      kind: "gap_replay"
      cursor: string
      frames: ReadonlyArray<WorldTransportFrame>
      replayedSequences: ReadonlyArray<number>
    }>
  | Readonly<{
      kind: "at_tail"
      cursor: string
      frames: ReadonlyArray<WorldTransportFrame>
    }>
  | Readonly<{
      kind: "snapshot_fallback"
      cursor: string
      reason: GapReplayFallbackReason
      diagnostic?: WorldDiagnostic
    }>

/**
 * Build the concrete reconnect plan for a cursor against the buffered rows.
 * Pure: the DO supplies `bufferedRows` (ascending) read from SQLite and the
 * heartbeat-frame factory; this returns either the exact gap-replay suffix, a
 * tail no-op, or a fallback signal the DO turns into the existing snapshot path.
 */
export const planReplayBufferReconnect = (input: {
  readonly regionRef: string
  readonly reconnectCursor: string | null
  readonly currentSeq: number
  readonly bufferedRows: ReadonlyArray<DeltaReplayBufferRow>
  readonly generatedAt: string
  readonly makeHeartbeatFrame: (cursor: string, generatedAt: string) => WorldTransportFrame
}): GapReplayPlan => {
  const currentCursor = cursorForSequence(input.regionRef, input.currentSeq)
  const cursorSeq =
    input.reconnectCursor === null || input.reconnectCursor.length === 0
      ? null
      : sequenceFromCursor(input.reconnectCursor, input.regionRef)

  const byteSeq = new Map(input.bufferedRows.map(row => [row.sequence, row]))
  const decision = planGapReplay({
    cursorSeq,
    currentSeq: input.currentSeq,
    bufferedSequences: input.bufferedRows.map(row => row.sequence),
  })

  if (decision.kind === "at_tail") {
    return {
      kind: "at_tail",
      cursor: currentCursor,
      frames: [input.makeHeartbeatFrame(currentCursor, input.generatedAt)],
    }
  }

  if (decision.kind === "within_window") {
    const frames: Array<WorldTransportFrame> = []
    for (const seq of decision.replaySequences) {
      const row = byteSeq.get(seq)
      if (row === undefined) {
        // Defensive: planGapReplay already guaranteed presence, but never apply
        // a non-contiguous suffix — fall back to a snapshot instead.
        return {
          kind: "snapshot_fallback",
          cursor: currentCursor,
          reason: "missing_payload",
          diagnostic: makeStaleReplayDiagnostic(input.reconnectCursor, input.generatedAt),
        }
      }
      frames.push(replayFrameFromBufferRow(row))
    }
    return {
      kind: "gap_replay",
      cursor: currentCursor,
      frames,
      replayedSequences: decision.replaySequences,
    }
  }

  // out_of_window: the DO maps this to the existing fresh-snapshot path.
  return {
    kind: "snapshot_fallback",
    cursor: currentCursor,
    reason: decision.reason,
    ...(decision.reason === "no_cursor"
      ? {}
      : { diagnostic: makeStaleReplayDiagnostic(input.reconnectCursor, input.generatedAt) }),
  }
}

const makeStaleReplayDiagnostic = (
  reconnectCursor: string | null,
  generatedAt: string,
): WorldDiagnostic =>
  makeDiagnostic({
    tag: "cursor",
    severity: "warn",
    message:
      "World reconnect cursor is older than the buffered delta-replay window; sending a fresh snapshot.",
    observedAt: generatedAt,
    sourceRefs: reconnectCursor === null ? [] : [reconnectCursor],
  })
