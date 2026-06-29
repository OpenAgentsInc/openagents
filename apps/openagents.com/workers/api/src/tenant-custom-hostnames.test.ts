import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type TenantCustomHostnamesRuntime,
  makeTenantCustomHostnames,
} from './tenant-custom-hostnames'

type StoredHostname = {
  id: string
  team_id: string
  hostname: string
  status: string
  verification_token: string
  verified_at: string | null
  created_at: string
  updated_at: string
}

const d1Meta = (): D1Meta & Record<string, unknown> => ({
  changed_db: false,
  changes: 1,
  duration: 0,
  last_row_id: 0,
  rows_read: 1,
  rows_written: 1,
  served_by: 'memory',
  served_by_primary: true,
  size_after: 0,
  timings: { sql_duration_ms: 0 },
})

const makeResult = <T>(results: Array<T> = []): D1Result<T> => ({
  meta: d1Meta(),
  results,
  success: true,
})

const textValue = (value: unknown): string => String(value)
const nullableText = (value: unknown): string | null =>
  value === null || value === undefined ? null : String(value)

// Minimal in-memory D1 fake covering the exact statements issued by
// tenant-custom-hostnames.ts: SELECT by hostname, INSERT, and UPDATE by
// hostname. The UNIQUE(hostname) constraint is enforced at insert time.
const makeMemoryD1 = (rows: Array<StoredHostname> = []): D1Database => {
  const prepare = (query: string): D1PreparedStatement => {
    let bound: ReadonlyArray<unknown> = []

    const statement: D1PreparedStatement = {
      bind: (...values: Array<unknown>) => {
        bound = values

        return statement
      },
      first: async <T>() => {
        if (query.includes('SELECT') && query.includes('WHERE hostname = ?')) {
          const hostname = textValue(bound[0])
          const row = rows.find(stored => stored.hostname === hostname)

          return (row === undefined ? null : (row as unknown as T)) as T
        }

        return null as unknown as T
      },
      run: async () => {
        if (query.includes('INSERT INTO tenant_custom_hostnames')) {
          const hostname = textValue(bound[2])

          if (rows.some(stored => stored.hostname === hostname)) {
            throw new Error(
              'UNIQUE constraint failed: tenant_custom_hostnames.hostname',
            )
          }

          rows.push({
            id: textValue(bound[0]),
            team_id: textValue(bound[1]),
            hostname,
            status: textValue(bound[3]),
            verification_token: textValue(bound[4]),
            verified_at: nullableText(bound[5]),
            created_at: textValue(bound[6]),
            updated_at: textValue(bound[7]),
          })

          return makeResult()
        }

        if (query.includes('UPDATE tenant_custom_hostnames')) {
          const hostname = textValue(bound[3])
          const index = rows.findIndex(stored => stored.hostname === hostname)
          const current = rows[index]

          if (current !== undefined) {
            rows[index] = {
              ...current,
              status: textValue(bound[0]),
              verified_at: nullableText(bound[1]),
              updated_at: textValue(bound[2]),
            }
          }

          return makeResult()
        }

        return makeResult()
      },
      all: async <T>() => makeResult<T>([]),
      raw: async () => [] as unknown as never,
    } as unknown as D1PreparedStatement

    return statement
  }

  return {
    prepare,
    batch: async () => [],
    dump: async () => new ArrayBuffer(0),
    exec: async () => ({ count: 0, duration: 0 }),
    withSession: () => {
      throw new Error('not implemented')
    },
  } as unknown as D1Database
}

const sequentialIds = (prefix: string) => {
  let counter = 0

  return () => `${prefix}-${(counter += 1)}`
}

const tickingClock = (startMs = 1_000, stepMs = 1_000) => {
  let now = startMs

  return () => {
    const value = new Date(now).toISOString()

    now += stepMs

    return value
  }
}

const makeRuntime = (): TenantCustomHostnamesRuntime => ({
  makeHostnameId: sequentialIds('tenant_hostname'),
  makeVerificationToken: sequentialIds('tenant_hostname_verify'),
  nowIso: tickingClock(),
})

describe('tenant-custom-hostnames', () => {
  test('registers a hostname for a team in pending status', async () => {
    const db = makeMemoryD1()
    const repo = makeTenantCustomHostnames(db, makeRuntime())

    const registered = await Effect.runPromise(
      repo.register({
        teamId: 'team_openagents_core',
        hostname: 'Brand.Example.com.',
      }),
    )

    expect(registered.teamId).toBe('team_openagents_core')
    expect(registered.hostname).toBe('brand.example.com')
    expect(registered.status).toBe('pending')
    expect(registered.verificationToken).not.toBe('')
    expect(registered.verifiedAt).toBeNull()
  })

  test('marks a hostname verified then active', async () => {
    const db = makeMemoryD1()
    const repo = makeTenantCustomHostnames(db, makeRuntime())

    await Effect.runPromise(
      repo.register({ teamId: 'team_a', hostname: 'brand.example.com' }),
    )

    const verified = await Effect.runPromise(
      repo.markVerified('brand.example.com'),
    )

    expect(verified.status).toBe('verified')
    expect(verified.verifiedAt).not.toBeNull()

    const active = await Effect.runPromise(
      repo.markActive('brand.example.com'),
    )

    expect(active.status).toBe('active')
    expect(active.verifiedAt).toBe(verified.verifiedAt)
  })

  test('resolves an active hostname to its tenant (hit)', async () => {
    const db = makeMemoryD1()
    const repo = makeTenantCustomHostnames(db, makeRuntime())

    await Effect.runPromise(
      repo.register({ teamId: 'team_brand', hostname: 'brand.example.com' }),
    )
    await Effect.runPromise(repo.markActive('brand.example.com'))

    const tenant = await Effect.runPromise(
      // Case/dot variation must resolve to the same tenant.
      repo.resolveTenantByHostname('BRAND.example.com.'),
    )

    expect(tenant).not.toBeNull()
    expect(tenant?.teamId).toBe('team_brand')
    expect(tenant?.hostname).toBe('brand.example.com')
    expect(tenant?.status).toBe('active')
  })

  test('returns null for an unknown hostname (miss)', async () => {
    const db = makeMemoryD1()
    const repo = makeTenantCustomHostnames(db, makeRuntime())

    const tenant = await Effect.runPromise(
      repo.resolveTenantByHostname('unknown.example.com'),
    )

    expect(tenant).toBeNull()
  })

  test('does not resolve a non-active (pending) hostname', async () => {
    const db = makeMemoryD1()
    const repo = makeTenantCustomHostnames(db, makeRuntime())

    await Effect.runPromise(
      repo.register({ teamId: 'team_brand', hostname: 'brand.example.com' }),
    )

    const tenant = await Effect.runPromise(
      repo.resolveTenantByHostname('brand.example.com'),
    )

    expect(tenant).toBeNull()
  })

  test('rejects a duplicate hostname registration', async () => {
    const db = makeMemoryD1()
    const repo = makeTenantCustomHostnames(db, makeRuntime())

    await Effect.runPromise(
      repo.register({ teamId: 'team_a', hostname: 'brand.example.com' }),
    )

    const failure = await Effect.runPromise(
      repo
        .register({ teamId: 'team_b', hostname: 'brand.example.com' })
        .pipe(Effect.flip),
    )

    expect(failure._tag).toBe('TenantCustomHostnameConflictError')

    if (failure._tag === 'TenantCustomHostnameConflictError') {
      expect(failure.hostname).toBe('brand.example.com')
    }
  })
})
