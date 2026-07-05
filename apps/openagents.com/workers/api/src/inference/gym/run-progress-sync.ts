import {
  type WorkerBindings,
  PUBLIC_GYM_RUN_PROGRESS_ID,
  makeD1SyncOutboxRepository,
  publicGymRunProgressScope,
} from '@openagentsinc/sync-worker'

import { defaultMakeKhalaSyncSqlClient } from '../../khala-sync-push-routes'
import { logWorkerRouteWarning, observedPromise } from '../../observability'
import { openAgentsDatabase, scheduleBackgroundWork } from '../../runtime'
import type { SyncNotificationContext } from '../../sync-notifier'
import { notifySyncScopes } from '../../sync-notifier'
import { projectGymRunProgress } from '../../khala-sync-gym-run-progress-projection'

import {
  type GymRunProgress,
  type GymRunProgressPublicProjection,
  checkGymRunProgressPublicSafety,
  projectPublicGymRunProgress,
} from './run-progress'

/**
 * LIVE GYM / HARBOR "FOLLOW AN ACTIVE TERMINAL-BENCH RUN" PANEL over the
 * OpenAgents sync engine (openagents #6261).
 *
 * The `/gym` follow-along panel used to be poll-based: the client fetched
 * `GET /api/public/gym/run-progress` every ~12s. This module pushes the moment a
 * snapshot is ingested. When the operator ingest (`POST /api/operator/gym/run-
 * progress`) upserts a run-progress snapshot, it publishes the SAME public-safe
 * projected object the GET serves (`projectPublicGymRunProgress`) onto a single
 * public, read-only sync room scope (`public-gym-run-progress:network`), keyed by
 * `runRef`. The `/gym` client subscribes to that scope and replaces that run's
 * card the instant the snapshot lands — no per-12s poll.
 *
 * Hard constraints, mirroring the legacy public sync publishers:
 *  - PUBLIC-SAFE ONLY: the published value is the already-redacted
 *    `openagents.gym.run_progress.v1` PROJECTION — counts / denominators /
 *    public-safe profile refs / freshness, or an "awaiting authorization" marker
 *    for `local_only` runs. NEVER a raw prompt, response, log, trajectory, key,
 *    private endpoint, or wallet/payment material. Every payload is scanned by
 *    the public projection guard before it can be broadcast.
 *  - FAIL-SOFT: a broadcast failure must NEVER break or slow the ingest. It is
 *    fired fire-and-forget from the ingest route and every error is swallowed.
 *  - REUSE: it goes through the same `makeD1SyncOutboxRepository` outbox +
 *    `SyncRoomDurableObject` poke that the settled feed and Khala tokens-served
 *    counter already use; no parallel realtime path and no new Durable Object.
 *
 * UPSERT-BY-runRef. The publish keys each put by `runRef`, so the outbox snapshot
 * collapses puts by entity id: a re-ingest of the same run replaces that run's
 * latest projection in the snapshot, and the client replaces just that card.
 * Other runs' cards are untouched, so the panel never shows stale or duplicate
 * runs.
 *
 * KHALA SYNC DUAL-WRITE (KS-6.5, #8415): every snapshot ALSO best-effort
 * projects into the new engine's `scope.public.gym-run-progress`
 * (`@openagentsinc/khala-sync-server`'s `projectGymRunProgressBestEffort`,
 * via this Worker's `khala-sync-gym-run-progress-projection.ts`), the same
 * fail-soft dual-write shape KS-6.1 (fleet cockpit) and KS-6.3
 * (tokens-served) already proved. This is a DUAL-WRITE ADDITION, not a
 * cutover: `GET/WS /api/sync/connect` (+ /log, /bootstrap) require an
 * authenticated actor even for `scope.public.*` reads, and the `/gym` panel
 * is read by ANONYMOUS/logged-out visitors — there is no anonymous read
 * path on the khala-sync connect surface yet. So the legacy outbox +
 * `notifySyncScopes` broadcast above remains the ONLY delivery path for
 * anonymous visitors; do NOT delete it or repoint
 * `apps/web/src/subscriptions.ts`'s `GYM_RUN_PROGRESS_SCOPE` until that gap
 * is closed (see docs/khala-sync/RUNBOOK.md). The Khala Sync write is
 * fire-and-forget alongside the legacy path and never blocks or fails the
 * ingest — same discipline as the poke below.
 */

export const GYM_RUN_PROGRESS_SYNC_COLLECTION = 'gym_run_progress'

type GymRunProgressSyncEnv = Pick<
  WorkerBindings,
  'OPENAGENTS_DB' | 'SYNC_ROOM' | 'KHALA_SYNC_DB'
>

/**
 * Publish ONE public-safe run-progress projection to the public gym run-progress
 * scope, then poke the room. The published value is exactly the
 * `projectPublicGymRunProgress` output the GET serves, keyed by `runRef`, so the
 * client applies it as an upsert-by-runRef.
 *
 * The payload is scanned for unsafe material before it can be written; a rejected
 * payload is skipped (it never reaches the outbox). The whole operation is
 * fail-soft via `observedPromise` so the caller (the ingest path) is never broken
 * or slowed by a broadcast failure.
 */
export const publishGymRunProgressSnapshot = async (
  env: GymRunProgressSyncEnv,
  progress: GymRunProgress,
  options: Readonly<{ ctx?: SyncNotificationContext; feedId?: string }> = {},
): Promise<void> => {
  await observedPromise('Sync.publishGymRunProgressSnapshot', async () => {
    // Belt-and-suspenders tripwire: the ingest already RE-ASSERTED public-safety
    // via `buildGymRunProgress` before this run was stored, but re-run the
    // gym-specific forbidden-marker scan here so a malformed run can never be
    // broadcast. (The generic `assertNexusPylonPublicSafe` scanner is for bare
    // payloads; it over-flags the run-progress projection's public-safe
    // serving-profile fields, so the dedicated guard is the right boundary here.)
    if (!checkGymRunProgressPublicSafety(progress).safe) {
      return
    }

    const projection: GymRunProgressPublicProjection =
      projectPublicGymRunProgress(progress)

    const scope = publicGymRunProgressScope(
      options.feedId ?? PUBLIC_GYM_RUN_PROGRESS_ID,
    )
    const db = openAgentsDatabase(env)
    const store = makeD1SyncOutboxRepository(db)

    await store.appendChange({
      actorId: 'system',
      collection: GYM_RUN_PROGRESS_SYNC_COLLECTION,
      id: projection.runRef,
      op: 'put',
      scope,
      value: projection,
    })

    const notify = notifySyncScopes(env, [scope])

    // KS-6.5 (#8415): best-effort dual-write into the new engine's
    // `scope.public.gym-run-progress` (see this module's docstring for why
    // the legacy outbox + notify above remains the ONLY delivery path for
    // anonymous `/gym` visitors). Never throws; always resolves to a typed
    // outcome, so it is always safe to race alongside the legacy notify.
    const khalaSyncProjection = projectGymRunProgress(
      {
        binding: env.KHALA_SYNC_DB,
        log: (event, fields) => logWorkerRouteWarning(event, fields),
        makeSqlClient: defaultMakeKhalaSyncSqlClient,
      },
      projection,
    )

    if (options.ctx === undefined) {
      await Promise.all([notify, khalaSyncProjection])
    } else {
      scheduleBackgroundWork(options.ctx, notify)
      scheduleBackgroundWork(
        options.ctx,
        khalaSyncProjection.then(() => undefined),
      )
    }
  })
}
