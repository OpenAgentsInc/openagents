import { describe, expect, test } from 'vitest'

import {
  isRetryablePostgresFailure,
  postgresFailureClass,
  postgresFailureCode,
  retryTransientPostgres,
} from './postgres-transient-failure'

/**
 * The exact error postgres.js produced during incident 2026-07-31: the Cloud
 * SQL Auth Proxy on a cold Cloud Run instance had not finished its first
 * instance refresh, so the connect to the unix socket timed out.
 */
const connectTimeout = (): Error =>
  Object.assign(
    new Error(
      'write CONNECT_TIMEOUT /cloudsql/openagentsgemini:us-central1:khala-sync-pg/.s.PGSQL.5432',
    ),
    {
      address:
        '/cloudsql/openagentsgemini:us-central1:khala-sync-pg/.s.PGSQL.5432',
      code: 'CONNECT_TIMEOUT',
      errno: 'CONNECT_TIMEOUT',
    },
  )

const connectionClosed = (): Error =>
  Object.assign(new Error('write CONNECTION_CLOSED'), {
    code: 'CONNECTION_CLOSED',
  })

const serverRefusal = (): Error => {
  const error = new Error('relation "oa_infra_kv" does not exist')
  error.name = 'PostgresError'
  return Object.assign(error, { code: '42P01' })
}

describe('Postgres failure classification', () => {
  test('names the cold-start Cloud SQL proxy connect timeout', () => {
    expect(postgresFailureClass(connectTimeout())).toBe('connect_unavailable')
    expect(postgresFailureCode(connectTimeout())).toBe('CONNECT_TIMEOUT')
  })

  test('separates a lost connection from one never established', () => {
    expect(postgresFailureClass(connectionClosed())).toBe('connection_lost')
  })

  test('never treats a Postgres refusal as transient', () => {
    expect(postgresFailureClass(serverRefusal())).toBe('server_error')
    expect(isRetryablePostgresFailure(serverRefusal(), 'connection')).toBe(
      false,
    )
  })

  test('classifies an unrecognized failure as unknown, not as retryable', () => {
    expect(postgresFailureClass(new Error('something else'))).toBe('unknown')
    expect(isRetryablePostgresFailure(new Error('x'), 'connection')).toBe(false)
    expect(postgresFailureCode(new Error('x'))).toBe('unclassified')
  })

  test('unwraps a wrapped cause so a nameable failure is still named', () => {
    const wrapped = new Error('An error occurred in Effect.tryPromise', {
      cause: connectTimeout(),
    })
    expect(postgresFailureClass(wrapped)).toBe('connect_unavailable')
    expect(postgresFailureCode(wrapped)).toBe('CONNECT_TIMEOUT')
  })

  test('a cyclic cause chain terminates instead of hanging an auth request', () => {
    const a = new Error('a')
    Object.assign(a, { cause: a })
    expect(postgresFailureClass(a)).toBe('unknown')
  })
})

describe('Postgres transient retry', () => {
  const noSleep = async (): Promise<void> => undefined

  test('rides out a cold-start connect timeout and returns the real value', async () => {
    let calls = 0
    const value = await retryTransientPostgres(
      async () => {
        calls += 1
        if (calls < 3) throw connectTimeout()
        return 'session'
      },
      { scope: 'connect-only', sleep: noSleep },
    )
    expect(value).toBe('session')
    expect(calls).toBe(3)
  })

  test('a write never retries a connection that was lost mid-flight', async () => {
    let calls = 0
    await expect(
      retryTransientPostgres(
        async () => {
          calls += 1
          throw connectionClosed()
        },
        { scope: 'connect-only', sleep: noSleep },
      ),
    ).rejects.toThrow('CONNECTION_CLOSED')
    // Exactly one attempt: retrying `putIfAbsent` whose first attempt may have
    // inserted would report a fresh single-use proof as already consumed.
    expect(calls).toBe(1)
  })

  test('a read does retry a lost connection', async () => {
    let calls = 0
    const value = await retryTransientPostgres(
      async () => {
        calls += 1
        if (calls < 2) throw connectionClosed()
        return 'row'
      },
      { scope: 'connection', sleep: noSleep },
    )
    expect(value).toBe('row')
    expect(calls).toBe(2)
  })

  test('a Postgres refusal fails immediately and unchanged', async () => {
    let calls = 0
    await expect(
      retryTransientPostgres(
        async () => {
          calls += 1
          throw serverRefusal()
        },
        { scope: 'connection', sleep: noSleep },
      ),
    ).rejects.toThrow('relation "oa_infra_kv" does not exist')
    expect(calls).toBe(1)
  })

  test('a persistent outage still surfaces the real cause after the budget', async () => {
    let calls = 0
    await expect(
      retryTransientPostgres(
        async () => {
          calls += 1
          throw connectTimeout()
        },
        { attempts: 3, scope: 'connect-only', sleep: noSleep },
      ),
    ).rejects.toThrow('CONNECT_TIMEOUT')
    expect(calls).toBe(3)
  })

  test('backs off exponentially and reports each retry', async () => {
    const delays: Array<number> = []
    const retried: Array<string> = []
    await expect(
      retryTransientPostgres(
        async () => {
          throw connectTimeout()
        },
        {
          attempts: 4,
          baseDelayMs: 100,
          onRetry: info => retried.push(`${info.attempt}:${info.failureClass}`),
          scope: 'connect-only',
          sleep: async ms => {
            delays.push(ms)
          },
        },
      ),
    ).rejects.toThrow('CONNECT_TIMEOUT')
    expect(delays).toEqual([100, 200, 400])
    expect(retried).toEqual([
      '1:connect_unavailable',
      '2:connect_unavailable',
      '3:connect_unavailable',
    ])
  })

  test('caps a single backoff wait', async () => {
    const delays: Array<number> = []
    await expect(
      retryTransientPostgres(
        async () => {
          throw connectTimeout()
        },
        {
          attempts: 4,
          baseDelayMs: 1_000,
          maxDelayMs: 1_500,
          scope: 'connect-only',
          sleep: async ms => {
            delays.push(ms)
          },
        },
      ),
    ).rejects.toThrow('CONNECT_TIMEOUT')
    expect(delays).toEqual([1_000, 1_500, 1_500])
  })
})
