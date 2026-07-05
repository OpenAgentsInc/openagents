// KS-8.10 remainder (#8338): forum remainder backfill core — idempotency,
// verify fidelity, and work-request set-membership referential checks.
//
// Load-bearing properties: converge upserts are IDEMPOTENT and converge to
// the LATEST D1 snapshot; the row hash canonicalizes D1 numbers and
// postgres.js bigint strings to the same digest; the work-request
// referential checks resolve within a store and flag orphans; the
// cross-domain ref-set digest is order-stable. Privacy:
// no assertion prints a subject, participant, or message content — hashes,
// keys, and counts only.

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
  buildForumRemainderVerifyReport,
  FORUM_REMAINDER_SCALAR_TALLIES,
  forumRemainderRowHash,
  forumRemainderVerifyReportClean,
  postgresForumRemainderNewestHashes,
  postgresForumRemainderRowCount,
  postgresForumRemainderScalar,
  postgresRefSetDigest,
  postgresScalarValue,
  refSetDigest,
  FORUM_WORK_REQUEST_CROSS_DOMAIN_REF_SETS,
  FORUM_WORK_REQUEST_REFERENTIAL_CHECKS,
  upsertForumRemainderRows,
  type D1RemainderRow,
} from "./forum-remainder-backfill.js"
import { runMigrations } from "./migrate.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const T0 = "2026-07-04T00:00:00.000Z"
const T1 = "2026-07-04T01:00:00.000Z"

const threadRow = (
  n: number,
  overrides: Partial<Record<string, unknown>> = {},
): D1RemainderRow => ({
  archived_at: null,
  created_at: T0,
  created_by_actor_ref: "agent_a",
  id: `thread_${n}`,
  latest_message_id: null,
  message_count: 0,
  participant_refs_json: '["agent_a","agent_b"]',
  slug: `thread-${n}`,
  subject: `Subject ${n}`,
  updated_at: T0,
  ...overrides,
})

const workRequestRow = (
  n: number,
  overrides: Partial<Record<string, unknown>> = {},
): D1RemainderRow => ({
  archived_at: null,
  budget_msats: 1000000,
  budget_sats: 1000,
  created_at: T0,
  deadline_ref: "deadline.wr",
  first_post_id: `post_${n}`,
  id: `wr_${n}`,
  idempotency_key: `wr-key-${n}`,
  job_event_id: `job_${n}`,
  job_event_kind: 5934,
  job_result_kind: 6934,
  objective_ref: "objective.wr",
  public_projection_json: "{}",
  quote_count: 0,
  relay_url: "wss://relay",
  repository_refs_json: "[]",
  required_capability_refs_json: "[]",
  requester_actor_ref: "agent_a",
  state: "open",
  title: `WR ${n}`,
  topic_id: `topic_${n}`,
  updated_at: T0,
  verification_command_ref: "verify.wr",
  ...overrides,
})

const offerRow = (
  n: number,
  workRequestId: string,
  overrides: Partial<Record<string, unknown>> = {},
): D1RemainderRow => ({
  amount_msats: 500000,
  amount_sats: 500,
  archived_at: null,
  capability_refs_json: "[]",
  created_at: T0,
  id: `offer_${n}`,
  provider_actor_ref: "agent_b",
  provider_pubkey: null,
  public_projection_json: "{}",
  quote_ref: `quote_${n}`,
  relay_event_ref: null,
  state: "offered",
  updated_at: T0,
  work_request_id: workRequestId,
  ...overrides,
})

// ---------------------------------------------------------------------------
// Pure comparators
// ---------------------------------------------------------------------------

describe("forumRemainderRowHash (pure)", () => {
  test("identical rows hash identically; any drift changes the hash", () => {
    const base = workRequestRow(1)
    expect(forumRemainderRowHash("forum_work_requests", base)).toBe(
      forumRemainderRowHash("forum_work_requests", { ...base }),
    )
    expect(forumRemainderRowHash("forum_work_requests", base)).not.toBe(
      forumRemainderRowHash("forum_work_requests", { ...base, state: "settled" }),
    )
  })

  test("D1 numbers and postgres.js bigint strings canonicalize equal", () => {
    const d1Side = workRequestRow(1, { budget_sats: 1000 })
    const pgSide = workRequestRow(1, { budget_sats: "1000" })
    expect(forumRemainderRowHash("forum_work_requests", d1Side)).toBe(
      forumRemainderRowHash("forum_work_requests", pgSide),
    )
  })
})

describe("refSetDigest (pure)", () => {
  test("order-independent, count-prefixed, opaque", () => {
    const a = refSetDigest([{ ref: "x" }, { ref: "y" }])
    const b = refSetDigest([{ ref: "y" }, { ref: "x" }])
    expect(a).toBe(b)
    expect(a.startsWith("2:")).toBe(true)
    expect(refSetDigest([{ ref: "x" }])).not.toBe(a)
  })
})

// ---------------------------------------------------------------------------
// Postgres integration (skipped without local Postgres binaries)
// ---------------------------------------------------------------------------

