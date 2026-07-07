// CFG-4 (#8519) Domain 3: a D1Database-SHAPED handle backed by Cloud SQL
// Postgres, for the Khala Code product-state tables (khala-sync migration
// 0017). It lets the ~19 existing store factories that speak the D1 API
// (`prepare(sql).bind(...).first()/.all()/.run()`, `batch()`) run UNCHANGED
// against Postgres — the hard cut off D1 without rewriting every consumer.
//
// Only the D1 surface those consumers actually use is implemented: prepared
// statements with `first`/`all`/`run`, and `batch` (ONE Postgres
// transaction). `raw`/`dump`/`withSession` are not used by the product-state
// consumers and throw if called.
//
// Dialect translation (the product-state SQL is otherwise Postgres-portable —
// no datetime()/strftime()/json_extract, `||` and COALESCE are native):
//   - `?`  -> `$n`  (outside string literals; via translateLedgerPlaceholders)
//   - `col IS ?`     -> `col IS NOT DISTINCT FROM ?`  (null-safe equality; the
//                       store's own read-back uses `IS ?`)
//   - `INSERT OR IGNORE`  -> `INSERT … ON CONFLICT DO NOTHING`
//   - `INSERT OR REPLACE` -> unsupported (none on the 25 tables) — throws
// int8 columns are parsed back as JS numbers (the 11 bigint twin columns are
// all semantically < 2^53: credits msat, counts, text-ISO timestamps), so a
// Postgres-served row is field-for-field the shape D1 returned.

import { translateLedgerPlaceholders } from './payments-ledger-db'

type UnsafeQuery = (
  text: string,
  params: Array<unknown>,
) => Promise<Array<Record<string, unknown>> & { count?: number }>

/** A Postgres client handle the adapter runs on, plus its teardown. */
export type PostgresD1Client = Readonly<{
  /** Structural postgres.js/Bun-SQL surface exposing `unsafe(text, params)`
   * and `begin(fn)` for transactions. */
  sql: {
    unsafe: UnsafeQuery
    begin: <A>(fn: (tx: { unsafe: UnsafeQuery }) => Promise<A>) => Promise<A>
  }
  end: () => Promise<void>
}>

export type MakePostgresD1DatabaseDependencies = Readonly<{
  /** Acquire a client. One per D1 operation; always ended (batch shares one
   * across its transaction). */
  acquireSql: () => Promise<PostgresD1Client>
}>

export class PostgresD1AdapterError extends Error {
  override readonly name = 'PostgresD1AdapterError'
}

// ---------------------------------------------------------------------------
// SQL translation
// ---------------------------------------------------------------------------

const IS_PLACEHOLDER_RE = /\bIS\s+\?/gi

/** Rewrite SQLite constructs the product-state consumers emit into Postgres,
 * then translate `?` placeholders to `$n`. Exported for the contract test. */
export const translateProductStateSql = (sql: string): string => {
  let out = sql

  // Null-safe equality: `col IS ?` (never `IS NULL` / `IS NOT ?`).
  out = out.replaceAll(IS_PLACEHOLDER_RE, 'IS NOT DISTINCT FROM ?')

  // INSERT OR IGNORE -> ON CONFLICT DO NOTHING. INSERT OR REPLACE is not used
  // on the 25 product-state tables; refuse it loudly rather than silently
  // mistranslate its overwrite semantics.
  const orMatch = /\binsert\s+or\s+(ignore|replace)\b/i.exec(out)
  if (orMatch !== undefined && orMatch !== null) {
    if (orMatch[1]!.toLowerCase() === 'replace') {
      throw new PostgresD1AdapterError(
        'INSERT OR REPLACE is not supported by the product-state Postgres adapter',
      )
    }
    out = out.replace(/\binsert\s+or\s+ignore\b/i, 'INSERT')
    if (!/\bon\s+conflict\b/i.test(out)) {
      const trimmed = out.replace(/;\s*$/, '')
      out = `${trimmed} ON CONFLICT DO NOTHING`
    }
  }

  return translateLedgerPlaceholders(out)
}

