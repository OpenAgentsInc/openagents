// KS-8.18 (#8329): identity/auth core domain repository CONTRACT suite —
// the LAST and most sensitive KS-8 domain.
//
// Three layers, one behavioral spec:
//
//  1. `IdentityAuthWriteStore` contract — the row seam's composite-PK
//     converge semantics run identically against BOTH implementations:
//     - D1: `makeD1IdentityAuthWriteStore` over real SQLite (node:sqlite —
//       the engine D1 is built on), schema from the worker migrations
//       (condensed in test/sqlite-d1.ts).
//     - Postgres: `makePostgresIdentityAuthStore` over a throwaway local
//       Postgres, schema from khala-sync migration 0028. Skipped when no
//       local Postgres binaries exist.
//     Includes a CUSTODY round-trip: the encrypted token vault
//     (ciphertext/IVs/key ids) converges byte-exact on both engines.
//
//  2. END-TO-END mirror fidelity — the REAL provider-account token custody
//     store runs UNCHANGED through `makeProviderAccountTokenCustodyStoreForEnv`
//     with SQLite as D1 authority and the real Postgres store as the
//     mirror: connected-auth store, refreshed-auth save, and an audit-only
//     event. Afterwards `provider_account_token_custody` and
//     `provider_account_token_custody_audit` are row-for-row IDENTICAL
//     across both stores (registry-column projection, value-normalized)
//     with ZERO drift diagnostics.
//
//  3. Fail-soft + custody redaction: a broken Postgres twin never fails a
//     custody write (typed diagnostic only), and NO custody value — token
//     ciphertext, IVs, key ids — ever reaches a diagnostic line (keys only).
//     KHALA_SYNC_IDENTITY_READS=postgres DEFERS (never serves an unproven
//     auth read path).

import {
  IDENTITY_AUTH_DOMAIN_TABLE_SPECS,
  normalizeIdentityAuthValue,
  type IdentityAuthDomainTable,
} from '@openagentsinc/khala-sync-server'
import {
  hasLocalPostgres,
  startLocalPostgres,
} from '@openagentsinc/khala-sync-server/test/local-postgres'
import { joinKey } from '@openauthjs/openauth/storage/storage'
import { readFileSync } from 'node:fs'
import * as path from 'node:path'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'

import {
  identityAuthFlagsFromEnv,
  identityAuthMirrorFromEnv,
  makeD1IdentityAuthWriteStore,
  makeOpenAuthStorageForEnv,
  makePostgresIdentityAuthStore,
  makeProviderAccountTokenCustodyStoreForEnv,
  type IdentityAuthDiagnostic,
  type IdentityAuthDiagnosticEvent,
  type IdentityAuthStoreEnv,
  type IdentityAuthWriteStore,
  type MakeIdentityAuthStoreOptions,
} from './identity-auth-domain-store'
import { IDENTITY_AUTH_DOMAIN_D1_SCHEMA, makeSqliteD1 } from './test/sqlite-d1'

const T0 = '2026-07-04T00:00:00.000Z'
const T1 = '2026-07-04T01:00:00.000Z'
const T2 = '2026-07-04T02:00:00.000Z'
const OWNER = 'user.owner.contract'
const REF = 'provider-account.contract'
const SECRET_CIPHERTEXT = 'CIPHERTEXT-refresh-must-never-leak'
const ACCESS_CIPHERTEXT = 'CIPHERTEXT-access-must-never-leak'

// ---------------------------------------------------------------------------
// Flags (pure)
// ---------------------------------------------------------------------------

