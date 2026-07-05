import {
  type WorkerBindings,
  PUBLIC_SETTLED_FEED_ID,
  makeD1SyncOutboxRepository,
  publicSettledFeedScope,
} from '@openagentsinc/sync-worker'
import { Effect } from 'effect'

import {
  projectSettledFeedBatchBestEffort,
  type SettledFeedProjectionLog,
} from './khala-sync-public-settled-feed'
import type { KhalaSyncHyperdriveBinding } from './khala-sync-push-routes'
import { assertNexusPylonPublicSafe } from './nexus-pylon-visibility'
import {
  logWorkerRouteError,
  observedPromise,
  unwrapEffectTryPromiseCause,
} from './observability'
import { openAgentsDatabase, scheduleBackgroundWork } from './runtime'
import type { SyncNotificationContext } from './sync-notifier'

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

type SettledFeedSyncEnv = Pick<WorkerBindings, 'OPENAGENTS_DB'> &
  Readonly<{
    /**
     * `env.KHALA_SYNC_DB` — absent until the binding is deployed. KS-6.4
     * (#8414) projects the SAME public-safe events this function already
     * writes to the legacy D1 sync-outbox into `scope.public.settled-feed`
     * via khala-sync. Never required; a missing binding simply skips the
     * new-path projection.
     */
    KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding
    /** Diagnostic sink for the khala-sync projection (public-safe only). */
    khalaSyncSettledFeedLog?: SettledFeedProjectionLog
  }>

/**
 * Publish a batch of public-safe settled events to the public settled-feed
 * scope. Each event is scanned for unsafe material before it can be written;
 * a rejected payload is skipped (it never reaches the outbox). The whole
 * operation is fail-soft via `observedPromise` so the caller is never broken
 * or slowed by a broadcast failure.
 *
 * KS-6.4 (#8414) full cutover: this ALSO best-effort projects the same safe
 * events + summary into the khala-sync `scope.public.settled-feed`
 * projection (same discipline as the KS-6.1 fleet / KS-6.3 tokens-served
 * cutovers) — that projection is now the ONLY live delivery path for the
 * homepage/stats settled feed's `WS /api/sync/connect` live-tail
 * (`apps/web/src/subscriptions.ts`). The legacy `notifySyncScopes`
 * `SyncRoomDurableObject` room-poke has been retired now that the KS-8.x
 * anonymous-read exception makes the new engine's connect/log/bootstrap
 * routes reachable by this feed's anonymous/logged-out audience (see
 * docs/khala-sync/RUNBOOK.md "Anonymous read scopes"). The D1 sync-outbox
 * writes below are UNRELATED to that legacy room and stay live: they are the
 * fail-open fallback source for `GET /api/public/settled-feed`
 * (`public-settled-feed-routes.ts`), a separate public read route.
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

    // Each settled event is an independent outbox write. Isolate them with
    // Effect structured concurrency instead of a bare `Promise.all`: one
    // event's D1 write failing must not silently drop every OTHER unrelated
    // settlement event in this batch. The outer caller fire-and-forgets this
    // whole function (`.catch(() => undefined)`), so without per-event
    // isolation a single failure here would previously have discarded
    // visibility into (and delivery of) every sibling event too.
    const appendOutcomes = await Effect.runPromise(
      Effect.forEach(
        safeEvents,
        event =>
          Effect.result(
            Effect.tryPromise(() =>
              store.appendChange({
                actorId: 'system',
                collection: SETTLED_FEED_SYNC_COLLECTION,
                id: event.eventRef,
                op: 'put',
                scope,
                value: event,
              }),
            ),
          ).pipe(Effect.map(outcome => ({ event, outcome }))),
        { concurrency: 'unbounded' },
      ),
    )

    for (const { event, outcome } of appendOutcomes) {
      if (outcome._tag === 'Failure') {
        logWorkerRouteError(
          'settled_feed_event_append_failed',
          unwrapEffectTryPromiseCause(outcome.failure),
          { eventRef: event.eventRef },
        )
      }
    }

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

    // Fail-soft projection into khala-sync (KS-6.4, #8414) — the ONLY live
    // delivery path now (the legacy room poke is retired). Scheduled into
    // background work via `ctx` when available, exactly like the retired
    // legacy notify was; a failure here is swallowed by
    // `projectSettledFeedBatchBestEffort` itself and only ever surfaces as a
    // typed diagnostic log, never a broken/slowed settlement dispatch.
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
      await projectToKhalaSync
    } else {
      scheduleBackgroundWork(options.ctx, projectToKhalaSync)
    }
  })
}
