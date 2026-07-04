// KS-8.12 (#8323): sites content repository CONTRACT suite.
//
// Two layers, one behavioral spec:
//
//  1. `SitesContentWriteStore` contract — the row seam's converge
//     semantics run identically against BOTH implementations:
//     - D1: `makeD1SitesContentWriteStore` over real SQLite (node:sqlite —
//       the engine D1 is built on), schema from the worker migrations
//       (condensed in test/sqlite-d1.ts).
//     - Postgres: `makePostgresSitesContentStore` over a throwaway local
//       Postgres (initdb/pg_ctl), schema from khala-sync-server migration
//       0020. Skipped when no local Postgres binaries exist.
//
//  2. END-TO-END mirror fidelity — the REAL sites write surfaces (the
//     sites.ts project/version/deployment statement shapes, the builder
//     session/message/event/phase-run/file-snapshot/preview/artifact
//     writers from sites-builder-sessions.ts, the repair loop, and the
//     site-library archival batch with its PARENT-KEYED deployment and
//     session transitions) run UNCHANGED through the mirroring database
//     with SQLite as D1 authority and the real Postgres store as the
//     mirror; afterwards every scoped table is row-for-row IDENTICAL
//     across both stores (registry-column projection, value-normalized).
//     This is the load-bearing KS-8.12 property: the closed write set
//     across the sites modules classifies + mirrors byte-faithfully,
//     including INSERT OR IGNORE dedupe, the one-active-deployment
//     rollback/disable transitions keyed by site_id, and batch() members
//     — with ZERO unclassified-write diagnostics.

import {
  normalizeSitesContentValue,
  SITES_CONTENT_TABLE_COLUMNS,
  SITES_CONTENT_TABLE_PK,
  SITES_CONTENT_TABLES,
  type SitesContentTable,
} from '@openagentsinc/khala-sync-server'
import {
  hasLocalPostgres,
  startLocalPostgres,
} from '@openagentsinc/khala-sync-server/test/local-postgres'
import { Effect } from 'effect'
import { readFileSync } from 'node:fs'
import * as path from 'node:path'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'

import { archiveSiteLibrarySite } from './site-library'
import {
  makeD1SitesContentWriteStore,
  makePostgresSitesContentStore,
  makeSitesContentMirror,
  makeSitesContentMirroringDatabase,
  type PostgresSitesContentStore,
  type SitesContentDiagnostic,
  type SitesContentDiagnosticEvent,
  type SitesContentRow,
  type SitesContentWriteStore,
} from './sites-content-store'
import { recordSiteBuilderRepairAttempt } from './sites-builder-repair-loop'
import {
  appendSiteBuilderEvent,
  appendSiteBuilderMessage,
  createSiteBuilderSession,
  recordSiteBuilderArtifact,
  recordSiteBuilderPhaseRun,
  recordSiteBuilderPreview,
  upsertSiteBuilderFileSnapshot,
  type SiteBuilderRuntime,
} from './sites-builder-sessions'
import { makeSqliteD1, SITES_CONTENT_D1_SCHEMA } from './test/sqlite-d1'

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const T0 = '2026-07-04T00:00:00.000Z'

let tick = 0
const builderRuntime: SiteBuilderRuntime = {
  nowIso: () => `2026-07-04T00:00:${String(tick % 60).padStart(2, '0')}.000Z`,
  randomId: prefix => `${prefix}_${++tick}`,
}

// ---------------------------------------------------------------------------
// Layer 1: write-store contract (both implementations)
// ---------------------------------------------------------------------------

type ContractHarness = Readonly<{
  store: SitesContentWriteStore
  query: (sql: string) => Promise<ReadonlyArray<Record<string, unknown>>>
}>

