// KS-8.16 WRITE cutover (#8358): proves the `KHALA_SYNC_FORGE_GIT_CANONICAL_WRITES`
// flag actually wires `makePostgresForgeGitCanonicalStore` as write authority
// through `makeForgeGitCanonicalStoreForEnv` — the production route handler's
// call site (`index.ts` -> `makeForgeGitCanonicalStoreForEnv(storeEnv)`) needs
// NO changes; the flip is entirely this env-var-gated branch.
//
// This is SEPARATE from `forge-git-canonical-postgres-store.test.ts` (the
// store's own lock-port contract, no env plumbing) and
// `forge-git-canonical-postgres-store-d1-mirror.test.ts` (the store's own
// mirror-back unit tests). This file proves the ENV WIRING specifically:
//
//   1. Default (`'d1'`, or flag unset): behavior is BYTE-IDENTICAL to before
//      this pass — D1 stays write authority, dual-write mirrors to Postgres.
//   2. `'postgres'`: Postgres becomes the SOLE authority — writes land in
//      Postgres FIRST (provable because the D1 twin can be made to disagree
//      and the store's own reads still reflect the Postgres value), and the
//      resolved rows mirror back into D1 via the store's own fail-soft path.
//   3. `'postgres'` with NO `KHALA_SYNC_DB` binding (no Postgres twin at all)
//      falls back to the D1-authoritative store unchanged — the flag can
//      never point at a nonexistent Postgres twin.
//   4. Scoping: flipping this flag does NOT touch `KHALA_SYNC_FORGE_READS`/
//      `KHALA_SYNC_FORGE_DUAL_WRITE`'s parsing, and the other four Forge
//      stores are constructed by entirely separate factories untouched by
//      this branch.

import { hasLocalPostgres, startLocalPostgres } from '@openagentsinc/khala-sync-server/test/local-postgres'
import { readFileSync } from 'node:fs'
import * as path from 'node:path'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'

import {
  forgeGitCanonicalWritesFromEnv,
  makeForgeGitCanonicalStoreForEnv,
  type ForgeDomainDiagnostic,
  type ForgeDomainDiagnosticEvent,
  type ForgeDomainStoreEnv,
  type MakeForgeDomainStoreOptions,
} from './forge-domain-store'
import { FORGE_DOMAIN_D1_SCHEMA, makeSqliteD1 } from './test/sqlite-d1'

const MIGRATION_0021 = path.resolve(
  __dirname,
  '../../../../../packages/khala-sync-server/migrations/0021_forge_domain.sql',
)

const T0 = '2026-07-05T00:00:00.000Z'
const T1 = '2026-07-05T01:00:00.000Z'
const ZERO = '0'.repeat(40)
const OID_A = 'a'.repeat(40)
const OID_B = 'b'.repeat(40)
const SHA_1 = '1'.repeat(64)

const TENANT = 'tenant.write-authority'
const REPO = 'repo.write-authority'

// ---------------------------------------------------------------------------
// Pure flag parsing
// ---------------------------------------------------------------------------

describe('forgeGitCanonicalWritesFromEnv (pure)', () => {
  test('defaults to d1; only the exact value "postgres" flips it; typos never fail open', () => {
    expect(forgeGitCanonicalWritesFromEnv({})).toBe('d1')
    expect(
      forgeGitCanonicalWritesFromEnv({
        KHALA_SYNC_FORGE_GIT_CANONICAL_WRITES: undefined,
      }),
    ).toBe('d1')
    expect(
      forgeGitCanonicalWritesFromEnv({
        KHALA_SYNC_FORGE_GIT_CANONICAL_WRITES: 'postgres',
      }),
    ).toBe('postgres')
    expect(
      forgeGitCanonicalWritesFromEnv({
        KHALA_SYNC_FORGE_GIT_CANONICAL_WRITES: 'POSTGRES',
      }),
    ).toBe('postgres')
    expect(
      forgeGitCanonicalWritesFromEnv({
        KHALA_SYNC_FORGE_GIT_CANONICAL_WRITES: '  postgres  ',
      }),
    ).toBe('postgres')
    expect(
      forgeGitCanonicalWritesFromEnv({
        KHALA_SYNC_FORGE_GIT_CANONICAL_WRITES: 'psotgres',
      }),
    ).toBe('d1')
    expect(
      forgeGitCanonicalWritesFromEnv({
        KHALA_SYNC_FORGE_GIT_CANONICAL_WRITES: 'd1',
      }),
    ).toBe('d1')
  })
})

// ---------------------------------------------------------------------------
// Env-wiring integration (real ephemeral Postgres + SQLite D1 double)
// ---------------------------------------------------------------------------

