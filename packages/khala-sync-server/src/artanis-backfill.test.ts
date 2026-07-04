// KS-8.6 (#8317): Artanis backfill core — idempotency + verify fidelity.
//
// The load-bearing property: running the same backfill page twice yields an
// IDENTICAL Postgres state (second run inserts zero rows, tallies and
// newest-N hashes unchanged), and `ON CONFLICT DO NOTHING` never clobbers a
// row the dual-write mirror already owns. Every one of the twenty artanis
// tables takes at least one row here, so the registry's column lists are
// proven against the 0010 DDL — not just eyeballed.

import { SQL } from "bun"
import {
  afterAll,
  beforeAll,
  describe,
  expect,
  setDefaultTimeout,
  test,
} from "bun:test"
import {
  ARTANIS_BACKFILL_TABLES,
  ARTANIS_TABLE_SPECS,
  artanisRowHash,
  compareArtanisTallies,
  d1ArtanisNewestRowHashes,
  postgresArtanisNewestRowHashes,
  postgresArtanisTally,
  upsertArtanisRows,
  type ArtanisBackfillTable,
  type D1SourceRow,
} from "./artanis-backfill.js"
import { runMigrations } from "./migrate.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const ISO = "2026-07-04T00:00:00.000Z"

const ledgerRow = (recordRef: string, state: string): D1SourceRow => ({
  active: 0,
  agent_id: "agent_artanis",
  closed_at: null,
  closeout_json: null,
  content_hash: `hash-${recordRef}`,
  created_at: ISO,
  id: `kind:${recordRef}`,
  idempotency_key: `idem-${recordRef}`,
  parent_ref: "loop.artanis.backfill",
  public_projection_json: "{}",
  record_json: "{}",
  record_ref: recordRef,
  scope_ref: "scope.public.artanis.backfill",
  source_kind: "loop_tick",
  state,
  updated_at: ISO,
})

/** One representative D1 row per table (registry-order columns must all
 * exist in the 0010 DDL or the INSERT fails loudly). */
