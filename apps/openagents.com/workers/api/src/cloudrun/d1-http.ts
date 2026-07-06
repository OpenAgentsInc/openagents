/**
 * CFG-9 (#8524): `OPENAGENTS_DB` bridge — a `D1Database`-compatible client
 * over the Cloudflare REST API, for the Cloud Run monolith.
 *
 * WHY THIS EXISTS: the emergency posture (#8515) is "no Worker deploy can
 * ship", not "D1 is already gone". Hundreds of routes are still hard-bound
 * to D1 until CFG-4 completes the Postgres hard cutover domain by domain.
 * This bridge keeps those routes SERVING from Cloud Run against the live D1
 * data over `POST /accounts/:account/d1/database/:id/query`, so the DNS flip
 * (CFG-10) does not have to wait for 100% route parity. As CFG-4 lands,
 * paths stop touching this bridge; if Cloudflare disables D1 account-wide,
 * every call degrades to the typed `BindingUnavailableError` 503 pattern
 * instead of crashing the process.
 *
 * KNOWN DEVIATIONS from workerd D1 (bridge-only, documented in #8524):
 * - `batch()` executes statements sequentially over HTTP and is NOT atomic.
 * - Blob (ArrayBuffer) params are not supported (JSON transport).
 * - `withSession()` returns a plain pass-through session (no bookmarks).
 * - `dump()` is unsupported.
 */

import { BindingUnavailableError } from './binding-unavailable'

export type D1HttpConfig = Readonly<{
  accountId: string
  apiToken: string
  databaseId: string
  fetchImpl?: typeof fetch
  /** Transient (429/5xx/network) retries per statement. Default 2. */
  retries?: number
}>

export class D1HttpError extends Error {
  readonly _tag = 'D1HttpError'

  constructor(
    message: string,
    readonly status?: number,
  ) {
    super(message)
  }
}

type RestQueryResult = Readonly<{
  results?: ReadonlyArray<Record<string, unknown>>
  success?: boolean
  meta?: Record<string, unknown>
}>

type RestEnvelope = Readonly<{
  success?: boolean
  result?: ReadonlyArray<RestQueryResult>
  errors?: ReadonlyArray<{ code?: number; message?: string }>
}>

const sleep = (ms: number) => new Promise(resolve => setTimeout(resolve, ms))

const normalizeParam = (value: unknown): unknown => {
  if (
    value instanceof ArrayBuffer ||
    ArrayBuffer.isView(value) ||
    typeof value === 'bigint'
  ) {
    throw new D1HttpError(
      'd1-http bridge: blob/bigint parameters are not supported over the REST transport (CFG-9, #8524)',
    )
  }
  return value === undefined ? null : value
}

const metaFor = (raw: Record<string, unknown> | undefined) => ({
  changed_db: Boolean(raw?.['changed_db'] ?? false),
  changes: Number(raw?.['changes'] ?? 0),
  duration: Number(raw?.['duration'] ?? 0),
  last_row_id: Number(raw?.['last_row_id'] ?? 0),
  rows_read: Number(raw?.['rows_read'] ?? 0),
  rows_written: Number(raw?.['rows_written'] ?? 0),
  served_by: String(raw?.['served_by'] ?? 'd1-http-bridge'),
  size_after: Number(raw?.['size_after'] ?? 0),
})

