// KS-8.10 (#8321): forum content backfill core — idempotency + verify
// fidelity.
//
// Load-bearing properties: converge upserts are IDEMPOTENT (a re-run with
// the same D1 page converges to the identical Postgres state) and
// converge to the LATEST D1 snapshot (counter bumps, moderation state
// flips, body edits), the per-topic post-chain comparator catches
// missing/extra/renumbered posts exactly, the per-thread spot hash
// detects any single-byte body drift while never exposing body text, and
// the row hash canonicalizes D1 numbers and postgres.js bigint strings to
// the same digest. Privacy: no assertion prints body text — hashes and
// keys only, same as the CLI.

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
  buildForumContentVerifyReport,
  comparePostChains,
  d1ForumContentNewestHashes,
  FORUM_CONTENT_SCALAR_TALLIES,
  forumContentRowHash,
  forumContentVerifyReportClean,
  postChainSql,
  postChainTallyFromRows,
  postgresForumContentNewestHashes,
  postgresForumContentRowCount,
  postgresForumContentScalar,
  postgresPostChainTally,
  postgresThreadSpotHash,
  threadSpotHashFromRows,
  upsertForumContentRows,
  type D1SourceRow,
} from "./forum-content-backfill.js"
import { runMigrations } from "./migrate.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

// ---------------------------------------------------------------------------
// Fixtures (snake_case rows exactly as `wrangler d1 execute --json` returns)
// ---------------------------------------------------------------------------

const T0 = "2026-07-04T00:00:00.000Z"
const T1 = "2026-07-04T01:00:00.000Z"

const topicRow = (
  n: number,
  overrides: Partial<Record<string, unknown>> = {},
): D1SourceRow => ({
  actor_json: '{"actorRef":"agent_raynor"}',
  actor_ref: "agent_raynor",
  archived_at: null,
  created_at: T0,
  first_post_id: `post_${n}_1`,
  forum_id: "forum_general",
  id: `topic_${n}`,
  idempotency_key: `topic-key-${n}`,
  latest_post_id: `post_${n}_1`,
  pin_state: "normal",
  post_count: 1,
  public_projection_json: "{}",
  score_ref: null,
  slug: `topic-${n}`,
  state: "open",
  title: `Backfill topic ${n}`,
  updated_at: T0,
  ...overrides,
})

const postRow = (
  topicN: number,
  postNumber: number,
  overrides: Partial<Record<string, unknown>> = {},
): D1SourceRow => ({
  actor_json: '{"actorRef":"agent_raynor"}',
  actor_ref: "agent_raynor",
  archived_at: null,
  content_ref: `content.forum.post.${topicN}.${postNumber}`,
  created_at: T0,
  forum_id: "forum_general",
  id: `post_${topicN}_${postNumber}`,
  idempotency_key: `post-key-${topicN}-${postNumber}`,
  parent_post_id: null,
  post_number: postNumber,
  public_projection_json: "{}",
  quote_post_id: null,
  receipt_refs_json: "[]",
  revision_ref: null,
  state: "visible",
  topic_id: `topic_${topicN}`,
  updated_at: T0,
  ...overrides,
})

const bodyRow = (
  topicN: number,
  postNumber: number,
  overrides: Partial<Record<string, unknown>> = {},
): D1SourceRow => ({
  archived_at: null,
  body_text: `body of post ${postNumber} in topic ${topicN}`,
  content_kind: "plain_text",
  created_at: T0,
  post_id: `post_${topicN}_${postNumber}`,
  updated_at: T0,
  ...overrides,
})

const forumRow = (
  overrides: Partial<Record<string, unknown>> = {},
): D1SourceRow => ({
  archived_at: null,
  board_id: "board_openagents",
  category_id: "category_general",
  created_at: T0,
  description_ref: null,
  discoverability: "listed",
  id: "forum_general",
  latest_post_id: null,
  latest_topic_id: null,
  locked: 0,
  post_count: 0,
  public_projection_json: "{}",
  slug: "general",
  title: "General",
  topic_count: 0,
  updated_at: T0,
  visibility: "public",
  ...overrides,
})

