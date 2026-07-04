// KS-8.2 (#8308): token ledger backfill core — idempotency + verify
// fidelity.
//
// Load-bearing properties: running the same events page twice yields an
// IDENTICAL Postgres state (second run inserts zero rows), the events
// path NEVER clobbers a dual-write-mirrored row, the rollup converge
// upsert converges to the D1 snapshot value (re-runs are stable), and the
// verify comparators catch count / token-sum / per-provider / hash drift
// exactly.

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
  compareTokenLedgerEventsTallies,
  compareTokenLedgerRollupTallies,
  d1TokenLedgerNewestHashes,
  postgresTokenLedgerEventsTally,
  postgresTokenLedgerNewestHashes,
  postgresTokenLedgerRollupTally,
  tallyFromRows,
  tokenLedgerRowHash,
  upsertTokenLedgerRows,
  type D1SourceRow,
} from "./token-ledger-backfill.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const eventSourceRow = (
  n: number,
  overrides: Partial<Record<string, unknown>> = {},
): D1SourceRow => ({
  account_ref: null,
  actor_team_id: null,
  actor_user_id: `user-${n}`,
  anonymized_source_ref: null,
  backend_profile: "backfill-backend",
  cache_read_tokens: 0,
  cache_write_1h_tokens: 0,
  cache_write_5m_tokens: 0,
  cost_amount: n === 2 ? 0.00125 : null,
  currency: n === 2 ? "USD" : null,
  demand_channel: "khala_api",
  demand_client: "khala-cli",
  demand_kind: "external",
  demand_source: "backfill-suite",
  id: `token_event_backfill_${n}`,
  idempotency_key: `backfill:${n}`,
  ingested_at: `2026-07-01T0${n}:00:01.000Z`,
  input_tokens: 100 * n,
  leaderboard_eligible: 1,
  model: "glm-4.7",
  observed_at: `2026-07-01T0${n}:00:00.000Z`,
  output_tokens: 10 * n,
  privacy_opt_out: 0,
  producer_system: "probe",
  provider: n === 3 ? "openai" : "zai",
  reasoning_tokens: 0,
  repository_ref: null,
  role_ref: null,
  run_ref: null,
  safe_metadata_json: "{}",
  session_ref: null,
  source_route: "khala_completions",
  task_ref: null,
  total_tokens: 110 * n,
  usage_truth: "exact",
  ...overrides,
})

const dailyRollupRow = (
  day: string,
  tokens: number,
  events: number,
): D1SourceRow => ({
  day,
  timezone: "America/Chicago",
  tokens_served: tokens,
  updated_at: "2026-07-01T12:00:00.000Z",
  usage_events: events,
})

describe("tokenLedgerRowHash (pure)", () => {
  test("identical rows hash identically; any column change diverges", () => {
    const a = eventSourceRow(1)
    const b = eventSourceRow(1)
    expect(tokenLedgerRowHash("token_usage_events", a)).toBe(
      tokenLedgerRowHash("token_usage_events", b),
    )
    expect(
      tokenLedgerRowHash("token_usage_events", { ...a, total_tokens: 111 }),
    ).not.toBe(tokenLedgerRowHash("token_usage_events", a))
  })

  test("extra columns in the D1 export (e.g. d1_rowid) do not affect the hash", () => {
    const row = eventSourceRow(2)
    expect(
      tokenLedgerRowHash("token_usage_events", { ...row, d1_rowid: 42 }),
    ).toBe(tokenLedgerRowHash("token_usage_events", row))
  })
})

