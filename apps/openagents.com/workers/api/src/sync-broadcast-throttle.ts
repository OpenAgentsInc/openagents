// High-frequency public sync broadcast throttle (openagents #6324).
//
// The public "Khala Tokens Served" counter scope (`public-khala-tokens-served:*`)
// can receive one sync poke per served completion. During the GLM surge that was
// ~42 pokes/sec. Pushing every poke down the public WebSocket fanout would melt
// the client frame rate, so we cap broadcasts on this scope to ≤3/sec.
//
// ROOT CAUSE OF THE FREEZE-THEN-JUMP (#6324, this fix):
// The previous throttle coalesced bursts into a single TRAILING broadcast scheduled
// via a sub-second `state.storage.setAlarm(~334ms)`, and tracked its coalescing
// state (`lastBroadcastAtMs` + `pendingTrailingScopes`) in Durable Object INSTANCE
// MEMORY. With hibernatable WebSockets the DO hibernates between events, which
// (a) DROPS the in-memory pending-trailing state and (b) makes sub-second alarms
// fire unreliably. Under a sustained burst the trailing broadcasts were lost (the
// alarm woke with empty in-memory pending), so the counter FROZE during the burst
// and then JUMPED when the burst subsided and a fresh leading-edge broadcast finally
// fired.
//
// FIX: a hibernation-safe LEADING-EDGE rate limiter keyed on a DURABLE
// `lastBroadcastAt` persisted in `state.storage`. No reliance on DO in-memory state
// and no sub-second alarm. On each poke:
//   - if `now - lastBroadcastAt >= MIN_INTERVAL_MS` → broadcast now + persist
//     `lastBroadcastAt = now`,
//   - else skip (drop this intermediate poke).
//
// Dropping an intermediate poke loses NOTHING: the authoritative running total rides
// EVERY event and the summary row, so the next poke ~MIN_INTERVAL_MS later carries the
// latest total. Under a sustained burst pokes are frequent, so the leading edge fires
// reliably at ~3/sec and the counter NEVER freezes. The client's periodic reconcile is
// the safety net for the final post-burst delta. The monotonic / no-double-count
// invariant is preserved on the client (it takes `max(displayed, total)` and seeds from
// one snapshot); skipping a broadcast can never move the counter backward or
// double-count.

export const HIGH_FREQUENCY_BROADCAST_MIN_INTERVAL_MS = 334

export const HIGH_FREQUENCY_BROADCAST_SCOPE_PREFIX =
  'public-khala-tokens-served:'

// Durable storage key under which the leading-edge limiter persists the last
// broadcast wall-clock (ms) for a throttled scope. Keyed per scope so it stays
// correct even though, in practice, each scope maps to its own DO instance.
export const highFrequencyBroadcastLastAtStorageKey = (scope: string): string =>
  `sync_broadcast_throttle:last_at:${scope}`

export const isThrottledBroadcastScope = (scope: string): boolean =>
  scope.startsWith(HIGH_FREQUENCY_BROADCAST_SCOPE_PREFIX)

export interface HighFrequencyBroadcastDecisionInput {
  readonly scope: string
  readonly nowMs: number
  // Durable last-broadcast wall-clock (ms) read from `state.storage`, or
  // `undefined`/`null` when this scope has never broadcast (or after hibernation
  // wiped any in-memory cache — the durable read is the source of truth).
  readonly lastBroadcastAtMs: number | null | undefined
  readonly minIntervalMs?: number
}

export interface HighFrequencyBroadcastDecision {
  // Whether the caller should broadcast `scope` to its sockets now.
  readonly broadcast: boolean
  // When `broadcast` is true, the value the caller MUST persist back to durable
  // storage as the new `lastBroadcastAt` (always `nowMs`). When `broadcast` is
  // false, the durable value is unchanged and this is `undefined`.
  readonly persistLastBroadcastAtMs: number | undefined
}

// Pure leading-edge throttle decision. Hibernation-safe because it takes the
// durable `lastBroadcastAtMs` as an explicit input and tells the caller exactly
// what to persist — it keeps no hidden in-memory state and schedules no alarm.
export const decideHighFrequencyBroadcast = (
  input: HighFrequencyBroadcastDecisionInput,
): HighFrequencyBroadcastDecision => {
  // Non-throttled scopes always broadcast immediately and never persist throttle
  // state.
  if (!isThrottledBroadcastScope(input.scope)) {
    return { broadcast: true, persistLastBroadcastAtMs: undefined }
  }

  const minIntervalMs =
    input.minIntervalMs ?? HIGH_FREQUENCY_BROADCAST_MIN_INTERVAL_MS
  const lastAt =
    input.lastBroadcastAtMs === null || input.lastBroadcastAtMs === undefined
      ? Number.NEGATIVE_INFINITY
      : input.lastBroadcastAtMs

  // Leading edge: fire when we are at/after the next slot. This is `>=` so the
  // very first poke (lastAt = -Infinity) always fires, and a poke landing exactly
  // on the slot boundary fires rather than stalling.
  if (input.nowMs - lastAt >= minIntervalMs) {
    return { broadcast: true, persistLastBroadcastAtMs: input.nowMs }
  }

  // Inside the window: skip this intermediate poke. The next poke ~minIntervalMs
  // later carries the latest authoritative total, so nothing is lost.
  return { broadcast: false, persistLastBroadcastAtMs: undefined }
}
