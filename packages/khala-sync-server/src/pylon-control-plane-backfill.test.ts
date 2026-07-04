// KS-8.4 (#8315): Pylon control-plane backfill spine.

import { SQL } from "bun"
import {
  afterAll,
  beforeAll,
  describe,
  expect,
  setDefaultTimeout,
  test,
} from "bun:test"
import { runMigrations } from "./migrate.js"
import {
  comparePylonControlPlaneTallies,
  d1PylonControlPlaneNewestHashes,
  postgresPylonControlPlaneNewestHashes,
  postgresPylonControlPlaneTally,
  PYLON_CONTROL_PLANE_TABLES,
  pylonControlPlaneRowHash,
  TABLE_COLUMNS_FOR_TEST,
  tallyFromRows,
  upsertPylonControlPlaneRows,
  type D1SourceRow,
} from "./pylon-control-plane-backfill.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const quarantineRow = (n: number, state = "active"): D1SourceRow => ({
  action_refs_json: "[]",
  archived_at: null,
  created_at: `2026-07-04T01:0${n}:00.000Z`,
  expires_at: "2026-07-05T00:00:00.000Z",
  id: `quarantine_${n}`,
  owner_agent_user_id: `owner-${n}`,
  public_projection_json: "{}",
  pylon_ref: `pylon.ks84.${n}`,
  quarantine_ref: `quarantine.ks84.${n}`,
  reason_refs_json: '["reason.rate_limit"]',
  released_at: null,
  source_refs_json: "[]",
  state,
  updated_at: `2026-07-04T01:0${n}:00.000Z`,
})

const marketplaceIntakeRow = (n: number): D1SourceRow => ({
  created_at: `2026-07-04T02:0${n}:00.000Z`,
  id: `intake_row_${n}`,
  idempotency_key: `intake-key-${n}`,
  intake_ref: `intake.ks84.${n}`,
  job_kind: "coding.assignment.v1",
  job_ref: `job.ks84.${n}`,
  privacy_class: "public_safe",
  record_json: "{}",
  request_hash: `hash-intake-${n}`,
  source: "operator",
  state: "accepted",
  updated_at: `2026-07-04T02:0${n}:00.000Z`,
})

const capacitySnapshotRow = (n: number): D1SourceRow => ({
  aggregate_json: "{}",
  archived_at: null,
  bucket_kind: "hourly",
  bucket_start_at: `2026-07-04T0${n}:00:00.000Z`,
  created_at: `2026-07-04T03:0${n}:00.000Z`,
  id: `capacity_${n}`,
  public_projection_json: "{}",
  snapshot_at: `2026-07-04T03:0${n}:00.000Z`,
  total_count: n,
  updated_at: `2026-07-04T03:0${n}:00.000Z`,
})

const rawChunkRow = (n: number): D1SourceRow => ({
  assignment_ref: `assignment.ks84.${n}`,
  byte_length: 512 + n,
  chunk_index: n,
  chunk_ref: `chunk.ks84.${n}`,
  content_digest: `sha256:chunk-${n}`,
  created_at: `2026-07-04T04:0${n}:00.000Z`,
  demand_kind: "own_capacity",
  demand_source: "khala_coding_delegation",
  event_count: 3,
  lease_ref: `lease.ks84.${n}`,
  observed_at: `2026-07-04T04:0${n}:00.000Z`,
  owner_user_id: `owner-${n}`,
  pylon_ref: `pylon.ks84.${n}`,
  r2_key: `trace-blobs/ks84/${n}.json`,
  run_ref: `run.ks84.${n}`,
  session_ref: `session.ks84.${n}`,
  turn_index: n,
  updated_at: `2026-07-04T04:0${n}:00.000Z`,
  workspace_ref: `workspace.ks84.${n}`,
})

