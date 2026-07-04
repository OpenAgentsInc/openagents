// KS-8.1 (#8307): pylon backfill core — idempotency + verify fidelity.
//
// The load-bearing property: running the same backfill page twice yields an
// IDENTICAL Postgres state (second run inserts zero rows, tallies and
// newest-N hashes unchanged), and `ON CONFLICT DO NOTHING` never clobbers a
// row the dual-write mirror already owns.

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
  comparePylonTallies,
  d1NewestRowHashes,
  postgresNewestRowHashes,
  postgresPylonTally,
  pylonRowHash,
  upsertPylonRows,
  type D1SourceRow,
} from "./pylon-backfill.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const registrationRow = (n: number): D1SourceRow => ({
  archived_at: null,
  capability_refs_json: '["capability.codex_worker.v1"]',
  client_protocol_version: "1",
  client_version: "0.2.0",
  created_at: `2026-07-01T00:0${n}:00.000Z`,
  display_name: `Backfill Pylon ${n}`,
  id: `pylon_registration_${n}`,
  latest_capacity_refs_json: "[]",
  latest_health_refs_json: "[]",
  latest_heartbeat_at: null,
  latest_heartbeat_status: null,
  latest_load_refs_json: "[]",
  latest_resource_mode: null,
  owner_agent_credential_id: `credential-${n}`,
  owner_agent_token_prefix: "oa_agent_x",
  owner_agent_user_id: `agent-user-${n}`,
  provider_market_relay_refs_json: "[]",
  provider_nip90_lane_refs_json: "[]",
  provider_nostr_npub: null,
  provider_nostr_pubkey: null,
  public_projection_json: "{}",
  pylon_ref: `pylon.backfill.${n}`,
  resource_mode: "dedicated",
  status: "registered",
  updated_at: `2026-07-01T00:0${n}:00.000Z`,
  wallet_ready: 0,
  wallet_ref: null,
})

const assignmentRow = (n: number, state: string): D1SourceRow => ({
  acceptance_criteria_refs_json: "[]",
  accepted_work_refs_json: "[]",
  archived_at: null,
  artifact_refs_json: "[]",
  assignment_ref: `assignment.backfill.${n}`,
  closeout_refs_json: "[]",
  coding_assignment_json: null,
  created_at: `2026-07-01T01:0${n}:00.000Z`,
  id: `pylon_assignment_${n}`,
  idempotency_key_hash: `assignment-hash-${n}`,
  job_kind: "coding.assignment.v1",
  lease_expires_at: "2026-07-01T02:00:00.000Z",
  owner_agent_user_id: `agent-user-${n}`,
  // pre-0256 rows can surface NULL payment_mode from D1 exports.
  payment_mode: n === 1 ? null : "unpaid_smoke",
  proof_refs_json: "[]",
  public_projection_json: "{}",
  pylon_ref: `pylon.backfill.${n}`,
  rejection_refs_json: "[]",
  result_expectation_refs_json: "[]",
  state,
  task_refs_json: '["task.backfill"]',
  updated_at: `2026-07-01T01:0${n}:00.000Z`,
})

const eventRow = (n: number): D1SourceRow => ({
  archived_at: null,
  assignment_ref: `assignment.backfill.${n}`,
  created_at: `2026-07-01T01:1${n}:00.000Z`,
  event_body_json: '{"note":"backfill"}',
  event_kind: "assignment_progress",
  event_ref: `event.backfill.${n}`,
  id: `pylon_event_${n}`,
  idempotency_key_hash: `event-hash-${n}`,
  owner_agent_user_id: `agent-user-${n}`,
  public_projection_json: "{}",
  pylon_ref: `pylon.backfill.${n}`,
  status: "running",
})

describe("pylonRowHash (pure)", () => {
  test("identical rows hash identically; any column change diverges", () => {
    const a = assignmentRow(3, "running")
    const b = assignmentRow(3, "running")
    expect(pylonRowHash("pylon_assignments", a)).toBe(
      pylonRowHash("pylon_assignments", b),
    )
    expect(
      pylonRowHash("pylon_assignments", { ...a, state: "accepted" }),
    ).not.toBe(pylonRowHash("pylon_assignments", a))
  })

  test("NULL payment_mode normalizes to unpaid_smoke on both sides", () => {
    const withNull = assignmentRow(1, "running")
    const withDefault = { ...withNull, payment_mode: "unpaid_smoke" }
    expect(pylonRowHash("pylon_assignments", withNull)).toBe(
      pylonRowHash("pylon_assignments", withDefault),
    )
  })

  test("extra columns in the D1 export (e.g. d1_rowid) do not affect the hash", () => {
    const row = eventRow(2)
    expect(pylonRowHash("pylon_assignment_events", { ...row, d1_rowid: 42 })).toBe(
      pylonRowHash("pylon_assignment_events", row),
    )
  })
})

