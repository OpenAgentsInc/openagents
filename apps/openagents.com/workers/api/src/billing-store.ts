// KS-8.7 (#8318): billing / Stripe / pay-ins domain — D1 → Cloud SQL
// migration machinery. Third KS-8 domain lane; follows the KS-8.1/8.2
// templates (`pylon-dispatch-store.ts`, `token-ledger-store.ts`).
//
// MONEY-DOMAIN DISCIPLINE (MIGRATION_PLAN §3.4): D1 is the SOLE authority
// for this domain. The Postgres side is a best-effort dual-write mirror
// plus backfill target ONLY in this lane. Three consequences are load-
// bearing here:
//
//   1. THE MIRROR COPIES, NEVER RECOMPUTES. `makeBillingDomainMirror`
//      reads back the FRESH authoritative D1 row(s) a write just touched
//      (`BillingMirrorRef`: table + key) and converge-upserts the
//      byte-identical copy into Postgres. Amounts, idempotency keys, and
//      Stripe webhook event ids round-trip byte-exact because they are the
//      same bytes D1 stored. The write-side idempotency decision (INSERT
//      OR IGNORE on `stripe_webhook_events.event_id`, ledger
//      `idempotency_key`, pay-in keys, …) is made ONCE, on D1, and never
//      re-evaluated against Postgres.
//
//   2. FAIL-SOFT, ALWAYS. A mirror failure logs the typed drift
//      diagnostic `khala_sync_billing_dual_write_failed` and the request
//      proceeds on the D1 result. No billing operation ever fails, blocks,
//      or double-fires because of the mirror. Side-effectful evaluators
//      (auto-top-up, sweeps, Stripe API calls) read exactly one store: D1.
//
//   3. READS MOSTLY STAY ON D1. `KHALA_SYNC_BILLING_READS` routes the
//      per-user balance SUM behind `readBillingSummary` only where a call
//      site explicitly opts in with
//      `billingRuntimeForEnv(env, { routeReads: true })` (the display
//      summary path). Gates, evaluators, and receipt inputs always read
//      D1. Flipping the flag to `postgres` in production for the BALANCE
//      is an EPIC-GATED ops decision (#8282) taken only after the backfill
//      `--verify` (exact per-account balance equality) is green; see
//      docs/khala-sync/RUNBOOK.md "Billing domain cutover".
//
//      #8337 (KS-8.7 follow-up) widens this with a SEPARATE, narrower,
//      always-on bounded read-serving allowlist — the same discipline as
//      the KS-8.14 business-domain lane's
//      `BUSINESS_DOMAIN_POSTGRES_SERVED_READ_TABLES` (#8360):
//      `BILLING_DOMAIN_POSTGRES_SERVED_READ_TABLES` names the tables behind
//      exactly four DISPLAY-ONLY, non-decision-critical read surfaces —
//      the recent-ledger-entries projection, the auto-top-up DISPLAY state
//      (never the charge decision), Stripe checkout receipt reads
//      (immutable, already-settled), and inference/pay-in receipt reads
//      (immutable, already-settled) — each wired through its own
//      hand-audited function, never a generic "does this SQL touch table
//      X" classifier. Every other billing read (balance-gating decisions,
//      idempotency/dedupe checks, webhook processing, auto-top-up charge
//      evaluation, buyer-payment pipeline dedupe) stays D1-served
//      PERMANENTLY, by design, because a lagging mirror read there could
//      silently double-charge, double-credit, or corrupt a payout-adjacent
//      decision. `KHALA_SYNC_BILLING_READS=postgres` unlocks serving for
//      ONLY this allowlisted surface; `compare` shadow-runs it (serves D1,
//      logs drift) exactly like the balance read.
//
// Flags:
//   KHALA_SYNC_BILLING_DUAL_WRITE  (default ON; 'off'|'0'|'false'|'disabled')
//   KHALA_SYNC_BILLING_READS       (default 'd1'; 'd1'|'compare'|'postgres')
// With no KHALA_SYNC_DB binding everything degrades to plain D1.
//
// Cutover order (docs/khala-sync/RUNBOOK.md "Billing domain cutover"):
// dual-write on → backfill (scripts/backfill-billing.ts) → catch-up sweep
// → --verify (money reconciliation) → compare reads → [EPIC-GATED]
// postgres reads (balance) / allowlisted bounded postgres reads (#8337) →
// decommission D1 tables in a follow-up.

import {
  BILLING_DOMAIN_TABLE_SPECS,
  noopCompareSoakMetrics,
  normalizeBillingValue,
  type BillingDomainTable,
  type CompareSoakMetrics,
} from '@openagentsinc/khala-sync-server'

import type {
  BillingAutoTopUpEventRow,
  BillingAutoTopUpPolicyRow,
  BillingAutoTopUpState,
  BillingAutoTopUpStateRead,
  BillingBalanceRead,
  BillingDomainMirror,
  BillingLedgerEntry,
  BillingLedgerEntryRow,
  BillingMirrorRef,
  BillingRecentEntriesRead,
  BillingRuntime,
  BillingSavedPaymentMethodRow,
} from './billing'
import {
  BILLING_AUTO_TOP_UP_EVENTS_LIMIT,
  BILLING_CURRENCY,
  BILLING_RECENT_LEDGER_ENTRIES_LIMIT,
  billingAutoTopUpStateFromRows,
  billingLedgerEntryFromRow,
  systemBillingRuntime,
} from './billing'
import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type KhalaSyncPushSqlClient,
  type MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { logWorkerRouteWarning } from './observability'

