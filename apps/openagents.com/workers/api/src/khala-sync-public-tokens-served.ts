// Khala Sync public tokens-served projection (KS-6.3, #8304).
//
// Makes the "Khala Tokens Served" headline a `scope.public.tokens-served`
// projection (SPEC §2.1/§7 invariant 8) and takes the unbounded
// `SUM(...) FROM token_usage_events` off the public read hot path:
//
//   PRODUCER (fail-soft dual-write, same discipline as the KS-6.1 fleet
//   projection): on every fresh token-usage ledger row — both the ledger
//   `ingestEvent` path and the khala-chat/MCP direct-insert paths — the
//   Worker best-effort increments the `khala_sync_public_counters` row AND
//   appends the `public_counter` post-image to `scope.public.tokens-served`
//   in ONE Postgres transaction via the KHALA_SYNC_DB Hyperdrive binding.
//   EXACT-ONCE per D1 ledger row: the increment is keyed by the event's
//   idempotency key through a `khala_sync_counter_applied` guard insert in
//   the same transaction (replay ⇒ no-op). A projection failure NEVER fails
//   the D1 business write — every failure is a typed diagnostic.
//
//   READER: `GET /api/public/khala-tokens-served` serves the single-row
//   projection read (~ms through Hyperdrive) behind a small in-isolate
//   cache (TOKENS_SERVED_PROJECTION_CACHE_TTL_MS, honestly declared in the
//   payload's projection_staleness.v1 contract). FAIL-OPEN: when the
//   binding is absent, Postgres is unreachable, or the counter has not been
//   backfilled yet, the route falls back to the existing live-at-read D1
//   SUM — public-counter availability never regresses.
//
//   RECONCILE (invariant 8): `reconcileTokensServedProjection` recomputes
//   the exact D1 SUM and compares it to the projection. Drift is reported
//   (typed diagnostic + route payload), NEVER silently overwritten; the
//   explicit `repair` action sets projection = exact SUM with an audit row
//   (`khala_sync_public_counter_repairs`). The first-deploy backfill IS the
//   repair action against an uninitialized counter.

import { TOKENS_SERVED_COUNTER_ID } from '@openagentsinc/khala-sync'
import {
  applyPublicCounterIncrementBestEffort,
  type PublicCounterProjectionDiagnostic,
  readPublicCounter,
  repairPublicCounter,
} from '@openagentsinc/khala-sync-server'

import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type KhalaSyncPushSqlClient,
  type MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { currentEpochMillis, currentIsoTimestamp } from './runtime-primitives'

export { TOKENS_SERVED_COUNTER_ID }

// ---------------------------------------------------------------------------
// Shared dependency slice
// ---------------------------------------------------------------------------

export type TokensServedProjectionLog = (
  event:
    | 'khala_sync_tokens_served_projection_failed'
    | 'khala_sync_tokens_served_projection_drift',
  fields: Readonly<Record<string, string | number>>,
) => void

export type TokensServedProjectionDeps = Readonly<{
  /** `env.KHALA_SYNC_DB` — absent until the binding is deployed. */
  binding: KhalaSyncHyperdriveBinding | undefined
  /**
   * Injectable transaction-mode-safe client factory (same seam as the
   * push/log routes). Tests inject a fake; production uses postgres.js.
   */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  /** Diagnostic sink (public-safe fields only). */
  log?: TokensServedProjectionLog | undefined
}>

const bindingConnectionString = (
  binding: KhalaSyncHyperdriveBinding | undefined,
): string | undefined =>
  binding !== undefined &&
  typeof binding.connectionString === 'string' &&
  binding.connectionString.length > 0
    ? binding.connectionString
    : undefined

const withSqlClient = async <A>(
  deps: TokensServedProjectionDeps,
  connectionString: string,
  fn: (client: KhalaSyncPushSqlClient) => Promise<A>,
): Promise<A> => {
  const makeSqlClient = deps.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  let client: KhalaSyncPushSqlClient | undefined
  try {
    client = await makeSqlClient(connectionString)
    return await fn(client)
  } finally {
    if (client !== undefined) {
      try {
        await client.end()
      } catch {
        // best-effort teardown (same discipline as the khala-sync routes).
      }
    }
  }
}