describe.skipIf(!hasLocalPostgres())(
  "pylon backfill against local Postgres",
  () => {
    let pg: LocalPostgres
    let rawSql: SQL
    let sql: SyncSql

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_pylon_backfill")
      await admin.end()
      const url = pg.urlFor("khala_pylon_backfill")
      const result = await runMigrations({ databaseUrl: url })
      expect(result.applied).toContain("0005_pylon_dispatch.sql")
      rawSql = new SQL({ url, max: 4 })
      sql = rawSql as unknown as SyncSql
    })

    afterAll(async () => {
      await rawSql?.end()
      await pg?.stop()
    })

    test("run twice → identical state (idempotency)", async () => {
      const registrations = [registrationRow(1), registrationRow(2)]
      const assignments = [
        assignmentRow(1, "running"),
        assignmentRow(2, "accepted_work"),
      ]
      const events = [eventRow(1), eventRow(2)]

      const first =
        (await upsertPylonRows(sql, "pylon_registrations", registrations)) +
        (await upsertPylonRows(sql, "pylon_assignments", assignments)) +
        (await upsertPylonRows(sql, "pylon_assignment_events", events))
      expect(first).toBe(6)

      const tallyAfterFirst = await postgresPylonTally(sql, "pylon_assignments")
      const hashesAfterFirst = await postgresNewestRowHashes(
        sql,
        "pylon_assignments",
        10,
      )

      // Second sweep: same pages again — nothing inserted, nothing changed.
      const second =
        (await upsertPylonRows(sql, "pylon_registrations", registrations)) +
        (await upsertPylonRows(sql, "pylon_assignments", assignments)) +
        (await upsertPylonRows(sql, "pylon_assignment_events", events))
      expect(second).toBe(0)

      expect(await postgresPylonTally(sql, "pylon_assignments")).toEqual(
        tallyAfterFirst,
      )
      expect(
        await postgresNewestRowHashes(sql, "pylon_assignments", 10),
      ).toEqual(hashesAfterFirst)
    })

    test("DO NOTHING never clobbers a dual-write-mirrored row", async () => {
      // The mirror wrote the LIVE state of assignment 1 (running → accepted).
      await rawSql`
        UPDATE pylon_assignments
           SET state = 'accepted', updated_at = '2026-07-01T03:00:00.000Z'
         WHERE assignment_ref = 'assignment.backfill.1'`

      // A stale backfill page (snapshot taken before the transition) re-runs.
      await upsertPylonRows(sql, "pylon_assignments", [
        assignmentRow(1, "running"),
      ])

      const rows = await rawSql`
        SELECT state FROM pylon_assignments
         WHERE assignment_ref = 'assignment.backfill.1'`
      expect(rows[0]?.state).toBe("accepted")
    })

    test("verify comparison catches count, tally, and hash drift", async () => {
      const d1Rows = [
        assignmentRow(1, "accepted"),
        assignmentRow(2, "accepted_work"),
      ]
      // update the row so PG matches the mirrored 'accepted' state from the
      // previous test — verify should then be green.
      const d1Tally = {
        byStatus: { accepted: 1, accepted_work: 1 },
        total: 2,
      }
      const pgTally = await postgresPylonTally(sql, "pylon_assignments")
      const pgNewest = await postgresNewestRowHashes(sql, "pylon_assignments", 10)
      const d1Newest = d1NewestRowHashes("pylon_assignments", d1Rows)
      // The clobber test changed updated_at on assignment 1, so its hash
      // MUST mismatch against the stale D1 snapshot — verify catches drift.
      const report = comparePylonTallies(
        "pylon_assignments",
        d1Tally,
        pgTally,
        d1Newest,
        pgNewest,
      )
      expect(report.countsMatch).toBe(true)
      expect(report.statusMismatches).toEqual([])
      expect(
        report.newestHashMismatches.map((mismatch) => mismatch.key),
      ).toEqual(["assignment.backfill.1"])

      // And with a faithful D1 snapshot (same updated_at), verify is green.
      const faithful = d1NewestRowHashes("pylon_assignments", [
        {
          ...assignmentRow(1, "accepted"),
          payment_mode: "unpaid_smoke",
          updated_at: "2026-07-01T03:00:00.000Z",
        },
        assignmentRow(2, "accepted_work"),
      ])
      const green = comparePylonTallies(
        "pylon_assignments",
        d1Tally,
        pgTally,
        faithful,
        pgNewest,
      )
      expect(green.countsMatch).toBe(true)
      expect(green.statusMismatches).toEqual([])
      expect(green.newestHashMismatches).toEqual([])
    })
  },
)