// ---------------------------------------------------------------------------
// Flags
// ---------------------------------------------------------------------------

export type BillingSyncReadsMode = 'd1' | 'postgres' | 'compare'

export type BillingSyncFlags = Readonly<{
  dualWrite: boolean
  reads: BillingSyncReadsMode
}>

export type BillingSyncFlagEnv = Readonly<{
  KHALA_SYNC_BILLING_DUAL_WRITE?: string | undefined
  KHALA_SYNC_BILLING_READS?: string | undefined
}>

const FLAG_OFF_VALUES = new Set(['0', 'off', 'false', 'disabled', 'no'])

/**
 * Parse the KS-8.7 flags. Dual-write defaults ON wherever the binding
 * exists; reads default to D1 authority. Unknown read values fall back to
 * 'd1' — never fail open into an unproven read path on a typo, least of
 * all for money.
 */
export const billingSyncFlagsFromEnv = (
  env: BillingSyncFlagEnv,
): BillingSyncFlags => {
  const dualWriteRaw = env.KHALA_SYNC_BILLING_DUAL_WRITE?.trim().toLowerCase()
  const readsRaw = env.KHALA_SYNC_BILLING_READS?.trim().toLowerCase()

  return {
    dualWrite:
      dualWriteRaw === undefined || !FLAG_OFF_VALUES.has(dualWriteRaw),
    reads:
      readsRaw === 'postgres' || readsRaw === 'compare' ? readsRaw : 'd1',
  }
}

// ---------------------------------------------------------------------------
// #8337 bounded Postgres-served read allowlist
// ---------------------------------------------------------------------------

/**
 * The bounded read surface that ACTUALLY serves from Postgres when
 * `KHALA_SYNC_BILLING_READS=postgres` (KS-8.7 follow-up, #8337) — mirrors
 * the KS-8.14 business-domain lane's
 * `BUSINESS_DOMAIN_POSTGRES_SERVED_READ_TABLES` discipline (#8360), adapted
 * to this domain's per-function (not generic-SQL-classifier) architecture.
 *
 * Unlike the business-domain lane, billing-store.ts never runs a generic
 * "does this SQL touch table X" classifier over arbitrary statements —
 * every read here is its own hand-audited, single-purpose function. This
 * Set is therefore a DOCUMENTATION registry (which tables a Postgres-served
 * read is ever allowed to touch) that each of the four allowlisted read
 * functions below asserts against; it is NOT a generic gate that would let
 * an unrelated future SQL statement touching one of these tables serve from
 * Postgres automatically.
 *
 * The four allowlisted surfaces, and why each is safe:
 *   - `billing_ledger_entries` — ONLY the recent-entries display projection
 *     (`readRecentLedgerEntries`, LIMIT 12). NOT the balance SUM (that has
 *     its own separate, still-D1-default `balanceRead` opt-in above) and
 *     NOT any ledger write-decision input.
 *   - `billing_auto_top_up_policies` / `billing_auto_top_up_events` /
 *     `stripe_saved_payment_methods` — ONLY the auto-top-up DISPLAY state
 *     (`readBillingAutoTopUpState`: saved-card summary, policy, recent
 *     events). NEVER the auto-top-up charge decision — `chargeAutoTopUp`
 *     (stripe-billing.ts) always reads its own dedicated D1 query directly
 *     and takes no runtime hook, so a lagging mirror read here can never
 *     double-charge or skip a top-up.
 *   - `stripe_checkout_sessions` — ONLY the public checkout-receipt read
 *     (`stripe-checkout-receipts.ts`), which projects an already-settled,
 *     immutable receipt for a completed session. Not the webhook
 *     processing_status write path, not the checkout-session creation.
 *   - `pay_ins` — ONLY the public inference-receipt read
 *     (`inference-receipts.ts`: `readInferenceReceiptByRef` /
 *     `listRecentInferenceReceipts`), scoped to `pay_in_type IN
 *     ('adjustment', 'usd_credit_grant')` and an immutable
 *     `public_receipt_ref`. Not any pay-in state-transition decision.
 *
 * Deliberately NOT allowlisted this pass (candidates for a future,
 * individually reviewed pass): the buyer-payment pipeline
 * (`buyer_payment_challenges`/`receipts`/`entitlements`/`redemptions`/
 * `reconciliation_events`) — every read there is SHARED with a
 * decision-critical idempotency/dedupe check
 * (`buyer-payment-ledger.ts`'s `makeD1BuyerPaymentLedgerStore`, used both
 * by the read-only checkout-return/payment-proof status routes AND by the
 * challenge/webhook/redemption creation paths) and cannot be split into a
 * decision-free surface without further store-interface surgery; and the
 * forum tip-earnings leaderboard/creator-earnings projections
 * (`forum/tip-earnings.ts`), which JOIN `pay_ins`/`pay_in_legs` against
 * `forum_posts` (a different domain's mirror) in a single statement.
 */
