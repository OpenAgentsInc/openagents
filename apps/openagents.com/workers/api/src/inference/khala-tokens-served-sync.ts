import {
  type WorkerBindings,
  PUBLIC_KHALA_TOKENS_SERVED_ID,
  makeD1SyncOutboxRepository,
  publicKhalaTokensServedScope,
} from '@openagentsinc/sync-worker'

import { assertNexusPylonPublicSafe } from '../nexus-pylon-visibility'
import { observedPromise } from '../observability'
import { openAgentsDatabase, scheduleBackgroundWork } from '../runtime'
import type { SyncNotificationContext } from '../sync-notifier'
import { notifySyncScopes } from '../sync-notifier'

/**
 * LIVE "KHALA TOKENS SERVED" COUNTER over the OpenAgents sync engine
 * (openagents #6231).
 *
 * The "Khala Tokens Served" homepage/stats counter used to be poll-based: the
 * client polled `GET /api/public/khala-tokens-served` every 1s and the server
 * cached a full-table D1 `SUM(input+output)` for 1s. Worst-case UI lag was ~2s
 * and the SUM ran ~1×/sec regardless of how many viewers watched.
 *
 * This module pushes a delta the moment a completion records. When the
 * served-tokens recorder writes ONE canonical `token_usage_events` row for a
 * served completion (and ONLY when that insert actually happened — never on a
 * duplicate/no-op or a failed completion), it publishes ONE public-safe
 * `{ tokensServedDelta, observedAt }` patch onto a single public, read-only sync
 * room scope (`public-khala-tokens-served:network`). The homepage subscribes to
 * that scope and rolls its odometer up instantly — no per-second polling/SUM.
 *
 * Hard constraints, mirroring `tassadar-settled-feed-sync.ts`:
 *  - PUBLIC-SAFE ONLY: a bare integer `tokensServedDelta` + `observedAt`. NEVER
 *    a per-user/team/account ref, provider, model id, prompt, completion, token
 *    content, key, or wallet/payment material. Every payload is scanned by the
 *    public projection guard before it can be broadcast.
 *  - ADDITIVE + FAIL-SOFT: a broadcast failure must NEVER break or slow the
 *    customer's already-delivered completion. It is fired fire-and-forget from
 *    the recorder path and every error is swallowed.
 *  - REUSE: it goes through the same `makeD1SyncOutboxRepository` outbox +
 *    `SyncRoomDurableObject` poke that the settled feed and team-sync already
 *    use; no parallel realtime path and no new Durable Object.
 *
 * SINGLE SOURCE OF TRUTH / MONOTONICITY (openagents #6231 follow-up).
 * The earlier shape pushed a bare delta and the client seeded from the scalar
 * `SUM` endpoint while subscribing from cursor 0. That double-counted: the
 * cursor-0 replay re-delivered every delta ALREADY inside the seed `SUM`, the
 * client added them on top (over-count, ~2M), then the periodic scalar reconcile
 * clobbered the value back down to the true `SUM` (~1.59M) — a visible backward
 * jump, oscillating. The fix mirrors `tassadar-settled-feed-sync.ts`: every
 * published event AND a single running-total summary record carry the
 * AUTHORITATIVE running total (`tokensServedTotal` = the live ledger `SUM` after
 * this row). The client seeds the running total + cursor from ONE snapshot read
 * and applies only events after that cursor, taking `max(displayed, total)`, so
 * the counter is monotonic by construction and converges exactly to the ledger.
 */

export const KHALA_TOKENS_SERVED_SYNC_COLLECTION = 'tokens_served_deltas'
export const KHALA_TOKENS_SERVED_SUMMARY_COLLECTION = 'tokens_served_summary'
export const KHALA_TOKENS_SERVED_SUMMARY_ENTITY_ID = 'summary'

// The public-safe shape that lands on the sync room (and in the homepage Model).
// By construction it carries ONLY bare integers + a timestamp — no per
// user/team/account/provider/model material. `tokensServedTotal` is the
// authoritative running ledger total AFTER this event, so the client advances
// monotonically (`max`) and converges to the true `SUM` with no double-count.
export type PublicKhalaTokensServedDelta = Readonly<{
  // Stable per-event ref so a reconnect/cursor-replay applies each delta at most
  // once (the client de-dupes on this ref). Public-safe by construction.
  eventRef: string
  observedAt: string
  tokensServedDelta: number
  tokensServedTotal: number
}>

// The single running-total record kept current on the scope. Because the outbox
// snapshot collapses puts by entity id, the snapshot's summary is always the
// latest authoritative running total + the snapshot cursor — the one-shot seed
// the client reads to start, with no double-count.
export type PublicKhalaTokensServedSummary = Readonly<{
  observedAt: string
  tokensServedTotal: number
}>