export const makeD1HttpDatabase = (config: D1HttpConfig): D1Database => {
  const fetchImpl = config.fetchImpl ?? fetch
  const retries = config.retries ?? 2
  const endpoint = `https://api.cloudflare.com/client/v4/accounts/${config.accountId}/d1/database/${config.databaseId}/query`

  const query = async (
    sql: string,
    params: ReadonlyArray<unknown>,
  ): Promise<RestQueryResult> => {
    let lastError: unknown
    for (let attempt = 0; attempt <= retries; attempt += 1) {
      try {
        const response = await fetchImpl(endpoint, {
          body: JSON.stringify({ params: params.map(normalizeParam), sql }),
          headers: {
            authorization: `Bearer ${config.apiToken}`,
            'content-type': 'application/json',
          },
          method: 'POST',
        })

        if (response.status === 429 || response.status >= 500) {
          lastError = new D1HttpError(
            `d1-http bridge: transient upstream ${response.status}`,
            response.status,
          )
          await response.body?.cancel()
          await sleep(100 * (attempt + 1))
          continue
        }

        const envelope = (await response.json()) as RestEnvelope

        if (!response.ok || envelope.success !== true) {
          const detail = envelope.errors
            ?.map(error => `${error.code ?? ''} ${error.message ?? ''}`.trim())
            .join('; ')
          throw new D1HttpError(
            `d1-http bridge query failed (${response.status}): ${detail ?? 'unknown error'}`,
            response.status,
          )
        }

        const first = envelope.result?.[0]
        if (first === undefined) {
          throw new D1HttpError('d1-http bridge: empty result envelope')
        }
        return first
      } catch (error) {
        if (error instanceof D1HttpError && error.status === undefined) {
          throw error
        }
        lastError = error
        if (attempt < retries) {
          await sleep(100 * (attempt + 1))
          continue
        }
      }
    }
    throw lastError instanceof Error
      ? lastError
      : new D1HttpError('d1-http bridge: request failed')
  }

  const makeStatement = (
    sql: string,
    params: ReadonlyArray<unknown> = [],
  ): D1PreparedStatement => {
    const statement = {
      all: async <T = Record<string, unknown>>() => {
        const result = await query(sql, params)
        return {
          meta: metaFor(result.meta),
          results: (result.results ?? []) as Array<T>,
          success: true as const,
        }
      },
      bind: (...values: ReadonlyArray<unknown>) => makeStatement(sql, values),
      first: async <T = Record<string, unknown>>(
        colName?: string,
      ): Promise<T | null> => {
        const result = await query(sql, params)
        const row = (result.results ?? [])[0]
        if (row === undefined) return null
        if (colName !== undefined) {
          return (row as Record<string, unknown>)[colName] as T
        }
        return row as T
      },
      raw: async <T = Array<unknown>>(options?: { columnNames?: boolean }) => {
        const result = await query(sql, params)
        const rows = (result.results ?? []) as Array<Record<string, unknown>>
        const columns = rows[0] === undefined ? [] : Object.keys(rows[0])
        const values = rows.map(row => columns.map(column => row[column]))
        return (
          options?.columnNames === true ? [columns, ...values] : values
        ) as Array<T>
      },
      run: async <T = Record<string, unknown>>() => {
        const result = await query(sql, params)
        return {
          meta: metaFor(result.meta),
          results: (result.results ?? []) as Array<T>,
          success: true as const,
        }
      },
    }

    return statement as unknown as D1PreparedStatement
  }

  const database = {
    batch: async <T = unknown>(
      statements: ReadonlyArray<D1PreparedStatement>,
    ) => {
      // NOT atomic over the REST transport — sequential, first failure throws.
      const out: Array<unknown> = []
      for (const statement of statements) {
        out.push(await (statement as unknown as { run: () => Promise<T>}).run())
      }
      return out as Array<T>
    },
    dump: async (): Promise<ArrayBuffer> => {
      throw new D1HttpError('d1-http bridge: dump() is unsupported')
    },
    exec: async (sql: string) => {
      const started = Date.now()
      await query(sql, [])
      return { count: 1, duration: Date.now() - started }
    },
    prepare: (sql: string) => makeStatement(sql),
    withSession: (_constraintOrBookmark?: string) => ({
      batch: (statements: ReadonlyArray<D1PreparedStatement>) =>
        database.batch(statements),
      getBookmark: () => null,
      prepare: (sql: string) => makeStatement(sql),
    }),
  }

  return database as unknown as D1Database
}

/**
 * Env-driven constructor: a real HTTP bridge when the Cloudflare API
 * credentials are configured, otherwise the typed per-call unavailable proxy
 * (503 pattern) so the monolith still boots and every non-D1 route serves.
 */
export const d1FromProcessEnv = (
  env: Readonly<Record<string, string | undefined>>,
): D1Database => {
  const accountId = env['CLOUDFLARE_ACCOUNT_ID']
  const apiToken = env['CLOUDFLARE_API_TOKEN']
  const databaseId = env['CLOUDFLARE_D1_DATABASE_ID']

  if (
    accountId === undefined ||
    accountId.length === 0 ||
    apiToken === undefined ||
    apiToken.length === 0 ||
    databaseId === undefined ||
    databaseId.length === 0
  ) {
    return new Proxy({} as D1Database, {
      get(_target, property) {
        if (typeof property !== 'string' || property === 'then') {
          return undefined
        }
        if (property === 'prepare' || property === 'withSession') {
          // Statement construction is sync in D1; degrade at execution time.
          return (..._args: ReadonlyArray<unknown>) =>
            unavailableStatement('OPENAGENTS_DB')
        }
        return (..._args: ReadonlyArray<unknown>) =>
          Promise.reject(
            new BindingUnavailableError({
              binding: 'OPENAGENTS_DB',
              operation: String(property),
            }),
          )
      },
    })
  }

  return makeD1HttpDatabase({ accountId, apiToken, databaseId })
}

const unavailableStatement = (binding: string): D1PreparedStatement => {
  const reject = (operation: string) =>
    Promise.reject(new BindingUnavailableError({ binding, operation }))
  const statement = {
    all: () => reject('all'),
    bind: () => statement,
    first: () => reject('first'),
    getBookmark: () => null,
    prepare: () => statement,
    raw: () => reject('raw'),
    run: () => reject('run'),
  }
  return statement as unknown as D1PreparedStatement
}