describe('identityAuthFlagsFromEnv (pure)', () => {
  test('dual-write defaults ON; reads default d1; off-values disable', () => {
    expect(identityAuthFlagsFromEnv({})).toEqual({
      dualWrite: true,
      reads: 'd1',
    })
    expect(
      identityAuthFlagsFromEnv({ KHALA_SYNC_IDENTITY_DUAL_WRITE: 'off' })
        .dualWrite,
    ).toBe(false)
    expect(
      identityAuthFlagsFromEnv({ KHALA_SYNC_IDENTITY_DUAL_WRITE: '0' })
        .dualWrite,
    ).toBe(false)
    expect(
      identityAuthFlagsFromEnv({ KHALA_SYNC_IDENTITY_DUAL_WRITE: 'on' })
        .dualWrite,
    ).toBe(true)
    expect(
      identityAuthFlagsFromEnv({ KHALA_SYNC_IDENTITY_READS: 'compare' }).reads,
    ).toBe('compare')
    expect(
      identityAuthFlagsFromEnv({ KHALA_SYNC_IDENTITY_READS: 'postgres' }).reads,
    ).toBe('postgres')
    // A typo can never fail open into an unproven auth read path.
    expect(
      identityAuthFlagsFromEnv({ KHALA_SYNC_IDENTITY_READS: 'psotgres' }).reads,
    ).toBe('d1')
  })
})

// ---------------------------------------------------------------------------
// Layer 1: write-store contract (both implementations)
// ---------------------------------------------------------------------------

type ContractHarness = Readonly<{
  store: IdentityAuthWriteStore
  query: (sql: string) => Promise<ReadonlyArray<Record<string, unknown>>>
}>

const userRow = (
  id: string,
  overrides: Partial<Record<string, unknown>> = {},
): Record<string, unknown> => ({
  avatar_url: null,
  created_at: T0,
  deleted_at: null,
  display_name: `User ${id}`,
  id,
  kind: 'human',
  primary_email: `${id}@contract.test`,
  status: 'active',
  updated_at: T0,
  ...overrides,
})

const custodyRow = (
  overrides: Partial<Record<string, unknown>> = {},
): Record<string, unknown> => ({
  access_ciphertext_b64: ACCESS_CIPHERTEXT,
  access_expires_at: T2,
  access_iv_b64: 'iv-access',
  access_key_id: 'key.v1',
  account_id: 'acct-1',
  created_at: T0,
  id_token_ciphertext_b64: null,
  id_token_iv_b64: null,
  id_token_key_id: null,
  last_refreshed_at: null,
  owner_user_id: OWNER,
  provider: 'chatgpt_codex',
  provider_account_ref: REF,
  refresh_ciphertext_b64: SECRET_CIPHERTEXT,
  refresh_iv_b64: 'iv-refresh',
  refresh_key_id: 'key.v1',
  secret_ref: 'secret-ref-1',
  updated_at: T0,
  ...overrides,
})

const specContractSuite = (harness: () => ContractHarness) => {
  test('upsertRows converges on the PK and is idempotent', async () => {
    const { query, store } = harness()
    expect(
      await store.upsertRows('users', [userRow('u-a'), userRow('u-b')]),
    ).toBe(2)
    // Re-run: converge, no duplication.
    expect(
      await store.upsertRows('users', [userRow('u-a'), userRow('u-b')]),
    ).toBe(2)
    const counted = await query(
      `SELECT COUNT(*) AS total FROM users WHERE id IN ('u-a', 'u-b')`,
    )
    expect(Number(counted[0]?.['total'])).toBe(2)

    // A newer snapshot wins (display-name change + soft delete).
    await store.upsertRows('users', [
      userRow('u-a', {
        display_name: 'Renamed',
        deleted_at: T1,
        updated_at: T1,
      }),
    ])
    const rows = await query(
      `SELECT display_name, deleted_at FROM users WHERE id = 'u-a'`,
    )
    expect(rows[0]?.['display_name']).toBe('Renamed')
    expect(rows[0]?.['deleted_at']).toBe(T1)
  })

  test('the encrypted token vault converges byte-exact (custody round-trip)', async () => {
    const { query, store } = harness()
    expect(
      await store.upsertRows('provider_account_token_custody', [custodyRow()]),
    ).toBe(1)
    // Refresh rotates the ciphertext; the newer snapshot wins.
    await store.upsertRows('provider_account_token_custody', [
      custodyRow({
        access_ciphertext_b64: 'CIPHERTEXT-access-rotated',
        last_refreshed_at: T1,
        refresh_ciphertext_b64: 'CIPHERTEXT-refresh-rotated',
        updated_at: T1,
      }),
    ])
    const rows = await query(
      `SELECT refresh_ciphertext_b64, access_ciphertext_b64, last_refreshed_at
       FROM provider_account_token_custody WHERE provider_account_ref = '${REF}'`,
    )
    expect(rows.length).toBe(1)
    expect(rows[0]?.['refresh_ciphertext_b64']).toBe('CIPHERTEXT-refresh-rotated')
    expect(rows[0]?.['access_ciphertext_b64']).toBe('CIPHERTEXT-access-rotated')
    expect(rows[0]?.['last_refreshed_at']).toBe(T1)
  })
}