// ---------------------------------------------------------------------------
// Pure comparators
// ---------------------------------------------------------------------------

describe("forumContentRowHash (pure)", () => {
  test("identical rows hash identically; any column drift changes the hash", () => {
    const base = topicRow(1)
    expect(forumContentRowHash("forum_topics", base)).toBe(
      forumContentRowHash("forum_topics", { ...base }),
    )
    expect(forumContentRowHash("forum_topics", base)).not.toBe(
      forumContentRowHash("forum_topics", { ...base, title: "drifted" }),
    )
  })

  test("D1 numbers and postgres.js bigint strings canonicalize equal", () => {
    const d1Side = postRow(1, 7, { post_number: 7 })
    const pgSide = postRow(1, 7, { post_number: "7" })
    expect(forumContentRowHash("forum_posts", d1Side)).toBe(
      forumContentRowHash("forum_posts", pgSide),
    )
  })

  test("NULL and empty string hash differently", () => {
    const withNull = topicRow(2, { score_ref: null })
    const withEmpty = topicRow(2, { score_ref: "" })
    expect(forumContentRowHash("forum_topics", withNull)).not.toBe(
      forumContentRowHash("forum_topics", withEmpty),
    )
  })
})

describe("post chain comparison (pure)", () => {
  const chainRows = (
    entries: ReadonlyArray<[string, number, number, number, number]>,
  ) =>
    postChainTallyFromRows(
      entries.map(([topicId, posts, distinct, min, max]) => ({
        distinct_numbers: distinct,
        max_number: max,
        min_number: min,
        posts,
        topic_id: topicId,
      })),
    )

  test("equal chains produce no mismatches", () => {
    const tally = chainRows([
      ["topic_1", 3, 3, 1, 3],
      ["topic_2", 1, 1, 1, 1],
    ])
    expect(comparePostChains(tally, tally)).toEqual([])
  })

  test("missing topic, short chain, and renumbering are all caught", () => {
    const d1 = chainRows([
      ["topic_1", 3, 3, 1, 3],
      ["topic_2", 1, 1, 1, 1],
      ["topic_3", 2, 2, 1, 2],
    ])
    const pg = chainRows([
      ["topic_1", 2, 2, 1, 2], // short chain
      ["topic_2", 1, 1, 2, 2], // renumbered
      // topic_3 missing entirely
      ["topic_4", 1, 1, 1, 1], // extra on pg
    ])
    const mismatches = comparePostChains(d1, pg)
    expect(mismatches.map((m) => m.topicId).sort()).toEqual([
      "topic_1",
      "topic_2",
      "topic_3",
      "topic_4",
    ])
  })
})

describe("threadSpotHashFromRows (pure)", () => {
  const chain = [
    { body_text: "first body", id: "p1", post_number: 1, state: "visible" },
    { body_text: "second body", id: "p2", post_number: 2, state: "visible" },
  ]

  test("stable for identical chains, sensitive to body drift and order", () => {
    expect(threadSpotHashFromRows(chain)).toBe(
      threadSpotHashFromRows([...chain]),
    )
    expect(threadSpotHashFromRows(chain)).not.toBe(
      threadSpotHashFromRows([
        chain[0]!,
        { ...chain[1]!, body_text: "second body." },
      ]),
    )
    expect(threadSpotHashFromRows(chain)).not.toBe(
      threadSpotHashFromRows([chain[1]!, chain[0]!]),
    )
  })

  test("tombstoned NULL bodies participate without throwing", () => {
    const tombstoned = [
      { body_text: null, id: "p1", post_number: 1, state: "tombstoned" },
    ]
    expect(threadSpotHashFromRows(tombstoned)).toBe(
      threadSpotHashFromRows([...tombstoned]),
    )
    expect(threadSpotHashFromRows(tombstoned)).not.toBe(
      threadSpotHashFromRows([
        { body_text: "", id: "p1", post_number: 1, state: "tombstoned" },
      ]),
    )
  })

  test("output is a hash, never body text", () => {
    const hash = threadSpotHashFromRows(chain)
    expect(hash).toMatch(/^[0-9a-f]{64}$/)
    expect(hash).not.toContain("first body")
  })
})

