// KS-8.16 follow-up (#8358): the Forge ref-lock protocol PORT contract
// suite. Proves `makePostgresForgeGitCanonicalStore`
// (`forge-git-canonical-postgres-store.ts`) — a real
// `pg_advisory_xact_lock` + `SELECT ... FOR UPDATE` transaction port,
// replacing the D1 held/applied/rejected lock-row dance — is behaviorally
// equivalent to the D1 store for ordinary sequential use AND, more
// importantly, actually serializes concurrent writers to the SAME ref
// without corrupting state or relying on any lock bookkeeping row.
//
// Runs against a real throwaway local Postgres (skips cleanly when no
// local Postgres binaries exist — the same `hasLocalPostgres()` gate every
// other KS-8 Postgres contract suite uses).
//
// NOT tested here because it is out of scope: production wiring. This
// store has no call site yet (see the file header) — that is the
// documented remaining work on issue #8358.

import {
  hasLocalPostgres,
  startLocalPostgres,
} from '@openagentsinc/khala-sync-server/test/local-postgres'
import { readFileSync } from 'node:fs'
import * as path from 'node:path'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'

import { makePostgresForgeGitCanonicalStore } from './forge-git-canonical-postgres-store'
import { ForgeGitCanonicalStoreError, type ForgeGitCanonicalStore } from './forge-git-canonical-store'
import type { ForgeGitPackfileRefUpdate } from './forge-git-packfile-archive-store'

const MIGRATION_0021 = path.resolve(
  __dirname,
  '../../../../../packages/khala-sync-server/migrations/0021_forge_domain.sql',
)

const T0 = '2026-07-05T00:00:00.000Z'
const T1 = '2026-07-05T01:00:00.000Z'
const ZERO = '0'.repeat(40)
const OID_A = 'a'.repeat(40)
const OID_B = 'b'.repeat(40)
const OID_C = 'c'.repeat(40)
const SHA_1 = '1'.repeat(64)
const SHA_2 = '2'.repeat(64)

const TENANT = 'tenant.lock-port'
const REPO = 'repo.lock-port'

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