export const BILLING_DOMAIN_POSTGRES_SERVED_READ_TABLES: ReadonlySet<BillingDomainTable> =
  new Set<BillingDomainTable>([
    'billing_ledger_entries',
    'billing_auto_top_up_policies',
    'billing_auto_top_up_events',
    'stripe_saved_payment_methods',
    'stripe_checkout_sessions',
    'pay_ins',
  ])

/** True when `table` is in the #8337 bounded Postgres-served read allowlist. */
export const billingPostgresServesTable = (
  table: BillingDomainTable,
): boolean => BILLING_DOMAIN_POSTGRES_SERVED_READ_TABLES.has(table)

// ---------------------------------------------------------------------------
// Diagnostics (the drift metric)
// ---------------------------------------------------------------------------

export type BillingSyncDiagnosticEvent =
  | 'khala_sync_billing_dual_write_failed'
  | 'khala_sync_billing_read_compare_mismatch'
  | 'khala_sync_billing_postgres_read_failed'
  | 'khala_sync_billing_postgres_read_fallback'

export type BillingSyncDiagnostic = Readonly<{
  /** The mirrored operation, e.g. 'mirror:billing_ledger_entries'. */
  op: string
  /** Public-safe refs (table:key values — never amounts or payloads). */
  refs: ReadonlyArray<string>
  /** Public-safe failure summary (error class/message head, no SQL). */
  messageSafe: string
}>

export type BillingSyncLog = (
  event: BillingSyncDiagnosticEvent,
  fields: BillingSyncDiagnostic,
) => void

const safeMessage = (error: unknown): string => {
  const raw = error instanceof Error ? error.message : String(error)
  return raw.replaceAll(/\s+/g, ' ').slice(0, 200)
}

const defaultLog: BillingSyncLog = (event, fields) => {
  logWorkerRouteWarning(event, {
    messageSafe: fields.messageSafe,
    op: fields.op,
    refs: fields.refs.slice(0, 10).join(','),
  })
}

export class BillingUnsafeSqlUnavailableError extends Error {}

export class BillingInvalidMirrorKeyError extends Error {}

// ---------------------------------------------------------------------------
// Postgres billing store
// ---------------------------------------------------------------------------

export type PostgresBillingStore = Readonly<{
  /**
   * Converge one batch of D1 rows into a Postgres twin
   * (`ON CONFLICT (pk) DO UPDATE` to the D1 snapshot bytes).
   */
  upsertRows: (
    table: BillingDomainTable,
    rows: ReadonlyArray<Readonly<Record<string, unknown>>>,
  ) => Promise<void>
  /** The routed read: per-user balance = SUM(amount_cents), exact. */
  readBalanceCents: (userId: string) => Promise<number>
  /**
   * #8337: the recent-ledger-entries display projection, byte-identical
   * `ORDER BY`/`LIMIT` to the D1 path (`readD1RecentLedgerEntries`,
   * billing.ts).
   */
  readRecentLedgerEntryRows: (
    userId: string,
  ) => Promise<ReadonlyArray<BillingLedgerEntryRow>>
  /**
   * #8337: the auto-top-up DISPLAY-state row set (saved card, policy,
   * recent events), byte-identical filters/`LIMIT` to the D1 path
   * (`readD1BillingAutoTopUpState`, billing.ts).
   */
  readAutoTopUpStateRows: (userId: string) => Promise<
    Readonly<{
      paymentMethod: BillingSavedPaymentMethodRow | null
      policy: BillingAutoTopUpPolicyRow | null
      events: ReadonlyArray<BillingAutoTopUpEventRow>
    }>
  >
}>

export type MakePostgresBillingStoreDependencies = Readonly<{
  /**
   * Acquire a transaction-mode-safe SQL client (Hyperdrive in production,
   * a direct local URL in tests). One client per store operation; always
   * ended, even on error — the KS-8.1 discipline.
   */
  acquireSql: () => Promise<KhalaSyncPushSqlClient>
}>

type UnsafeQuery = (
  text: string,
  params: Array<unknown>,
) => Promise<Array<Record<string, unknown>>>

/** postgres.js (and Bun SQL) expose unsafe(text, params); SyncSql hides it. */
const requireUnsafe = (client: KhalaSyncPushSqlClient): UnsafeQuery => {
  const unsafe = (client.sql as unknown as { unsafe?: UnsafeQuery }).unsafe
  if (typeof unsafe !== 'function') {
    throw new BillingUnsafeSqlUnavailableError(
      'billing mirror requires a driver exposing unsafe(text, params)',
    )
  }
  return unsafe
}

