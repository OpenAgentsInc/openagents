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
//   3. READS STAY ON D1. `KHALA_SYNC_BILLING_READS` routes exactly ONE
//      read — the per-user balance SUM behind `readBillingSummary` — and
//      only where a call site explicitly opts in with
//      `billingRuntimeForEnv(env, { routeReads: true })` (the display
//      summary path). Gates, evaluators, and receipt inputs always read
//      D1. Flipping the flag to `postgres` in production is an EPIC-GATED
//      ops decision (#8282) taken only after the backfill `--verify`
//      (exact per-account balance equality) is green; see
//      docs/khala-sync/RUNBOOK.md "Billing domain cutover".
//
// Flags:
//   KHALA_SYNC_BILLING_DUAL_WRITE  (default ON; 'off'|'0'|'false'|'disabled')
//   KHALA_SYNC_BILLING_READS       (default 'd1'; 'd1'|'compare'|'postgres')
// With no KHALA_SYNC_DB binding everything degrades to plain D1.
//
// Cutover order (docs/khala-sync/RUNBOOK.md "Billing domain cutover"):
// dual-write on → backfill (scripts/backfill-billing.ts) → catch-up sweep
// → --verify (money reconciliation) → compare reads → [EPIC-GATED]
// postgres reads → decommission D1 tables in a follow-up.

import {
  BILLING_DOMAIN_TABLE_SPECS,
  normalizeBillingValue,
  type BillingDomainTable,
} from '@openagentsinc/khala-sync-server'

import type {
  BillingBalanceRead,
  BillingDomainMirror,
  BillingMirrorRef,
  BillingRuntime,
} from './billing'
import { systemBillingRuntime } from './billing'
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
    throw new Error(
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
          throw new Error(`invalid mirror key for ${ref.table}`)
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
        }
      } catch (error) {
        log('khala_sync_billing_postgres_read_failed', {
          messageSafe: safeMessage(error),
          op: 'readBalanceCents',
          refs: [userId],
        })
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

  const mirror = flags.dualWrite
    ? makeBillingDomainMirror({ log, postgres })
    : undefined
  const balanceRead =
    options.routeReads === true && flags.reads !== 'd1'
      ? makeRoutedBillingBalanceRead({ log, postgres, reads: flags.reads })
      : undefined

  if (mirror === undefined && balanceRead === undefined) {
    return systemBillingRuntime
  }

  return {
    ...systemBillingRuntime,
    ...(mirror === undefined ? {} : { mirror }),
    ...(balanceRead === undefined ? {} : { balanceRead }),
  }
}