// Build the public-safe delta patch for one served completion. The delta is the
// served input + output tokens for that completion; the event ref reuses the
// recorder's stable per-request event id so a retried/replayed publish (same
// request) carries the SAME ref and the client de-dupes it. `tokensServedTotal`
// is filled in by `publishKhalaTokensServedDelta` from the authoritative ledger
// SUM; callers pass only the per-request delta + refs.
export const buildKhalaTokensServedDelta = (
  input: Readonly<{
    eventRef: string
    observedAt: string
    tokensServedDelta: number
  }>,
): Omit<PublicKhalaTokensServedDelta, 'tokensServedTotal'> => ({
  eventRef: input.eventRef,
  observedAt: input.observedAt,
  tokensServedDelta: Math.max(0, Math.trunc(input.tokensServedDelta)),
})

type KhalaTokensServedSyncEnv = Pick<WorkerBindings, 'OPENAGENTS_DB' | 'SYNC_ROOM'>

// Read the AUTHORITATIVE running total: the live `SUM(input+output)` over the
// canonical ledger — the SAME aggregate the scalar `GET /api/public/
// khala-tokens-served` endpoint serves. Read here (after the row this publish
// announces is already committed) so the summary/event totals equal the ledger.
// A plain D1 read keeps this Promise-based module out of any Effect bridge; a
// failure returns -1 so the caller skips publishing a non-authoritative total
// rather than risking a non-monotonic client.
const readAuthoritativeTokensServedTotal = async (
  db: D1Database,
): Promise<number> => {
  try {
    const row = await db
      .prepare(
        `SELECT
            COALESCE(SUM(input_tokens), 0) + COALESCE(SUM(output_tokens), 0)
              AS tokens_served
           FROM token_usage_events`,
      )
      .first<{ tokens_served: number | null }>()

    return Math.max(0, Math.trunc(row?.tokens_served ?? 0))
  } catch {
    return -1
  }
}

// How many recent per-completion `tokens_served_deltas` rows to keep on the scope
// (openagents #6324). The on-load snapshot reads EVERY row in the scope and the
// stream replays from the snapshot cursor, so an untrimmed per-completion delta
// scope grows without bound — at the GLM surge it reached thousands of rows and
// the replay became a ~42-frame/s firehose that the client could not apply. The
// AUTHORITATIVE running total lives on the single collapsed `summary` row, so the
// historical per-completion deltas are NOT needed to seed or to stay correct:
// trimming them is safe (the snapshot summary still carries the exact ledger
// total). We keep a small recent tail purely so a just-reconnected client that
// seeded a slightly-stale cursor still receives the last few authoritative-total
// events instead of a cursor gap. The summary row is NEVER trimmed.
export const KHALA_TOKENS_SERVED_DELTA_TAIL = 32

// Trim the per-completion delta scope to its most-recent tail so the on-load
// snapshot + cursor-replay stay bounded regardless of lifetime token volume
// (openagents #6324). Deletes only `tokens_served_deltas` rows whose seq is below
// the (max delta seq - tail) watermark; the collapsed `summary` row (a different
// collection, carrying the authoritative running total) is never touched, so the
// snapshot seed and monotonic convergence are preserved. Fail-soft: a trim error
// is swallowed (an oversized scope is a perf issue, never a correctness one — the
// summary stays authoritative), so it can never break or slow a published delta.
const trimKhalaTokensServedDeltaScope = async (
  db: D1Database,
  scope: string,
): Promise<void> => {
  try {
    // 1) Compact the per-completion delta collection to its recent tail.
    const deltaWatermark = await db
      .prepare(
        `SELECT MAX(seq) AS max_seq
           FROM sync_changes
          WHERE scope = ? AND collection = ?`,
      )
      .bind(scope, KHALA_TOKENS_SERVED_SYNC_COLLECTION)
      .first<{ max_seq: number | null }>()

    const maxDeltaSeq = Math.max(0, Math.trunc(deltaWatermark?.max_seq ?? 0))
    const keepDeltaFromSeq = maxDeltaSeq - KHALA_TOKENS_SERVED_DELTA_TAIL

    if (keepDeltaFromSeq > 0) {
      await db
        .prepare(
          `DELETE FROM sync_changes
            WHERE scope = ? AND collection = ? AND seq < ?`,
        )
        .bind(scope, KHALA_TOKENS_SERVED_SYNC_COLLECTION, keepDeltaFromSeq)
        .run()
    }

    // 2) Compact the `summary` collection to ONLY its latest row. Every publish
    // appends a fresh `summary` put (the outbox never upserts); the snapshot
    // collapses puts by entity id so only the highest-seq summary is ever read.
    // Deleting the superseded summary rows keeps the scope bounded without
    // changing what the client seeds (the latest authoritative total survives).
    const summaryWatermark = await db
      .prepare(
        `SELECT MAX(seq) AS max_seq
           FROM sync_changes
          WHERE scope = ? AND collection = ?`,
      )
      .bind(scope, KHALA_TOKENS_SERVED_SUMMARY_COLLECTION)
      .first<{ max_seq: number | null }>()

    const maxSummarySeq = Math.max(0, Math.trunc(summaryWatermark?.max_seq ?? 0))

    if (maxSummarySeq > 0) {
      await db
        .prepare(
          `DELETE FROM sync_changes
            WHERE scope = ? AND collection = ? AND seq < ?`,
        )
        .bind(scope, KHALA_TOKENS_SERVED_SUMMARY_COLLECTION, maxSummarySeq)
        .run()
    }
  } catch {
    // Swallow: trimming is a bound on replay size, never a correctness gate. The
    // latest authoritative `summary` total is always preserved and keeps the
    // counter honest even if a trim pass fails.
  }
}