/** The converge upsert text (columns/keys from the shared domain specs). */
const convergeUpsertSql = (
  table: BillingDomainTable,
  rowCount: number,
): string => {
  const spec = BILLING_DOMAIN_TABLE_SPECS[table]
  const columns = spec.columns
  const tuples: Array<string> = []
  for (let row = 0; row < rowCount; row++) {
    const placeholders = columns.map(
      (_, index) => `$${row * columns.length + index + 1}`,
    )
    tuples.push(`(${placeholders.join(', ')})`)
  }
  const setClauses = columns
    .filter(column => !spec.keyColumns.includes(column))
    .map(column => `${column} = EXCLUDED.${column}`)
    .join(', ')
  return `INSERT INTO ${table} (${columns.join(', ')}) VALUES ${tuples.join(', ')} ON CONFLICT (${spec.keyColumns.join(', ')}) DO UPDATE SET ${setClauses}`
}

/**
 * postgres.js returns `bigint` (int8) columns as JS `string` by default (to
 * avoid precision loss) while D1 returns them as JS `number`; `smallint`/
 * `int4` columns already parse as `number` on both sides. The #8337 bounded
 * read surfaces reconvert exactly the bigint-typed columns their row shapes
 * carry so a Postgres-served row is field-for-field identical in JS type to
 * the D1 row the same call site would otherwise have gotten.
 */
const toNullableNumber = (value: unknown): number | null =>
  value === null || value === undefined ? null : Number(value)

const ledgerEntryRowFromPostgres = (
  row: Readonly<Record<string, unknown>>,
): BillingLedgerEntryRow => ({
  amount_cents: Number(row['amount_cents']),
  created_at: String(row['created_at']),
  description: String(row['description']),
  id: String(row['id']),
  quantity: toNullableNumber(row['quantity']),
  source: row['source'] as BillingLedgerEntryRow['source'],
  unit: row['unit'] === null ? null : String(row['unit']),
})

const autoTopUpEventRowFromPostgres = (
  row: Readonly<Record<string, unknown>>,
): BillingAutoTopUpEventRow => ({
  amount_cents: Number(row['amount_cents']),
  created_at: String(row['created_at']),
  id: String(row['id']),
  reason: row['reason'] === null ? null : String(row['reason']),
  status: row['status'] as BillingAutoTopUpEventRow['status'],
})

const autoTopUpPolicyRowFromPostgres = (
  row: Readonly<Record<string, unknown>>,
): BillingAutoTopUpPolicyRow => ({
  amount_cents: Number(row['amount_cents']),
  enabled: Number(row['enabled']),
  monthly_cap_cents: Number(row['monthly_cap_cents']),
  pause_reason: row['pause_reason'] === null ? null : String(row['pause_reason']),
  spent_this_month_cents: Number(row['spent_this_month_cents']),
  status: row['status'] as BillingAutoTopUpPolicyRow['status'],
  threshold_cents: Number(row['threshold_cents']),
  updated_at: String(row['updated_at']),
})

const savedPaymentMethodRowFromPostgres = (
  row: Readonly<Record<string, unknown>>,
): BillingSavedPaymentMethodRow => ({
  brand: row['brand'] === null ? null : String(row['brand']),
  exp_month: toNullableNumber(row['exp_month']),
  exp_year: toNullableNumber(row['exp_year']),
  last4: row['last4'] === null ? null : String(row['last4']),
  status: row['status'] as BillingSavedPaymentMethodRow['status'],
  stripe_payment_method_id: String(row['stripe_payment_method_id']),
  updated_at: String(row['updated_at']),
})

export const makePostgresBillingStore = (
  deps: MakePostgresBillingStoreDependencies,
): PostgresBillingStore => {
  const withClient = async <A>(
    fn: (client: KhalaSyncPushSqlClient) => Promise<A>,
  ): Promise<A> => {
    const client = await deps.acquireSql()
    try {
      return await fn(client)
    } finally {
      try {
        await client.end()
      } catch {
        // best-effort teardown, same discipline as the push route.
      }
    }
  }

  return {
    readAutoTopUpStateRows: userId =>
      withClient(async client => {
        const unsafe = requireUnsafe(client)
        const [paymentMethodRows, policyRows, eventRows] = await Promise.all([
          unsafe(
            `SELECT stripe_payment_method_id, brand, last4, exp_month, exp_year,
                    status, updated_at
               FROM stripe_saved_payment_methods
              WHERE user_id = $1 AND currency = $2 AND livemode = 0`,
            [userId, BILLING_CURRENCY],
          ),
          unsafe(
            `SELECT enabled, threshold_cents, amount_cents, monthly_cap_cents,
                    spent_this_month_cents, status, pause_reason, updated_at
               FROM billing_auto_top_up_policies
              WHERE user_id = $1 AND currency = $2`,
            [userId, BILLING_CURRENCY],
          ),
          unsafe(
            `SELECT id, status, amount_cents, reason, created_at
               FROM billing_auto_top_up_events
              WHERE user_id = $1
              ORDER BY created_at DESC
              LIMIT $2`,
            [userId, BILLING_AUTO_TOP_UP_EVENTS_LIMIT],
          ),
        ])
        return {
          events: eventRows.map(autoTopUpEventRowFromPostgres),
          paymentMethod:
            paymentMethodRows[0] === undefined
              ? null
              : savedPaymentMethodRowFromPostgres(paymentMethodRows[0]),
          policy:
            policyRows[0] === undefined
              ? null
              : autoTopUpPolicyRowFromPostgres(policyRows[0]),
        }
      }),

    readBalanceCents: userId =>
      withClient(async client => {
        const unsafe = requireUnsafe(client)
        const rows = await unsafe(
          `SELECT COALESCE(SUM(amount_cents), 0) AS balance_cents
             FROM billing_ledger_entries
            WHERE user_id = $1`,
          [userId],
        )
        return Math.trunc(Number(rows[0]?.['balance_cents'] ?? 0))
      }),

    readRecentLedgerEntryRows: userId =>
      withClient(async client => {
        const unsafe = requireUnsafe(client)
        const rows = await unsafe(
          `SELECT id, source, description, amount_cents, quantity, unit, created_at
             FROM billing_ledger_entries
            WHERE user_id = $1
            ORDER BY created_at DESC
            LIMIT $2`,
          [userId, BILLING_RECENT_LEDGER_ENTRIES_LIMIT],
        )
        return rows.map(ledgerEntryRowFromPostgres)
      }),

    upsertRows: (table, rows) =>
      withClient(async client => {
        if (rows.length === 0) return
        const unsafe = requireUnsafe(client)
        const columns = BILLING_DOMAIN_TABLE_SPECS[table].columns
        const params: Array<unknown> = []
        for (const row of rows) {
          for (const column of columns) {
            params.push(normalizeBillingValue(row[column]))
          }
        }
        await unsafe(convergeUpsertSql(table, rows.length), params)
      }),
  }
}

