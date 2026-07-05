// KS-8.16 (#8327): forge domain repository CONTRACT suite.
//
// Three layers, one behavioral spec:
//
//  1. `ForgeDomainWriteStore` contract — the row seam's composite-PK
//     converge semantics run identically against BOTH implementations:
//     - D1: `makeD1ForgeDomainWriteStore` over real SQLite (node:sqlite —
//       the engine D1 is built on), schema from the worker migrations
//       (condensed in test/sqlite-d1.ts, D1 uniques KEPT).
//     - Postgres: `makePostgresForgeDomainStore` over a throwaway local
//       Postgres (initdb/pg_ctl), schema from khala-sync-server migration
//       0019. Skipped when no local Postgres binaries exist.
//
//  2. END-TO-END mirror fidelity — the REAL five forge stores run
//     UNCHANGED through the `makeForge*StoreForEnv` factories with SQLite
//     as D1 authority and the real Postgres store as the mirror: tenant
//     upsert, token mint/authenticate/revoke (custody discipline
//     asserted), packfile archive put (R2 faked, bytes NEVER touch a
//     relational row), receive-pack apply (locks + refs + objects +
//     intake), external ref import, coordination issue/PR/status,
//     dispatch lease double-fire idempotency, merge-queue ledger
//     converge, verification receipt, promotion decision, GitHub mirror
//     receipt attempt-count bump. Afterwards ALL SIXTEEN tables are
//     row-for-row IDENTICAL across both stores (registry-column
//     projection, value-normalized) with ZERO drift diagnostics.
//
//  3. Fail-soft + custody: a broken Postgres twin never fails a write
//     (typed diagnostic only), and the one token_hash-keyed mirror path
//     redacts its diagnostic refs — no custody value ever reaches a log
//     line. Compare-mode `listRefs` serves D1 and flags ref-set drift;
//     KHALA_SYNC_FORGE_READS=postgres (KS-8.16 follow-up #8358 read
//     cutover) SERVES the ref advertisement from the Postgres twin and is
//     fail-soft — a dead twin falls back to the D1 authority so the
//     advertisement can never break.

import {
  FORGE_DOMAIN_TABLE_SPECS,
  FORGE_DOMAIN_TABLES,
  normalizeForgeDomainValue,
  type CompareSoakSample,
  type ForgeDomainTable,
} from '@openagentsinc/khala-sync-server'
import {
  hasLocalPostgres,
  startLocalPostgres,
} from '@openagentsinc/khala-sync-server/test/local-postgres'
import { readFileSync } from 'node:fs'
import * as path from 'node:path'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'

import {
  forgeDomainFlagsFromEnv,
  makeD1ForgeDomainWriteStore,
  makeForgeCoordinationStoreForEnv,
  makeForgeGitCanonicalStoreForEnv,
  makeForgeGitHubMirrorStoreForEnv,
  makeForgeGitPackfileArchiveStoreForEnv,
  makeForgeTenantGitAuthStoreForEnv,
  makePostgresForgeDomainStore,
  type ForgeDomainDiagnostic,
  type ForgeDomainDiagnosticEvent,
  type ForgeDomainRow,
  type ForgeDomainStoreEnv,
  type ForgeDomainWriteStore,
  type MakeForgeDomainStoreOptions,
} from './forge-domain-store'
import { FORGE_DOMAIN_D1_SCHEMA, makeSqliteD1 } from './test/sqlite-d1'

const T0 = '2026-07-04T00:00:00.000Z'
const T1 = '2026-07-04T01:00:00.000Z'
const T2 = '2026-07-04T02:00:00.000Z'
const FUTURE = '2027-01-01T00:00:00.000Z'

const TENANT = 'tenant.contract'
const REPO = 'repo.core'

const ZERO40 = '0'.repeat(40)
const OID_A = 'a'.repeat(40)
const OID_B = 'b'.repeat(40)
const OID_C = 'c'.repeat(40)
const SHA_1 = '1'.repeat(64)
const SHA_2 = '2'.repeat(64)