describe.skipIf(!hasLocalPostgres())(
  'makePostgresForgeGitCanonicalStore — real FOR UPDATE ref-lock port',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: {
      end: (options?: { timeout?: number }) => Promise<void>
    }
    let store: ForgeGitCanonicalStore

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE forge_lock_port_contract')
      await admin.end({ timeout: 5 })
      const raw = postgres(pg.urlFor('forge_lock_port_contract'), {
        max: 8,
        prepare: false,
      })
      client = raw as unknown as { end: (options?: { timeout?: number }) => Promise<void> }
      await raw.unsafe(readFileSync(MIGRATION_0021, 'utf8'))
      store = makePostgresForgeGitCanonicalStore(raw as never)
    }, 120_000)

    afterAll(async () => {
      await client?.end({ timeout: 5 })
      await pg?.stop()
    }, 60_000)

    test('create → update → delete lifecycle matches D1 CAS semantics', async () => {
      const refName = 'refs/heads/main'

      const created = await store.applyReceivePack({
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
      expect(created.refs).toHaveLength(1)
      expect(created.refs[0]?.object_id).toBe(OID_A)
      expect(created.refs[0]?.state).toBe('active')
      expect(created.intake.state).toBe('accepted')

      const readBack = await store.readRef(TENANT, REPO, refName)
      expect(readBack?.object_id).toBe(OID_A)

      // A stale CAS (wrong old_object_id) is rejected before any write.
      await expect(
        store.applyReceivePack({
          changeRef: 'change.2',
          nowIso: T1,
          objectFormat: 'sha1',
          packfileBytes: 128,
          packfileRef: 'packfile.2',
          packfileSha256: SHA_2,
          receivePackRef: 'receive-pack.2',
          refUpdates: [updateUpdate(refName, OID_B, OID_C)],
          repositoryRef: REPO,
          sourceRefs: ['source.2'],
          subjectRef: 'subject.2',
          tenantRef: TENANT,
          tokenRef: 'token.2',
        }),
      ).rejects.toThrow(ForgeGitCanonicalStoreError)

      // Ref is unchanged after the rejected apply.
      const afterRejected = await store.readRef(TENANT, REPO, refName)
      expect(afterRejected?.object_id).toBe(OID_A)

      // The correct CAS succeeds.
      const updated = await store.applyReceivePack({
        changeRef: 'change.3',
        nowIso: T1,
        objectFormat: 'sha1',
        packfileBytes: 256,
        packfileRef: 'packfile.3',
        packfileSha256: SHA_2,
        receivePackRef: 'receive-pack.3',
        refUpdates: [updateUpdate(refName, OID_A, OID_B)],
        repositoryRef: REPO,
        sourceRefs: ['source.3'],
        subjectRef: 'subject.3',
        tenantRef: TENANT,
        tokenRef: 'token.3',
      })
      expect(updated.refs[0]?.object_id).toBe(OID_B)
      expect(updated.refs[0]?.previous_object_id).toBe(OID_A)

      const list = await store.listRefs(TENANT, REPO, { state: 'active' })
      expect(list.map(ref => ref.ref_name)).toContain(refName)
    })

    test('importExternalRef is idempotent: identical replay reports changed=false', async () => {
      const refName = 'refs/heads/imported'
      const first = await store.importExternalRef({
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
      expect(first.changed).toBe(true)
      expect(first.ref.object_id).toBe(OID_A)

      const replay = await store.importExternalRef({
        changeRef: 'change.import.1',
        nowIso: T1,
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
      expect(replay.changed).toBe(false)
      // updated_at must NOT have moved on the no-op replay.
      expect(replay.ref.updated_at).toBe(first.ref.updated_at)
    })

    test('two concurrent CREATEs of the SAME brand-new ref: exactly one wins, no corruption', async () => {
      const refName = 'refs/heads/concurrent-create'

      const attempt = (receivePackRef: string, objectId: string) =>
        store
          .applyReceivePack({
            changeRef: `change.${receivePackRef}`,
            nowIso: T0,
            objectFormat: 'sha1',
            packfileBytes: 64,
            packfileRef: `packfile.${receivePackRef}`,
            packfileSha256: SHA_1,
            receivePackRef,
            refUpdates: [createUpdate(refName, objectId)],
            repositoryRef: REPO,
            sourceRefs: [receivePackRef],
            subjectRef: `subject.${receivePackRef}`,
            tenantRef: TENANT,
            tokenRef: `token.${receivePackRef}`,
          })
          .then(result => ({ ok: true as const, result }))
          .catch(error => ({ error, ok: false as const }))

      // Fired truly concurrently — this is the actual test of the
      // `pg_advisory_xact_lock` mutex: without it, both transactions could
      // observe "no row yet" and both INSERT, corrupting the ref to
      // whichever commits last with no CAS rejection at all.
      const [left, right] = await Promise.all([
        attempt('receive-pack.race-a', OID_A),
        attempt('receive-pack.race-b', OID_B),
      ])

      const outcomes = [left, right]
      const wins = outcomes.filter(outcome => outcome.ok)
      const losses = outcomes.filter(outcome => !outcome.ok)
      expect(wins).toHaveLength(1)
      expect(losses).toHaveLength(1)
      expect((losses[0] as { error: unknown }).error).toBeInstanceOf(
        ForgeGitCanonicalStoreError,
      )

      const finalRef = await store.readRef(TENANT, REPO, refName)
      expect(finalRef?.state).toBe('active')
      const winningObjectId =
        wins[0]!.ok && wins[0]!.result.refs[0]?.object_id
      expect(finalRef?.object_id).toBe(winningObjectId)
      expect([OID_A, OID_B]).toContain(finalRef?.object_id)
    })

    test('two concurrent UPDATEs racing the same old_object_id: exactly one wins', async () => {
      const refName = 'refs/heads/concurrent-update'
      await store.applyReceivePack({
        changeRef: 'change.race-update.seed',
        nowIso: T0,
        objectFormat: 'sha1',
        packfileBytes: 64,
        packfileRef: 'packfile.race-update.seed',
        packfileSha256: SHA_1,
        receivePackRef: 'receive-pack.race-update.seed',
        refUpdates: [createUpdate(refName, OID_A)],
        repositoryRef: REPO,
        sourceRefs: ['seed'],
        subjectRef: 'subject.seed',
        tenantRef: TENANT,
        tokenRef: 'token.seed',
      })

      const attempt = (receivePackRef: string, newObjectId: string) =>
        store
          .applyReceivePack({
            changeRef: `change.${receivePackRef}`,
            nowIso: T1,
            objectFormat: 'sha1',
            packfileBytes: 64,
            packfileRef: `packfile.${receivePackRef}`,
            packfileSha256: SHA_2,
            receivePackRef,
            refUpdates: [updateUpdate(refName, OID_A, newObjectId)],
            repositoryRef: REPO,
            sourceRefs: [receivePackRef],
            subjectRef: `subject.${receivePackRef}`,
            tenantRef: TENANT,
            tokenRef: `token.${receivePackRef}`,
          })
          .then(result => ({ ok: true as const, result }))
          .catch(error => ({ error, ok: false as const }))

      const [left, right] = await Promise.all([
        attempt('receive-pack.race-update-a', OID_B),
        attempt('receive-pack.race-update-b', OID_C),
      ])

      const outcomes = [left, right]
      expect(outcomes.filter(outcome => outcome.ok)).toHaveLength(1)
      expect(outcomes.filter(outcome => !outcome.ok)).toHaveLength(1)

      const finalRef = await store.readRef(TENANT, REPO, refName)
      expect([OID_B, OID_C]).toContain(finalRef?.object_id)
      expect(finalRef?.previous_object_id).toBe(OID_A)
    })

    test('never writes forge_git_ref_locks — there is no lock-row bookkeeping in this port', async () => {
      const raw = (await import('postgres')).default(
        pg.urlFor('forge_lock_port_contract'),
        { max: 1, prepare: false },
      )
      try {
        const rows: Array<{ count: string }> = await raw.unsafe(
          'SELECT COUNT(*)::text AS count FROM forge_git_ref_locks',
        )
        expect(rows[0]?.count).toBe('0')
      } finally {
        await raw.end({ timeout: 5 })
      }
    })
  },
)
