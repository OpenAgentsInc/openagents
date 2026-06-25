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
 */

export const KHALA_TOKENS_SERVED_SYNC_COLLECTION = 'tokens_served_deltas'

// The public-safe shape that lands on the sync room (and in the homepage Model).
// By construction it carries ONLY a bare integer delta + a timestamp — no per
// user/team/account/provider/model material.
export type PublicKhalaTokensServedDelta = Readonly<{
  // Stable per-event ref so a reconnect/cursor-replay applies each delta at most
  // once (the client de-dupes on this ref). Public-safe by construction.
  eventRef: string
  observedAt: string
  tokensServedDelta: number
}>

// Build the public-safe delta patch for one served completion. The delta is the
// served input + output tokens for that completion; the event ref reuses the
// recorder's stable per-request event id so a retried/replayed publish (same
// request) carries the SAME ref and the client de-dupes it.
export const buildKhalaTokensServedDelta = (
  input: Readonly<{
    eventRef: string
    observedAt: string
    tokensServedDelta: number
  }>,
): PublicKhalaTokensServedDelta => ({
  eventRef: input.eventRef,
  observedAt: input.observedAt,
  tokensServedDelta: Math.max(0, Math.trunc(input.tokensServedDelta)),
})

type KhalaTokensServedSyncEnv = Pick<WorkerBindings, 'OPENAGENTS_DB' | 'SYNC_ROOM'>

/**
 * Publish ONE public-safe tokens-served delta to the public tokens-served scope,
 * then poke the room. The payload is scanned for unsafe material before it can
 * be written; a rejected or zero/negative delta is skipped (it never reaches the
 * outbox). The whole operation is fail-soft via `observedPromise` so the caller
 * is never broken or slowed by a broadcast failure.
 */
export const publishKhalaTokensServedDelta = async (
  env: KhalaTokensServedSyncEnv,
  delta: PublicKhalaTokensServedDelta,
  options: Readonly<{ ctx?: SyncNotificationContext; feedId?: string }> = {},
): Promise<void> => {
  if (delta.tokensServedDelta <= 0) {
    return
  }

  await observedPromise('Sync.publishKhalaTokensServedDelta', async () => {
    try {
      assertNexusPylonPublicSafe('Public khala tokens served delta', delta)
    } catch {
      return
    }

    const scope = publicKhalaTokensServedScope(
      options.feedId ?? PUBLIC_KHALA_TOKENS_SERVED_ID,
    )
    const store = makeD1SyncOutboxRepository(openAgentsDatabase(env))

    await store.appendChange({
      actorId: 'system',
      collection: KHALA_TOKENS_SERVED_SYNC_COLLECTION,
      id: delta.eventRef,
      op: 'put',
      scope,
      value: delta,
    })

    const notify = notifySyncScopes(env, [scope])

    if (options.ctx === undefined) {
      await notify
    } else {
      scheduleBackgroundWork(options.ctx, notify)
    }
  })
}