// ---------------------------------------------------------------------------
// Producer (fail-soft, exact-once per D1 ledger row)
// ---------------------------------------------------------------------------

/** One fresh D1 ledger row's public-counter contribution. */
export type TokensServedIngestEvent = Readonly<{
  /** The ledger row's idempotency key — the exact-once guard key. */
  idempotencyKey: string
  /** input+output tokens (total_tokens fallback), per the public policy. */
  tokensServedDelta: number
  observedAt: string
}>

export type TokensServedProjectionOutcome =
  | { readonly outcome: 'applied'; readonly total: number }
  | { readonly outcome: 'duplicate_idempotency_key' }
  | { readonly outcome: 'skipped_no_binding' }
  | { readonly outcome: 'skipped_zero_delta' }
  | {
      readonly outcome: 'failed'
      readonly diagnostic: PublicCounterProjectionDiagnostic
    }

/**
 * Best-effort increment of the tokens-served projection for ONE fresh D1
 * ledger row. NEVER throws and never fails the caller's business write;
 * failures land in the injected log as a typed public-safe diagnostic. A
 * `counter_not_initialized` refusal is expected before the first admin
 * backfill and is deliberately not logged as an error.
 */
export const recordTokensServedProjectionBestEffort = async (
  deps: TokensServedProjectionDeps,
  event: TokensServedIngestEvent,
): Promise<TokensServedProjectionOutcome> => {
  const delta = Math.trunc(event.tokensServedDelta)
  if (delta <= 0) {
    return { outcome: 'skipped_zero_delta' }
  }
  const connectionString = bindingConnectionString(deps.binding)
  if (connectionString === undefined) {
    return { outcome: 'skipped_no_binding' }
  }
  try {
    const result = await withSqlClient(deps, connectionString, client =>
      applyPublicCounterIncrementBestEffort(client.sql, {
        counterId: TOKENS_SERVED_COUNTER_ID,
        delta,
        idempotencyKey: event.idempotencyKey,
        observedAt: event.observedAt,
      }),
    )
    if (result.ok) {
      return result.result.applied
        ? { outcome: 'applied', total: result.result.counter.total }
        : { outcome: 'duplicate_idempotency_key' }
    }
    if (result.diagnostic.reason !== 'counter_not_initialized') {
      deps.log?.('khala_sync_tokens_served_projection_failed', {
        messageSafe: result.diagnostic.messageSafe,
        reason: result.diagnostic.reason,
      })
    }
    return { diagnostic: result.diagnostic, outcome: 'failed' }
  } catch {
    // Client construction/teardown failures: still fail-soft. Never echo
    // driver errors (they can embed the DSN).
    const diagnostic: PublicCounterProjectionDiagnostic = {
      messageSafe: 'tokens-served projection client failed',
      reason: 'projection_failed',
    }
    deps.log?.('khala_sync_tokens_served_projection_failed', {
      messageSafe: diagnostic.messageSafe,
      reason: diagnostic.reason,
    })
    return { diagnostic, outcome: 'failed' }
  }
}

// ---------------------------------------------------------------------------
// Reader (single-row projection read behind a small in-isolate cache)
// ---------------------------------------------------------------------------

/**
 * The in-isolate cache TTL for the projection read — the declared
 * `maxStalenessSeconds` of the served projection is this bound in seconds
 * (the 2-second public-stats cache precedent from the 2026-06-29
 * after-action). The projection itself is bumped by the ingest write path,
 * so the served value is at most this cache window behind the ledger.
 */
export const TOKENS_SERVED_PROJECTION_CACHE_TTL_MS = 2_000
export const TOKENS_SERVED_PROJECTION_MAX_STALENESS_SECONDS =
  TOKENS_SERVED_PROJECTION_CACHE_TTL_MS / 1000