// ---------------------------------------------------------------------------
// Flags (pure)
// ---------------------------------------------------------------------------

describe('forgeDomainFlagsFromEnv (pure)', () => {
  test('dual-write defaults ON; reads default d1; off-values disable', () => {
    expect(forgeDomainFlagsFromEnv({})).toEqual({
      dualWrite: true,
      reads: 'd1',
    })
    expect(
      forgeDomainFlagsFromEnv({ KHALA_SYNC_FORGE_DUAL_WRITE: 'off' }).dualWrite,
    ).toBe(false)
    expect(
      forgeDomainFlagsFromEnv({ KHALA_SYNC_FORGE_DUAL_WRITE: '0' }).dualWrite,
    ).toBe(false)
    expect(
      forgeDomainFlagsFromEnv({ KHALA_SYNC_FORGE_DUAL_WRITE: 'on' }).dualWrite,
    ).toBe(true)
    expect(
      forgeDomainFlagsFromEnv({ KHALA_SYNC_FORGE_READS: 'compare' }).reads,
    ).toBe('compare')
    expect(
      forgeDomainFlagsFromEnv({ KHALA_SYNC_FORGE_READS: 'postgres' }).reads,
    ).toBe('postgres')
    // A typo can never fail open into an unproven read path.
    expect(
      forgeDomainFlagsFromEnv({ KHALA_SYNC_FORGE_READS: 'psotgres' }).reads,
    ).toBe('d1')
  })
})

// ---------------------------------------------------------------------------
// Layer 1: write-store contract (both implementations)
// ---------------------------------------------------------------------------

type ContractHarness = Readonly<{
  store: ForgeDomainWriteStore
  query: (sql: string) => Promise<ReadonlyArray<Record<string, unknown>>>
}>

const refRow = (
  name: string,
  overrides: Partial<Record<string, unknown>> = {},
): ForgeDomainRow => ({
  created_at: T0,
  object_format: 'sha1',
  object_id: OID_A,
  previous_object_id: null,
  ref_name: name,
  repository_ref: REPO,
  source_refs_json: '[]',
  state: 'active',
  tenant_ref: TENANT,
  updated_at: T0,
  updated_by_change_ref: 'change.seed',
  updated_by_packfile_ref: 'pack.seed',
  updated_by_receive_pack_ref: 'rp.seed',
  ...overrides,
})

const specContractSuite = (harness: () => ContractHarness) => {
  test('upsertRows converges on the composite PK and is idempotent', async () => {
    const { query, store } = harness()
    expect(
      await store.upsertRows('forge_git_refs', [
        refRow('refs/heads/contract-a'),
        refRow('refs/heads/contract-b'),
      ]),
    ).toBe(2)
    // Re-run: converge, no duplication.
    expect(
      await store.upsertRows('forge_git_refs', [
        refRow('refs/heads/contract-a'),
        refRow('refs/heads/contract-b'),
      ]),
    ).toBe(2)
    const counted = await query(
      `SELECT COUNT(*) AS total FROM forge_git_refs WHERE ref_name LIKE 'refs/heads/contract-%'`,
    )
    expect(Number(counted[0]?.['total'])).toBe(2)

    // A newer snapshot wins (ref fast-forward).
    await store.upsertRows('forge_git_refs', [
      refRow('refs/heads/contract-a', {
        object_id: OID_B,
        previous_object_id: OID_A,
        updated_at: T1,
      }),
    ])
    const rows = await query(
      `SELECT object_id, previous_object_id FROM forge_git_refs WHERE ref_name = 'refs/heads/contract-a'`,
    )
    expect(rows[0]?.['object_id']).toBe(OID_B)
    expect(rows[0]?.['previous_object_id']).toBe(OID_A)
  })

  test('three-column composite keys never cross-contaminate', async () => {
    const { query, store } = harness()
    await store.upsertRows('forge_git_refs', [
      refRow('refs/heads/shared', { repository_ref: 'repo.one' }),
      refRow('refs/heads/shared', { repository_ref: 'repo.two' }),
    ])
    const rows = await query(
      `SELECT COUNT(*) AS total FROM forge_git_refs WHERE ref_name = 'refs/heads/shared'`,
    )
    expect(Number(rows[0]?.['total'])).toBe(2)
  })
}

