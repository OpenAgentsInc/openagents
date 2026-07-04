// KS-8.9 (#8320): inference entitlements backfill core — idempotency +
// verify fidelity.
//
// Load-bearing properties: running the same EVENT page twice yields an
// IDENTICAL Postgres state (second run inserts zero rows), the events
// path NEVER clobbers a dual-write-mirrored row, the converge upsert
// converges STATE tables to the D1 snapshot value (re-runs are stable),
// the cache-entry special case keeps the one-active-entry-per-key partial
// unique index satisfied, and the verify comparators + the
// tally = SUM(events) enforcement invariant catch drift exactly — a lost
// increment is a free-tier leak, a doubled one a false denial.

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
  compareEntitlementsTallies,
  d1EntitlementsNewestHashes,
  entitlementsRowHash,
  entitlementsTallyFromRows,
  INFERENCE_ENTITLEMENTS_TABLE_NAMES,
  postgresEntitlementsNewestHashes,
  postgresEntitlementsTableTally,
  postgresEntitlementsTallyInvariants,
  upsertEntitlementsRows,
  type D1SourceRow,
} from "./inference-entitlements-backfill.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const NOW = "2026-07-04T12:00:00.000Z"

const freeTierUsageEventRow = (n: number): D1SourceRow => ({
  account_ref: `agent:acct-${n % 2}`,
  created_at: NOW,
  request_id: `req-${n}`,
  served_model: "openagents/khala",
  total_tokens: 100 * n,
  usage_day: "2026-07-04",
})

const freeTierUsageTallyRow = (
  accountRef: string,
  requests: number,
  tokens: number,
): D1SourceRow => ({
  account_ref: accountRef,
  created_at: NOW,
  free_request_count: requests,
  free_total_tokens: tokens,
  updated_at: NOW,
  usage_day: "2026-07-04",
})

describe("entitlementsRowHash (pure)", () => {
  test("identical rows hash identically; any column change diverges", () => {
    const a = freeTierUsageEventRow(1)
    const b = freeTierUsageEventRow(1)
    expect(entitlementsRowHash("inference_free_tier_usage_events", a)).toBe(
      entitlementsRowHash("inference_free_tier_usage_events", b),
    )
    expect(
      entitlementsRowHash("inference_free_tier_usage_events", {
        ...a,
        total_tokens: 1,
      }),
    ).not.toBe(entitlementsRowHash("inference_free_tier_usage_events", a))
  })

  test("extra columns in the D1 export (e.g. d1_rowid) do not affect the hash", () => {
    const row = freeTierUsageEventRow(2)
    expect(
      entitlementsRowHash("inference_free_tier_usage_events", {
        ...row,
        d1_rowid: 42,
      }),
    ).toBe(entitlementsRowHash("inference_free_tier_usage_events", row))
  })

  test("registry covers all 29 migrated tables (metric events excluded)", () => {
    expect(INFERENCE_ENTITLEMENTS_TABLE_NAMES).toHaveLength(29)
    expect(INFERENCE_ENTITLEMENTS_TABLE_NAMES).not.toContain(
      "agent_search_metric_events",
    )
  })
})

describe("compareEntitlementsTallies (pure)", () => {
  test("flags row-count, per-group, and newest-hash drift exactly", () => {
    const report = compareEntitlementsTallies(
      "orange_check_entitlements",
      entitlementsTallyFromRows({ total_rows: 3 }, [
        { group_key: "active", row_count: 2 },
        { group_key: "revoked", row_count: 1 },
      ]),
      { byGroup: { active: 3 }, totalRows: 3 },
      [{ hash: "aaa", key: "orange_check_u1" }],
      [{ hash: "bbb", key: "orange_check_u1" }],
    )
    expect(report.countsMatch).toBe(true)
    expect(report.groupMismatches).toEqual([
      { d1Rows: 2, group: "active", postgresRows: 3 },
      { d1Rows: 1, group: "revoked", postgresRows: 0 },
    ])
    expect(report.newestHashMismatches).toHaveLength(1)
  })
})