describe.skipIf(!hasLocalPostgres())(
  "forum remainder backfill — Postgres",
  () => {
    let pg: LocalPostgres
    let rawSql: SQL
    let sql: SyncSql

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_forum_remainder_backfill")
      await admin.end()
      const url = pg.urlFor("khala_forum_remainder_backfill")
      const result = await runMigrations({ databaseUrl: url })
      expect(result.applied).toContain("0027_forum_remainder.sql")
      rawSql = new SQL({ url, max: 4 })
      sql = rawSql as unknown as SyncSql
    })

    afterAll(async () => {
      await rawSql?.end()
      await pg?.stop()
    })

    test("converge upsert is idempotent and converges to the latest snapshot", async () => {
      const first = [threadRow(1), threadRow(2)]
      expect(
        await upsertForumRemainderRows(sql, "forum_private_message_threads", first),
      ).toBe(2)
      expect(
        await upsertForumRemainderRows(sql, "forum_private_message_threads", first),
      ).toBe(2)
      expect(
        await postgresForumRemainderRowCount(sql, "forum_private_message_threads"),
      ).toBe(2)

      await upsertForumRemainderRows(sql, "forum_private_message_threads", [
        threadRow(1, {
          latest_message_id: "msg_9",
          message_count: 4,
          updated_at: T1,
        }),
      ])
      const rows = await (
        sql as unknown as {
          unsafe: (q: string, p: Array<unknown>) => Promise<Array<Record<string, unknown>>>
        }
      ).unsafe(
        `SELECT message_count, latest_message_id FROM forum_private_message_threads WHERE id = $1`,
        ["thread_1"],
      )
      expect(Number(rows[0]?.["message_count"])).toBe(4)
      expect(rows[0]?.["latest_message_id"]).toBe("msg_9")
    })

    test("scalar tallies and newest hashes agree with the D1-side helpers", async () => {
      await upsertForumRemainderRows(sql, "forum_work_requests", [
        workRequestRow(10, { quote_count: 2, state: "open" }),
        workRequestRow(11, { quote_count: 3, state: "settled" }),
      ])
      const sumTally = FORUM_REMAINDER_SCALAR_TALLIES.forum_work_requests.find(
        t => t.metric === "sum_quote_count",
      )
      expect(sumTally).toBeDefined()
      expect(await postgresForumRemainderScalar(sql, sumTally!.sql)).toBe(5)

      const rows = [workRequestRow(10, { quote_count: 2 }), workRequestRow(11, { quote_count: 3, state: "settled" })]
      const pgNewest = await postgresForumRemainderNewestHashes(
        sql,
        "forum_work_requests",
        10,
      )
      const report = buildForumRemainderVerifyReport({
        d1Newest: rows.map(row => ({
          hash: forumRemainderRowHash("forum_work_requests", row),
          key: String(row["id"]),
        })),
        d1Total: 2,
        postgresNewest: pgNewest.filter(h => h.key.startsWith("wr_1")),
        postgresTotal: 2,
        scalars: [],
        table: "forum_work_requests",
      })
      expect(report.newestHashMismatches).toEqual([])
      expect(forumRemainderVerifyReportClean(report)).toBe(true)
    })

    test("work-request set-membership: referential checks clean, orphan detected", async () => {
      await upsertForumRemainderRows(sql, "forum_work_requests", [
        workRequestRow(20),
      ])
      await upsertForumRemainderRows(sql, "forum_work_request_offers", [
        offerRow(20, "wr_20"),
      ])
      await upsertForumRemainderRows(sql, "forum_work_request_acceptances", [
        {
          acceptance_event_ref: "accept_event_20",
          amount_msats: 500000,
          archived_at: null,
          created_at: T0,
          escrow_id: "escrow_20",
          id: "accept_20",
          idempotency_key: "accept-key-20",
          offer_id: "offer_20",
          provider_actor_ref: "agent_b",
          public_projection_json: "{}",
          quote_ref: "quote_20",
          requester_actor_ref: "agent_a",
          reserve_receipt_ref: "reserve_20",
          work_request_id: "wr_20",
        },
      ])

      for (const check of FORUM_WORK_REQUEST_REFERENTIAL_CHECKS) {
        expect(await postgresScalarValue(sql, check.sql), check.name).toBe(0)
      }

      // Orphan an offer at a non-existent work request; the WR->offers check
      // for that offer's parent must now report 1.
      await upsertForumRemainderRows(sql, "forum_work_request_offers", [
        offerRow(21, "wr_MISSING", { quote_ref: "quote_21" }),
      ])
      const orphanCheck = FORUM_WORK_REQUEST_REFERENTIAL_CHECKS.find(
        c => c.name === "offers.work_request_id -> work_requests.id",
      )
      expect(orphanCheck).toBeDefined()
      expect(await postgresScalarValue(sql, orphanCheck!.sql)).toBe(1)
    })

    test("cross-domain ref-set digest is computable per store", async () => {
      for (const set of FORUM_WORK_REQUEST_CROSS_DOMAIN_REF_SETS) {
        const digest = await postgresRefSetDigest(sql, set.sql)
        expect(digest, set.name).toMatch(/^\d+:[0-9a-f]{64}$/)
      }
    })
  },
)