describe('identity/auth write-store contract — D1 (real SQLite)', () => {
  let sqlite: ReturnType<typeof makeSqliteD1> | undefined
  let harness: ContractHarness

  beforeAll(() => {
    sqlite = makeSqliteD1()
    sqlite.exec(IDENTITY_AUTH_DOMAIN_D1_SCHEMA)
    harness = {
      query: async sql =>
        (await sqlite!.db.prepare(sql).all<Record<string, unknown>>())
          .results ?? [],
      store: makeD1IdentityAuthWriteStore(sqlite.db),
    }
  })

  afterAll(() => {
    sqlite?.close()
  })

  specContractSuite(() => harness)
})

const MIGRATION_0028 = path.resolve(
  __dirname,
  '../../../../../packages/khala-sync-server/migrations/0028_identity_auth_domain.sql',
)

type PgClient = Readonly<{
  end: (options?: { timeout?: number }) => Promise<void>
  unsafe: (
    sql: string,
    params?: Array<unknown>,
  ) => Promise<Array<Record<string, unknown>>>
}>

describe.skipIf(!hasLocalPostgres())(
  'identity/auth write-store contract — Postgres',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let harness: ContractHarness

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE identity_auth_contract')
      await admin.end({ timeout: 5 })
      const raw = postgres(pg.urlFor('identity_auth_contract'), {
        max: 4,
        prepare: false,
      })
      client = raw as unknown as PgClient
      await raw.unsafe(readFileSync(MIGRATION_0028, 'utf8'))
      harness = {
        query: async sql => (client as PgClient).unsafe(sql),
        store: makePostgresIdentityAuthStore({
          acquireSql: () =>
            Promise.resolve({
              end: () => Promise.resolve(),
              sql: raw as never,
            }),
        }),
      }
    }, 120_000)

    afterAll(async () => {
      await client?.end({ timeout: 5 })
      await pg?.stop()
    }, 60_000)

    specContractSuite(() => harness)
  },
)

// ---------------------------------------------------------------------------
// Layer 2 + 3: e2e mirror fidelity + fail-soft custody redaction
// ---------------------------------------------------------------------------

const CUSTODY_TABLES: ReadonlyArray<IdentityAuthDomainTable> = [
  'provider_account_token_custody',
  'provider_account_token_custody_audit',
]

const projectRow = (
  table: IdentityAuthDomainTable,
  row: Record<string, unknown>,
): Record<string, string | null> =>
  Object.fromEntries(
    IDENTITY_AUTH_DOMAIN_TABLE_SPECS[table].columns.map(column => {
      const value = normalizeIdentityAuthValue(row[column])
      return [column, value === null ? null : String(value)]
    }),
  )

const encrypted = (label: string) => ({
  ciphertextB64: `CIPHERTEXT-${label}-must-never-leak`,
  ivB64: `iv-${label}`,
  keyId: 'key.v1',
})

const custodyRecord = (
  overrides: Record<string, unknown> = {},
): Parameters<
  ReturnType<typeof makeProviderAccountTokenCustodyStoreForEnv>['upsertConnectedAuth']