const sampleRow = (
  table: ArtanisBackfillTable,
  n: number,
): D1SourceRow => {
  switch (table) {
    case "artanis_runtime_snapshots":
    case "artanis_loop_records":
    case "artanis_loop_ticks":
    case "artanis_approval_gates":
    case "artanis_health_snapshots":
    case "artanis_work_routing_proposals":
    case "artanis_forum_publication_intents":
    case "artanis_nexus_pylon_adapter_dispatches":
      return ledgerRow(`${table}.backfill.${n}`, n === 1 ? "running" : "completed")
    case "artanis_responder_state":
      return {
        id: 1,
        responses_day: "2026-07-04",
        responses_today: n,
        scan_cursor_iso: ISO,
        updated_at: ISO,
      }
    case "artanis_responder_actions":
      return {
        asked_at: ISO,
        asker_actor_ref: "actor.forum.someone",
        asker_provenance: "external",
        created_at: ISO,
        first_post_id: `post-${n}`,
        id: `action-${n}`,
        proposal_json: "{}",
        question_class: "payout",
        replied_at: null,
        reply_post_id: null,
        state: n === 1 ? "proposed" : "skipped",
        tip_ladder_reason: null,
        tip_ladder_rung: null,
        tip_pay_in_id: null,
        tip_receipt_ref: null,
        topic_id: `topic-${n}`,
        updated_at: ISO,
      }
    case "artanis_responder_ticks":
      return {
        compose_blocked: 0,
        compose_considered: 0,
        compose_responded: 0,
        compose_skipped_reason: null,
        compose_state: "pending",
        compose_tipped: 0,
        created_at: ISO,
        scan_blocked: 0,
        scan_proposed: n,
        scan_scanned: n,
        scan_skipped: 0,
        scan_skipped_reason: null,
        scan_state: "ran",
        scheduled_at: `2026-07-04T00:0${n}:00.000Z`,
        tick_ref: `receipt.artanis_responder.tick.${n}`,
        updated_at: ISO,
      }
    case "artanis_admin_tick_decisions":
      return {
        action_json: "{}",
        assignment_ref: null,
        created_at: ISO,
        id: `admin-decision-${n}`,
        state: n === 1 ? "no_action" : "dispatched",
      }
    case "artanis_closeout_verdicts":
      return {
        accept_state: "accepted",
        assignment_ref: `assignment.artanis_admin.backfill.${n}`,
        claimed_trace_digest_prefix: "abcd1234abcd1234",
        created_at: ISO,
        detail: "backfill sample",
        id: `verdict-${n}`,
        outcome: "verified",
      }
    case "artanis_fleet_overseer_decisions":
      return {
        action_json: "{}",
        approval_gate_ref: null,
        context_json: "{}",
        created_at: ISO,
        health_snapshot_ref: null,
        id: `fleet-decision-${n}`,
        state: "no_action",
      }
    case "artanis_standing_spend_grants":
      return {
        active: 1,
        authority_ref: "authority.owner.backfill",
        created_at: ISO,
        grant_ref: `grant-${n}`,
        per_day_cap_sat: 10_000,
        per_payout_cap_sat: 1_000,
        revoked_at: null,
      }
    case "artanis_spend_decisions":
      return {
        created_at: ISO,
        destination_source_ref: "source.tip_recipient.backfill",
        grant_ref: "grant-1",
        id: `spend-${n}`,
        intended_amount_sat: 500,
        paid_amount_sat: n === 1 ? 500 : null,
        payment_ref: n === 1 ? `payment-${n}` : null,
        policy_applied: null,
        rationale: "backfill sample",
        recipient_ref: `recipient-${n}`,
        state: n === 1 ? "paid" : "refused",
        updated_at: ISO,
      }
    case "artanis_labor_unattended_receipts":
      return {
        created_at: ISO,
        receipt_ref: `receipt.artanis_labor.backfill.${n}`,
        serialized_json: "{}",
        terminal_state: "settled",
      }
    case "artanis_owner_memory":
      return {
        body: "backfill sample memory",
        created_at: ISO,
        kind: "note",
        memory_ref: `artanis_memory:mem-${n}`,
        note_category: "fact",
        owner_id: "owner-1",
        role: null,
      }
    case "artanis_threads":
      return {
        caller_id: "owner-1",
        caller_kind: "owner",
        created_at: ISO,
        last_message_at: ISO,
        metadata_json: "{}",
        source_ref: null,
        status: "open",
        subject_agent_kind: "artanis",
        subject_agent_ref: "artanis",
        thread_ref: `artanis_thread:thread-${n}`,
        title: "Backfill thread",
        updated_at: ISO,
      }
    case "artanis_messages":
      return {
        author_id: "owner-1",
        author_kind: "owner",
        body: "backfill sample message",
        caller_id: "owner-1",
        created_at: ISO,
        message_ref: `artanis_message:msg-${n}`,
        metadata_json: "{}",
        thread_ref: "artanis_thread:thread-1",
      }
  }
}

describe("artanisRowHash (pure)", () => {
  test("identical rows hash identically; any column change diverges", () => {
    const a = sampleRow("artanis_spend_decisions", 1)
    const b = sampleRow("artanis_spend_decisions", 1)
    expect(artanisRowHash("artanis_spend_decisions", a)).toBe(
      artanisRowHash("artanis_spend_decisions", b),
    )
    expect(
      artanisRowHash("artanis_spend_decisions", { ...a, state: "refused" }),
    ).not.toBe(artanisRowHash("artanis_spend_decisions", a))
  })

  test("extra columns in the D1 export (e.g. d1_rowid) do not affect the hash", () => {
    const row = sampleRow("artanis_owner_memory", 2)
    expect(
      artanisRowHash("artanis_owner_memory", { ...row, d1_rowid: 42 }),
    ).toBe(artanisRowHash("artanis_owner_memory", row))
  })

  test("registry covers all twenty artanis tables", () => {
    expect(ARTANIS_BACKFILL_TABLES.length).toBe(20)
  })
})

