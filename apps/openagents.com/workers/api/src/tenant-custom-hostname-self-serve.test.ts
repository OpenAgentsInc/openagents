import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type TenantCustomHostnamesRuntime,
  makeTenantCustomHostnames,
} from './tenant-custom-hostnames'
import {
  defaultSelfServeConfig,
  makeTenantCustomHostnameSelfServe,
} from './tenant-custom-hostname-self-serve'

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

// In-memory D1 fake covering the statements issued by tenant-custom-hostnames.ts
// (SELECT by hostname, INSERT, UPDATE by hostname) plus the team-scoped SELECT
// ... WHERE team_id used by the self-serve core's listForTeam/claim.
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
      all: async <T>() => {
        if (
          query.includes('SELECT') &&
          query.includes('WHERE team_id = ?')
        ) {
          const teamId = textValue(bound[0])
          const matched = rows.filter(stored => stored.team_id === teamId)

          return makeResult<T>(matched as unknown as Array<T>)
        }

        return makeResult<T>([])
      },
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

describe('tenant-custom-hostname-self-serve', () => {
  test('customer claims a hostname as a pending row with DNS instructions', async () => {
    const db = makeMemoryD1()
    const selfServe = makeTenantCustomHostnameSelfServe(
      db,
      defaultSelfServeConfig,
      makeRuntime(),
    )

    const view = await Effect.runPromise(
      selfServe.claim({
        teamId: 'team_brand',
        hostname: 'Brand.Example.com.',
      }),
    )

    expect(view.teamId).toBe('team_brand')
    expect(view.hostname).toBe('brand.example.com')
    expect(view.status).toBe('pending')
    // INERT: nothing serves until the owner arms provisioning.
    expect(view.servingLive).toBe(false)
    expect(view.verification).not.toBeNull()
    expect(view.verification?.recordType).toBe('TXT')
    expect(view.verification?.recordName).toBe(
      '_openagents-verify.brand.example.com',
    )
    expect(view.verification?.recordValue).toContain(
      'openagents-site-verification=',
    )
  })

  test('claim is idempotent for the same team', async () => {
    const db = makeMemoryD1()
    const selfServe = makeTenantCustomHostnameSelfServe(
      db,
      defaultSelfServeConfig,
      makeRuntime(),
    )

    const first = await Effect.runPromise(
      selfServe.claim({ teamId: 'team_a', hostname: 'brand.example.com' }),
    )
    const second = await Effect.runPromise(
      selfServe.claim({ teamId: 'team_a', hostname: 'brand.example.com' }),
    )

    expect(second.id).toBe(first.id)
    expect(second.status).toBe('pending')
  })

  test('claim of a hostname owned by another team fails (taken)', async () => {
    const db = makeMemoryD1()
    const selfServe = makeTenantCustomHostnameSelfServe(
      db,
      defaultSelfServeConfig,
      makeRuntime(),
    )

    await Effect.runPromise(
      selfServe.claim({ teamId: 'team_a', hostname: 'brand.example.com' }),
    )

    const failure = await Effect.runPromise(
      selfServe
        .claim({ teamId: 'team_b', hostname: 'brand.example.com' })
        .pipe(Effect.flip),
    )

    expect(failure._tag).toBe('TenantCustomHostnameStorageError')
  })

  test('listForTeam returns only that team and not others', async () => {
    const db = makeMemoryD1()
    const selfServe = makeTenantCustomHostnameSelfServe(
      db,
      defaultSelfServeConfig,
      makeRuntime(),
    )

    await Effect.runPromise(
      selfServe.claim({ teamId: 'team_a', hostname: 'a-one.example.com' }),
    )
    await Effect.runPromise(
      selfServe.claim({ teamId: 'team_a', hostname: 'a-two.example.com' }),
    )
    await Effect.runPromise(
      selfServe.claim({ teamId: 'team_b', hostname: 'b-one.example.com' }),
    )

    const teamA = await Effect.runPromise(selfServe.listForTeam('team_a'))
    const teamB = await Effect.runPromise(selfServe.listForTeam('team_b'))

    expect(teamA.map(h => h.hostname).sort()).toEqual([
      'a-one.example.com',
      'a-two.example.com',
    ])
    expect(teamB.map(h => h.hostname)).toEqual(['b-one.example.com'])
  })

  test('an active hostname reports servingLive only when armed', async () => {
    const db = makeMemoryD1()
    const runtime = makeRuntime()
    const repo = makeTenantCustomHostnames(db, runtime)

    await Effect.runPromise(
      repo.register({ teamId: 'team_live', hostname: 'live.example.com' }),
    )
    await Effect.runPromise(repo.markActive('live.example.com'))

    const inert = makeTenantCustomHostnameSelfServe(
      db,
      { selfServeLiveDnsVerificationArmed: false },
      runtime,
    )
    const armed = makeTenantCustomHostnameSelfServe(
      db,
      { selfServeLiveDnsVerificationArmed: true },
      runtime,
    )

    const inertView = await Effect.runPromise(inert.listForTeam('team_live'))
    const armedView = await Effect.runPromise(armed.listForTeam('team_live'))

    expect(inertView[0]?.status).toBe('active')
    // Default INERT: never claims live, and no DNS instruction is needed once
    // active (nothing for the customer left to publish).
    expect(inertView[0]?.servingLive).toBe(false)
    expect(inertView[0]?.verification).toBeNull()
    // Only when the owner arms provisioning does an active row report live.
    expect(armedView[0]?.servingLive).toBe(true)
  })
})