>[0] => ({
  accessExpiresAt: T2,
  accessToken: encrypted('access'),
  accountId: 'acct-1',
  createdAt: T0,
  ownerUserId: OWNER,
  provider: 'chatgpt_codex',
  providerAccountRef: REF,
  refreshToken: encrypted('refresh'),
  secretRef: 'secret-ref-1',
  updatedAt: T0,
  ...overrides,
})

const auditEvent = (
  id: string,
  overrides: Record<string, unknown> = {},
): Parameters<
  ReturnType<typeof makeProviderAccountTokenCustodyStoreForEnv>['insertAuditEvent']
>[0] => ({
  createdAt: T0,
  eventKind: 'auth_stored',
  id,
  ownerUserId: OWNER,
  provider: 'chatgpt_codex',
  providerAccountRef: REF,
  status: 'succeeded',
  ...overrides,
})

describe.skipIf(!hasLocalPostgres())(
  'token custody writes mirror byte-faithfully into Postgres',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let sqlite: ReturnType<typeof makeSqliteD1> | undefined
    let env: IdentityAuthStoreEnv
    let options: MakeIdentityAuthStoreOptions
    const diagnostics: Array<{
      event: IdentityAuthDiagnosticEvent
      fields: IdentityAuthDiagnostic
    }> = []

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE identity_auth_mirror')
      await admin.end({ timeout: 5 })
      const raw = postgres(pg.urlFor('identity_auth_mirror'), {
        max: 4,
        prepare: false,
      })
      client = raw as unknown as PgClient
      await raw.unsafe(readFileSync(MIGRATION_0028, 'utf8'))

      sqlite = makeSqliteD1()
      sqlite.exec(IDENTITY_AUTH_DOMAIN_D1_SCHEMA)

      env = {
        KHALA_SYNC_DB: { connectionString: 'postgres://contract' },
        OPENAGENTS_DB: sqlite.db,
      }
      options = {
        db: sqlite.db,
        log: (event, fields) => {
          diagnostics.push({ event, fields })
        },
        makeSqlClient: () =>
          Promise.resolve({
            end: () => Promise.resolve(),
            sql: raw as never,
          }),
      }
    }, 120_000)

    afterAll(async () => {
      await client?.end({ timeout: 5 })
      await pg?.stop()
      sqlite?.close()
    }, 60_000)

    test('connected-auth, refresh, and audit-only all mirror; both tables converge row-for-row with zero drift', async () => {
      const store = makeProviderAccountTokenCustodyStoreForEnv(env, options)

      await store.upsertConnectedAuth(custodyRecord(), auditEvent('audit.1'))
      // Refresh rotates ciphertext + touches last_refreshed_at.
      await store.saveRefreshedAuth(
        custodyRecord({
          accessToken: encrypted('access-rotated'),
          lastRefreshedAt: T1,
          refreshToken: encrypted('refresh-rotated'),
          updatedAt: T1,
        }),
        auditEvent('audit.2', { createdAt: T1, eventKind: 'refresh_succeeded' }),
      )
      // Audit-only event (no custody row change).
      await store.insertAuditEvent(
        auditEvent('audit.3', { createdAt: T2, eventKind: 'access_issued' }),
      )

      expect(diagnostics).toEqual([])

      for (const table of CUSTODY_TABLES) {
        const order = IDENTITY_AUTH_DOMAIN_TABLE_SPECS[table].keyColumns.join(
          ', ',
        )
        const d1Rows =
          (
            await sqlite!.db
              .prepare(`SELECT * FROM ${table} ORDER BY ${order}`)
              .all<Record<string, unknown>>()
          ).results ?? []
        const pgRows = await (client as PgClient).unsafe(
          `SELECT * FROM ${table} ORDER BY ${order}`,
        )
        expect(
          pgRows.map(row => projectRow(table, row)),
          `table ${table} must mirror byte-faithfully`,
        ).toEqual(d1Rows.map(row => projectRow(table, row)))
        expect(d1Rows.length).toBeGreaterThan(0)
      }

      // The rotated refresh ciphertext really did round-trip to Postgres.
      const custody = await (client as PgClient).unsafe(
        `SELECT refresh_ciphertext_b64 FROM provider_account_token_custody WHERE provider_account_ref = '${REF}'`,
      )
      expect(custody[0]?.['refresh_ciphertext_b64']).toBe(
        'CIPHERTEXT-refresh-rotated-must-never-leak',
      )
    })

    test('KHALA_SYNC_IDENTITY_READS=postgres defers (never serves an auth read path)', () => {
      diagnostics.length = 0
      const deferredEnv: IdentityAuthStoreEnv = {
        ...env,
        KHALA_SYNC_IDENTITY_READS: 'postgres',
      }
      const mirror = identityAuthMirrorFromEnv(deferredEnv, options)
      // Still returns a working mirror (dual-write stays ON)…
      expect(mirror).toBeDefined()
      // …and logs the deferral exactly once so a premature flip is visible.
      expect(
        diagnostics.filter(
          d => d.event === 'khala_sync_identity_postgres_reads_deferred',
        ).length,
      ).toBe(1)
    })

    test('a broken Postgres twin never fails a custody write, and NO ciphertext reaches a diagnostic', async () => {
      const broken: Array<{
        event: IdentityAuthDiagnosticEvent
        fields: IdentityAuthDiagnostic
      }> = []
      const brokenOptions: MakeIdentityAuthStoreOptions = {
        db: sqlite!.db,
        log: (event, fields) => {
          broken.push({ event, fields })
        },
        makeSqlClient: () => Promise.reject(new Error('postgres is down')),
      }
      const store = makeProviderAccountTokenCustodyStoreForEnv(
        env,
        brokenOptions,
      )

      // The custody write succeeds against D1 authority despite the dead twin.
      await store.upsertConnectedAuth(
        custodyRecord({ providerAccountRef: 'provider-account.failsoft' }),
        auditEvent('audit.failsoft', {
          providerAccountRef: 'provider-account.failsoft',
        }),
      )
      const persisted = await sqlite!.db
        .prepare(
          `SELECT refresh_ciphertext_b64 FROM provider_account_token_custody WHERE provider_account_ref = 'provider-account.failsoft'`,
        )
        .first<{ refresh_ciphertext_b64: string }>()
      expect(persisted?.refresh_ciphertext_b64).toBe(
        'CIPHERTEXT-refresh-must-never-leak',
      )

      // The mirror failure surfaced as a typed diagnostic…
      expect(broken.map(d => d.event)).toContain(
        'khala_sync_identity_dual_write_failed',
      )
      // …carrying row KEYS only. NO ciphertext / IV / key material leaks.
      for (const diagnostic of broken) {
        const line = JSON.stringify(diagnostic)
        expect(line).not.toContain('CIPHERTEXT')
        expect(line).not.toContain('iv-refresh')
        expect(line).not.toContain('iv-access')
      }
      // The custody-table diagnostic keyed on the safe PK (provider_account_ref).
      expect(
        broken.some(d =>
          d.fields.refs.includes('provider-account.failsoft'),
        ),
      ).toBe(true)
    })
  },
)

