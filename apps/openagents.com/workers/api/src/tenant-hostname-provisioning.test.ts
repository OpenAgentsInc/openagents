import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type CloudflareCustomHostname,
  type CloudflareCustomHostnameStatus,
  type CustomHostnameClient,
  CustomHostnameClientError,
  makeTenantHostnameProvisioning,
} from './tenant-hostname-provisioning'
import type { TenantCustomHostnamesRuntime } from './tenant-custom-hostnames'

// ---------------------------------------------------------------------------
// In-memory D1 fake (same shape/statements as tenant-custom-hostnames.test.ts,
// plus the bare SELECT issued by the provisioning module's readRecord).
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Fake Cloudflare for SaaS custom-hostname client.
// ---------------------------------------------------------------------------

type FakeClientOptions = Readonly<{
  // Status returned the FIRST time a hostname is created/read.
  initialStatus?: CloudflareCustomHostnameStatus
  // Optional per-hostname status overrides keyed by normalized hostname; later
  // reconcile reads pick these up (drives the verification-pending -> active
  // retry path).
  statusByHostname?: Record<string, CloudflareCustomHostnameStatus>
  // When set, createCustomHostname fails on the Nth call (1-based) before
  // succeeding on the next, to exercise failure/retry of the client itself.
  failCreateUntilCall?: number
}>

const makeFakeClient = (options: FakeClientOptions = {}) => {
  // Cloudflare-side store: id + hostname + status. Idempotent on hostname.
  const records = new Map<string, CloudflareCustomHostname>()
  const idByHostname = new Map<string, string>()
  let idCounter = 0
  let createCalls = 0
  let getStatusCalls = 0
  let deleteCalls = 0

  const statusFor = (
    hostname: string,
  ): CloudflareCustomHostnameStatus =>
    options.statusByHostname?.[hostname] ??
    options.initialStatus ??
    'pending'

  const client: CustomHostnameClient = {
    createCustomHostname: input =>
      Effect.gen(function* () {
        createCalls += 1

        if (
          options.failCreateUntilCall !== undefined &&
          createCalls <= options.failCreateUntilCall
        ) {
          return yield* new CustomHostnameClientError({
            operation: 'createCustomHostname',
            error: new Error('cloudflare transient failure'),
          })
        }

        const existingId = idByHostname.get(input.hostname)
        const id = existingId ?? `cf_hostname_${(idCounter += 1)}`

        idByHostname.set(input.hostname, id)

        const record: CloudflareCustomHostname = {
          id,
          hostname: input.hostname,
          status: statusFor(input.hostname),
        }

        records.set(id, record)

        return record
      }),
    getStatus: cloudflareId =>
      Effect.gen(function* () {
        getStatusCalls += 1

        const record = records.get(cloudflareId)

        if (record === undefined) {
          return yield* new CustomHostnameClientError({
            operation: 'getStatus',
            error: new Error(`unknown cloudflare id: ${cloudflareId}`),
          })
        }

        // Re-evaluate status from the (possibly updated) override map so a
        // later reconcile can observe a transition to 'active'.
        const fresh: CloudflareCustomHostname = {
          ...record,
          status: statusFor(record.hostname),
        }

        records.set(cloudflareId, fresh)

        return fresh
      }),
    deleteCustomHostname: cloudflareId =>
      Effect.sync(() => {
        deleteCalls += 1

        const record = records.get(cloudflareId)

        if (record !== undefined) {
          records.delete(cloudflareId)
          idByHostname.delete(record.hostname)
        }
      }),
  }

  return {
    client,
    options,
    counts: () => ({ createCalls, getStatusCalls, deleteCalls }),
  }
}