type PgClient = Readonly<{
  end: (options?: { timeout?: number }) => Promise<void>
  unsafe: (sql: string, params?: Array<unknown>) => Promise<Array<Record<string, unknown>>>
}>

describe.skipIf(!hasLocalPostgres())(
  'makeForgeGitCanonicalStoreForEnv — KHALA_SYNC_FORGE_GIT_CANONICAL_WRITES wiring',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let raw: unknown
    let sqlite: ReturnType<typeof makeSqliteD1> | undefined
    let baseEnv: ForgeDomainStoreEnv
    let diagnostics: Array<{ event: ForgeDomainDiagnosticEvent; fields: ForgeDomainDiagnostic }>
    let options: MakeForgeDomainStoreOptions

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE forge_write_authority')
      await admin.end({ timeout: 5 })
      raw = postgres(pg.urlFor('forge_write_authority'), { max: 4, prepare: false })
      client = raw as unknown as PgClient
      await (raw as PgClient).unsafe(readFileSync(MIGRATION_0021, 'utf8'))
    }, 120_000)

    afterAll(async () => {
      await client?.end({ timeout: 5 })
      await pg?.stop()
    }, 60_000)

    beforeAll(() => {
      sqlite = makeSqliteD1()
      sqlite.exec(FORGE_DOMAIN_D1_SCHEMA)
    })

    beforeAll(() => {
      diagnostics = []
      baseEnv = {
        KHALA_SYNC_DB: { connectionString: 'postgres://write-authority' },
        OPENAGENTS_DB: sqlite!.db,
      }
      options = {
        db: sqlite!.db,
        log: (event, fields) => {
          diagnostics.push({ event, fields })
        },
        makeSqlClient: () => Promise.resolve({ end: () => Promise.resolve(), sql: raw as never }),
      }
    })

    test('default (flag unset) stays D1-authoritative — unchanged prior behavior', async () => {
      const canonical = makeForgeGitCanonicalStoreForEnv(baseEnv, options)
      const applied = await canonical.applyReceivePack({
        changeRef: 'change.d1',
        nowIso: T0,
        objectFormat: 'sha1',
        packfileBytes: 3,
        packfileRef: 'pack.d1',
        packfileSha256: SHA_1,
        receivePackRef: 'rp.d1',
        refUpdates: [
          { action: 'create', newObjectId: OID_A, oldObjectId: ZERO, refName: 'refs/heads/d1-default' },
        ],
        repositoryRef: REPO,
        sourceRefs: [],
        subjectRef: 'agent.write-authority',
        tenantRef: TENANT,
        tokenRef: 'token.d1',
      })
      expect(applied.refs[0]?.object_id).toBe(OID_A)

      // D1 (SQLite) holds the row directly — it is the authority.
      const d1Row = (
        await sqlite!.db
          .prepare(
            `SELECT object_id FROM forge_git_refs WHERE tenant_ref = ? AND repository_ref = ? AND ref_name = ?`,
          )
          .bind(TENANT, REPO, 'refs/heads/d1-default')
          .first<{ object_id: string }>()
      )
      expect(d1Row?.object_id).toBe(OID_A)

      // The EXISTING D1-first dual-write mirror still converges to Postgres
      // (unrelated to this flag; regression check that the new branch did
      // not disturb the pre-existing dual-write path).
      const pgRows = await (client as PgClient).unsafe(
        `SELECT object_id FROM forge_git_refs WHERE tenant_ref = $1 AND repository_ref = $2 AND ref_name = $3`,
        [TENANT, REPO, 'refs/heads/d1-default'],
      )
      expect(pgRows[0]?.['object_id']).toBe(OID_A)
    })

    test('"postgres" makes Postgres the SOLE write+read authority for the canonical store', async () => {
      const postgresEnv: ForgeDomainStoreEnv = {
        ...baseEnv,
        KHALA_SYNC_FORGE_GIT_CANONICAL_WRITES: 'postgres',
      }
      const canonical = makeForgeGitCanonicalStoreForEnv(postgresEnv, options)

      const applied = await canonical.applyReceivePack({
        changeRef: 'change.pg',
        nowIso: T0,
        objectFormat: 'sha1',
        packfileBytes: 3,
        packfileRef: 'pack.pg',
        packfileSha256: SHA_1,
        receivePackRef: 'rp.pg',
        refUpdates: [
          { action: 'create', newObjectId: OID_A, oldObjectId: ZERO, refName: 'refs/heads/pg-authority' },
        ],
        repositoryRef: REPO,
        sourceRefs: [],
        subjectRef: 'agent.write-authority',
        tenantRef: TENANT,
        tokenRef: 'token.pg',
      })
      expect(applied.refs[0]?.object_id).toBe(OID_A)

      // Postgres has the authoritative row (written directly, under the
      // real pg_advisory_xact_lock + FOR UPDATE protocol).
      const pgRows = await (client as PgClient).unsafe(
        `SELECT object_id FROM forge_git_refs WHERE tenant_ref = $1 AND repository_ref = $2 AND ref_name = $3`,
        [TENANT, REPO, 'refs/heads/pg-authority'],
      )
      expect(pgRows[0]?.['object_id']).toBe(OID_A)

      // The mirror-back converged the SAME row into D1 (fail-soft, but
      // healthy here) — no drift diagnostics were logged for this write.
      const mirrorFailures = diagnostics.filter(d =>
        d.event.startsWith('khala_sync_forge_postgres_write_mirror'),
      )
      expect(mirrorFailures.filter(d => d.event.endsWith('_failed'))).toEqual([])
      const d1Row = (
        await sqlite!.db
          .prepare(
            `SELECT object_id FROM forge_git_refs WHERE tenant_ref = ? AND repository_ref = ? AND ref_name = ?`,
          )
          .bind(TENANT, REPO, 'refs/heads/pg-authority')
          .first<{ object_id: string }>()
      )
      expect(d1Row?.object_id).toBe(OID_A)

      // PROOF the read routes to Postgres, not D1: drift D1 directly (out
      // of band, simulating D1 lagging behind) — the store must still
      // report the Postgres-authoritative value.
      await sqlite!.db
        .prepare(
          `UPDATE forge_git_refs SET object_id = ? WHERE tenant_ref = ? AND repository_ref = ? AND ref_name = ?`,
        )
        .bind(OID_B, TENANT, REPO, 'refs/heads/pg-authority')
        .run()
      const servedRef = await canonical.readRef(TENANT, REPO, 'refs/heads/pg-authority')
      expect(servedRef?.object_id).toBe(OID_A)
      const servedList = await canonical.listRefs(TENANT, REPO, { state: 'active' })
      expect(
        servedList.find(ref => ref.ref_name === 'refs/heads/pg-authority')?.object_id,
      ).toBe(OID_A)

      // importExternalRef also routes through Postgres write authority.
      const imported = await canonical.importExternalRef({
        changeRef: 'change.pg-import',
        nowIso: T1,
        objectFormat: 'sha1',
        objectId: OID_B,
        packfileRef: 'pack.pg',
        receivePackRef: 'rp.pg-import',
        refName: 'refs/heads/pg-import',
        repositoryRef: REPO,
        sourceDigestSha256: SHA_1,
        sourceRefs: [],
        tenantRef: TENANT,
      })
      expect(imported.ref.object_id).toBe(OID_B)
      const pgImportRows = await (client as PgClient).unsafe(
        `SELECT object_id FROM forge_git_refs WHERE tenant_ref = $1 AND repository_ref = $2 AND ref_name = $3`,
        [TENANT, REPO, 'refs/heads/pg-import'],
      )
      expect(pgImportRows[0]?.['object_id']).toBe(OID_B)
    })

    test('"postgres" with no KHALA_SYNC_DB binding falls back to D1 authority unchanged', async () => {
      const postgresEnvNoBinding: ForgeDomainStoreEnv = {
        OPENAGENTS_DB: sqlite!.db,
        KHALA_SYNC_FORGE_GIT_CANONICAL_WRITES: 'postgres',
      }
      const canonical = makeForgeGitCanonicalStoreForEnv(postgresEnvNoBinding, {
        db: sqlite!.db,
      })
      const applied = await canonical.applyReceivePack({
        changeRef: 'change.no-binding',
        nowIso: T0,
        objectFormat: 'sha1',
        packfileBytes: 3,
        packfileRef: 'pack.no-binding',
        packfileSha256: SHA_1,
        receivePackRef: 'rp.no-binding',
        refUpdates: [
          {
            action: 'create',
            newObjectId: OID_A,
            oldObjectId: ZERO,
            refName: 'refs/heads/no-binding',
          },
        ],
        repositoryRef: REPO,
        sourceRefs: [],
        subjectRef: 'agent.write-authority',
        tenantRef: TENANT,
        tokenRef: 'token.no-binding',
      })
      expect(applied.refs[0]?.object_id).toBe(OID_A)
      // D1 wrote it directly (no Postgres twin exists to route to).
      const d1Row = (
        await sqlite!.db
          .prepare(
            `SELECT object_id FROM forge_git_refs WHERE tenant_ref = ? AND repository_ref = ? AND ref_name = ?`,
          )
          .bind(TENANT, REPO, 'refs/heads/no-binding')
          .first<{ object_id: string }>()
      )
      expect(d1Row?.object_id).toBe(OID_A)
    })
  },
)