describe("verify report (pure)", () => {
  test("clean report on matching inputs; drift flips it", () => {
    const clean = buildForumContentVerifyReport({
      d1Newest: [{ hash: "abc", key: "topic_1" }],
      d1Total: 1,
      postgresNewest: [{ hash: "abc", key: "topic_1" }],
      postgresTotal: 1,
      scalars: [{ d1: 5, metric: "sum_post_count", postgres: 5 }],
      table: "forum_topics",
    })
    expect(forumContentVerifyReportClean(clean)).toBe(true)

    const drifted = buildForumContentVerifyReport({
      d1Newest: [{ hash: "abc", key: "topic_1" }],
      d1Total: 2,
      postgresNewest: [{ hash: "def", key: "topic_1" }],
      postgresTotal: 1,
      scalars: [{ d1: 5, metric: "sum_post_count", postgres: 4 }],
      table: "forum_topics",
    })
    expect(forumContentVerifyReportClean(drifted)).toBe(false)
    expect(drifted.countsMatch).toBe(false)
    expect(drifted.scalarMismatches).toHaveLength(1)
    expect(drifted.newestHashMismatches).toHaveLength(1)
  })
})

// ---------------------------------------------------------------------------
// Postgres integration (skipped without local Postgres binaries)
// ---------------------------------------------------------------------------

