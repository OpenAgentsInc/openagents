// Khala Sync per-user credit-balance projection (issue #8505, Part 2).
//
// Makes the mobile balance chip a `scope.user.<userId>` `credit_balance`
// projection, following the EXACT same pattern as
// `khala-sync-public-tokens-served.ts` (KS-6.3, #8304), just per-user instead
// of global. See
// docs/khala-code/2026-07-06-credits-ledger-vs-khala-sync-architecture-audit.md
// for why the D1 `agent_balances` ledger stays the sole authority for
// balance-gating/charge decisions — this module only mirrors the RESULT of an
// already-committed D1 write into Khala Sync so subscribed mobile clients see
// it update live, without a REST poll.
//
//   PRODUCER (fail-soft, exact-once per D1 ledger event): every known D1
//   ledger write site (inference charges, cloud/agent-computer charges, the
//   $10 signup grant, Aiur admin grants/clawbacks) calls
//   `recordUserCreditBalanceDeltaBestEffort` with the SAME idempotency key
//   the D1 write already used. A projection failure NEVER fails, retries, or
//   reverses the real D1 write — it is a typed diagnostic only.
//
//   BACKFILL: before a user's projection row exists, deltas refuse
//   (`credit_balance_not_initialized`) rather than serve a fabricated zero.
//   `backfillUserCreditBalancesBatch` pages through EVERY human user (not
//   just ones with an existing `agent_balances` row — a user who has never
//   been charged or granted still needs an initialized $0 projection row) and
//   seeds each to their exact current D1 balance.

import {
  applyUserCreditBalanceDeltaBestEffort,
  readUserCreditBalance,
  repairUserCreditBalance,
  type UserCreditBalanceProjectionDiagnostic,
} from '@openagentsinc/khala-sync-server'

import { msatToUsdCentsRound } from './inference/usd-msat-conversion'
import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type KhalaSyncPushSqlClient,
  type MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { currentIsoTimestamp } from './runtime-primitives'

export type { UserCreditBalanceProjectionDiagnostic }

// ---------------------------------------------------------------------------
// Shared dependency slice
// ---------------------------------------------------------------------------

export type UserCreditBalanceProjectionLog = (
  event:
    | 'khala_sync_user_credit_balance_projection_failed'
    | 'khala_sync_user_credit_balance_backfill_failed',
  fields: Readonly<Record<string, string | number>>,
) => void