describe('tenant-hostname-provisioning', () => {
  test('happy path: provision drives register -> active', async () => {
    const db = makeMemoryD1()
    const fake = makeFakeClient({ initialStatus: 'active' })
    const service = makeTenantHostnameProvisioning(
      db,
      fake.client,
      makeRuntime(),
    )

    const outcome = await Effect.runPromise(
      service.provision({
        teamId: 'team_brand',
        hostname: 'Brand.Example.com.',
      }),
    )

    expect(outcome.cloudflareStatus).toBe('active')
    expect(outcome.cloudflareId).toBe('cf_hostname_1')
    expect(outcome.record.hostname).toBe('brand.example.com')
    expect(outcome.record.teamId).toBe('team_brand')
    expect(outcome.record.status).toBe('active')
    expect(outcome.record.verifiedAt).not.toBeNull()
  })

  test('verification-pending: provision leaves the row pending', async () => {
    const db = makeMemoryD1()
    const fake = makeFakeClient({ initialStatus: 'pending' })
    const service = makeTenantHostnameProvisioning(
      db,
      fake.client,
      makeRuntime(),
    )

    const outcome = await Effect.runPromise(
      service.provision({ teamId: 'team_a', hostname: 'brand.example.com' }),
    )

    expect(outcome.cloudflareStatus).toBe('pending')
    expect(outcome.record.status).toBe('pending')
    expect(outcome.record.verifiedAt).toBeNull()
  })

  test('reconcile advances a pending provision to active once CF validates', async () => {
    const db = makeMemoryD1()
    const statusByHostname: Record<string, CloudflareCustomHostnameStatus> = {
      'brand.example.com': 'pending',
    }
    const fake = makeFakeClient({ statusByHostname })
    const service = makeTenantHostnameProvisioning(
      db,
      fake.client,
      makeRuntime(),
    )

    const first = await Effect.runPromise(
      service.provision({ teamId: 'team_a', hostname: 'brand.example.com' }),
    )

    expect(first.record.status).toBe('pending')

    // Cloudflare finishes validation; reconcile should flip our row to active.
    statusByHostname['brand.example.com'] = 'active'

    const second = await Effect.runPromise(
      service.reconcile({
        hostname: 'brand.example.com',
        cloudflareId: first.cloudflareId,
      }),
    )

    expect(second.cloudflareStatus).toBe('active')
    expect(second.record.status).toBe('active')
    expect(second.record.verifiedAt).not.toBeNull()
    expect(fake.counts().getStatusCalls).toBe(1)
  })

  test('failure: CF failed status disables the row and fails', async () => {
    const db = makeMemoryD1()
    const fake = makeFakeClient({ initialStatus: 'failed' })
    const service = makeTenantHostnameProvisioning(
      db,
      fake.client,
      makeRuntime(),
    )

    const failure = await Effect.runPromise(
      service
        .provision({ teamId: 'team_a', hostname: 'brand.example.com' })
        .pipe(Effect.flip),
    )

    expect(failure._tag).toBe('TenantHostnameProvisionFailedError')

    if (failure._tag === 'TenantHostnameProvisionFailedError') {
      expect(failure.hostname).toBe('brand.example.com')
      expect(failure.cloudflareId).toBe('cf_hostname_1')
    }

    // The row must be disabled so the resolver never treats it as live.
    const tenant = await Effect.runPromise(
      service
        .reconcile({
          hostname: 'brand.example.com',
          cloudflareId: 'cf_hostname_1',
        })
        .pipe(Effect.flip),
    )

    // Reconcile re-reads CF 'failed' and disables/fails again (idempotent).
    expect(tenant._tag).toBe('TenantHostnameProvisionFailedError')
  })

  test('client retry: provision surfaces a transient client error, retry succeeds', async () => {
    const db = makeMemoryD1()
    const fake = makeFakeClient({
      initialStatus: 'active',
      failCreateUntilCall: 1,
    })
    const service = makeTenantHostnameProvisioning(
      db,
      fake.client,
      makeRuntime(),
    )

    const failure = await Effect.runPromise(
      service
        .provision({ teamId: 'team_a', hostname: 'brand.example.com' })
        .pipe(Effect.flip),
    )

    expect(failure._tag).toBe('CustomHostnameClientError')

    // The row was registered before the client call failed; a retry of
    // provision reuses it (idempotent register) and completes to active.
    const retried = await Effect.runPromise(
      service.provision({ teamId: 'team_a', hostname: 'brand.example.com' }),
    )

    expect(retried.record.status).toBe('active')
    expect(retried.record.teamId).toBe('team_a')
  })

  test('idempotency: re-provisioning an active hostname for the same team is a no-op upsert', async () => {
    const db = makeMemoryD1()
    const fake = makeFakeClient({ initialStatus: 'active' })
    const service = makeTenantHostnameProvisioning(
      db,
      fake.client,
      makeRuntime(),
    )

    const first = await Effect.runPromise(
      service.provision({ teamId: 'team_a', hostname: 'brand.example.com' }),
    )
    const second = await Effect.runPromise(
      service.provision({ teamId: 'team_a', hostname: 'brand.example.com' }),
    )

    expect(second.record.status).toBe('active')
    // Same Cloudflare record id both times (idempotent upsert).
    expect(second.cloudflareId).toBe(first.cloudflareId)
    expect(second.record.verifiedAt).toBe(first.record.verifiedAt)
  })

  test('conflict: provisioning a hostname owned by another team is rejected', async () => {
    const db = makeMemoryD1()
    const fake = makeFakeClient({ initialStatus: 'active' })
    const service = makeTenantHostnameProvisioning(
      db,
      fake.client,
      makeRuntime(),
    )

    await Effect.runPromise(
      service.provision({ teamId: 'team_a', hostname: 'brand.example.com' }),
    )

    const failure = await Effect.runPromise(
      service
        .provision({ teamId: 'team_b', hostname: 'brand.example.com' })
        .pipe(Effect.flip),
    )

    expect(failure._tag).toBe('TenantCustomHostnameValidationError')
  })
})