describe.skipIf(!hasLocalPostgres())(
  "token ledger backfill against local Postgres",
  () => {
    let pg: LocalPostgres
    let rawSql: SQL
    let sql: SyncSql

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_token_ledger_backfill")
      await admin.end()
      const url = pg.urlFor("khala_token_ledger_backfill")
      const result = await runMigrations({ databaseUrl: url })
      expect(result.applied).toContain("0008_token_usage_ledger.sql")
      rawSql = new SQL({ url, max: 4 })
      sql = rawSql as unknown as SyncSql
    })

    afterAll(async () => {
      await rawSql?.end()
      await pg?.stop()
    })

    test("events: run twice → identical state (idempotency), one statement per page", async () => {
      const page = [eventSourceRow(1), eventSourceRow(2), eventSourceRow(3)]

      const first = await upsertTokenLedgerRows(sql, "token_usage_events", page)
      expect(first).toBe(3)

      const tallyAfterFirst = await postgresTokenLedgerEventsTally(sql)
      const hashesAfterFirst = await postgresTokenLedgerNewestHashes(
        sql,
        "token_usage_events",
        10,
      )

      const second = await upsertTokenLedgerRows(sql, "token_usage_events", page)
      expect(second).toBe(0)

      expect(await postgresTokenLedgerEventsTally(sql)).toEqual(tallyAfterFirst)
      expect(
        await postgresTokenLedgerNewestHashes(sql, "token_usage_events", 10),
      ).toEqual(hashesAfterFirst)

      // The tally itself is exact: 110 + 220 + 330 total, split by provider.
      expect(tallyAfterFirst.totalRows).toBe(3)
      expect(tallyAfterFirst.sumTotalTokens).toBe(660)
      expect(tallyAfterFirst.sumPublicTokensServed).toBe(660)
      expect(tallyAfterFirst.byProvider["zai"]).toEqual({
        rows: 2,
        totalTokens: 330,
      })
      expect(tallyAfterFirst.byProvider["openai"]).toEqual({
        rows: 1,
        totalTokens: 330,
      })
    })

    test("events: DO NOTHING never clobbers a dual-write-mirrored row", async () => {
      // A stale backfill page (different token counts under the same
      // idempotency key/id) re-runs: the mirrored row wins.
      await upsertTokenLedgerRows(sql, "token_usage_events", [
        eventSourceRow(1, { total_tokens: 999_999 }),
      ])
      const rows = await rawSql`
        SELECT total_tokens FROM token_usage_events
         WHERE id = 'token_event_backfill_1'`
      expect(Number(rows[0]?.total_tokens)).toBe(110)
    })

    test("rollups: converge upsert sets the D1 snapshot value and re-runs stably", async () => {
      const snapshot = [dailyRollupRow("2026-07-01", 660, 3)]
      await upsertTokenLedgerRows(
        sql,
        "public_khala_tokens_served_daily_rollups",
        snapshot,
      )
      // A live mirror increment lands after the snapshot copy...
      await rawSql`
        UPDATE public_khala_tokens_served_daily_rollups
           SET tokens_served = tokens_served + 40, usage_events = usage_events + 1
         WHERE timezone = 'America/Chicago' AND day = '2026-07-01'`
      // ...and the catch-up sweep converges to the NEW D1 snapshot (which
      // includes that same event on the D1 side).
      await upsertTokenLedgerRows(
        sql,
        "public_khala_tokens_served_daily_rollups",
        [dailyRollupRow("2026-07-01", 700, 4)],
      )
      const tally = await postgresTokenLedgerRollupTally(
        sql,
        "public_khala_tokens_served_daily_rollups",
      )
      expect(tally).toEqual({
        sumTokensServed: 700,
        sumUsageEvents: 4,
        totalRows: 1,
      })
    })

    test("verify comparators catch count, token-sum, provider, and hash drift", async () => {
      const d1Rows = [
        eventSourceRow(1),
        eventSourceRow(2),
        eventSourceRow(3),
      ]
      const d1Tally = tallyFromRows(
        {
          sum_public: 660,
          sum_total_tokens: 660,
          total_rows: 3,
        },
        [
          { provider_key: "openai", row_count: 1, total_tokens: 330 },
          { provider_key: "zai", row_count: 2, total_tokens: 330 },
        ],
      )
      const pgTally = await postgresTokenLedgerEventsTally(sql)
      const pgNewest = await postgresTokenLedgerNewestHashes(
        sql,
        "token_usage_events",
        10,
      )

      const green = compareTokenLedgerEventsTallies(
        d1Tally,
        pgTally,
        d1TokenLedgerNewestHashes("token_usage_events", d1Rows),
        pgNewest,
      )
      expect(green.countsMatch).toBe(true)
      expect(green.scalarMismatches).toEqual([])
      expect(green.providerMismatches).toEqual([])
      expect(green.newestHashMismatches).toEqual([])

      // Token-sum drift by a single token is caught.
      const driftTally = tallyFromRows(
        { sum_public: 661, sum_total_tokens: 661, total_rows: 3 },
        [
          { provider_key: "openai", row_count: 1, total_tokens: 331 },
          { provider_key: "zai", row_count: 2, total_tokens: 330 },
        ],
      )
      const red = compareTokenLedgerEventsTallies(
        driftTally,
        pgTally,
        d1TokenLedgerNewestHashes("token_usage_events", d1Rows),
        pgNewest,
      )
      expect(red.scalarMismatches.map((entry) => entry.metric)).toEqual([
        "sum_total_tokens",
        "sum_public_tokens_served",
      ])
      expect(red.providerMismatches.map((entry) => entry.provider)).toEqual([
        "openai",
      ])

      // A stale row hash is caught.
      const staleHashes = d1TokenLedgerNewestHashes("token_usage_events", [
        eventSourceRow(1, { total_tokens: 111 }),
        eventSourceRow(2),
        eventSourceRow(3),
      ])
      const hashDrift = compareTokenLedgerEventsTallies(
        d1Tally,
        pgTally,
        staleHashes,
        pgNewest,
      )
      expect(
        hashDrift.newestHashMismatches.map((entry) => entry.key),
      ).toEqual(["token_event_backfill_1"])

      // Rollup comparator: sums must match exactly.
      const rollupTally = await postgresTokenLedgerRollupTally(
        sql,
        "public_khala_tokens_served_daily_rollups",
      )
      const rollupRed = compareTokenLedgerRollupTallies(
        "public_khala_tokens_served_daily_rollups",
        { sumTokensServed: 699, sumUsageEvents: 4, totalRows: 1 },
        rollupTally,
        [],
        [],
      )
      expect(rollupRed.scalarMismatches.map((entry) => entry.metric)).toEqual([
        "sum_tokens_served",
      ])
    })

    test("leaderboard preferences converge upsert is stable", async () => {
      const preference: D1SourceRow = {
        leaderboard_participation: "opted_out",
        leaderboard_visibility: "private",
        subject_kind: "user",
        subject_ref: "user-backfill",
        updated_at: "2026-07-01T12:00:00.000Z",
        updated_by_user_id: "user-backfill",
      }
      await upsertTokenLedgerRows(sql, "token_usage_leaderboard_preferences", [
        preference,
      ])
      await upsertTokenLedgerRows(sql, "token_usage_leaderboard_preferences", [
        { ...preference, leaderboard_participation: "eligible" },
      ])
      const rows = await rawSql`
        SELECT leaderboard_participation
          FROM token_usage_leaderboard_preferences
         WHERE subject_kind = 'user' AND subject_ref = 'user-backfill'`
      expect(rows[0]?.leaderboard_participation).toBe("eligible")
    })
  },
)