// ---------------------------------------------------------------------------
// The dual-write mirror (D1 read-back → Postgres converge, fail-soft)
// ---------------------------------------------------------------------------

export type MakeBillingDomainMirrorDependencies = Readonly<{
  postgres: Pick<PostgresBillingStore, 'upsertRows'>
  log?: BillingSyncLog | undefined
}>

const refLabel = (ref: BillingMirrorRef): string =>
  `${ref.table}:${Object.values(ref.key).join(':')}`

/**
 * Build the production `BillingDomainMirror`: for each ref, SELECT the
 * fresh authoritative row(s) from D1 by key and converge-upsert the
 * byte-identical copy into Postgres. NEVER throws — any failure (D1
 * read-back, connection, upsert) logs `khala_sync_billing_dual_write_failed`
 * and returns; the D1 write stands and the backfill re-sweep converges the
 * missed row.
 */
export const makeBillingDomainMirror = (
  deps: MakeBillingDomainMirrorDependencies,
): BillingDomainMirror => {
  const log = deps.log ?? defaultLog

  return async (db, refs) => {
    for (const ref of refs) {
      try {
        const spec = BILLING_DOMAIN_TABLE_SPECS[ref.table]
        const keyEntries = Object.entries(ref.key)
        if (
          keyEntries.length === 0 ||
          keyEntries.some(([column]) => !spec.columns.includes(column))
        ) {
          throw new BillingInvalidMirrorKeyError(
            `invalid mirror key for ${ref.table}`,
          )
        }
        const where = keyEntries
          .map(([column]) => `${column} = ?`)
          .join(' AND ')
        const rows = await db
          .prepare(`SELECT * FROM ${ref.table} WHERE ${where}`)
          .bind(...keyEntries.map(([, value]) => value))
          .all<Record<string, unknown>>()
        if (rows.results.length === 0) {
          // Nothing to mirror (e.g. an UPDATE that matched no row) — not a
          // failure; D1 holds no row either.
          continue
        }
        await deps.postgres.upsertRows(ref.table, rows.results)
      } catch (error) {
        log('khala_sync_billing_dual_write_failed', {
          messageSafe: safeMessage(error),
          op: `mirror:${ref.table}`,
          refs: [refLabel(ref)],
        })
      }
    }
  }
}

// ---------------------------------------------------------------------------
// Routed balance read (d1 | compare | postgres with bounded retry + fallback)
// ---------------------------------------------------------------------------

const READ_RETRY_DELAYS_MS: ReadonlyArray<number> = [50, 150]

export type MakeRoutedBillingBalanceReadDependencies = Readonly<{
  postgres: Pick<PostgresBillingStore, 'readBalanceCents'>
  reads: Exclude<BillingSyncReadsMode, 'd1'>
  log?: BillingSyncLog | undefined
  /** Bounded-retry backoff hook (tests inject a no-op). */
  wait?: ((ms: number) => Promise<void>) | undefined
  /** Compare-mode soak observability (#8282 shared follow-up). No-op recorder by default. */
  metrics?: CompareSoakMetrics | undefined
}>

/**
 * The KHALA_SYNC_BILLING_READS router for the balance SUM:
 *   compare  — read both, SERVE D1 (authority), log any cent of divergence
 *   postgres — serve Postgres with bounded retry (50/150ms), D1 fallback
 * Wired ONLY where a call site opts in via
 * `billingRuntimeForEnv(env, { routeReads: true })`.
 */