// ---------------------------------------------------------------------------
// #8362 follow-up: makeOpenAuthStorageForEnv — the drop-in with the NEW
// `mirrorDeleteByKey` capability (the only hard-delete writer in this
// domain: `remove()`). Verifies set→mirror AND remove→delete-mirror, plus
// custody redaction (value_json never leaks into a diagnostic).
// ---------------------------------------------------------------------------

describe.skipIf(!hasLocalPostgres())(
  'makeOpenAuthStorageForEnv mirrors set() and delete-mirrors remove()',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let sqlite: ReturnType<typeof makeSqliteD1> | undefined
    let env: IdentityAuthStoreEnv
    let options: MakeIdentityAuthStoreOptions
    const diagnostics: Array<{
      event: IdentityAuthDiagnosticEvent
      fields: IdentityAuthDiagnostic
    }> = []

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE identity_auth_openauth_storage')
      await admin.end({ timeout: 5 })
      const raw = postgres(pg.urlFor('identity_auth_openauth_storage'), {
        max: 4,
        prepare: false,
      })
      client = raw as unknown as PgClient
      await raw.unsafe(readFileSync(MIGRATION_0028, 'utf8'))

      sqlite = makeSqliteD1()
      sqlite.exec(IDENTITY_AUTH_DOMAIN_D1_SCHEMA)

      env = {
        KHALA_SYNC_DB: { connectionString: 'postgres://contract' },
        OPENAGENTS_DB: sqlite.db,
      }
      options = {
        db: sqlite.db,
        log: (event, fields) => {
          diagnostics.push({ event, fields })
        },
        makeSqlClient: () =>
          Promise.resolve({
            end: () => Promise.resolve(),
            sql: raw as never,
          }),
      }
    }, 120_000)

    afterAll(async () => {
      await client?.end({ timeout: 5 })
      await pg?.stop()
      sqlite?.close()
    }, 60_000)

    test('set() mirrors the row byte-faithfully; remove() converges the delete to Postgres', async () => {
      const storage = makeOpenAuthStorageForEnv(env, undefined, options)
      const key = ['session', 'contract-1']
      const storageKey = joinKey(key)

      await storage.set(key, { hello: 'world' }, undefined)

      expect(diagnostics).toEqual([])

      const pgRowsAfterSet = await (client as PgClient).unsafe(
        `SELECT key, value_json FROM openauth_storage WHERE key = $1`,
        [storageKey],
      )
      expect(pgRowsAfterSet.length).toBe(1)
      expect(JSON.parse(String(pgRowsAfterSet[0]?.['value_json']))).toEqual({
        hello: 'world',
      })

      // D1 remains sole authority — the mirror never invents a read path.
      const fromStorage = await storage.get(key)
      expect(fromStorage).toEqual({ hello: 'world' })

      await storage.remove(key)

      const d1RowsAfterRemove = await sqlite!.db
        .prepare(`SELECT key FROM openauth_storage WHERE key = ?`)
        .bind(storageKey)
        .all<{ key: string }>()
      expect(d1RowsAfterRemove.results.length).toBe(0)

      const pgRowsAfterRemove = await (client as PgClient).unsafe(
        `SELECT key FROM openauth_storage WHERE key = $1`,
        [storageKey],
      )
      expect(
        pgRowsAfterRemove.length,
        'remove() must delete-mirror the Postgres twin, not just leave it stale',
      ).toBe(0)
      expect(diagnostics).toEqual([])
    })

    test('a broken Postgres twin never fails set()/remove(), and value_json never leaks', async () => {
      const broken: Array<{
        event: IdentityAuthDiagnosticEvent
        fields: IdentityAuthDiagnostic
      }> = []
      const brokenOptions: MakeIdentityAuthStoreOptions = {
        db: sqlite!.db,
        log: (event, fields) => {
          broken.push({ event, fields })
        },
        makeSqlClient: () => Promise.reject(new Error('postgres is down')),
      }
      const storage = makeOpenAuthStorageForEnv(env, undefined, brokenOptions)
      const key = ['session', 'contract-failsoft']
      const storageKey = joinKey(key)

      await storage.set(key, { secretPayload: 'never-leak-this' }, undefined)
      const persisted = await sqlite!.db
        .prepare(`SELECT value_json FROM openauth_storage WHERE key = ?`)
        .bind(storageKey)
        .first<{ value_json: string }>()
      expect(JSON.parse(String(persisted?.value_json))).toEqual({
        secretPayload: 'never-leak-this',
      })

      await storage.remove(key)

      expect(broken.map(d => d.event)).toContain(
        'khala_sync_identity_dual_write_failed',
      )
      for (const diagnostic of broken) {
        const line = JSON.stringify(diagnostic)
        expect(line).not.toContain('secretPayload')
        expect(line).not.toContain('never-leak-this')
      }
    })
  },
)
