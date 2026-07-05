// KS-8.16 follow-up (#8358): the Postgres→D1 mirror-back contract suite.
//
// The prior blocker on wiring `makePostgresForgeGitCanonicalStore` as write
// authority was that it had NO path to mirror its writes back into D1 —
// flipping write authority would have made the existing FAIL-SOFT-to-D1
// read fallback (`KHALA_SYNC_FORGE_READS=postgres`) silently serve STALE D1
// state on any Postgres read error. This suite proves the mirror-back this
// pass adds: after a real Postgres write (the same `pg_advisory_xact_lock`
// + `SELECT ... FOR UPDATE` port proven in
// `forge-git-canonical-postgres-store.test.ts`), the RESOLVED rows
// converge-upsert into a real D1 twin (SQLite, the engine D1 is built on),
// fail-soft with bounded retry, never failing the already-committed
// Postgres write.
//
// Runs against a real throwaway local Postgres (skips cleanly when no local
// Postgres binaries exist) + real SQLite as the D1 double — the same
// doubles the sibling suites already use.

import {
  hasLocalPostgres,
  startLocalPostgres,
} from '@openagentsinc/khala-sync-server/test/local-postgres'
import { readFileSync } from 'node:fs'
import * as path from 'node:path'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'

import type { ForgeGitCanonicalD1MirrorLog } from './forge-git-canonical-postgres-store'
import { makePostgresForgeGitCanonicalStore } from './forge-git-canonical-postgres-store'
import type { ForgeGitPackfileRefUpdate } from './forge-git-packfile-archive-store'
import { FORGE_DOMAIN_D1_SCHEMA, makeSqliteD1, type SqliteD1 } from './test/sqlite-d1'

const MIGRATION_0021 = path.resolve(
  __dirname,
  '../../../../../packages/khala-sync-server/migrations/0021_forge_domain.sql',
)
const MIGRATION_0035 = path.resolve(
  __dirname,
  '../../../../../packages/khala-sync-server/migrations/0035_forge_domain_ref_lock_uniques.sql',
)

const T0 = '2026-07-05T00:00:00.000Z'
const T1 = '2026-07-05T01:00:00.000Z'
const ZERO = '0'.repeat(40)
const OID_A = 'a'.repeat(40)
const OID_B = 'b'.repeat(40)
const SHA_1 = '1'.repeat(64)
const SHA_2 = '2'.repeat(64)

const TENANT = 'tenant.d1-mirror'
const REPO = 'repo.d1-mirror'

const createUpdate = (
  refName: string,
  newObjectId: string,
): ForgeGitPackfileRefUpdate => ({
  action: 'create',
  newObjectId,
  oldObjectId: ZERO,
  refName,
})

const updateUpdate = (
  refName: string,
  oldObjectId: string,
  newObjectId: string,
): ForgeGitPackfileRefUpdate => ({
  action: 'update',
  newObjectId,
  oldObjectId,
  refName,
})

type LoggedEvent = Readonly<{
  event: string
  op: string
  refs: ReadonlyArray<string>
}>

const makeCapturingLog = (): { log: ForgeGitCanonicalD1MirrorLog; events: Array<LoggedEvent> } => {
  const events: Array<LoggedEvent> = []
  return {
    events,
    log: (event, fields) => {
      events.push({ event, op: fields.op, refs: fields.refs })
    },
  }
}

/** A D1Database double whose `prepare().bind().run()` throws for the first
 * `failures` calls, then delegates to a real SQLite-backed D1. Proves the
 * bounded-retry recovery path without a real flaky connection. */
const makeFlakyD1 = (real: SqliteD1['db'], failures: number): SqliteD1['db'] => {
  let remaining = failures
  return {
    ...real,
    prepare: (query: string) => {
      const realStatement = real.prepare(query)
      return {
        ...realStatement,
        bind: (...values: ReadonlyArray<unknown>) => {
          const bound = realStatement.bind(...values)
          return {
            ...bound,
            run: async () => {
              if (remaining > 0) {
                remaining -= 1
                throw new Error('flaky d1 mirror failure (test double)')
              }
              return bound.run()
            },
          }
        },
      }
    },
  } as unknown as SqliteD1['db']
}