/**
 * Publish ONE public-safe tokens-served event to the public tokens-served scope,
 * then poke the room. The event AND a single running-total summary record carry
 * the AUTHORITATIVE running ledger total (the live `SUM(input+output)` AFTER this
 * row, read here — the recorder calls this AFTER the canonical row is committed,
 * so the SUM already includes it). Because the snapshot collapses puts by entity
 * id, the snapshot's summary is always the latest authoritative total + cursor:
 * the client seeds from it and applies only events after that cursor, taking
 * `max(displayed, total)`, so the counter never double-counts and never moves
 * backward.
 *
 * The payload is scanned for unsafe material before it can be written; a rejected
 * or zero/negative delta is skipped (it never reaches the outbox). The whole
 * operation is fail-soft via `observedPromise` so the caller is never broken or
 * slowed by a broadcast failure. Reading the SUM here is bounded by completion
 * throughput (one per served row), runs off the customer's response path, and
 * replaces the per-second client poll + per-second SUM the original #6231 work
 * removed — it does not reintroduce per-viewer or per-second load.
 */
export const publishKhalaTokensServedDelta = async (
  env: KhalaTokensServedSyncEnv,
  delta: Omit<PublicKhalaTokensServedDelta, 'tokensServedTotal'>,
  options: Readonly<{ ctx?: SyncNotificationContext; feedId?: string }> = {},
): Promise<void> => {
  if (delta.tokensServedDelta <= 0) {
    return
  }

  await observedPromise('Sync.publishKhalaTokensServedDelta', async () => {
    const db = openAgentsDatabase(env)

    const tokensServedTotal = await readAuthoritativeTokensServedTotal(db)

    // If the authoritative SUM read fails we cannot publish a trustworthy total
    // (a wrong total would risk a non-monotonic client). Skip — the client's
    // socket-down fallback seed keeps the counter honest, and the next served
    // completion republishes a fresh authoritative total.
    if (tokensServedTotal < 0) {
      return
    }

    const event: PublicKhalaTokensServedDelta = {
      eventRef: delta.eventRef,
      observedAt: delta.observedAt,
      tokensServedDelta: delta.tokensServedDelta,
      tokensServedTotal,
    }
    const summary: PublicKhalaTokensServedSummary = {
      observedAt: delta.observedAt,
      tokensServedTotal,
    }

    try {
      assertNexusPylonPublicSafe('Public khala tokens served delta', event)
      assertNexusPylonPublicSafe('Public khala tokens served summary', summary)
    } catch {
      return
    }

    const scope = publicKhalaTokensServedScope(
      options.feedId ?? PUBLIC_KHALA_TOKENS_SERVED_ID,
    )
    const store = makeD1SyncOutboxRepository(db)

    await store.appendChange({
      actorId: 'system',
      collection: KHALA_TOKENS_SERVED_SYNC_COLLECTION,
      id: event.eventRef,
      op: 'put',
      scope,
      value: event,
    })
    await store.appendChange({
      actorId: 'system',
      collection: KHALA_TOKENS_SERVED_SUMMARY_COLLECTION,
      id: KHALA_TOKENS_SERVED_SUMMARY_ENTITY_ID,
      op: 'put',
      scope,
      value: summary,
    })

    // Keep the on-load snapshot + cursor-replay bounded regardless of lifetime
    // token volume (openagents #6324): compact the per-completion delta tail. The
    // authoritative running total stays on the collapsed `summary` row, so this
    // never affects what the client seeds or converges to.
    await trimKhalaTokensServedDeltaScope(db, scope)

    const notify = notifySyncScopes(env, [scope])

    if (options.ctx === undefined) {
      await notify
    } else {
      scheduleBackgroundWork(options.ctx, notify)
    }
  })
}