describe("pylon control-plane backfill metadata", () => {
  test("the KS-8.4 table set is explicit and column-backed", () => {
    expect(PYLON_CONTROL_PLANE_TABLES).toContain("pylon_quarantines")
    expect(PYLON_CONTROL_PLANE_TABLES).toContain("pylon_codex_raw_event_chunks")
    expect(PYLON_CONTROL_PLANE_TABLES).toContain("fleet_alerts")
    for (const table of PYLON_CONTROL_PLANE_TABLES) {
      expect(TABLE_COLUMNS_FOR_TEST[table].length).toBeGreaterThan(0)
    }
  })

  test("row hashes ignore D1 export-only columns and detect data drift", () => {
    const row = rawChunkRow(1)
    expect(
      pylonControlPlaneRowHash("pylon_codex_raw_event_chunks", {
        ...row,
        d1_rowid: 42,
      }),
    ).toBe(pylonControlPlaneRowHash("pylon_codex_raw_event_chunks", row))
    expect(
      pylonControlPlaneRowHash("pylon_codex_raw_event_chunks", {
        ...row,
        chunk_index: 2,
      }),
    ).not.toBe(pylonControlPlaneRowHash("pylon_codex_raw_event_chunks", row))
  })
})

describe.skipIf(!hasLocalPostgres())(
  "pylon control-plane backfill against local Postgres",
  () => {
    let pg: LocalPostgres
    let rawSql: SQL
    let sql: SyncSql

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_pylon_control_backfill")
      await admin.end()
      const url = pg.urlFor("khala_pylon_control_backfill")
      const result = await runMigrations({ databaseUrl: url })
      expect(result.applied).toContain("0009_pylon_control_plane_remainder.sql")
      rawSql = new SQL({ url, max: 4 })
      sql = rawSql as unknown as SyncSql
    })

    afterAll(async () => {
      await rawSql?.end()
      await pg?.stop()
    })

    test("representative tables are idempotent, including composite keys", async () => {
      const first =
        (await upsertPylonControlPlaneRows(sql, "pylon_quarantines", [
          quarantineRow(1),
        ])) +
        (await upsertPylonControlPlaneRows(sql, "pylon_marketplace_job_intakes", [
          marketplaceIntakeRow(1),
        ])) +
        (await upsertPylonControlPlaneRows(sql, "pylon_capacity_funnel_snapshots", [
          capacitySnapshotRow(1),
        ])) +
        (await upsertPylonControlPlaneRows(sql, "pylon_codex_raw_event_chunks", [
          rawChunkRow(1),
        ]))
      expect(first).toBe(4)

      const tally = await postgresPylonControlPlaneTally(
        sql,
        "pylon_quarantines",
      )
      expect(tally).toEqual({ byStatus: { active: 1 }, total: 1 })

      const second =
        (await upsertPylonControlPlaneRows(sql, "pylon_quarantines", [
          quarantineRow(1),
        ])) +
        (await upsertPylonControlPlaneRows(sql, "pylon_marketplace_job_intakes", [
          marketplaceIntakeRow(1),
        ])) +
        (await upsertPylonControlPlaneRows(sql, "pylon_capacity_funnel_snapshots", [
          capacitySnapshotRow(1),
        ])) +
        (await upsertPylonControlPlaneRows(sql, "pylon_codex_raw_event_chunks", [
          rawChunkRow(1),
        ]))
      expect(second).toBe(0)
    })

    test("DO NOTHING protects a newer mirrored row and verify catches drift", async () => {
      await rawSql`
        UPDATE pylon_quarantines
           SET state = 'released',
               updated_at = '2026-07-04T05:00:00.000Z',
               released_at = '2026-07-04T05:00:00.000Z'
         WHERE quarantine_ref = 'quarantine.ks84.1'`

      await upsertPylonControlPlaneRows(sql, "pylon_quarantines", [
        quarantineRow(1, "active"),
      ])

      const rows = await rawSql`
        SELECT state FROM pylon_quarantines
         WHERE quarantine_ref = 'quarantine.ks84.1'`
      expect(rows[0]?.state).toBe("released")

      const d1Rows = [quarantineRow(1, "active")]
      const report = comparePylonControlPlaneTallies(
        "pylon_quarantines",
        tallyFromRows("pylon_quarantines", d1Rows),
        await postgresPylonControlPlaneTally(sql, "pylon_quarantines"),
        d1PylonControlPlaneNewestHashes("pylon_quarantines", d1Rows),
        await postgresPylonControlPlaneNewestHashes(
          sql,
          "pylon_quarantines",
          10,
        ),
      )

      expect(report.countsMatch).toBe(true)
      expect(report.statusMismatches).toEqual([
        { d1: 1, postgres: 0, status: "active" },
        { d1: 0, postgres: 1, status: "released" },
      ])
      expect(report.newestHashMismatches.map((mismatch) => mismatch.key)).toEqual(
        ["quarantine.ks84.1"],
      )
    })
  },
)
