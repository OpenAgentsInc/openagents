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
  postgresPylonCodexRawEventChunkAggregates,
  postgresPylonCodexRawEventTurnAggregates,
  postgresPylonControlPlaneNewestHashes,
  postgresPylonControlPlaneTally,
  PYLON_CONTROL_PLANE_TABLES,
  pylonControlPlaneRowHash,
  reconcilePylonCodexRawEventMetadata,
  TABLE_COLUMNS_FOR_TEST,
  tallyFromRows,
  upsertPylonControlPlaneRows,
  type D1SourceRow,
  type PylonCodexRawEventAggregateRow,
  type PylonCodexRawEventChunkAggregateRow,
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

const rawTurnRow = (n: number): D1SourceRow => ({
  assignment_ref: `assignment.ks84.${n}`,
  byte_length: 2048 + n,
  content_digest: `sha256:turn-${n}`,
  created_at: `2026-07-04T04:1${n}:00.000Z`,
  demand_kind: "own_capacity",
  demand_source: "khala_coding_delegation",
  event_count: 9,
  lease_ref: `lease.ks84.${n}`,
  observed_at: `2026-07-04T04:1${n}:00.000Z`,
  owner_user_id: `owner-${n}`,
  pylon_ref: `pylon.ks84.${n}`,
  r2_key: `trace-blobs/ks84/${n}-turn.json`,
  raw_event_ref: `raw.ks84.${n}`,
  run_ref: `run.ks84.${n}`,
  session_ref: `session.ks84.${n}`,
  turn_index: n,
  updated_at: `2026-07-04T04:1${n}:00.000Z`,
  workspace_ref: `workspace.ks84.${n}`,
})

const rawTurnAggregate = (
  overrides: Partial<PylonCodexRawEventAggregateRow> = {},
): PylonCodexRawEventAggregateRow => ({
  assignment_ref: "assignment.ks84.1",
  byte_length: 2049,
  event_count: 9,
  lease_ref: "lease.ks84.1",
  owner_user_id: "owner-1",
  pylon_ref: "pylon.ks84.1",
  row_count: 1,
  turn_index: 1,
  ...overrides,
})

const rawChunkAggregate = (
  overrides: Partial<PylonCodexRawEventChunkAggregateRow> = {},
): PylonCodexRawEventChunkAggregateRow => ({
  assignment_ref: "assignment.ks84.1",
  byte_length: 1542,
  distinct_chunk_indexes: 3,
  event_count: 9,
  lease_ref: "lease.ks84.1",
  max_chunk_index: 3,
  min_chunk_index: 1,
  owner_user_id: "owner-1",
  pylon_ref: "pylon.ks84.1",
  row_count: 3,
  turn_index: 1,
  ...overrides,
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

  test("raw-event metadata reconciliation accepts exact aggregates and contiguous chunk chains", () => {
    const report = reconcilePylonCodexRawEventMetadata({
      d1Chunks: [rawChunkAggregate()],
      d1TurnEvents: [rawTurnAggregate()],
      postgresChunks: [
        rawChunkAggregate({
          byte_length: "1542",
          distinct_chunk_indexes: "3",
          event_count: "9",
          max_chunk_index: "3",
          min_chunk_index: "1",
          row_count: "3",
        }),
      ],
      postgresTurnEvents: [
        rawTurnAggregate({
          byte_length: "2049",
          event_count: "9",
          row_count: "1",
        }),
      ],
    })

    expect(report.ok).toBe(true)
    expect(report.turnEvents.mismatches).toEqual([])
    expect(report.chunks.mismatches).toEqual([])
    expect(report.chunks.chainGaps).toEqual([])
  })

  test("raw-event metadata reconciliation catches drift and per-turn chunk gaps", () => {
    const report = reconcilePylonCodexRawEventMetadata({
      d1Chunks: [
        rawChunkAggregate({
          distinct_chunk_indexes: 2,
          max_chunk_index: 3,
          min_chunk_index: 1,
          row_count: 2,
        }),
      ],
      d1TurnEvents: [rawTurnAggregate()],
      postgresChunks: [rawChunkAggregate()],
      postgresTurnEvents: [rawTurnAggregate({ event_count: 8 })],
    })

    expect(report.ok).toBe(false)
    expect(report.turnEvents.mismatches).toHaveLength(1)
    expect(report.chunks.mismatches).toHaveLength(1)
    expect(report.chunks.chainGaps).toEqual([
      {
        chunkCount: 2,
        distinctChunkIndexes: 2,
        expectedChunkCount: 3,
        key: "owner-1:assignment.ks84.1:lease.ks84.1:pylon.ks84.1:1",
        maxChunkIndex: 3,
        minChunkIndex: 1,
        source: "d1",
      },
    ])
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

    test("raw-event SQL aggregate queries feed the reconciliation report", async () => {
      await rawSql`DELETE FROM pylon_codex_raw_event_chunks`
      await rawSql`DELETE FROM pylon_codex_raw_events`

      await upsertPylonControlPlaneRows(sql, "pylon_codex_raw_events", [
        rawTurnRow(8),
      ])
      await upsertPylonControlPlaneRows(sql, "pylon_codex_raw_event_chunks", [
        {
          ...rawChunkRow(8),
          chunk_index: 1,
          event_count: 3,
        },
        {
          ...rawChunkRow(8),
          chunk_index: 2,
          chunk_ref: "chunk.ks84.8.2",
          content_digest: "sha256:chunk-8-2",
          event_count: 3,
        },
        {
          ...rawChunkRow(8),
          chunk_index: 3,
          chunk_ref: "chunk.ks84.8.3",
          content_digest: "sha256:chunk-8-3",
          event_count: 3,
        },
      ])

      const report = reconcilePylonCodexRawEventMetadata({
        d1Chunks: [
          {
            assignment_ref: "assignment.ks84.8",
            byte_length: 1560,
            distinct_chunk_indexes: 3,
            event_count: 9,
            lease_ref: "lease.ks84.8",
            max_chunk_index: 3,
            min_chunk_index: 1,
            owner_user_id: "owner-8",
            pylon_ref: "pylon.ks84.8",
            row_count: 3,
            turn_index: 8,
          },
        ],
        d1TurnEvents: [
          {
            assignment_ref: "assignment.ks84.8",
            byte_length: 2056,
            event_count: 9,
            lease_ref: "lease.ks84.8",
            owner_user_id: "owner-8",
            pylon_ref: "pylon.ks84.8",
            row_count: 1,
            turn_index: 8,
          },
        ],
        postgresChunks: await postgresPylonCodexRawEventChunkAggregates(sql),
        postgresTurnEvents: await postgresPylonCodexRawEventTurnAggregates(sql),
      })

      expect(report.ok).toBe(true)
    })
  },
)
