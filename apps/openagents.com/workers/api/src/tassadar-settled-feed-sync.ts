import {
  type WorkerBindings,
  PUBLIC_SETTLED_FEED_ID,
  makeD1SyncOutboxRepository,
  publicSettledFeedScope,
} from '@openagentsinc/sync-worker'

import {
  projectSettledFeedBatchBestEffort,
  type SettledFeedProjectionLog,
} from './khala-sync-public-settled-feed'
import type { KhalaSyncHyperdriveBinding } from './khala-sync-push-routes'
import { assertNexusPylonPublicSafe } from './nexus-pylon-visibility'
import { observedPromise } from './observability'
import { openAgentsDatabase, scheduleBackgroundWork } from './runtime'
import type { SyncNotificationContext } from './sync-notifier'
import { notifySyncScopes } from './sync-notifier'

/**
 * LIVE SETTLED FEED over the OpenAgents sync engine (openagents #5311).
 *
 * As real Bitcoin settlements stream in (the #5309/#5310 auto-stream path), this
 * publishes ONE public-safe "settled" event per settled leg onto a single
 * public, read-only sync room scope (`public-settled-feed:tassadar`). The
 * homepage (and any public surface) subscribes to that scope and renders the
 * settled total / latest settlement / feed count live — no reload.
 *
 * Hard constraints, mirroring the auto-settlement module it hooks:
 *  - PUBLIC-SAFE ONLY: amountSats, a public contributor digest ref
 *    (`pylon.<…>`), run/window/challenge refs, settledAt, and the running
 *    settled total/count. NEVER a raw `spark1…` address, invoice, preimage, or
 *    wallet material. Every payload is scanned by the public projection guard
 *    AND a settled-feed-specific guard before it can be broadcast.
 *  - ADDITIVE + FAIL-SOFT: a broadcast failure must NEVER break or slow the
 *    settlement dispatch. This is fired fire-and-forget from the verdict path
 *    and every error is swallowed.
 *  - REUSE: it goes through the same `makeD1SyncOutboxRepository` outbox +
 *    `SyncRoomDurableObject` poke that team-sync and goal-sync already use; no
 *    parallel realtime path.
 */

export const SETTLED_FEED_SYNC_COLLECTION = 'settled_events'
export const SETTLED_FEED_SUMMARY_COLLECTION = 'settled_summary'
export const SETTLED_FEED_SUMMARY_ENTITY_ID = 'summary'

export type SettledFeedActor = 'worker' | 'validator'

// The public-safe shape that lands in the homepage Model. By construction it
// carries ONLY refs and integer amounts — no raw payment material.
export type PublicSettledFeedEvent = Readonly<{
  amountSats: number
  challengeRef: string
  // Public contributor digest ref (e.g. `pylon.worker.orrery`). Public-safe by
  // construction; never a raw spark/lightning destination.
  contributorRef: string
  eventRef: string
  party: SettledFeedActor
  runRef: string
  settledAt: string
  // Running cumulative totals AFTER this event, for live counters.
  totalSettledCount: number
  totalSettledSats: number
  windowRef: string | null
}>

export type PublicSettledFeedSummary = Readonly<{
  latestEventRef: string | null
  latestSettledAt: string | null
  totalSettledCount: number
  totalSettledSats: number
  updatedAt: string
}>

export type SettledFeedLegInput = Readonly<{
  amountSats: number
  challengeRef: string
  contributorRef: string
  party: SettledFeedActor
  runRef: string
  windowRef: string | null
}>

const stableRefSuffix = (value: string): string =>
  value.replace(/[^A-Za-z0-9_.:/-]/g, '_').slice(0, 180)

// Defensive, settled-feed-specific raw-payment-material guard layered ON TOP of
// the shared public projection scanner. The shared scanner already rejects
// lightning invoices, preimages, mnemonics, secrets, and bearer tokens; this
// adds explicit on-chain / Spark destination shapes that must never appear in a
// broadcast even though the payload never contains them by construction.
const rawPaymentMaterialPattern =
  /(spark1[a-z0-9]+|bc1[a-z0-9]+|lnbc[0-9]|lntb[0-9]|lnbcrt[0-9]|lno1[a-z0-9]+|preimage|[0-9a-f]{64})/i