export type UserCreditBalanceProjectionDeps = Readonly<{
  /** `env.KHALA_SYNC_DB` — absent until the binding is deployed. */
  binding: KhalaSyncHyperdriveBinding | undefined
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  log?: UserCreditBalanceProjectionLog | undefined
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
  deps: UserCreditBalanceProjectionDeps,
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
// Producer (fail-soft, exact-once per D1 ledger event)
// ---------------------------------------------------------------------------

/** One D1 ledger write's contribution to a user's projected balance. */
export type UserCreditBalanceLedgerEvent = Readonly<{
  userId: string
  /** The D1 ledger write's OWN idempotency key — reused verbatim, never a
   * new key scheme (`inference:charge:<requestId>`,
   * `<primitive>:charge:<chargeId>`, `signup:github:<githubUserId>`, an admin
   * grant/clawback ref, ...). */
  idempotencyKey: string
  /** Signed USD-cents delta: positive for a grant, negative for a
   * charge/clawback. */
  deltaUsdCents: number
  observedAt: string
}>

export type UserCreditBalanceProjectionOutcome =
  | { readonly outcome: 'applied'; readonly balanceUsdCents: number }
  | { readonly outcome: 'duplicate_idempotency_key' }
  | { readonly outcome: 'skipped_no_binding' }
  | { readonly outcome: 'skipped_zero_delta' }
  | {
      readonly outcome: 'failed'
      readonly diagnostic: UserCreditBalanceProjectionDiagnostic
    }

/**
 * Best-effort projection of ONE fresh D1 ledger event into
 * `scope.user.<userId>`. NEVER throws and never fails the caller's business
 * write; failures land in the injected log as a typed public-safe
 * diagnostic. A `credit_balance_not_initialized` refusal is expected before
 * that user's backfill runs and is deliberately not logged as an error (same
 * quiet-refusal discipline as the public tokens-served counter).
 */
export const recordUserCreditBalanceDeltaBestEffort = async (
  deps: UserCreditBalanceProjectionDeps,
  event: UserCreditBalanceLedgerEvent,
): Promise<UserCreditBalanceProjectionOutcome> => {
  const delta = Math.trunc(event.deltaUsdCents)
  if (delta === 0) {
    return { outcome: 'skipped_zero_delta' }
  }
  const connectionString = bindingConnectionString(deps.binding)
  if (connectionString === undefined) {
    return { outcome: 'skipped_no_binding' }
  }
  try {
    const result = await withSqlClient(deps, connectionString, client =>
      applyUserCreditBalanceDeltaBestEffort(client.sql, {
        deltaUsdCents: delta,
        idempotencyKey: event.idempotencyKey,
        observedAt: event.observedAt,
        userId: event.userId,
      }),
    )
    if (result.ok) {
      return result.result.applied
        ? {
            balanceUsdCents: result.result.balance.balanceUsdCents,
            outcome: 'applied',
          }
        : { outcome: 'duplicate_idempotency_key' }
    }
    if (result.diagnostic.reason !== 'credit_balance_not_initialized') {
      deps.log?.('khala_sync_user_credit_balance_projection_failed', {
        messageSafe: result.diagnostic.messageSafe,
        reason: result.diagnostic.reason,
      })
    }
    return { diagnostic: result.diagnostic, outcome: 'failed' }
  } catch {
    // Client construction/teardown failures: still fail-soft. Never echo
    // driver errors (they can embed the DSN).
    const diagnostic: UserCreditBalanceProjectionDiagnostic = {
      messageSafe: 'user credit balance projection client failed',
      reason: 'projection_failed',
    }
    deps.log?.('khala_sync_user_credit_balance_projection_failed', {
      messageSafe: diagnostic.messageSafe,
      reason: diagnostic.reason,
    })
    return { diagnostic, outcome: 'failed' }
  }
}

/**
 * Parse the stable OpenAgents user id back out of an `agentRefForUser`
 * (`usd-credit-bridge.ts`) shaped account ref, e.g. `agent:<userId>`. Every
 * known credits ledger write site authenticates the mobile/coding user this
 * way; a ref that does NOT match this shape (a non-user agent account) is
 * intentionally not projected — `undefined` means "skip the projection for
 * this event", never a guess.
 */
export const userIdFromAgentRef = (accountRef: string): string | undefined => {
  const prefix = 'agent:'
  if (!accountRef.startsWith(prefix)) return undefined
  const userId = accountRef.slice(prefix.length)
  return userId.length === 0 ? undefined : userId
}

// ---------------------------------------------------------------------------
// Backfill (explicit, audited, paginated over every human user)
// ---------------------------------------------------------------------------

export type UserCreditBalanceBackfillCandidate = Readonly<{
  userId: string
  balanceMsat: number
}>

/**
 * One page of human users needing a credit-balance backfill/reconcile pass,
 * ordered by user id with a keyset cursor (never OFFSET, so a page is stable
 * even as new users sign up between calls). `balanceMsat` is read via the
 * SAME `agent:<userId>` actor-ref convention `agentRefForUser` uses,
 * defaulting to 0 for a user who has never been charged or granted — that
 * user still needs an initialized $0 projection row, not a permanent
 * "not initialized" refusal.
 */
export const listUsersForCreditBalanceBackfill = async (
  db: D1Database,
  input: Readonly<{ cursor?: string | undefined; limit: number }>,
): Promise<ReadonlyArray<UserCreditBalanceBackfillCandidate>> => {
  const result = await db
    .prepare(
      `SELECT users.id AS user_id,
              COALESCE(agent_balances.balance_msat, 0) AS balance_msat
         FROM users
         LEFT JOIN agent_balances
           ON agent_balances.actor_ref = 'agent:' || users.id
        WHERE users.kind = 'human'
          AND users.deleted_at IS NULL
          AND (? IS NULL OR users.id > ?)
        ORDER BY users.id ASC
        LIMIT ?`,
    )
    .bind(input.cursor ?? null, input.cursor ?? null, input.limit)
    .all<{ user_id: string; balance_msat: number | string }>()

  return (result.results ?? []).map(row => ({
    balanceMsat: Number(row.balance_msat),
    userId: String(row.user_id),
  }))
}

export type UserCreditBalanceBackfillDeps = UserCreditBalanceProjectionDeps &
  Readonly<{
    db: D1Database
    nowIso?: (() => string) | undefined
  }>

export type UserCreditBalanceBackfillReport = Readonly<{
  processedCount: number
  backfilledCount: number
  reconciledCount: number
  unchangedCount: number
  failedCount: number
  nextCursor: string | null
}>

export type UserCreditBalanceBackfillResult =
  | { readonly ok: true; readonly report: UserCreditBalanceBackfillReport }
  | { readonly ok: false; readonly reason: 'no_binding'; readonly messageSafe: string }

/**
 * Backfill/reconcile one bounded page of human users' credit-balance
 * projections against their exact current D1 `agent_balances` balance.
 * Per-user work is fail-soft (one failure never aborts the page); the
 * caller repeats with `nextCursor` until it is `null`. Idempotent: a user
 * already at the exact D1 balance is a no-op read (`unchangedCount`); one
 * with real drift is an audited `reconcile_repair`; a never-initialized user
 * is the first-deploy `backfill`.
 */
export const backfillUserCreditBalancesBatch = async (
  deps: UserCreditBalanceBackfillDeps,
  input: Readonly<{ cursor?: string | undefined; limit: number }>,
): Promise<UserCreditBalanceBackfillResult> => {
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

  const candidates = await listUsersForCreditBalanceBackfill(deps.db, input)
  const nowIso = deps.nowIso ?? currentIsoTimestamp

  let backfilledCount = 0
  let reconciledCount = 0
  let unchangedCount = 0
  let failedCount = 0

  await withSqlClient(deps, connectionString, async client => {
    for (const candidate of candidates) {
      const exactBalanceUsdCents = msatToUsdCentsRound(candidate.balanceMsat)
      try {
        const existing = await readUserCreditBalance(client.sql, candidate.userId)
        if (existing !== null && existing.balanceUsdCents === exactBalanceUsdCents) {
          unchangedCount += 1
          continue
        }
        const source = existing === null ? 'backfill' : 'reconcile_repair'
        await repairUserCreditBalance(client.sql, {
          auditNote:
            source === 'backfill'
              ? `backfill: seed projection to exact D1 balance ${exactBalanceUsdCents} cents (${nowIso()})`
              : `reconcile_repair: realign projection from ${existing?.balanceUsdCents ?? 'none'} ` +
                `to exact D1 balance ${exactBalanceUsdCents} cents (${nowIso()})`,
          exactBalanceUsdCents,
          source,
          userId: candidate.userId,
        })
        if (source === 'backfill') backfilledCount += 1
        else reconciledCount += 1
      } catch (error) {
        failedCount += 1
        deps.log?.('khala_sync_user_credit_balance_backfill_failed', {
          messageSafe:
            error instanceof Error ? error.message : 'backfill repair failed',
          userIdLength: candidate.userId.length,
        })
      }
    }
  })

  return {
    ok: true,
    report: {
      backfilledCount,
      failedCount,
      nextCursor:
        candidates.length < input.limit
          ? null
          : (candidates[candidates.length - 1]?.userId ?? null),
      processedCount: candidates.length,
      reconciledCount,
      unchangedCount,
    },
  }
}