describe('forge domain write-store contract — D1 (real SQLite)', () => {
  let sqlite: ReturnType<typeof makeSqliteD1> | undefined
  let harness: ContractHarness

  beforeAll(() => {
    sqlite = makeSqliteD1()
    sqlite.exec(FORGE_DOMAIN_D1_SCHEMA)
    harness = {
      query: async sql =>
        (await sqlite!.db.prepare(sql).all<Record<string, unknown>>())
          .results ?? [],
      store: makeD1ForgeDomainWriteStore(sqlite.db),
    }
  })

  afterAll(() => {
    sqlite?.close()
  })

  specContractSuite(() => harness)
})

const MIGRATION_0021 = path.resolve(
  __dirname,
  '../../../../../packages/khala-sync-server/migrations/0021_forge_domain.sql',
)

type PgClient = Readonly<{
  end: (options?: { timeout?: number }) => Promise<void>
  unsafe: (
    sql: string,
    params?: Array<unknown>,
  ) => Promise<Array<Record<string, unknown>>>
}>

describe.skipIf(!hasLocalPostgres())(
  'forge domain write-store contract — Postgres',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let harness: ContractHarness

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE forge_domain_contract')
      await admin.end({ timeout: 5 })
      const raw = postgres(pg.urlFor('forge_domain_contract'), {
        max: 4,
        prepare: false,
      })
      client = raw as unknown as PgClient
      await raw.unsafe(readFileSync(MIGRATION_0021, 'utf8'))
      harness = {
        query: async sql => (client as PgClient).unsafe(sql),
        store: makePostgresForgeDomainStore({
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
// Layer 2: end-to-end mirror fidelity through the REAL forge stores
// ---------------------------------------------------------------------------

const projectRow = (
  table: ForgeDomainTable,
  row: Record<string, unknown>,
): Record<string, string | null> =>
  Object.fromEntries(
    FORGE_DOMAIN_TABLE_SPECS[table].columns.map(column => {
      const value = normalizeForgeDomainValue(row[column])
      return [column, value === null ? null : String(value)]
    }),
  )

/** Minimal R2 double for the packfile archive store (head/put only). */
const makeFakeR2 = (): R2Bucket => {
  const objects = new Map<string, { size: number }>()
  return {
    get: () => Promise.resolve(null),
    head: (key: string) =>
      Promise.resolve(
        objects.has(key) ? ({ key } as unknown as R2Object) : null,
      ),
    put: (key: string, body: unknown) => {
      objects.set(key, { size: 0 })
      return Promise.resolve({ key, size: 0 } as unknown as R2Object)
    },
  } as unknown as R2Bucket
}

describe.skipIf(!hasLocalPostgres())(
  'forge store writes mirror byte-faithfully into Postgres',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let sqlite: ReturnType<typeof makeSqliteD1> | undefined
    let env: ForgeDomainStoreEnv
    let options: MakeForgeDomainStoreOptions
    const diagnostics: Array<{
      event: ForgeDomainDiagnosticEvent
      fields: ForgeDomainDiagnostic
    }> = []
    // Compare-mode soak observability (#8282 shared follow-up) — durable
    // samples emitted alongside the diagnostics above.
    const soakSamples: CompareSoakSample[] = []

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE forge_domain_mirror')
      await admin.end({ timeout: 5 })
      const raw = postgres(pg.urlFor('forge_domain_mirror'), {
        max: 4,
        prepare: false,
      })
      client = raw as unknown as PgClient
      await raw.unsafe(readFileSync(MIGRATION_0021, 'utf8'))

      sqlite = makeSqliteD1()
      sqlite.exec(FORGE_DOMAIN_D1_SCHEMA)

      env = {
        KHALA_SYNC_DB: { connectionString: 'postgres://contract' },
        OPENAGENTS_DB: sqlite.db,
      }
      options = {
        log: (event, fields) => {
          diagnostics.push({ event, fields })
        },
        makeSqlClient: () =>
          Promise.resolve({
            end: () => Promise.resolve(),
            sql: raw as never,
          }),
        metrics: { record: sample => soakSamples.push(sample) },
      }
    }, 120_000)

    afterAll(async () => {
      await client?.end({ timeout: 5 })
      await pg?.stop()
      sqlite?.close()
    }, 60_000)

    test('the full write surface mirrors, double-fires idempotently, and every table converges row-for-row', async () => {
      const auth = makeForgeTenantGitAuthStoreForEnv(env, options)
      const coordination = makeForgeCoordinationStoreForEnv(env, options)
      const canonical = makeForgeGitCanonicalStoreForEnv(env, options)
      const archive = makeForgeGitPackfileArchiveStoreForEnv(
        env,
        makeFakeR2(),
        options,
      )
      const githubMirror = makeForgeGitHubMirrorStoreForEnv(env, options)

      // Tenant + token custody family.
      await auth.upsertTenant({
        displayName: 'Contract Tenant',
        nowIso: T0,
        state: 'active',
        tenantRef: TENANT,
      })
      const minted = await auth.mintGitAccessToken(
        {
          expiresAt: FUTURE,
          nowIso: T0,
          repositoryRef: REPO,
          scopes: ['git:receive-pack', 'git:upload-pack'],
          sourceRefs: ['contract'],
          subjectRef: 'agent.contract',
          tenantRef: TENANT,
          tokenRef: 'token.contract.1',
        },
        { makeToken: () => `oa_forge_git_${'c'.repeat(43)}` },
      )
      // Authenticate (success path: last_used_at transition mirrors).
      const session = await auth.authenticateGitAccessToken({
        nowIso: T1,
        repositoryRef: REPO,
        requiredScope: 'git:receive-pack',
        token: minted.token,
      })
      expect(session?.tokenRef).toBe('token.contract.1')
      // Mint + revoke a second token (revoked-state transition mirrors).
      await auth.mintGitAccessToken(
        {
          expiresAt: FUTURE,
          nowIso: T0,
          repositoryRef: REPO,
          scopes: ['git:upload-pack'],
          sourceRefs: [],
          subjectRef: 'agent.contract',
          tenantRef: TENANT,
          tokenRef: 'token.contract.2',
        },
        { makeToken: () => `oa_forge_git_${'d'.repeat(43)}` },
      )
      await auth.revokeGitAccessToken(TENANT, 'token.contract.2', T1)

      // Packfile archive (R2 bytes faked; only metadata rows exist).
      const put = await archive.putPackfile({
        body: new Uint8Array([1, 2, 3]) as never,
        capabilities: ['report-status'],
        changeRef: 'change.1',
        nowIso: T0,
        objectFormat: 'sha1',
        packfileBytes: 3,
        packfileRef: 'pack.1',
        packfileSha256: SHA_1,
        receivePackRef: 'rp.1',
        refUpdates: [
          {
            action: 'create',
            newObjectId: OID_A,
            oldObjectId: ZERO40,
            refName: 'refs/heads/main',
          },
        ],
        repositoryRef: REPO,
        sourceRefs: [],
        tenantRef: TENANT,
      })
      expect(put.created).toBe(true)
      // Digest dedupe double-fire: same digest, different ref — the
      // resolved EXISTING record mirrors, no duplicate row appears.
      const deduped = await archive.putPackfile({
        body: new Uint8Array([1, 2, 3]) as never,
        capabilities: [],
        changeRef: 'change.1',
        nowIso: T1,
        objectFormat: 'sha1',
        packfileBytes: 3,
        packfileRef: 'pack.1-retry',
        packfileSha256: SHA_1,
        receivePackRef: 'rp.1',
        refUpdates: [],
        repositoryRef: REPO,
        sourceRefs: [],
        tenantRef: TENANT,
      })
      expect(deduped.created).toBe(false)

      // Receive-pack apply: locks held → refs written → objects deduped
      // → locks applied → intake recorded. All four tables mirror.
      const applied = await canonical.applyReceivePack({
        changeRef: 'change.1',
        nowIso: T1,
        objectFormat: 'sha1',
        packfileBytes: 3,
        packfileRef: 'pack.1',
        packfileSha256: SHA_1,
        receivePackRef: 'rp.1',
        refUpdates: [
          {
            action: 'create',
            newObjectId: OID_A,
            oldObjectId: ZERO40,
            refName: 'refs/heads/main',
          },
          {
            action: 'create',
            newObjectId: OID_B,
            oldObjectId: ZERO40,
            refName: 'refs/heads/dev',
          },
        ],
        repositoryRef: REPO,
        sourceRefs: [],
        subjectRef: 'agent.contract',
        tenantRef: TENANT,
        tokenRef: 'token.contract.1',
      })
      expect(applied.refs.length).toBe(2)

      // External ref import (mirror path for GitHub-sourced tips).
      await canonical.importExternalRef({
        changeRef: 'change.import',
        nowIso: T1,
        objectFormat: 'sha1',
        objectId: OID_C,
        packfileRef: 'pack.1',
        receivePackRef: 'rp.import',
        refName: 'refs/heads/import',
        repositoryRef: REPO,
        sourceDigestSha256: SHA_2,
        sourceRefs: [],
        tenantRef: TENANT,
      })

      // Coordination family.
      await coordination.upsertIssue({
        issueRef: 'issue.1',
        nowIso: T0,
        sourceRefs: [],
        state: 'open',
        tenantRef: TENANT,
        title: 'Port the forge domain',
      })
      await coordination.upsertChange({
        baseHead: OID_A,
        blockerRefs: [],
        changeRef: 'change.1',
        issueRef: 'issue.1',
        nowIso: T1,
        patchHead: OID_B,
        prRef: 'pr.1',
        sourceRefs: [],
        state: 'open',
        tenantRef: TENANT,
      })
      await coordination.recordStatus({
        actorRef: 'agent.contract',
        createdAt: T1,
        sourceRefs: [],
        state: 'open',
        statusRef: 'status.1',
        subjectRef: 'issue.1',
        tenantRef: TENANT,
      })

      // Dispatch lease DOUBLE-FIRE idempotency: the second acquire for
      // the same live work_ref must not acquire, and the mirror must
      // still converge to exactly the D1 rows.
      const lease1 = await coordination.acquireDispatchLease({
        acquiredAt: T1,
        expiresAt: FUTURE,
        leaseRef: 'lease.1',
        ownerAgentRef: 'agent.contract',
        sourceRefs: [],
        tenantRef: TENANT,
        workRef: 'work.1',
      })
      expect(lease1.acquired).toBe(true)
      const lease2 = await coordination.acquireDispatchLease({
        acquiredAt: T2,
        expiresAt: FUTURE,
        leaseRef: 'lease.2',
        ownerAgentRef: 'agent.other',
        sourceRefs: [],
        tenantRef: TENANT,
        workRef: 'work.1',
      })
      expect(lease2.acquired).toBe(false)

      // Merge-queue ledger converge (same queue twice = one row).
      await coordination.recordMergeQueueLedger({
        actualHead: OID_A,
        baseHead: OID_A,
        blocked: [],
        nowIso: T1,
        queueRef: 'queue.main',
        ready: ['change.1'],
        sourceRefs: [],
        state: 'projected',
        tenantRef: TENANT,
        virtualHead: OID_B,
      })
      await coordination.recordMergeQueueLedger({
        actualHead: OID_B,
        baseHead: OID_A,
        blocked: [],
        nowIso: T2,
        queueRef: 'queue.main',
        ready: [],
        sourceRefs: [],
        state: 'promoted',
        tenantRef: TENANT,
        virtualHead: OID_B,
      })

      // Receipts.
      await coordination.recordVerificationReceipt(
        {
          artifact_refs: [],
          base_head: OID_A,
          base_ref: 'refs/heads/main',
          change_ref: 'change.1',
          command_args: ['test'],
          command_ref: 'verify.suite',
          completed_at: T2,
          executor_identity_ref: 'executor.contract',
          exit_code: 0,
          head_head: OID_B,
          head_ref: 'refs/forge/change.1',
          log_sha256: SHA_2,
          packfile_ref: 'pack.1',
          packfile_sha256: SHA_1,
          redacted: true,
          repository_ref: REPO,
          schema: 'openagents.forge.verification.receipt.v0.1',
          source_refs: [],
          started_at: T1,
          tenant_ref: TENANT,
          verdict: 'passed',
          verification_ref: 'verify.1',
        },
        T2,
      )
      await coordination.recordPromotionDecisionReceipt(
        {
          base_head: OID_A,
          blocker_refs: [],
          candidate_head: OID_B,
          change_ref: 'change.1',
          decided_at: T2,
          decided_by_ref: 'forge.merge_queue',
          decision: 'approved',
          gate_refs: [],
          gate_results: [],
          promoted_head: OID_B,
          promotion_ref: 'promo.1',
          queue_position: 1,
          queue_ref: 'queue.main',
          redacted: true,
          schema: 'openagents.forge.promotion.decision.v0.1',
          source_refs: [],
          target_ref: 'refs/heads/main',
          tenant_ref: TENANT,
          verification_ref: 'verify.1',
        },
        T2,
      )

      // GitHub mirror receipt DOUBLE-FIRE: the conflict-path attempt
      // bump (attempt_count + 1) must mirror the D1-resolved value.
      const receiptInput = {
        change_ref: 'change.1',
        commit_id: OID_B,
        completed_at: T2,
        destination_github_ref: 'refs/heads/main',
        destination_github_repository: 'OpenAgentsInc/contract',
        error_reason: null,
        first_attempted_at: T1,
        last_attempted_at: T2,
        mirror_ref: 'mirror.1',
        promotion_ref: 'promo.1',
        redacted: true as const,
        refusal_reason: null,
        repository_ref: REPO,
        source_canonical_ref: 'refs/heads/main',
        source_refs: [],
        status: 'mirrored' as const,
        tenant_ref: TENANT,
      }
      await githubMirror.recordReceipt(receiptInput)
      const bumped = await githubMirror.recordReceipt(receiptInput)
      expect(bumped.attempt_count).toBe(2)

      // ZERO drift diagnostics across the whole surface.
      expect(diagnostics).toEqual([])

      // Row-for-row equality across ALL SIXTEEN tables.
      for (const table of FORGE_DOMAIN_TABLES) {
        const spec = FORGE_DOMAIN_TABLE_SPECS[table]
        const order = spec.keyColumns.join(', ')
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
        if (table !== 'forge_git_access_token_scopes') {
          expect(
            d1Rows.length,
            `table ${table} should have been exercised`,
          ).toBeGreaterThan(0)
        }
      }
    }, 120_000)

    test('compare-mode listRefs serves D1 and flags ref-set drift', async () => {
      const compareEnv: ForgeDomainStoreEnv = {
        ...env,
        KHALA_SYNC_FORGE_READS: 'compare',
      }
      const canonical = makeForgeGitCanonicalStoreForEnv(compareEnv, options)

      diagnostics.length = 0
      soakSamples.length = 0
      const clean = await canonical.listRefs(TENANT, REPO)
      expect(clean.length).toBeGreaterThan(0)
      expect(diagnostics).toEqual([])
      // A clean compare still emits a durable "match" soak sample (#8282) —
      // proof the pipeline sees every compare-mode read, not just drifts.
      expect(soakSamples).toEqual([
        { domain: 'forge', outcome: 'match', readKind: 'listRefs' },
      ])

      // Drift the Postgres twin: the shadow compare flags it, D1 is
      // still served unchanged (compare mode SERVES D1).
      await (client as PgClient).unsafe(
        `UPDATE forge_git_refs SET object_id = '${'f'.repeat(40)}' WHERE tenant_ref = $1 AND repository_ref = $2 AND ref_name = 'refs/heads/main'`,
        [TENANT, REPO],
      )
      soakSamples.length = 0
      const served = await canonical.listRefs(TENANT, REPO)
      expect(served.length).toBe(clean.length)
      // Compare mode serves the D1 value, not the drifted Postgres value.
      expect(
        served.find(ref => ref.ref_name === 'refs/heads/main')?.object_id,
      ).toBe(OID_A)
      expect(
        diagnostics.map(d => d.event),
      ).toContain('khala_sync_forge_read_compare_mismatch')
      expect(soakSamples).toEqual([
        { domain: 'forge', outcome: 'mismatch', readKind: 'listRefs' },
      ])

      // Heal the twin for the later assertions.
      await (client as PgClient).unsafe(
        `UPDATE forge_git_refs SET object_id = '${OID_A}' WHERE tenant_ref = $1 AND repository_ref = $2 AND ref_name = 'refs/heads/main'`,
        [TENANT, REPO],
      )
    })

    test('postgres-mode listRefs SERVES the ref advertisement from Postgres (KS-8.16 read cutover)', async () => {
      const postgresEnv: ForgeDomainStoreEnv = {
        ...env,
        KHALA_SYNC_FORGE_READS: 'postgres',
      }
      const canonical = makeForgeGitCanonicalStoreForEnv(postgresEnv, options)

      // Sentinel: set ONLY the Postgres twin's main tip to a value D1 does
      // NOT hold. A postgres-served read must return the Postgres value,
      // proving the read is genuinely served from the twin (not D1).
      const sentinel = 'e'.repeat(40)
      await (client as PgClient).unsafe(
        `UPDATE forge_git_refs SET object_id = '${sentinel}' WHERE tenant_ref = $1 AND repository_ref = $2 AND ref_name = 'refs/heads/main'`,
        [TENANT, REPO],
      )

      diagnostics.length = 0
      const served = await canonical.listRefs(TENANT, REPO)
      expect(
        served.find(ref => ref.ref_name === 'refs/heads/main')?.object_id,
      ).toBe(sentinel)
      // No serve failure, no compare shadow in postgres serve mode.
      expect(diagnostics).toEqual([])

      // A state-filtered advertisement (the live `state:'active'` path)
      // is also served from Postgres.
      const activeServed = await canonical.listRefs(TENANT, REPO, {
        limit: 500,
        state: 'active',
      })
      expect(
        activeServed.find(ref => ref.ref_name === 'refs/heads/main')
          ?.object_id,
      ).toBe(sentinel)

      // Heal the twin back to the D1 value.
      await (client as PgClient).unsafe(
        `UPDATE forge_git_refs SET object_id = '${OID_A}' WHERE tenant_ref = $1 AND repository_ref = $2 AND ref_name = 'refs/heads/main'`,
        [TENANT, REPO],
      )
    })

    test('postgres-mode listRefs is FAIL-SOFT: a dead twin falls back to the D1 authority', async () => {
      const failSoft: Array<{
        event: ForgeDomainDiagnosticEvent
        fields: ForgeDomainDiagnostic
      }> = []
      const brokenOptions: MakeForgeDomainStoreOptions = {
        log: (event, fields) => {
          failSoft.push({ event, fields })
        },
        makeSqlClient: () => Promise.reject(new Error('postgres is down')),
      }
      const postgresEnv: ForgeDomainStoreEnv = {
        ...env,
        KHALA_SYNC_FORGE_READS: 'postgres',
      }
      const canonical = makeForgeGitCanonicalStoreForEnv(
        postgresEnv,
        brokenOptions,
      )

      // The ref advertisement is STILL answered — from D1 — and the serve
      // failure is logged. The advertisement can never break.
      const served = await canonical.listRefs(TENANT, REPO)
      expect(
        served.find(ref => ref.ref_name === 'refs/heads/main')?.object_id,
      ).toBe(OID_A)
      expect(failSoft.map(d => d.event)).toContain(
        'khala_sync_forge_postgres_read_serve_failed',
      )
    })

    test('a broken Postgres twin never fails a write, and token_hash mirror refs are redacted', async () => {
      const broken: Array<{
        event: ForgeDomainDiagnosticEvent
        fields: ForgeDomainDiagnostic
      }> = []
      const brokenOptions: MakeForgeDomainStoreOptions = {
        log: (event, fields) => {
          broken.push({ event, fields })
        },
        makeSqlClient: () => Promise.reject(new Error('postgres is down')),
      }
      const auth = makeForgeTenantGitAuthStoreForEnv(env, brokenOptions)

      // Write succeeds against D1 authority despite the dead twin.
      const tenant = await auth.upsertTenant({
        displayName: 'Fail Soft',
        nowIso: T2,
        state: 'active',
        tenantRef: 'tenant.failsoft',
      })
      expect(tenant.tenant_ref).toBe('tenant.failsoft')
      expect(broken.map(d => d.event)).toContain(
        'khala_sync_forge_dual_write_failed',
      )

      // Mint under the healthy tenant, then expire-authenticate under the
      // broken twin: the only usable mirror key is the token hash, and
      // the diagnostic must carry ONLY the redacted ref.
      const healthyAuth = makeForgeTenantGitAuthStoreForEnv(env, options)
      const shortLived = await healthyAuth.mintGitAccessToken(
        {
          expiresAt: T1,
          nowIso: T0,
          repositoryRef: REPO,
          scopes: ['git:upload-pack'],
          sourceRefs: [],
          subjectRef: 'agent.contract',
          tenantRef: TENANT,
          tokenRef: 'token.contract.expiring',
        },
        { makeToken: () => `oa_forge_git_${'e'.repeat(43)}` },
      )
      broken.length = 0
      const expired = await auth.authenticateGitAccessToken({
        nowIso: T2,
        repositoryRef: REPO,
        requiredScope: 'git:upload-pack',
        token: shortLived.token,
      })
      expect(expired).toBeUndefined()
      const tokenDiagnostics = broken.filter(d =>
        d.fields.op.includes('forge_git_access_tokens'),
      )
      expect(tokenDiagnostics.length).toBeGreaterThan(0)
      for (const diagnostic of broken) {
        const line = JSON.stringify(diagnostic)
        expect(line).not.toContain(shortLived.token)
        expect(line).not.toContain('e'.repeat(43))
        // sha256 hex of the token must not leak either.
        expect(line).not.toMatch(/[0-9a-f]{64}/)
      }
      expect(
        tokenDiagnostics.some(d =>
          d.fields.refs.includes('<redacted:token_hash>'),
        ),
      ).toBe(true)

      // The D1 expiry transition still landed (authority unaffected).
      const record = await healthyAuth.readGitAccessToken(
        TENANT,
        'token.contract.expiring',
      )
      expect(record?.state).toBe('expired')
    })
  },
)