const scanForRawPaymentMaterial = (
  value: unknown,
  path: ReadonlyArray<string> = [],
): string | undefined => {
  if (typeof value === 'string') {
    return rawPaymentMaterialPattern.test(value) ? path.join('.') : undefined
  }

  if (Array.isArray(value)) {
    return value
      .map((item, index) =>
        scanForRawPaymentMaterial(item, [...path, String(index)]),
      )
      .find((found): found is string => found !== undefined)
  }

  if (value === null || typeof value !== 'object') {
    return undefined
  }

  return Object.entries(value)
    .map(([key, item]) => scanForRawPaymentMaterial(item, [...path, key]))
    .find((found): found is string => found !== undefined)
}

export class SettledFeedPayloadUnsafe extends Error {
  readonly _tag = 'SettledFeedPayloadUnsafe'
}

/**
 * Throw if a settled-feed payload carries any unsafe public material. Runs the
 * shared public projection scanner first, then the settled-feed raw-payment
 * guard. Tests assert that any `spark1…`, on-chain address, invoice, preimage,
 * or 64-hex secret in a payload is rejected here before broadcast.
 */
export const assertSettledFeedPayloadPublicSafe = (
  label: string,
  payload: unknown,
): void => {
  assertNexusPylonPublicSafe(label, payload)

  const unsafePath = scanForRawPaymentMaterial(payload)

  if (unsafePath !== undefined) {
    throw new SettledFeedPayloadUnsafe(
      `${label} contains raw payment material at ${unsafePath || '(root)'}.`,
    )
  }
}

/**
 * Build the ordered public-safe settled-feed events for a freshly-settled pair,
 * threading the running cumulative totals. `priorSettledSats` / `priorCount`
 * are the totals BEFORE these legs (e.g. today's already-settled real total),
 * so the homepage counters advance monotonically as legs stream in.
 */
export const buildSettledFeedEvents = (
  input: Readonly<{
    legs: ReadonlyArray<SettledFeedLegInput>
    priorCount: number
    priorSettledSats: number
    settledAt: string
  }>,
): ReadonlyArray<PublicSettledFeedEvent> => {
  const seeded: ReadonlyArray<PublicSettledFeedEvent> = []

  return input.legs.reduce<ReadonlyArray<PublicSettledFeedEvent>>(
    (events, leg, index) => {
      const totalSettledSats =
        input.priorSettledSats +
        events.reduce((sum, event) => sum + event.amountSats, 0) +
        leg.amountSats
      const totalSettledCount = input.priorCount + events.length + 1
      const eventRef = `settled.${stableRefSuffix(
        `${leg.challengeRef}.${leg.party}`,
      )}.${index}`

      return [
        ...events,
        {
          amountSats: leg.amountSats,
          challengeRef: leg.challengeRef,
          contributorRef: leg.contributorRef,
          eventRef,
          party: leg.party,
          runRef: leg.runRef,
          settledAt: input.settledAt,
          totalSettledCount,
          totalSettledSats,
          windowRef: leg.windowRef,
        },
      ]
    },
    seeded,
  )
}

export const settledFeedSummaryFromEvents = (
  events: ReadonlyArray<PublicSettledFeedEvent>,
): PublicSettledFeedSummary => {
  const latest = events.at(-1)

  return {
    latestEventRef: latest?.eventRef ?? null,
    latestSettledAt: latest?.settledAt ?? null,
    totalSettledCount: latest?.totalSettledCount ?? 0,
    totalSettledSats: latest?.totalSettledSats ?? 0,
    // The feed's clock is the latest settlement's own `settledAt` (set by the
    // publish hook from the worker's typed Clock). No raw Date primitive here.
    updatedAt: latest?.settledAt ?? '',
  }
}