export const makeRoutedBillingBalanceRead = (
  deps: MakeRoutedBillingBalanceReadDependencies,
): BillingBalanceRead => {
  const log = deps.log ?? defaultLog
  const metrics = deps.metrics ?? noopCompareSoakMetrics
  const wait =
    deps.wait ??
    ((ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms)))

  if (deps.reads === 'compare') {
    return async (userId, readD1) => {
      const d1Balance = await readD1()
      try {
        const postgresBalance = await deps.postgres.readBalanceCents(userId)
        if (postgresBalance !== d1Balance) {
          log('khala_sync_billing_read_compare_mismatch', {
            messageSafe: `balance differs: d1=${d1Balance} postgres=${postgresBalance}`,
            op: 'readBalanceCents',
            refs: [userId],
          })
          metrics.record({ domain: 'billing', outcome: 'mismatch', readKind: 'readBalanceCents' })
        } else {
          metrics.record({ domain: 'billing', outcome: 'match', readKind: 'readBalanceCents' })
        }
      } catch (error) {
        log('khala_sync_billing_postgres_read_failed', {
          messageSafe: safeMessage(error),
          op: 'readBalanceCents',
          refs: [userId],
        })
        metrics.record({ domain: 'billing', outcome: 'error', readKind: 'readBalanceCents' })
      }
      return d1Balance
    }
  }

  return async (userId, readD1) => {
    for (let attempt = 0; ; attempt++) {
      try {
        return await deps.postgres.readBalanceCents(userId)
      } catch (error) {
        const delay = READ_RETRY_DELAYS_MS[attempt]
        if (delay === undefined) {
          log('khala_sync_billing_postgres_read_fallback', {
            messageSafe: safeMessage(error),
            op: 'readBalanceCents',
            refs: [userId],
          })
          return readD1()
        }
        log('khala_sync_billing_postgres_read_failed', {
          messageSafe: safeMessage(error),
          op: 'readBalanceCents',
          refs: [userId],
        })
        await wait(delay)
      }
    }
  }
}

// ---------------------------------------------------------------------------
// #8337 routed reads (compare | postgres, single attempt, fail-soft)
// ---------------------------------------------------------------------------

/**
 * A key-order-INSENSITIVE structural stringify: object keys are sorted
 * before serializing (arrays keep their order — order is semantically
 * meaningful for `ORDER BY`-produced lists) so two field-for-field-equal
 * values compare equal even if the two code paths built their object
 * literals with keys in a different order. Plain `JSON.stringify` equality
 * would be a false-positive-drift trap: it is only safe if D1 and Postgres
 * ALWAYS construct their result objects via the exact same shared
 * projection function (they do today — `billingLedgerEntryFromRow`,
 * `billingAutoTopUpStateFromRows` — but a future refactor of either path
 * alone should never manufacture a spurious
 * `khala_sync_billing_read_compare_mismatch`).
 */
const stableStringify = (value: unknown): string => {
  if (Array.isArray(value)) {
    return `[${value.map(stableStringify).join(',')}]`
  }
  if (value !== null && typeof value === 'object') {
    const entries = Object.entries(value as Record<string, unknown>).sort(
      ([left], [right]) => (left < right ? -1 : left > right ? 1 : 0),
    )
    return `{${entries.map(([key, entryValue]) => `${JSON.stringify(key)}:${stableStringify(entryValue)}`).join(',')}}`
  }
  return JSON.stringify(value)
}

const deepEqualJson = (left: unknown, right: unknown): boolean =>
  stableStringify(left) === stableStringify(right)

export type MakeRoutedBillingRecentEntriesReadDependencies = Readonly<{
  postgres: Pick<PostgresBillingStore, 'readRecentLedgerEntryRows'>
  reads: Exclude<BillingSyncReadsMode, 'd1'>
  log?: BillingSyncLog | undefined
  /** Compare-mode soak observability (#8282 shared follow-up). No-op recorder by default. */
  metrics?: CompareSoakMetrics | undefined
}>

/**
 * The #8337 KHALA_SYNC_BILLING_READS router for the recent-entries display
 * projection (`billing_ledger_entries`, allowlisted — see
 * `BILLING_DOMAIN_POSTGRES_SERVED_READ_TABLES`):
 *   compare  — read both, SERVE D1, log any divergence
 *   postgres — serve Postgres (single attempt), D1 fallback on failure
 * Unlike the balance read, this is wired unconditionally by
 * `billingRuntimeForEnv` whenever `reads !== 'd1'` — there is no separate
 * opt-in, because only the display summary path ever calls this hook.
 */