export type TokensServedProjectionSnapshot = Readonly<{
  tokensServed: number
  lastEventAt: string | null
}>

/** Injectable projection-read seam for route tests. */
export type ReadTokensServedProjection = (
  sql: KhalaSyncPushSqlClient['sql'],
  counterId: string,
) => Promise<{ total: number; lastEventAt: string | null } | null>

type ProjectionCacheState = Readonly<{
  snapshot: TokensServedProjectionSnapshot
  expiresAtMs: number
}>

let projectionCache: ProjectionCacheState | undefined

/** Drop the cached snapshot (repair path + tests). */
export const invalidateTokensServedProjectionCache = (): void => {
  projectionCache = undefined
}

export const resetTokensServedProjectionCacheForTests =
  invalidateTokensServedProjectionCache

export type TokensServedProjectionReadDeps = TokensServedProjectionDeps &
  Readonly<{
    readProjection?: ReadTokensServedProjection | undefined
    nowMs?: (() => number) | undefined
  }>

/**
 * Read the tokens-served projection through the in-isolate cache. Returns
 * `undefined` on ANY miss that the caller must fail open from: binding
 * absent, Postgres unreachable, or counter not yet backfilled. Only
 * successful reads are cached — failures stay live so recovery is
 * immediate and the D1 fallback path keeps its live-at-read honesty.
 */
export const readTokensServedProjectionCached = async (
  deps: TokensServedProjectionReadDeps,
): Promise<TokensServedProjectionSnapshot | undefined> => {
  const nowMs = (deps.nowMs ?? currentEpochMillis)()
  if (projectionCache !== undefined && projectionCache.expiresAtMs > nowMs) {
    return projectionCache.snapshot
  }
  const connectionString = bindingConnectionString(deps.binding)
  if (connectionString === undefined) {
    return undefined
  }
  try {
    const row = await withSqlClient(deps, connectionString, client =>
      (deps.readProjection ?? readPublicCounter)(
        client.sql,
        TOKENS_SERVED_COUNTER_ID,
      ),
    )
    if (row === null) {
      // Not backfilled yet — fail open to the exact D1 SUM; a partial
      // projection total must never be served as the network aggregate.
      return undefined
    }
    const snapshot: TokensServedProjectionSnapshot = {
      lastEventAt: row.lastEventAt,
      tokensServed: row.total,
    }
    projectionCache = {
      expiresAtMs: nowMs + TOKENS_SERVED_PROJECTION_CACHE_TTL_MS,
      snapshot,
    }
    return snapshot
  } catch {
    return undefined
  }
}

// ---------------------------------------------------------------------------
// Reconcile (invariant 8) + explicit audited repair/backfill
// ---------------------------------------------------------------------------

export type TokensServedReconcileReport = Readonly<{
  counterId: string
  /** SUM over exact `token_usage_events` rows (the source of truth). */
  exactTokensServed: number
  /** Projection total, or null when the counter is not initialized. */
  projectedTokensServed: number | null
  projectionLastEventAt: string | null
  /** exact - projected; null when the counter is not initialized. */
  driftTokens: number | null
  inSync: boolean
  repaired: boolean
  repairSource: 'backfill' | 'reconcile_repair' | null
  previousTotal: number | null
  generatedAt: string
}>

export type TokensServedReconcileResult =
  | { readonly ok: true; readonly report: TokensServedReconcileReport }
  | {
      readonly ok: false
      readonly reason:
        | 'no_binding'
        | 'exact_read_failed'
        | 'projection_read_failed'
        | 'repair_failed'
      readonly messageSafe: string
    }

export type TokensServedReconcileDeps = TokensServedProjectionDeps &
  Readonly<{
    /** Exact-source read: the D1 ledger's public tokens-served SUM. */
    readExactTokensServed: () => Promise<number>
    nowIso?: (() => string) | undefined
  }>

