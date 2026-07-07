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
// Dialect translation. The Khala Code product-state SQL is Postgres-portable
// on its own (no SQLite functions; `||` and COALESCE are native), but #8515
// widened this adapter to the money + dialect-heavy domains (treasury /
// artanis / business database-shaped handles, forum money paths), whose raw
// SQL DOES use SQLite scalar functions. The translator now covers:
//   - `?`  -> `$n`  (outside string literals; via translateLedgerPlaceholders)
//   - `col IS ?`     -> `col IS NOT DISTINCT FROM ?`  (null-safe equality; the
//                       store's own read-back uses `IS ?`)
//   - `INSERT OR IGNORE`  -> `INSERT … ON CONFLICT DO NOTHING`
//   - `INSERT OR REPLACE` -> `INSERT … <explicit ON CONFLICT (pk) DO UPDATE>`
//                       (the PK can't be inferred from SQL text with no schema,
//                       so the call site MUST carry the ON CONFLICT target; a
//                       bare `INSERT OR REPLACE` still throws loudly)
//   - `json_extract(col, '$.a.b')` -> `(col)::jsonb #>> '{a,b}'` (nested) or
//                       `(col)::jsonb ->> 'a'` (top-level key). JSON scalars
//                       come back as TEXT (Postgres `->>`/`#>>` semantics), so
//                       call sites that compared a JSON boolean/number must
//                       compare against the text form.
//   - `datetime('now')` -> `now()`
//   - `julianday(x)`    -> `(extract(epoch from (x)::timestamptz)/86400.0
//                       + 2440587.5)` — the Julian Day Number; the +2440587.5
//                       offset cancels in `julianday(a) - julianday(b)` day
//                       differences and is exact for absolute values too.
//   - `strftime('%s', x)` -> `floor(extract(epoch from (x)::timestamptz))
//                       ::bigint` (unix seconds).
// Deliberately NOT translated (they fail loud at Postgres — fail-closed, never
// mistranslated): `strftime` with a non-`%s` format, and `datetime`/`date`
// with relative modifiers (`'start of month'`, `'+1 month'`, …). The one
// consumer that needs those (business-factory-metrics.ts, a recursive
// month-window READ that also uses `?1/?2` numbered placeholders) is handled
// per-call-site, not through this generic path.
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

// json_extract(<column ref>, '<$ path>') — the column ref is a simple or
// table-qualified identifier (every product/money consumer passes a column,
// never a subexpression); the path is a SQLite JSON path (`$.a`, `$.a.b`,
// `$.a[0]`). Anything else is left untouched and fails loud at Postgres.
const JSON_EXTRACT_RE =
  /\bjson_extract\(\s*([A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)?)\s*,\s*'(\$(?:\.[A-Za-z_][A-Za-z0-9_]*|\[\d+\])+)'\s*\)/gi

const jsonPathSegments = (jsonPath: string): ReadonlyArray<string> => {
  const segments: Array<string> = []
  const re = /\.([A-Za-z_][A-Za-z0-9_]*)|\[(\d+)\]/g
  let match: RegExpExecArray | null
  while ((match = re.exec(jsonPath)) !== null) {
    segments.push(match[1] ?? match[2]!)
  }
  return segments
}

/** `json_extract(col, '$.a.b')` -> Postgres jsonb TEXT extraction. A single
 * top-level key uses `->>`; a deeper path uses `#>> '{a,b}'`. The result is
 * TEXT either way (matching how these call sites compare against string
 * literals); a JSON boolean/number therefore surfaces as its text form. */
const translateJsonExtract = (sql: string): string =>
  sql.replaceAll(JSON_EXTRACT_RE, (_whole, expr: string, jsonPath: string) => {
    const segments = jsonPathSegments(jsonPath)
    if (segments.length === 1) {
      return `(${expr})::jsonb ->> '${segments[0]}'`
    }
    return `(${expr})::jsonb #>> '{${segments.join(',')}}'`
  })

const DATETIME_NOW_RE = /\bdatetime\(\s*'now'\s*\)/gi

// julianday(x) — x is a column ref or the literal 'now'. Julian Day Number =
// unix-epoch-days + 2440587.5; the offset cancels in date differences and is
// exact for absolute values, so `(julianday(a) - julianday(b)) * 1440` minutes
// stays correct.
const JULIANDAY_RE =
  /\bjulianday\(\s*(?:'now'|([A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)?))\s*\)/gi

// strftime('%s', x) — unix seconds. Only the '%s' format translates
// generically; any other format string is left untouched (fails loud).
const STRFTIME_EPOCH_RE =
  /\bstrftime\(\s*'%s'\s*,\s*(?:'now'|([A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)?))\s*\)/gi

/** Translate the SQLite date/time scalar functions the money consumers emit.
 * Only the fully generic, semantics-preserving forms are handled; relative
 * `datetime`/`strftime`-format constructs are left for a Postgres fail-loud. */
const translateSqliteDateFns = (sql: string): string => {
  let out = sql.replaceAll(DATETIME_NOW_RE, 'now()')
  out = out.replaceAll(JULIANDAY_RE, (_whole, expr?: string) => {
    const source = expr === undefined ? 'now()' : `(${expr})::timestamptz`
    return `(extract(epoch from ${source}) / 86400.0 + 2440587.5)`
  })
  out = out.replaceAll(STRFTIME_EPOCH_RE, (_whole, expr?: string) => {
    const source = expr === undefined ? 'now()' : `(${expr})::timestamptz`
    return `floor(extract(epoch from ${source}))::bigint`
  })
  return out
}

/** Rewrite SQLite constructs the product/money consumers emit into Postgres,
 * then translate `?` placeholders to `$n`. Exported for the contract test. */
export const translateProductStateSql = (sql: string): string => {
  let out = sql

  // Null-safe equality: `col IS ?` (never `IS NULL` / `IS NOT ?`).
  out = out.replaceAll(IS_PLACEHOLDER_RE, 'IS NOT DISTINCT FROM ?')

  // SQLite scalar functions -> Postgres equivalents. Run before placeholder
  // translation; none of these rewrites introduce a `?`.
  out = translateJsonExtract(out)
  out = translateSqliteDateFns(out)

  // INSERT OR IGNORE -> ON CONFLICT DO NOTHING. INSERT OR REPLACE needs the
  // conflict target (PK), which can't be inferred from SQL text without the
  // schema: accept it only when the call site already carries an explicit
  // `ON CONFLICT (...) DO UPDATE`; otherwise refuse loudly rather than
  // silently mistranslate its overwrite semantics.
  const orMatch = /\binsert\s+or\s+(ignore|replace)\b/i.exec(out)
  if (orMatch !== undefined && orMatch !== null) {
    if (orMatch[1]!.toLowerCase() === 'replace') {
      if (!/\bon\s+conflict\b/i.test(out)) {
        throw new PostgresD1AdapterError(
          'INSERT OR REPLACE needs an explicit ON CONFLICT (<pk>) DO UPDATE target for the Postgres adapter; rewrite the call site to carry one',
        )
      }
      out = out.replace(/\binsert\s+or\s+replace\b/i, 'INSERT')
    } else {
      out = out.replace(/\binsert\s+or\s+ignore\b/i, 'INSERT')
      if (!/\bon\s+conflict\b/i.test(out)) {
        const trimmed = out.replace(/;\s*$/, '')
        out = `${trimmed} ON CONFLICT DO NOTHING`
      }
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
