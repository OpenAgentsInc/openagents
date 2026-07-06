// CFG-4 (#8519, epic #8515): the Postgres payments-ledger executor — the
// HARD-CUT replacement for the D1 credits authority.
//
// The credits domain (`pay_ins`, `pay_in_legs`, `agent_balances`, plus the
// labor-escrow rows that ride the same atomic batches) is Cloud SQL
// Postgres-authoritative. Every ledger batch built by `payments-ledger.ts`
// (and the statement builders in `labor-escrow.ts`, `tip-ladder.ts`,
// `tips-sweep.ts`, the inference grant/charge paths, …) executes here as ONE
// Postgres transaction through the `KHALA_SYNC_DB` Hyperdrive binding —
// exactly the same driver discipline as `POST /api/sync/push`
// (`defaultMakeKhalaSyncSqlClient`: postgres.js, `prepare: false`, `max: 1`,
// no session state, always ended).
//
// D1 is GONE for this domain: there is no dual-write, no mirror, no read
// flag. A Postgres outage fails the money write loudly (fail-hard — a
// ledger write must never silently succeed against a store nobody reads).
//
// Statement portability: the ledger builders keep D1-era `?` placeholders;
// `translateLedgerPlaceholders` rewrites them to `$1..$n` outside string
// literals/quoted identifiers. The builders' SQL is deliberately
// dialect-portable (no `datetime('now')`, no `INSERT OR IGNORE` — guarded by
// the test adapter in `test/payments-ledger-sqlite.ts` and by the real
// Postgres contract suite in `payments-ledger-postgres.contract.test.ts`).

import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type KhalaSyncPushSqlClient,
  type MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'

export type LedgerParam = string | number | null

export type LedgerSqlStatement = Readonly<{
  sql: string
  params: ReadonlyArray<LedgerParam>
}>

export type LedgerRow = Readonly<Record<string, unknown>>

/**
 * The credits-domain database handle. Postgres-only in production
 * (`paymentsLedgerDbForEnv`); tests may back it with the SQLite adapter in
 * `test/payments-ledger-sqlite.ts` for fast behavioral coverage, with the
 * Postgres contract suite proving dialect equivalence.
 */
export type PaymentsLedgerDb = Readonly<{
  /**
   * Execute the statements as ONE atomic transaction (BEGIN…COMMIT; all
   * or nothing). A CHECK-constraint violation (e.g. an over-debit against
   * `agent_balances.balance_msat >= 0`) or a UNIQUE conflict (idempotency
   * replay) aborts the whole transaction — the same atomic-failure
   * semantics the D1 batch had.
   */
  batch: (statements: ReadonlyArray<LedgerSqlStatement>) => Promise<void>
  /** Run one read statement (D1-era `?` placeholders) and return its rows. */
  query: (
    sql: string,
    params?: ReadonlyArray<LedgerParam>,
  ) => Promise<Array<LedgerRow>>
}>

export class PaymentsLedgerUnavailableError extends Error {
  override readonly name = 'PaymentsLedgerUnavailableError'
}

/**
 * Rewrite D1-style `?` placeholders to Postgres `$1..$n`, skipping `?`
 * characters inside single-quoted string literals and double-quoted
 * identifiers.
 */
export const translateLedgerPlaceholders = (sql: string): string => {
  let out = ''
  let next = 0
  let inSingle = false
  let inDouble = false
  for (const ch of sql) {
    if (inSingle) {
      out += ch
      if (ch === "'") inSingle = false
      continue
    }
    if (inDouble) {
      out += ch
      if (ch === '"') inDouble = false
      continue
    }
    if (ch === "'") {
      inSingle = true
      out += ch
      continue
    }
    if (ch === '"') {
      inDouble = true
      out += ch
      continue
    }
    if (ch === '?') {
      next += 1
      out += `$${next}`
      continue
    }
    out += ch
  }
  return out
}

type UnsafeQuery = (
  text: string,
  params: Array<unknown>,
) => Promise<Array<Record<string, unknown>>>