// ---------------------------------------------------------------------------
// D1 result shapes
// ---------------------------------------------------------------------------

type D1ResultLike<T> = Readonly<{
  results: Array<T>
  success: true
  meta: Readonly<{ changes: number; duration: number; last_row_id: number }>
}>

const resultFromRows = <T>(
  rows: Array<Record<string, unknown>> & { count?: number },
): D1ResultLike<T> => ({
  meta: {
    changes: typeof rows.count === 'number' ? rows.count : rows.length,
    duration: 0,
    last_row_id: 0,
  },
  results: rows as unknown as Array<T>,
  success: true,
})

// ---------------------------------------------------------------------------
// Adapter
// ---------------------------------------------------------------------------

type BoundStatement = Readonly<{ sql: string; params: ReadonlyArray<unknown> }>

export const makePostgresD1Database = (
  deps: MakePostgresD1DatabaseDependencies,
): D1Database => {
  const withClient = async <A>(
    fn: (client: PostgresD1Client) => Promise<A>,
  ): Promise<A> => {
    const client = await deps.acquireSql()
    try {
      return await fn(client)
    } finally {
      try {
        await client.end()
      } catch {
        // best-effort teardown, same discipline as the Khala Sync route stores.
      }
    }
  }

  const runOne = async <T>(
    unsafe: UnsafeQuery,
    bound: BoundStatement,
  ): Promise<D1ResultLike<T>> => {
    const rows = await unsafe(translateProductStateSql(bound.sql), [
      ...bound.params,
    ])
    return resultFromRows<T>(rows)
  }

  const makeStatement = (bound: BoundStatement): D1PreparedStatement => {
    const statement = {
      bind: (...values: ReadonlyArray<unknown>) =>
        makeStatement({ params: values, sql: bound.sql }),
      first: async <T>(column?: string): Promise<T | null> => {
        const result = await withClient(client =>
          runOne<Record<string, unknown>>(client.sql.unsafe, bound),
        )
        const row = result.results[0]
        if (row === undefined) return null
        return (column === undefined ? row : row[column]) as T
      },
      all: <T>(): Promise<D1ResultLike<T>> =>
        withClient(client => runOne<T>(client.sql.unsafe, bound)),
      run: <T>(): Promise<D1ResultLike<T>> =>
        withClient(client => runOne<T>(client.sql.unsafe, bound)),
      raw: (): Promise<never> => {
        throw new PostgresD1AdapterError(
          'raw() is not supported by the product-state Postgres adapter',
        )
      },
      // Carried so batch() can re-run the bound statement inside its own
      // transaction client instead of the statement's own per-op client.
      __postgresD1Bound: bound,
    }
    return statement as unknown as D1PreparedStatement
  }

  const proxied = {
    prepare: (sql: string) => makeStatement({ params: [], sql }),
    batch: async <T>(
      statements: ReadonlyArray<D1PreparedStatement>,
    ): Promise<Array<D1ResultLike<T>>> =>
      withClient(client =>
        client.sql.begin(async tx => {
          const results: Array<D1ResultLike<T>> = []
          for (const statement of statements) {
            const bound = (
              statement as unknown as { __postgresD1Bound?: BoundStatement }
            ).__postgresD1Bound
            if (bound === undefined) {
              throw new PostgresD1AdapterError(
                'batch() received a statement not created by this adapter',
              )
            }
            results.push(await runOne<T>(tx.unsafe, bound))
          }
          return results
        }),
      ),
    exec: async (sql: string) => {
      await withClient(client => client.sql.unsafe(translateProductStateSql(sql), []))
      return { count: 0, duration: 0 }
    },
    dump: (): Promise<never> => {
      throw new PostgresD1AdapterError(
        'dump() is not supported by the product-state Postgres adapter',
      )
    },
    withSession: (): D1Database => proxied as unknown as D1Database,
  }

  return proxied as unknown as D1Database
}