const versionRow = (
  n: number,
  overrides: Partial<Record<string, unknown>> = {},
): SitesContentRow => ({
  artifact_manifest_r2_key: null,
  build_command: null,
  build_log_r2_key: null,
  build_status: 'saved',
  created_at: T0,
  created_by_run_id: null,
  created_by_user_id: 'user_owner',
  d1_binding_name: null,
  id: `store_version_${n}`,
  metadata_json: '{}',
  r2_binding_name: null,
  rejected_at: null,
  saved_at: T0,
  site_id: 'store_site_1',
  source_archive_r2_key: null,
  source_commit_sha: null,
  source_kind: 'autopilot_generated',
  static_assets_manifest_json: '{}',
  worker_module_r2_key: null,
  ...overrides,
})

const sessionRow = (
  n: number,
  overrides: Partial<Record<string, unknown>> = {},
): SitesContentRow => ({
  active_artifact_id: null,
  active_preview_id: null,
  archived_at: null,
  created_at: T0,
  created_by_actor_ref: 'agent_raynor',
  customer_user_id: null,
  id: `store_session_${n}`,
  idempotency_key: `store-session-key-${n}`,
  metadata_json: '{}',
  order_id: null,
  owner_user_id: 'user_owner',
  prompt_summary: 'Store contract session.',
  site_id: 'store_site_1',
  source_revision_id: null,
  source_site_version_id: null,
  status: 'draft',
  updated_at: T0,
  workroom_id: null,
  ...overrides,
})

const specContractSuite = (harness: () => ContractHarness) => {
  test('upsertRows converges on the PK and is idempotent', async () => {
    const { query, store } = harness()
    expect(
      await store.upsertRows('site_versions', [versionRow(1), versionRow(2)]),
    ).toBe(2)
    // Re-run: converge, no duplication.
    expect(
      await store.upsertRows('site_versions', [versionRow(1), versionRow(2)]),
    ).toBe(2)
    const counted = await query(
      `SELECT COUNT(*) AS total FROM site_versions WHERE id LIKE 'store_version_%'`,
    )
    expect(Number(counted[0]?.['total'])).toBe(2)

    // A newer D1 snapshot wins (build status flip).
    await store.upsertRows('site_versions', [
      versionRow(1, { build_status: 'superseded', saved_at: null }),
    ])
    const rows = await query(
      `SELECT build_status, saved_at FROM site_versions WHERE id = 'store_version_1'`,
    )
    expect(rows[0]?.['build_status']).toBe('superseded')
    expect(rows[0]?.['saved_at']).toBeNull()
  })

  test('idempotency dedupe keys port exactly: same key on a new id rejects', async () => {
    const { store } = harness()
    await store.upsertRows('site_builder_sessions', [sessionRow(3)])
    await expect(
      store.upsertRows('site_builder_sessions', [
        sessionRow(4, { idempotency_key: 'store-session-key-3' }),
      ]),
    ).rejects.toThrow()
  })

  test('builder sequence natural keys port exactly: same (session, sequence) on a new id rejects', async () => {
    const { store } = harness()
    await store.upsertRows('site_builder_sessions', [sessionRow(5)])
    const messageRow = (id: string, key: string): SitesContentRow => ({
      actor_kind: 'agent',
      archived_at: null,
      body: 'contract body',
      created_at: T0,
      id,
      idempotency_key: key,
      metadata_json: '{}',
      sequence: 1,
      session_id: 'store_session_5',
      visibility: 'customer',
    })
    await store.upsertRows('site_builder_messages', [
      messageRow('store_message_1', 'store-message-key-1'),
    ])
    await expect(
      store.upsertRows('site_builder_messages', [
        messageRow('store_message_2', 'store-message-key-2'),
      ]),
    ).rejects.toThrow()
  })
}

describe('sites content write-store contract — D1 (real SQLite)', () => {
  let sqlite: ReturnType<typeof makeSqliteD1> | undefined
  let harness: ContractHarness

  beforeAll(() => {
    sqlite = makeSqliteD1()
    sqlite.exec(SITES_CONTENT_D1_SCHEMA)
    harness = {
      query: async sql =>
        (await sqlite!.db.prepare(sql).all<Record<string, unknown>>())
          .results ?? [],
      store: makeD1SitesContentWriteStore(sqlite.db),
    }
  })

  afterAll(() => {
    sqlite?.close()
  })

  specContractSuite(() => harness)
})