/** A D1Database double whose writes always throw. */
const makeBrokenD1 = (): SqliteD1['db'] =>
  ({
    prepare: () => ({
      bind: () => ({
        run: async () => {
          throw new Error('broken d1 mirror (test double)')
        },
      }),
    }),
  }) as unknown as SqliteD1['db']

describe.skipIf(!hasLocalPostgres())(
  'makePostgresForgeGitCanonicalStore — Postgres write mirror-back to D1',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: { end: (options?: { timeout?: number }) => Promise<void> }
    let sqlite: SqliteD1
    let pgRaw: import('postgres').Sql

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE forge_d1_mirror_contract')
      await admin.end({ timeout: 5 })
      const raw = postgres(pg.urlFor('forge_d1_mirror_contract'), {
        max: 8,
        prepare: false,
      })
      client = raw as unknown as { end: (options?: { timeout?: number }) => Promise<void> }
      await raw.unsafe(readFileSync(MIGRATION_0021, 'utf8'))
      await raw.unsafe(readFileSync(MIGRATION_0035, 'utf8'))
      pgRaw = raw

      sqlite = makeSqliteD1()
      sqlite.exec(FORGE_DOMAIN_D1_SCHEMA)
    }, 120_000)

    afterAll(async () => {
      await client?.end({ timeout: 5 })
      await pg?.stop()
      sqlite?.close()
    }, 60_000)

    const rawSql = () => pgRaw

    test('applyReceivePack create mirrors the resolved ref, object, and intake into D1', async () => {
      const refName = 'refs/heads/main'
      const store = makePostgresForgeGitCanonicalStore(rawSql() as never, {
        d1: sqlite.db,
      })

      const result = await store.applyReceivePack({
        changeRef: 'change.1',
        nowIso: T0,
        objectFormat: 'sha1',
        packfileBytes: 128,
        packfileRef: 'packfile.1',
        packfileSha256: SHA_1,
        receivePackRef: 'receive-pack.1',
        refUpdates: [createUpdate(refName, OID_A)],
        repositoryRef: REPO,
        sourceRefs: ['source.1'],
        subjectRef: 'subject.1',
        tenantRef: TENANT,
        tokenRef: 'token.1',
      })
      expect(result.refs[0]?.object_id).toBe(OID_A)

      const d1Ref = await sqlite.db
        .prepare(
          'SELECT * FROM forge_git_refs WHERE tenant_ref = ? AND repository_ref = ? AND ref_name = ?',
        )
        .bind(TENANT, REPO, refName)
        .first<Record<string, unknown>>()
      expect(d1Ref?.['object_id']).toBe(OID_A)
      expect(d1Ref?.['state']).toBe('active')

      const d1Object = await sqlite.db
        .prepare(
          'SELECT * FROM forge_git_objects WHERE tenant_ref = ? AND repository_ref = ? AND object_id = ?',
        )
        .bind(TENANT, REPO, OID_A)
        .first<Record<string, unknown>>()
      expect(d1Object?.['packfile_sha256']).toBe(SHA_1)

      const d1Intake = await sqlite.db
        .prepare(
          'SELECT * FROM forge_git_receive_pack_intakes WHERE tenant_ref = ? AND receive_pack_ref = ?',
        )
        .bind(TENANT, 'receive-pack.1')
        .first<Record<string, unknown>>()
      expect(d1Intake?.['state']).toBe('accepted')
    })

    test('applyReceivePack update mirrors the fast-forwarded ref (previous_object_id included)', async () => {
      const refName = 'refs/heads/ff'
      const store = makePostgresForgeGitCanonicalStore(rawSql() as never, {
        d1: sqlite.db,
      })

      await store.applyReceivePack({
        changeRef: 'change.ff.1',
        nowIso: T0,
        objectFormat: 'sha1',
        packfileBytes: 64,
        packfileRef: 'packfile.ff.1',
        packfileSha256: SHA_1,
        receivePackRef: 'receive-pack.ff.1',
        refUpdates: [createUpdate(refName, OID_A)],
        repositoryRef: REPO,
        sourceRefs: ['source.ff.1'],
        subjectRef: 'subject.ff.1',
        tenantRef: TENANT,
        tokenRef: 'token.ff.1',
      })

      await store.applyReceivePack({
        changeRef: 'change.ff.2',
        nowIso: T1,
        objectFormat: 'sha1',
        packfileBytes: 64,
        packfileRef: 'packfile.ff.2',
        packfileSha256: SHA_2,
        receivePackRef: 'receive-pack.ff.2',
        refUpdates: [updateUpdate(refName, OID_A, OID_B)],
        repositoryRef: REPO,
        sourceRefs: ['source.ff.2'],
        subjectRef: 'subject.ff.2',
        tenantRef: TENANT,
        tokenRef: 'token.ff.2',
      })

      const d1Ref = await sqlite.db
        .prepare(
          'SELECT * FROM forge_git_refs WHERE tenant_ref = ? AND repository_ref = ? AND ref_name = ?',
        )
        .bind(TENANT, REPO, refName)
        .first<Record<string, unknown>>()
      expect(d1Ref?.['object_id']).toBe(OID_B)
      expect(d1Ref?.['previous_object_id']).toBe(OID_A)
    })

    test('importExternalRef mirrors the resolved ref and object into D1', async () => {
      const refName = 'refs/heads/imported'
      const store = makePostgresForgeGitCanonicalStore(rawSql() as never, {
        d1: sqlite.db,
      })

      const result = await store.importExternalRef({
        changeRef: 'change.import.1',
        nowIso: T0,
        objectFormat: 'sha1',
        objectId: OID_A,
        packfileRef: 'packfile.import.1',
        receivePackRef: 'receive-pack.import.1',
        refName,
        repositoryRef: REPO,
        sourceDigestSha256: SHA_1,
        sourceRefs: ['source.import.1'],
        tenantRef: TENANT,
      })
      expect(result.changed).toBe(true)

      const d1Ref = await sqlite.db
        .prepare(
          'SELECT * FROM forge_git_refs WHERE tenant_ref = ? AND repository_ref = ? AND ref_name = ?',
        )
        .bind(TENANT, REPO, refName)
        .first<Record<string, unknown>>()
      expect(d1Ref?.['object_id']).toBe(OID_A)
    })

    test('a broken D1 mirror NEVER fails the write, and logs the typed failure diagnostic', async () => {
      const { events, log } = makeCapturingLog()
      const store = makePostgresForgeGitCanonicalStore(rawSql() as never, {
        d1: makeBrokenD1(),
        log,
        wait: () => Promise.resolve(),
      })

      const result = await store.applyReceivePack({
        changeRef: 'change.broken.1',
        nowIso: T0,
        objectFormat: 'sha1',
        packfileBytes: 64,
        packfileRef: 'packfile.broken.1',
        packfileSha256: SHA_1,
        receivePackRef: 'receive-pack.broken.1',
        refUpdates: [createUpdate('refs/heads/broken-mirror', OID_A)],
        repositoryRef: REPO,
        sourceRefs: ['source.broken.1'],
        subjectRef: 'subject.broken.1',
        tenantRef: TENANT,
        tokenRef: 'token.broken.1',
      })

      // The Postgres write itself succeeded despite the dead D1 mirror.
      expect(result.refs[0]?.object_id).toBe(OID_A)
      const readBack = await store.readRef(TENANT, REPO, 'refs/heads/broken-mirror')
      expect(readBack?.object_id).toBe(OID_A)

      const failedEvents = events.filter(
        e => e.event === 'khala_sync_forge_postgres_write_mirror_failed',
      )
      // One failure per mirrored table (refs, objects, intake) — the
      // per-table retry loop is independent, so a fully-broken D1 fails
      // all three, each with its own typed diagnostic.
      expect(failedEvents.map(e => e.op).sort()).toEqual(
        [
          'mirror-to-d1:forge_git_receive_pack_intakes',
          'mirror-to-d1:forge_git_refs',
          'mirror-to-d1:forge_git_objects',
        ].sort(),
      )
      // Diagnostic refs are row KEYS (tenant/repo/ref_name or
      // tenant/repo/object_id) — legitimate, public-safe git identifiers,
      // never token hashes/prefixes (the one custody-bearing column family
      // in this domain, per `forge-domain-store.ts`'s custody discipline).
      for (const e of failedEvents) {
        expect(e.refs.length).toBeGreaterThan(0)
        expect(e.refs.every(ref => ref.startsWith(`${TENANT}/`))).toBe(true)
      }
    })

    test('a transient D1 failure recovers on retry with no failure logged', async () => {
      const { events, log } = makeCapturingLog()
      const store = makePostgresForgeGitCanonicalStore(rawSql() as never, {
        d1: makeFlakyD1(sqlite.db, 1),
        log,
        wait: () => Promise.resolve(),
      })

      await store.applyReceivePack({
        changeRef: 'change.flaky.1',
        nowIso: T0,
        objectFormat: 'sha1',
        packfileBytes: 64,
        packfileRef: 'packfile.flaky.1',
        packfileSha256: SHA_1,
        receivePackRef: 'receive-pack.flaky.1',
        refUpdates: [createUpdate('refs/heads/flaky-mirror', OID_A)],
        repositoryRef: REPO,
        sourceRefs: ['source.flaky.1'],
        subjectRef: 'subject.flaky.1',
        tenantRef: TENANT,
        tokenRef: 'token.flaky.1',
      })

      const d1Ref = await sqlite.db
        .prepare(
          'SELECT * FROM forge_git_refs WHERE tenant_ref = ? AND repository_ref = ? AND ref_name = ?',
        )
        .bind(TENANT, REPO, 'refs/heads/flaky-mirror')
        .first<Record<string, unknown>>()
      expect(d1Ref?.['object_id']).toBe(OID_A)

      expect(events.some(e => e.event === 'khala_sync_forge_postgres_write_mirror_retry')).toBe(
        true,
      )
      expect(
        events.some(e => e.event === 'khala_sync_forge_postgres_write_mirror_failed'),
      ).toBe(false)
    })

    test('no mirror deps: behaves exactly as before (Postgres-only, no D1 side effect)', async () => {
      const store = makePostgresForgeGitCanonicalStore(rawSql() as never)
      const refName = 'refs/heads/no-mirror'

      await store.applyReceivePack({
        changeRef: 'change.no-mirror.1',
        nowIso: T0,
        objectFormat: 'sha1',
        packfileBytes: 64,
        packfileRef: 'packfile.no-mirror.1',
        packfileSha256: SHA_1,
        receivePackRef: 'receive-pack.no-mirror.1',
        refUpdates: [createUpdate(refName, OID_A)],
        repositoryRef: REPO,
        sourceRefs: ['source.no-mirror.1'],
        subjectRef: 'subject.no-mirror.1',
        tenantRef: TENANT,
        tokenRef: 'token.no-mirror.1',
      })

      const d1Ref = await sqlite.db
        .prepare(
          'SELECT * FROM forge_git_refs WHERE tenant_ref = ? AND repository_ref = ? AND ref_name = ?',
        )
        .bind(TENANT, REPO, refName)
        .first<Record<string, unknown>>()
      expect(d1Ref).toBeNull()
    })
  },
)