describe.skipIf(!hasLocalPostgres())(
  "forum content backfill — Postgres",
  () => {
    let pg: LocalPostgres
    let rawSql: SQL
    let sql: SyncSql

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_forum_content_backfill")
      await admin.end()
      const url = pg.urlFor("khala_forum_content_backfill")
      const result = await runMigrations({ databaseUrl: url })
      expect(result.applied).toContain("0014_forum_content.sql")
      rawSql = new SQL({ url, max: 4 })
      sql = rawSql as unknown as SyncSql
    })

    afterAll(async () => {
      await rawSql?.end()
      await pg?.stop()
    })

    test("converge upsert is idempotent and converges to the latest D1 snapshot", async () => {
      const first = [topicRow(1), topicRow(2)]
      expect(
        await upsertForumContentRows(sql, "forum_topics", first),
      ).toBe(2)
      // Re-running the SAME page converges without duplication.
      expect(
        await upsertForumContentRows(sql, "forum_topics", first),
      ).toBe(2)
      expect(await postgresForumContentRowCount(sql, "forum_topics")).toBe(2)

      // A later D1 snapshot (reply bump) converges the row forward.
      await upsertForumContentRows(sql, "forum_topics", [
        topicRow(1, {
          latest_post_id: "post_1_2",
          post_count: 2,
          updated_at: T1,
        }),
      ])
      const rows = await (
        sql as unknown as {
          unsafe: (q: string, p: Array<unknown>) => Promise<Array<Record<string, unknown>>>
        }
      ).unsafe(`SELECT post_count, latest_post_id FROM forum_topics WHERE id = $1`, [
        "topic_1",
      ])
      expect(Number(rows[0]?.["post_count"])).toBe(2)
      expect(rows[0]?.["latest_post_id"]).toBe("post_1_2")
    })

    test("scalar tallies, post chains, and newest hashes agree with the D1-side helpers", async () => {
      await upsertForumContentRows(sql, "forum_forums", [
        forumRow({ post_count: 3, topic_count: 1 }),
      ])
      const posts = [postRow(1, 1), postRow(1, 2), postRow(2, 1)]
      await upsertForumContentRows(sql, "forum_posts", posts)
      const bodies = [bodyRow(1, 1), bodyRow(1, 2), bodyRow(2, 1)]
      await upsertForumContentRows(sql, "forum_post_bodies", bodies)

      // Scalar tally: sum of post numbers is 1+2+1 = 4.
      const sumTally = FORUM_CONTENT_SCALAR_TALLIES.forum_posts.find(
        (tally) => tally.metric === "sum_post_number",
      )
      expect(sumTally).toBeDefined()
      expect(await postgresForumContentScalar(sql, sumTally!.sql)).toBe(4)

      // Post chains: postgres tally equals the tally built from the same
      // rows on the D1 side (simulated with the exact chain SQL shape).
      const pgChains = await postgresPostChainTally(sql)
      const d1Chains = postChainTallyFromRows([
        { distinct_numbers: 2, max_number: 2, min_number: 1, posts: 2, topic_id: "topic_1" },
        { distinct_numbers: 1, max_number: 1, min_number: 1, posts: 1, topic_id: "topic_2" },
      ])
      expect(comparePostChains(d1Chains, pgChains)).toEqual([])
      expect(postChainSql()).toContain("GROUP BY topic_id")

      // Newest hashes: hashing the same source rows on both sides matches.
      const pgNewest = await postgresForumContentNewestHashes(
        sql,
        "forum_posts",
        10,
      )
      const d1Newest = d1ForumContentNewestHashes(
        "forum_posts",
        [...posts].sort((a, b) =>
          String(b["id"]).localeCompare(String(a["id"])),
        ),
      )
      const report = buildForumContentVerifyReport({
        d1Newest,
        d1Total: 3,
        postgresNewest: pgNewest,
        postgresTotal: await postgresForumContentRowCount(sql, "forum_posts"),
        scalars: [],
        table: "forum_posts",
      })
      expect(report.countsMatch).toBe(true)
      expect(report.newestHashMismatches).toEqual([])
    })

    test("thread spot hash matches the pure hash over the same chain and catches body drift", async () => {
      const d1Side = threadSpotHashFromRows([
        { body_text: "body of post 1 in topic 1", id: "post_1_1", post_number: 1, state: "visible" },
        { body_text: "body of post 2 in topic 1", id: "post_1_2", post_number: 2, state: "visible" },
      ])
      expect(await postgresThreadSpotHash(sql, "topic_1")).toBe(d1Side)

      // Drift one body byte on the Postgres side — the hash must diverge.
      await upsertForumContentRows(sql, "forum_post_bodies", [
        bodyRow(1, 2, { body_text: "body of post 2 in topic 1!" }),
      ])
      expect(await postgresThreadSpotHash(sql, "topic_1")).not.toBe(d1Side)
      // Converge back to the D1 snapshot — the hash must match again
      // (backfill catch-up sweep semantics).
      await upsertForumContentRows(sql, "forum_post_bodies", [bodyRow(1, 2)])
      expect(await postgresThreadSpotHash(sql, "topic_1")).toBe(d1Side)
    })

    test("dedupe keys port exactly: duplicate idempotency_key rejected on a DIFFERENT id", async () => {
      await upsertForumContentRows(sql, "forum_reports", [
        {
          archived_at: null,
          created_at: T0,
          id: "report_1",
          idempotency_key: "report-key-1",
          public_projection_json: "{}",
          reason_ref: "reason.forum.report.spam",
          reporter_actor_ref: "agent_raynor",
          status: "open",
          target_id: "post_1_1",
          target_kind: "post",
          updated_at: T0,
        },
      ])
      await expect(
        upsertForumContentRows(sql, "forum_reports", [
          {
            archived_at: null,
            created_at: T1,
            id: "report_2",
            idempotency_key: "report-key-1",
            public_projection_json: "{}",
            reason_ref: "reason.forum.report.spam",
            reporter_actor_ref: "agent_kerrigan",
            status: "open",
            target_id: "post_1_2",
            target_kind: "post",
            updated_at: T1,
          },
        ]),
      ).rejects.toThrow()
    })
  },
)