/**
 * Recompute the exact D1 SUM, compare it to the projection, and (only when
 * `repair` is set) realign the projection to the exact SUM with an audit
 * note. Drift detection NEVER writes; a missing counter row plus `repair`
 * is the first-deploy backfill. Drift (or an uninitialized counter) is
 * logged as a typed diagnostic either way.
 */
export const reconcileTokensServedProjection = async (
  deps: TokensServedReconcileDeps,
  options: Readonly<{ repair: boolean; auditNote?: string | undefined }>,
): Promise<TokensServedReconcileResult> => {
  const nowIso = deps.nowIso ?? currentIsoTimestamp
  const connectionString = bindingConnectionString(deps.binding)
  if (connectionString === undefined) {
    return {
      messageSafe:
        'Khala Sync storage is not configured on this deployment ' +
        '(env.KHALA_SYNC_DB Hyperdrive binding is absent).',
      ok: false,
      reason: 'no_binding',
    }
  }

  let exactTokensServed: number
  try {
    exactTokensServed = Math.max(
      0,
      Math.trunc(await deps.readExactTokensServed()),
    )
  } catch {
    return {
      messageSafe: 'exact D1 tokens-served SUM read failed; retry.',
      ok: false,
      reason: 'exact_read_failed',
    }
  }

  try {
    return await withSqlClient(deps, connectionString, async client => {
      let projected: { total: number; lastEventAt: string | null } | null
      try {
        projected = await readPublicCounter(
          client.sql,
          TOKENS_SERVED_COUNTER_ID,
        )
      } catch {
        return {
          messageSafe: 'tokens-served projection read failed; retry.',
          ok: false,
          reason: 'projection_read_failed',
        } as const
      }

      const driftTokens =
        projected === null ? null : exactTokensServed - projected.total
      const inSync = driftTokens === 0

      if (!inSync) {
        deps.log?.('khala_sync_tokens_served_projection_drift', {
          counterId: TOKENS_SERVED_COUNTER_ID,
          driftTokens: driftTokens ?? -1,
          exactTokensServed,
          projectedTokensServed: projected?.total ?? -1,
          projectionInitialized: projected === null ? 0 : 1,
        })
      }

      if (!options.repair) {
        return {
          ok: true,
          report: {
            counterId: TOKENS_SERVED_COUNTER_ID,
            driftTokens,
            exactTokensServed,
            generatedAt: nowIso(),
            inSync,
            previousTotal: null,
            projectedTokensServed: projected?.total ?? null,
            projectionLastEventAt: projected?.lastEventAt ?? null,
            repairSource: null,
            repaired: false,
          },
        } as const
      }

      const repairSource = projected === null ? 'backfill' : 'reconcile_repair'
      const trimmedAuditNote = options.auditNote?.trim()
      try {
        const repaired = await repairPublicCounter(client.sql, {
          auditNote:
            trimmedAuditNote !== undefined && trimmedAuditNote.length > 0
              ? trimmedAuditNote
              : `${repairSource}: set projection to exact D1 SUM ` +
                `${exactTokensServed} (previous ${projected?.total ?? 'none'})`,
          counterId: TOKENS_SERVED_COUNTER_ID,
          exactTotal: exactTokensServed,
          source: repairSource,
        })
        // The projection changed under any cached copy — drop it so the
        // route serves the repaired total on the next read.
        invalidateTokensServedProjectionCache()
        return {
          ok: true,
          report: {
            counterId: TOKENS_SERVED_COUNTER_ID,
            driftTokens,
            exactTokensServed,
            generatedAt: nowIso(),
            inSync,
            previousTotal: repaired.previousTotal,
            projectedTokensServed: repaired.counter.total,
            projectionLastEventAt: repaired.counter.lastEventAt,
            repairSource,
            repaired: true,
          },
        } as const
      } catch {
        return {
          messageSafe: 'tokens-served projection repair failed; retry.',
          ok: false,
          reason: 'repair_failed',
        } as const
      }
    })
  } catch {
    return {
      messageSafe: 'tokens-served projection read failed; retry.',
      ok: false,
      reason: 'projection_read_failed',
    }
  }
}