export const makeRoutedBillingRecentEntriesRead = (
  deps: MakeRoutedBillingRecentEntriesReadDependencies,
): BillingRecentEntriesRead => {
  const log = deps.log ?? defaultLog
  const metrics = deps.metrics ?? noopCompareSoakMetrics
  const readPostgres = async (
    userId: string,
  ): Promise<ReadonlyArray<BillingLedgerEntry>> =>
    (await deps.postgres.readRecentLedgerEntryRows(userId)).map(
      billingLedgerEntryFromRow,
    )

  if (deps.reads === 'compare') {
    return async (userId, readD1) => {
      const d1Entries = await readD1()
      try {
        const postgresEntries = await readPostgres(userId)
        if (!deepEqualJson(d1Entries, postgresEntries)) {
          log('khala_sync_billing_read_compare_mismatch', {
            messageSafe: `recent ledger entries differ: d1=${d1Entries.length} postgres=${postgresEntries.length} rows`,
            op: 'readRecentLedgerEntries',
            refs: [userId],
          })
          metrics.record({ domain: 'billing', outcome: 'mismatch', readKind: 'readRecentLedgerEntries' })
        } else {
          metrics.record({ domain: 'billing', outcome: 'match', readKind: 'readRecentLedgerEntries' })
        }
      } catch (error) {
        log('khala_sync_billing_postgres_read_failed', {
          messageSafe: safeMessage(error),
          op: 'readRecentLedgerEntries',
          refs: [userId],
        })
        metrics.record({ domain: 'billing', outcome: 'error', readKind: 'readRecentLedgerEntries' })
      }
      return d1Entries
    }
  }

  return async (userId, readD1) => {
    try {
      return await readPostgres(userId)
    } catch (error) {
      log('khala_sync_billing_postgres_read_fallback', {
        messageSafe: safeMessage(error),
        op: 'readRecentLedgerEntries',
        refs: [userId],
      })
      return readD1()
    }
  }
}

export type MakeRoutedBillingAutoTopUpStateReadDependencies = Readonly<{
  postgres: Pick<PostgresBillingStore, 'readAutoTopUpStateRows'>
  reads: Exclude<BillingSyncReadsMode, 'd1'>
  log?: BillingSyncLog | undefined
  runtime?: BillingRuntime | undefined
  /** Compare-mode soak observability (#8282 shared follow-up). No-op recorder by default. */
  metrics?: CompareSoakMetrics | undefined
}>

/**
 * The #8337 KHALA_SYNC_BILLING_READS router for the auto-top-up DISPLAY
 * state (allowlisted — see `BILLING_DOMAIN_POSTGRES_SERVED_READ_TABLES`).
 * NEVER wire this into the auto-top-up charge decision (`chargeAutoTopUp`,
 * stripe-billing.ts) — that evaluator always reads its own dedicated D1
 * query directly and takes no runtime hook.
 */
export const makeRoutedBillingAutoTopUpStateRead = (
  deps: MakeRoutedBillingAutoTopUpStateReadDependencies,
): BillingAutoTopUpStateRead => {
  const log = deps.log ?? defaultLog
  const metrics = deps.metrics ?? noopCompareSoakMetrics
  const runtime = deps.runtime ?? systemBillingRuntime
  const readPostgres = async (
    userId: string,
  ): Promise<BillingAutoTopUpState> =>
    billingAutoTopUpStateFromRows(
      await deps.postgres.readAutoTopUpStateRows(userId),
      runtime,
    )

  if (deps.reads === 'compare') {
    return async (userId, readD1) => {
      const d1State = await readD1()
      try {
        const postgresState = await readPostgres(userId)
        if (!deepEqualJson(d1State, postgresState)) {
          log('khala_sync_billing_read_compare_mismatch', {
            messageSafe: 'auto-top-up display state differs between d1 and postgres',
            op: 'readBillingAutoTopUpState',
            refs: [userId],
          })
          metrics.record({ domain: 'billing', outcome: 'mismatch', readKind: 'readBillingAutoTopUpState' })
        } else {
          metrics.record({ domain: 'billing', outcome: 'match', readKind: 'readBillingAutoTopUpState' })
        }
      } catch (error) {
        log('khala_sync_billing_postgres_read_failed', {
          messageSafe: safeMessage(error),
          op: 'readBillingAutoTopUpState',
          refs: [userId],
        })
        metrics.record({ domain: 'billing', outcome: 'error', readKind: 'readBillingAutoTopUpState' })
      }
      return d1State
    }
  }

  return async (userId, readD1) => {
    try {
      return await readPostgres(userId)
    } catch (error) {
      log('khala_sync_billing_postgres_read_fallback', {
        messageSafe: safeMessage(error),
        op: 'readBillingAutoTopUpState',
        refs: [userId],
      })
      return readD1()
    }
  }
}

// ---------------------------------------------------------------------------
// Env factories (the call-site drop-ins)
// ---------------------------------------------------------------------------

/**
 * The env slice billing call sites need. All fields optional so any route
 * input that carries the whole Worker env satisfies it structurally.
 */
export type BillingSyncEnv = BillingSyncFlagEnv &
  Readonly<{
    KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
  }>

export type MakeBillingStoreOptions = Readonly<{
  /** Injectable client factory (tests). Default: postgres.js/Hyperdrive. */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  log?: BillingSyncLog | undefined
  /** Compare-mode soak metrics override (tests inject a collector). */
  metrics?: CompareSoakMetrics | undefined
}>

export const postgresBillingStoreForEnv = (
  env: BillingSyncEnv,
  options: MakeBillingStoreOptions = {},
): PostgresBillingStore | undefined => {
  const connectionString = env.KHALA_SYNC_DB?.connectionString
  if (connectionString === undefined || connectionString.length === 0) {
    return undefined
  }
  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  return makePostgresBillingStore({
    acquireSql: () => makeSqlClient(connectionString),
  })
}