/** postgres.js and Bun SQL both expose `unsafe(text, params)`; the typed
 * `SyncSql` seam hides it, so reach it structurally (same discipline as
 * `billing-store.ts`). */
const requireUnsafe = (handle: unknown): UnsafeQuery => {
  const unsafe = (handle as { unsafe?: UnsafeQuery }).unsafe
  if (typeof unsafe !== 'function') {
    throw new PaymentsLedgerUnavailableError(
      'payments ledger requires a SQL driver exposing unsafe(text, params)',
    )
  }
  return unsafe
}

export type MakePostgresPaymentsLedgerDbDependencies = Readonly<{
  /**
   * Acquire a transaction-mode-safe SQL client (Hyperdrive in production,
   * a direct local URL in tests). One client per ledger operation; always
   * ended, even on error.
   */
  acquireSql: () => Promise<KhalaSyncPushSqlClient>
}>

export const makePostgresPaymentsLedgerDb = (
  deps: MakePostgresPaymentsLedgerDbDependencies,
): PaymentsLedgerDb => {
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
        // best-effort teardown — the operation's own outcome already
        // propagated.
      }
    }
  }

  return {
    batch: statements =>
      withClient(async client => {
        if (statements.length === 0) return
        await client.sql.begin(async tx => {
          const unsafe = requireUnsafe(tx)
          for (const statement of statements) {
            await unsafe(translateLedgerPlaceholders(statement.sql), [
              ...statement.params,
            ])
          }
        })
      }),
    query: (sql, params = []) =>
      withClient(async client => {
        const unsafe = requireUnsafe(client.sql)
        return unsafe(translateLedgerPlaceholders(sql), [...params])
      }),
  }
}

export type PaymentsLedgerEnv = Readonly<{
  KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding
}>

/**
 * The production wiring: the credits ledger over the `KHALA_SYNC_DB`
 * Hyperdrive binding. FAIL-HARD AT USE when the binding is absent — a money
 * operation has no fallback store anymore. Construction itself never throws
 * (deps objects are built per request for many routes; a binding-less
 * environment must only fail the paths that actually touch the ledger).
 */
export const paymentsLedgerDbForEnv = (
  env: PaymentsLedgerEnv,
  makeSqlClient?: MakeKhalaSyncPushSqlClient,
): PaymentsLedgerDb => {
  const binding = env.KHALA_SYNC_DB
  if (binding === undefined) {
    const refuse = (): never => {
      throw new PaymentsLedgerUnavailableError(
        'KHALA_SYNC_DB binding is required for the credits ledger (CFG-4 hard cutover)',
      )
    }
    return { batch: async () => refuse(), query: async () => refuse() }
  }
  const make = makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  return makePostgresPaymentsLedgerDb({
    acquireSql: () => make(binding.connectionString),
  })
}

// ---------------------------------------------------------------------------
// Typed constraint-failure classification (dialect-neutral).
//
// D1/SQLite raised `D1_ERROR: … CHECK constraint failed …` /
// `UNIQUE constraint failed`; Postgres raises SQLSTATE 23514 / 23505. Call
// sites that branch on "insufficient balance" or "idempotency replay" use
// these helpers instead of matching driver-specific message text.
// ---------------------------------------------------------------------------

const errorText = (error: unknown): string =>
  error instanceof Error ? `${error.message}` : String(error)

const errorCode = (error: unknown): string | undefined => {
  const code = (error as { code?: unknown } | null)?.code
  return typeof code === 'string' ? code : undefined
}

/** CHECK-constraint violation — e.g. an over-debit hitting
 * `agent_balances.balance_msat >= 0` (insufficient funds). */
export const isLedgerCheckConstraintError = (error: unknown): boolean =>
  errorCode(error) === '23514' || /CHECK constraint/i.test(errorText(error))

/** UNIQUE violation — e.g. an idempotency-key replay. */
export const isLedgerUniqueConstraintError = (error: unknown): boolean =>
  errorCode(error) === '23505' || /UNIQUE constraint/i.test(errorText(error))