describe.skipIf(!hasLocalPostgres())(
  "inference entitlements backfill against local Postgres",
  () => {
    let pg: LocalPostgres
    let rawSql: SQL
    let sql: SyncSql

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_entitlements_backfill")
      await admin.end()
      const url = pg.urlFor("khala_entitlements_backfill")
      const result = await runMigrations({ databaseUrl: url })
      expect(result.applied).toContain("0013_inference_entitlements.sql")
      rawSql = new SQL({ url, max: 4 })
      sql = rawSql as unknown as SyncSql
    })

    afterAll(async () => {
      await rawSql?.end()
      await pg?.stop()
    })

    test("event pages are idempotent: second run inserts zero rows", async () => {
      const rows = [1, 2, 3].map(freeTierUsageEventRow)
      expect(
        await upsertEntitlementsRows(
          sql,
          "inference_free_tier_usage_events",
          rows,
        ),
      ).toBe(3)
      expect(
        await upsertEntitlementsRows(
          sql,
          "inference_free_tier_usage_events",
          rows,
        ),
      ).toBe(0)
      const tally = await postgresEntitlementsTableTally(
        sql,
        "inference_free_tier_usage_events",
      )
      expect(tally.totalRows).toBe(3)
      expect(tally.byGroup["2026-07-04"]).toBe(3)
    })

    test("converge tables re-run stable and converge to the D1 snapshot", async () => {
      const first = freeTierUsageTallyRow("agent:acct-0", 1, 100)
      await upsertEntitlementsRows(sql, "inference_free_tier_usage", [first])
      // D1 snapshot moved (more accruals landed) — converge overwrites.
      const updated = freeTierUsageTallyRow("agent:acct-0", 2, 400)
      await upsertEntitlementsRows(sql, "inference_free_tier_usage", [
        updated,
      ])
      const rows = (await rawSql.unsafe(
        `SELECT free_request_count, free_total_tokens
           FROM inference_free_tier_usage
          WHERE account_ref = 'agent:acct-0' AND usage_day = '2026-07-04'`,
      )) as Array<Record<string, unknown>>
      expect(Number(rows[0]?.["free_request_count"])).toBe(2)
      expect(Number(rows[0]?.["free_total_tokens"])).toBe(400)
      // Re-running the same snapshot is stable.
      await upsertEntitlementsRows(sql, "inference_free_tier_usage", [
        updated,
      ])
      const again = (await rawSql.unsafe(
        `SELECT COUNT(*) AS c FROM inference_free_tier_usage WHERE account_ref = 'agent:acct-0'`,
      )) as Array<Record<string, unknown>>
      expect(Number(again[0]?.["c"])).toBe(1)
    })

    test("tally = SUM(events) invariant: exact per-key equality passes, drift is flagged", async () => {
      // acct-0 events: req-2 (200 tokens); acct-1 events: req-1, req-3
      // (100 + 300). Converge acct-1's tally to the CONSISTENT snapshot.
      await upsertEntitlementsRows(sql, "inference_free_tier_usage", [
        freeTierUsageTallyRow("agent:acct-1", 2, 400),
      ])
      // acct-0's tally says 2 requests/400 tokens but only ONE event (200)
      // exists — a doubled increment (false-denial risk). The invariant
      // must flag it.
      const withDrift = await postgresEntitlementsTallyInvariants(sql)
      const freeTier = withDrift.find(
        (report) => report.family === "free_tier_usage",
      )
      expect(freeTier).toBeDefined()
      expect(freeTier?.mismatches).toEqual([
        {
          eventsAmount: 200,
          eventsCount: 1,
          key: "agent:acct-0:2026-07-04",
          tallyAmount: 400,
          tallyCount: 2,
        },
      ])

      // Converge acct-0 to the event-derived truth — invariant goes green.
      await upsertEntitlementsRows(sql, "inference_free_tier_usage", [
        freeTierUsageTallyRow("agent:acct-0", 1, 200),
      ])
      const clean = await postgresEntitlementsTallyInvariants(sql)
      for (const report of clean) {
        expect({ family: report.family, mismatches: report.mismatches }).toEqual(
          { family: report.family, mismatches: [] },
        )
      }
    })

    test("cache-entry converge archives the conflicting ACTIVE twin (partial unique index survives)", async () => {
      const active = (id: string, createdAt: string): D1SourceRow => ({
        archived_at: null,
        cache_key: "khala:query:1",
        cost_dollars: 0.003,
        created_at: createdAt,
        expires_at: "2026-07-05T00:00:00.000Z",
        id,
        mode: "basic",
        provider: "exa",
        result_count: 2,
        results_json: `[{"id":"${id}"}]`,
      })
      await upsertEntitlementsRows(sql, "agent_search_cache_entries", [
        active("cache-1", "2026-07-04T10:00:00.000Z"),
      ])
      // A NEWER active D1 row for the same cache_key must not violate the
      // one-active-entry-per-key index: the old twin is archived first.
      await upsertEntitlementsRows(sql, "agent_search_cache_entries", [
        active("cache-2", "2026-07-04T11:00:00.000Z"),
      ])
      const rows = (await rawSql.unsafe(
        `SELECT id, archived_at FROM agent_search_cache_entries
          WHERE cache_key = 'khala:query:1' ORDER BY id`,
      )) as Array<Record<string, unknown>>
      expect(rows).toHaveLength(2)
      expect(rows[0]?.["id"]).toBe("cache-1")
      expect(rows[0]?.["archived_at"]).not.toBeNull()
      expect(rows[1]?.["id"]).toBe("cache-2")
      expect(rows[1]?.["archived_at"]).toBeNull()
    })

    test("newest-N hashes: D1 export rows and Postgres twins hash identically", async () => {
      const d1Rows = [3, 2, 1].map(freeTierUsageEventRow)
      const d1Newest = d1EntitlementsNewestHashes(
        "inference_free_tier_usage_events",
        d1Rows.map((row, index) => ({ ...row, d1_rowid: index + 1 })),
      )
      const postgresNewest = await postgresEntitlementsNewestHashes(
        sql,
        "inference_free_tier_usage_events",
        10,
      )
      const postgresByKey = new Map(
        postgresNewest.map((entry) => [entry.key, entry.hash]),
      )
      for (const entry of d1Newest) {
        expect(postgresByKey.get(entry.key)).toBe(entry.hash)
      }
    })
  },
)