/**
 * A single unsafe-SQL round trip (one connection acquired and ended per
 * call, the same discipline as every other billing Postgres read). Used by
 * the two #8337 standalone allowlisted read stores that live outside
 * `BillingRuntime` — `stripe-checkout-receipts.ts` and
 * `inference-receipts.ts` — so they can build their own narrow,
 * hand-audited Postgres query without depending on `PostgresBillingStore`'s
 * wider surface.
 */
export type BillingPostgresRawQuery = (
  text: string,
  params: ReadonlyArray<unknown>,
) => Promise<ReadonlyArray<Readonly<Record<string, unknown>>>>

/**
 * The #8337 raw-query factory for env-wired call sites, or `undefined` when
 * the KHALA_SYNC_DB binding is absent — the same degrade-to-D1-only
 * posture as every other billing Postgres factory in this file.
 */
export const billingPostgresRawQueryForEnv = (
  env: BillingSyncEnv,
  options: MakeBillingStoreOptions = {},
): BillingPostgresRawQuery | undefined => {
  const connectionString = env.KHALA_SYNC_DB?.connectionString
  if (connectionString === undefined || connectionString.length === 0) {
    return undefined
  }
  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  return async (text, params) => {
    const client = await makeSqlClient(connectionString)
    try {
      const unsafe = requireUnsafe(client)
      return await unsafe(text, [...params])
    } finally {
      try {
        await client.end()
      } catch {
        // best-effort teardown, same discipline as the push route.
      }
    }
  }
}

/**
 * The production fail-soft mirror for this env, or undefined when the
 * binding is absent or KHALA_SYNC_BILLING_DUAL_WRITE is off. Modules that
 * do not take a `BillingRuntime` (stripe-billing, pay-ins, buyer ledger,
 * paid-plan intents, …) thread this value directly.
 */
export const billingDomainMirrorFromEnv = (
  env: BillingSyncEnv,
  options: MakeBillingStoreOptions = {},
): BillingDomainMirror | undefined => {
  const flags = billingSyncFlagsFromEnv(env)
  if (!flags.dualWrite) {
    return undefined
  }
  const postgres = postgresBillingStoreForEnv(env, options)
  if (postgres === undefined) {
    return undefined
  }
  return makeBillingDomainMirror({
    log: options.log ?? defaultLog,
    postgres,
  })
}

export type BillingRuntimeForEnvOptions = MakeBillingStoreOptions &
  Readonly<{
    /**
     * Opt IN to KHALA_SYNC_BILLING_READS routing for the balance read.
     * Only display/summary call sites pass true; gates, evaluators, and
     * receipt inputs must never route (they take the default false and
     * read D1).
     */
    routeReads?: boolean | undefined
  }>

/**
 * The `BillingRuntime` drop-in for Worker call sites (replaces the default
 * `systemBillingRuntime` argument): system clock/ids plus the KS-8.7
 * fail-soft Postgres mirror, and — only where `routeReads: true` — the
 * flag-routed balance read. Degrades to plain `systemBillingRuntime` when
 * the binding is absent or every flag is off.
 */
export const billingRuntimeForEnv = (
  env: BillingSyncEnv,
  options: BillingRuntimeForEnvOptions = {},
): BillingRuntime => {
  const flags = billingSyncFlagsFromEnv(env)
  const postgres = postgresBillingStoreForEnv(env, options)
  if (postgres === undefined) {
    return systemBillingRuntime
  }
  const log = options.log ?? defaultLog
  // The durable Analytics Engine soak sink was removed with the account-level
  // Analytics Engine feature (#8516); the default recorder is a no-op and the
  // per-call compare-mismatch diagnostics are unaffected.
  const metrics = options.metrics ?? noopCompareSoakMetrics

  const mirror = flags.dualWrite
    ? makeBillingDomainMirror({ log, postgres })
    : undefined
  const balanceRead =
    options.routeReads === true && flags.reads !== 'd1'
      ? makeRoutedBillingBalanceRead({ log, metrics, postgres, reads: flags.reads })
      : undefined
  // #8337: unlike `balanceRead`, these two are wired unconditionally
  // whenever reads !== 'd1' — no separate `routeReads`-style opt-in is
  // needed because only the display summary path (`readBillingSummary`,
  // billing.ts) ever calls either hook; nothing decision-critical takes a
  // `BillingRuntime` with these fields set.
  const recentEntriesRead =
    flags.reads === 'd1'
      ? undefined
      : makeRoutedBillingRecentEntriesRead({ log, metrics, postgres, reads: flags.reads })
  const autoTopUpStateRead =
    flags.reads === 'd1'
      ? undefined
      : makeRoutedBillingAutoTopUpStateRead({ log, metrics, postgres, reads: flags.reads })

  if (
    mirror === undefined &&
    balanceRead === undefined &&
    recentEntriesRead === undefined &&
    autoTopUpStateRead === undefined
  ) {
    return systemBillingRuntime
  }

  return {
    ...systemBillingRuntime,
    ...(mirror === undefined ? {} : { mirror }),
    ...(balanceRead === undefined ? {} : { balanceRead }),
    ...(recentEntriesRead === undefined ? {} : { recentEntriesRead }),
    ...(autoTopUpStateRead === undefined ? {} : { autoTopUpStateRead }),
  }
}