const MIGRATION_0020 = path.resolve(
  __dirname,
  '../../../../../packages/khala-sync-server/migrations/0020_sites_core.sql',
)

type PgClient = Readonly<{
  end: (options?: { timeout?: number }) => Promise<void>
  unsafe: (
    sql: string,
    params?: Array<unknown>,
  ) => Promise<Array<Record<string, unknown>>>
}>

describe.skipIf(!hasLocalPostgres())(
  'sites content write-store contract — Postgres',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let harness: ContractHarness

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE sites_content_contract')
      await admin.end({ timeout: 5 })
      const raw = postgres(pg.urlFor('sites_content_contract'), {
        max: 4,
        prepare: false,
      })
      client = raw as unknown as PgClient
      await raw.unsafe(readFileSync(MIGRATION_0020, 'utf8'))
      harness = {
        query: async sql => (client as PgClient).unsafe(sql),
        store: makePostgresSitesContentStore({
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
// Layer 2: end-to-end mirror fidelity through the REAL sites writes
// ---------------------------------------------------------------------------

const projectRow = (
  table: SitesContentTable,
  row: Record<string, unknown>,
): Record<string, string | null> =>
  Object.fromEntries(
    SITES_CONTENT_TABLE_COLUMNS[table].map(column => {
      const value = normalizeSitesContentValue(row[column])
      return [column, value === null ? null : String(value)]
    }),
  )

describe.skipIf(!hasLocalPostgres())(
  'sites module writes mirror byte-faithfully into Postgres',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let sqlite: ReturnType<typeof makeSqliteD1> | undefined
    let db: D1Database
    let postgresStore: PostgresSitesContentStore
    const diagnostics: Array<{
      event: SitesContentDiagnosticEvent
      fields: SitesContentDiagnostic
    }> = []

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE sites_content_mirror')
      await admin.end({ timeout: 5 })
      const raw = postgres(pg.urlFor('sites_content_mirror'), {
        max: 4,
        prepare: false,
      })
      client = raw as unknown as PgClient
      await raw.unsafe(readFileSync(MIGRATION_0020, 'utf8'))
      postgresStore = makePostgresSitesContentStore({
        acquireSql: () =>
          Promise.resolve({
            end: () => Promise.resolve(),
            sql: raw as never,
          }),
      })

      sqlite = makeSqliteD1()
      sqlite.exec(SITES_CONTENT_D1_SCHEMA)

      const log = (
        event: SitesContentDiagnosticEvent,
        fields: SitesContentDiagnostic,
      ) => {
        diagnostics.push({ event, fields })
      }
      db = makeSitesContentMirroringDatabase({
        compareStore: undefined,
        db: sqlite.db,
        log,
        mirror: makeSitesContentMirror({
          db: sqlite.db,
          log,
          postgres: postgresStore,
        }),
      })
    }, 120_000)

    afterAll(async () => {
      sqlite?.close()
      await client?.end({ timeout: 5 })
      await pg?.stop()
    }, 60_000)

    const expectStoresConverged = async (): Promise<void> => {
      for (const table of SITES_CONTENT_TABLES) {
        const pk = SITES_CONTENT_TABLE_PK[table]
        const d1Rows = (
          (await sqlite!.db
            .prepare(`SELECT * FROM ${table}`)
            .all<Record<string, unknown>>()).results ?? []
        )
          .map(row => projectRow(table, row))
          .sort((a, b) => String(a[pk]).localeCompare(String(b[pk])))
        const pgRows = (await (client as PgClient).unsafe(
          `SELECT * FROM ${table}`,
        ))
          .map(row => projectRow(table, row))
          .sort((a, b) => String(a[pk]).localeCompare(String(b[pk])))
        expect(pgRows, `table ${table}`).toEqual(d1Rows)
      }
    }

    test('the full sites write surface converges both stores row-for-row', async () => {
      // --- project + versions + deployments (the sites.ts shapes) --------
      await db
        .prepare(
          `INSERT INTO site_projects
             (id,
              software_order_id,
              owner_user_id,
              team_id,
              project_id,
              slug,
              title,
              prompt,
              status,
              access_mode,
              visibility,
              source_repository_provider,
              source_repository_owner,
              source_repository_name,
              source_repository_ref,
              active_version_id,
              active_deployment_id,
              created_at,
              updated_at,
              archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'draft', ?, ?, ?, ?, ?, ?, NULL, NULL, ?, ?, NULL)`,
        )
        .bind(
          'site_e2e_1',
          null,
          'user_owner',
          null,
          null,
          'contract-site',
          'Contract Site',
          'Build the contract site.',
          'customer_owner',
          'private',
          null,
          null,
          null,
          null,
          T0,
          T0,
        )
        .run()

      for (const n of [1, 2]) {
        await db
          .prepare(
            `INSERT INTO site_versions
               (id, site_id, source_kind, source_commit_sha, source_archive_r2_key,
                artifact_manifest_r2_key, build_log_r2_key, build_status, build_command,
                worker_module_r2_key, static_assets_manifest_json, d1_binding_name,
                r2_binding_name, metadata_json, created_by_user_id, created_by_run_id,
                created_at, saved_at, rejected_at)
             VALUES (?, ?, 'autopilot_generated', NULL, NULL, NULL, NULL, ?, NULL, NULL, '{}', NULL, NULL, '{}', ?, NULL, ?, ?, NULL)`,
          )
          .bind(
            `version_e2e_${n}`,
            'site_e2e_1',
            'saved',
            'user_owner',
            T0,
            T0,
          )
          .run()
        await db
          .prepare(
            `INSERT INTO site_deployments
               (id, site_id, version_id, slug, url, runtime_kind, runtime_script_name,
                dispatch_namespace, status, deployed_by_user_id, external_deployment_id,
                started_at, activated_at, failed_at, disabled_at, rolled_back_at,
                created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, 'workers_for_platforms', NULL, NULL, ?, ?, NULL, ?, NULL, NULL, NULL, NULL, ?, ?)`,
          )
          .bind(
            `deployment_e2e_${n}`,
            'site_e2e_1',
            `version_e2e_${n}`,
            'contract-site',
            'https://contract-site.openagents.dev',
            'queued',
            'user_owner',
            T0,
            T0,
            T0,
          )
          .run()
      }

      await db
        .prepare(
          `INSERT INTO site_deployment_attempts
             (id, site_id, version_id, deployment_id, runtime_kind, runtime_script_name,
              dispatch_namespace, external_deployment_id, status, upload_receipt_ref,
              health_status, health_url, health_ref, rollback_ref, observability_ref,
              metadata_json, created_at, updated_at, archived_at)
           VALUES (?, ?, ?, ?, 'workers_for_platforms', NULL, NULL, NULL, ?, NULL, ?, NULL, NULL, NULL, NULL, '{}', ?, ?, NULL)`,
        )
        .bind(
          'attempt_e2e_1',
          'site_e2e_1',
          'version_e2e_1',
          'deployment_e2e_1',
          'succeeded',
          'healthy',
          T0,
          T0,
        )
        .run()

      await db
        .prepare(
          `INSERT INTO site_access_grants
             (id, site_id, principal_kind, principal_ref, role, created_at, revoked_at)
           VALUES (?, ?, ?, ?, ?, ?, NULL)`,
        )
        .bind('grant_e2e_1', 'site_e2e_1', 'user', 'user_owner', 'owner', T0)
        .run()

      // --- deployment state machine: activate then roll back --------------
      // The adjutant activate shape (PK preferred over the composite key).
      await db
        .prepare(
          `UPDATE site_deployments
              SET status = 'active',
                  activated_at = COALESCE(activated_at, ?),
                  updated_at = ?
            WHERE id = ?
              AND site_id = ?`,
        )
        .bind(T0, T0, 'deployment_e2e_1', 'site_e2e_1')
        .run()
      // The sites.ts rollback shape — PARENT-keyed (site_id), no PK.
      await db
        .prepare(
          `UPDATE site_deployments
              SET status = 'rolled_back',
                  rolled_back_at = ?,
                  updated_at = ?
            WHERE site_id = ?
              AND status = 'active'`,
        )
        .bind(T0, T0, 'site_e2e_1')
        .run()
      // Activate the second deployment (state machine continues).
      await db
        .prepare(
          `UPDATE site_deployments
              SET status = 'active',
                  activated_at = COALESCE(activated_at, ?),
                  updated_at = ?
            WHERE id = ?
              AND site_id = ?`,
        )
        .bind(T0, T0, 'deployment_e2e_2', 'site_e2e_1')
        .run()

      // --- builder session + full satellite fan (REAL writer functions) --
      const session = await Effect.runPromise(
        createSiteBuilderSession(
          db,
          {
            createdByActorRef: 'agent_raynor',
            idempotencyKey: 'e2e-session-1',
            ownerUserId: 'user_owner',
            promptSummary: 'Contract builder session.',
            siteId: 'site_e2e_1',
          },
          builderRuntime,
        ),
      )
      // Dedupe replay: same key returns the surviving session, mirrors no
      // phantom row.
      const replay = await Effect.runPromise(
        createSiteBuilderSession(
          db,
          {
            createdByActorRef: 'agent_raynor',
            idempotencyKey: 'e2e-session-1',
            ownerUserId: 'user_owner',
            promptSummary: 'Contract builder session.',
            siteId: 'site_e2e_1',
          },
          builderRuntime,
        ),
      )
      expect(replay.id).toBe(session.id)

      await Effect.runPromise(
        appendSiteBuilderMessage(
          db,
          {
            actorKind: 'customer',
            body: 'Please build the contract site.',
            idempotencyKey: 'e2e-message-1',
            sessionId: session.id,
          },
          builderRuntime,
        ),
      )
      await Effect.runPromise(
        appendSiteBuilderMessage(
          db,
          {
            actorKind: 'agent',
            body: 'Building it now.',
            idempotencyKey: 'e2e-message-2',
            sessionId: session.id,
          },
          builderRuntime,
        ),
      )
      await Effect.runPromise(
        appendSiteBuilderEvent(
          db,
          {
            eventKind: 'phase_started',
            idempotencyKey: 'e2e-event-1',
            phaseKind: 'planning',
            sessionId: session.id,
            status: 'running',
            summary: 'Planning started.',
            title: 'Planning',
          },
          builderRuntime,
        ),
      )
      await Effect.runPromise(
        recordSiteBuilderPhaseRun(
          db,
          {
            idempotencyKey: 'e2e-phase-1',
            phaseKind: 'planning',
            sessionId: session.id,
            status: 'succeeded',
            summary: 'Planning finished.',
            title: 'Planning',
          },
          builderRuntime,
        ),
      )
      await Effect.runPromise(
        upsertSiteBuilderFileSnapshot(
          db,
          {
            byteSize: 512,
            contentHash: 'sha256:contract',
            idempotencyKey: 'e2e-snapshot-1',
            language: 'html',
            path: 'index.html',
            previewText: '<!doctype html>',
            sessionId: session.id,
          },
          builderRuntime,
        ),
      )
      const preview = await Effect.runPromise(
        recordSiteBuilderPreview(
          db,
          {
            idempotencyKey: 'e2e-preview-1',
            previewKind: 'static_r2',
            previewUrl: 'https://sites.openagents.com/contract/preview',
            sessionId: session.id,
            status: 'ready',
          },
          builderRuntime,
        ),
      )
      await Effect.runPromise(
        recordSiteBuilderArtifact(
          db,
          {
            artifactKind: 'preview_bundle',
            artifactRef: 'r2://artifacts/contract-preview.tar',
            byteSize: 2048,
            idempotencyKey: 'e2e-artifact-1',
            sessionId: session.id,
          },
          builderRuntime,
        ),
      )
      await Effect.runPromise(
        recordSiteBuilderRepairAttempt(
          db,
          {
            failureKind: 'build_error',
            failureSummary: 'Bundle failed to compile.',
            idempotencyKey: 'e2e-repair-1',
            previewId: preview.id,
            retryBudget: 3,
            sessionId: session.id,
          },
          builderRuntime,
        ),
      )

      // --- saved version + timeline event (the module SQL shapes) --------
      await db
        .prepare(
          `INSERT OR IGNORE INTO site_builder_saved_versions (
             id, idempotency_key, session_id, site_id, site_version_id, preview_id,
             artifact_ref, build_receipt_ref, source_hash, notes, site_metadata_json,
             created_at, archived_at
           ) VALUES (?, ?, ?, ?, ?, ?, ?, NULL, ?, NULL, '{}', ?, NULL)`,
        )
        .bind(
          'saved_version_e2e_1',
          'e2e-saved-1',
          session.id,
          'site_e2e_1',
          'version_e2e_2',
          preview.id,
          'r2://artifacts/contract-preview.tar',
          'sha256:contract',
          T0,
        )
        .run()
      await db
        .prepare(
          `INSERT INTO site_events
             (id, site_id, version_id, deployment_id, type, summary,
              actor_user_id, actor_run_id, payload_json, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)`,
        )
        .bind(
          'event_e2e_1',
          'site_e2e_1',
          'version_e2e_2',
          'deployment_e2e_2',
          'site.deployment.activated',
          'Deployment activated.',
          'user_owner',
          '{}',
          T0,
        )
        .run()

      // --- site-library archival (batch + PARENT-keyed transitions) -------
      await Effect.runPromise(
        archiveSiteLibrarySite(
          db,
          {
            makeEventId: () => 'event_e2e_archive',
            nowIso: () => T0,
          },
          {
            actorUserId: 'user_owner',
            idempotencyKey: 'e2e-archive-1',
            isAdmin: false,
            siteId: 'site_e2e_1',
          },
        ),
      )

      // Every write above classified: zero unclassified diagnostics, zero
      // mirror failures.
      expect(
        diagnostics.map(entry => `${entry.event}:${entry.fields.op}`),
      ).toEqual([])

      // The fifteen scoped tables are row-for-row identical.
      await expectStoresConverged()

      // Spot-check the interesting converged values on the Postgres side.
      const deployments = await (client as PgClient).unsafe(
        `SELECT id, status FROM site_deployments ORDER BY id`,
      )
      expect(
        deployments.map(row => [row['id'], row['status']]),
      ).toEqual([
        ['deployment_e2e_1', 'rolled_back'],
        // Archived at the end: the library batch disables the active one.
        ['deployment_e2e_2', 'disabled'],
      ])
      const project = await (client as PgClient).unsafe(
        `SELECT status, visibility, archived_at FROM site_projects WHERE id = 'site_e2e_1'`,
      )
      expect(project[0]?.['status']).toBe('archived')
      expect(project[0]?.['visibility']).toBe('private')
      expect(project[0]?.['archived_at']).not.toBeNull()
      const sessions = await (client as PgClient).unsafe(
        `SELECT COUNT(*) AS total, MAX(status) AS status FROM site_builder_sessions`,
      )
      // The dedupe replay created no phantom row; archival flipped status.
      expect(Number(sessions[0]?.['total'])).toBe(1)
      expect(sessions[0]?.['status']).toBe('archived')
    }, 120_000)
  },
)