describe.skipIf(!hasLocalPostgres())(
  "artanis backfill against local Postgres",
  () => {
    let pg: LocalPostgres
    let rawSql: SQL
    let sql: SyncSql

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_artanis_backfill")
      await admin.end()
      const url = pg.urlFor("khala_artanis_backfill")
      const result = await runMigrations({ databaseUrl: url })
      expect(result.applied).toContain("0011_artanis_domain.sql")
      rawSql = new SQL({ url, max: 4 })
      sql = rawSql as unknown as SyncSql
    })

    afterAll(async () => {
      await rawSql?.end()
      await pg?.stop()
    })

    test("all twenty tables accept registry-shaped rows; run twice → identical state", async () => {
      // responder_state is a singleton (id = 1): one row only.
      const pageFor = (table: ArtanisBackfillTable): Array<D1SourceRow> =>
        table === "artanis_responder_state"
          ? [sampleRow(table, 1)]
          : [sampleRow(table, 1), sampleRow(table, 2)]

      let firstInserted = 0
      for (const table of ARTANIS_BACKFILL_TABLES) {
        firstInserted += await upsertArtanisRows(sql, table, pageFor(table))
      }
      expect(firstInserted).toBe(19 * 2 + 1)

      const talliesAfterFirst = await Promise.all(
        ARTANIS_BACKFILL_TABLES.map((table) =>
          postgresArtanisTally(sql, table),
        ),
      )
      const hashesAfterFirst = await Promise.all(
        ARTANIS_BACKFILL_TABLES.map((table) =>
          postgresArtanisNewestRowHashes(sql, table, 10),
        ),
      )

      // Second sweep: same pages again — nothing inserted, nothing changed.
      let secondInserted = 0
      for (const table of ARTANIS_BACKFILL_TABLES) {
        secondInserted += await upsertArtanisRows(sql, table, pageFor(table))
      }
      expect(secondInserted).toBe(0)

      expect(
        await Promise.all(
          ARTANIS_BACKFILL_TABLES.map((table) =>
            postgresArtanisTally(sql, table),
          ),
        ),
      ).toEqual(talliesAfterFirst)
      expect(
        await Promise.all(
          ARTANIS_BACKFILL_TABLES.map((table) =>
            postgresArtanisNewestRowHashes(sql, table, 10),
          ),
        ),
      ).toEqual(hashesAfterFirst)
    })

    test("DO NOTHING never clobbers a dual-write-mirrored row", async () => {
      // The mirror wrote the LIVE state of the responder action
      // (proposed → responded).
      await rawSql`
        UPDATE artanis_responder_actions
           SET state = 'responded', updated_at = '2026-07-04T01:00:00.000Z'
         WHERE topic_id = 'topic-1'`

      // A stale backfill page (snapshot taken before the transition) re-runs.
      await upsertArtanisRows(sql, "artanis_responder_actions", [
        sampleRow("artanis_responder_actions", 1),
      ])

      const rows = await rawSql`
        SELECT state FROM artanis_responder_actions WHERE topic_id = 'topic-1'`
      expect(rows[0]?.state).toBe("responded")
    })

    test("verify comparison catches count, tally, and hash drift", async () => {
      const spec = ARTANIS_TABLE_SPECS.artanis_closeout_verdicts
      expect(spec.conflictKey).toBe("assignment_ref")

      const pgTally = await postgresArtanisTally(
        sql,
        "artanis_closeout_verdicts",
      )
      const pgNewest = await postgresArtanisNewestRowHashes(
        sql,
        "artanis_closeout_verdicts",
        10,
      )

      // Matching D1 side → green report.
      const d1Rows = [
        sampleRow("artanis_closeout_verdicts", 2),
        sampleRow("artanis_closeout_verdicts", 1),
      ]
      const green = compareArtanisTallies(
        "artanis_closeout_verdicts",
        { byStatus: { verified: 2 }, total: 2 },
        pgTally,
        d1ArtanisNewestRowHashes("artanis_closeout_verdicts", d1Rows),
        pgNewest,
      )
      expect(green.countsMatch).toBe(true)
      expect(green.statusMismatches).toEqual([])
      expect(green.newestHashMismatches).toEqual([])

      // Drifted D1 side (extra row, different outcome, changed detail) →
      // every check trips.
      const drifted = compareArtanisTallies(
        "artanis_closeout_verdicts",
        { byStatus: { rejected: 1, verified: 2 }, total: 3 },
        pgTally,
        d1ArtanisNewestRowHashes("artanis_closeout_verdicts", [
          { ...d1Rows[0]!, detail: "drifted" },
          d1Rows[1]!,
        ]),
        pgNewest,
      )
      expect(drifted.countsMatch).toBe(false)
      expect(drifted.statusMismatches.length).toBeGreaterThan(0)
      expect(drifted.newestHashMismatches.length).toBe(1)
    })
  },
)