type SettledFeedSyncEnv = Pick<WorkerBindings, 'OPENAGENTS_DB' | 'SYNC_ROOM'> &
  Readonly<{
    /**
     * `env.KHALA_SYNC_DB` — absent until the binding is deployed. KS-6.4
     * (#8414) dual-write: best-effort projects the SAME public-safe events
     * this function already writes to the legacy sync room into
     * `scope.public.settled-feed` via khala-sync. Never required; a missing
     * binding simply skips the new-path projection.
     */
    KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding
    /** Diagnostic sink for the khala-sync projection (public-safe only). */
    khalaSyncSettledFeedLog?: SettledFeedProjectionLog
  }>

/**
 * Publish a batch of public-safe settled events to the public settled-feed
 * scope, then poke the room. Each event is scanned for unsafe material before
 * it can be written; a rejected payload is skipped (it never reaches the
 * outbox). The whole operation is fail-soft via `observedPromise` so the caller
 * is never broken or slowed by a broadcast failure.
 *
 * KS-6.4 (#8414): ALSO best-effort projects the same safe events + summary
 * into the khala-sync `scope.public.settled-feed` projection (dual-write,
 * same discipline as the KS-6.1 fleet / KS-6.3 tokens-served cutovers). The
 * legacy `notifySyncScopes` room-poke stays live until the new projection
 * has real production evidence AND an anonymous-safe read path exists for
 * it (the new `/api/sync/connect` live-tail route requires an authenticated
 * actor and cannot serve this feed's anonymous/logged-out audience today —
 * see docs/khala-sync/RUNBOOK.md).
 */
export const publishSettledFeedEvents = async (
  env: SettledFeedSyncEnv,
  events: ReadonlyArray<PublicSettledFeedEvent>,
  options: Readonly<{ ctx?: SyncNotificationContext; feedId?: string }> = {},
): Promise<void> => {
  if (events.length === 0) {
    return
  }

  await observedPromise('Sync.publishSettledFeed', async () => {
    const scope = publicSettledFeedScope(options.feedId ?? PUBLIC_SETTLED_FEED_ID)
    const store = makeD1SyncOutboxRepository(openAgentsDatabase(env))

    const safeEvents = events.filter(event => {
      try {
        assertSettledFeedPayloadPublicSafe('Public settled feed event', event)

        return true
      } catch {
        return false
      }
    })

    if (safeEvents.length === 0) {
      return
    }

    await Promise.all(
      safeEvents.map(event =>
        store.appendChange({
          actorId: 'system',
          collection: SETTLED_FEED_SYNC_COLLECTION,
          id: event.eventRef,
          op: 'put',
          scope,
          value: event,
        }),
      ),
    )

    const summary = settledFeedSummaryFromEvents(safeEvents)
    let summaryIsSafe = true

    try {
      assertSettledFeedPayloadPublicSafe('Public settled feed summary', summary)
      await store.appendChange({
        actorId: 'system',
        collection: SETTLED_FEED_SUMMARY_COLLECTION,
        id: SETTLED_FEED_SUMMARY_ENTITY_ID,
        op: 'put',
        scope,
        value: summary,
      })
    } catch {
      // Summary is best-effort; skip it if it ever scans unsafe.
      summaryIsSafe = false
    }

    const notify = notifySyncScopes(env, [scope])
    // Fail-soft dual-write into khala-sync (KS-6.4, #8414): never awaited
    // into the caller's critical path via `ctx`-scheduled background work
    // when a context is available, exactly like the legacy room notify;
    // a failure here is swallowed by `projectSettledFeedBatchBestEffort`
    // itself and only ever surfaces as a typed diagnostic log.
    const projectToKhalaSync = summaryIsSafe
      ? projectSettledFeedBatchBestEffort(
          {
            binding: env.KHALA_SYNC_DB,
            ...(env.khalaSyncSettledFeedLog === undefined
              ? {}
              : { log: env.khalaSyncSettledFeedLog }),
          },
          { events: safeEvents, summary },
        ).then(() => undefined)
      : Promise.resolve(undefined)

    if (options.ctx === undefined) {
      await Promise.all([notify, projectToKhalaSync])
    } else {
      scheduleBackgroundWork(options.ctx, notify)
      scheduleBackgroundWork(options.ctx, projectToKhalaSync)
    }
  })
}
