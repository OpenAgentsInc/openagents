import { SQL } from "bun"
import { afterAll, beforeAll, describe, expect, setDefaultTimeout, test } from "bun:test"
import {
  cleanupLoadTestRows,
  defaultRunId,
  formatHumanSummary,
  loadClientGroupLikePattern,
  loadScopeLikePattern,
  MetricsRecorder,
  parseLoadTestArgs,
  percentile,
  runSubstrateLoad,
} from "./load-test.js"
import { runMigrations } from "./migrate.js"
import type { SyncSql } from "./sql.js"
import type { LocalPostgres } from "./test/local-postgres.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

// ---------------------------------------------------------------------------
// Config parsing (pure)
// ---------------------------------------------------------------------------

describe("parseLoadTestArgs", () => {
  test("substrate mode with defaults", () => {
    const config = parseLoadTestArgs([
      "--mode",
      "substrate",
      "--database-url",
      "postgres://u@h/db",
    ])
    if ("error" in config) throw new Error(config.error)
    expect(config.mode).toBe("substrate")
    expect(config.databaseUrl).toBe("postgres://u@h/db")
    expect(config.workers).toBe(40)
    expect(config.pushesPerSecond).toBe(2)
    expect(config.batchSize).toBe(2)
    expect(config.readers).toBe(10)
    expect(config.durationSec).toBe(300)
    expect(config.pool).toBe(16)
    expect(config.cleanup).toBe(true)
    expect(config.ssl).toBe("require")
  })

  test("--flag=value form and overrides", () => {
    const config = parseLoadTestArgs([
      "--mode=http",
      "--base-url=https://staging.example",
      "--workers=3",
      "--pushes-per-second=0.5",
      "--batch=4",
      "--readers=0",
      "--duration-sec=10",
      "--run-id=abc-123",
      "--no-cleanup",
      "--json-out=/tmp/x.json",
    ], { KHALA_LOAD_TOKEN: "tok" })
    if ("error" in config) throw new Error(config.error)
    expect(config.mode).toBe("http")
    expect(config.baseUrl).toBe("https://staging.example")
    expect(config.token).toBe("tok")
    expect(config.workers).toBe(3)
    expect(config.pushesPerSecond).toBe(0.5)
    expect(config.batchSize).toBe(4)
    expect(config.readers).toBe(0)
    expect(config.durationSec).toBe(10)
    expect(config.runId).toBe("abc-123")
    expect(config.cleanup).toBe(false)
    expect(config.jsonOut).toBe("/tmp/x.json")
  })

  test("env fallbacks fill database url and token", () => {
    const config = parseLoadTestArgs(["--mode", "substrate"], {
      KHALA_LOAD_DATABASE_URL: "postgres://env@h/db",
    })
    if ("error" in config) throw new Error(config.error)
    expect(config.databaseUrl).toBe("postgres://env@h/db")
  })

  test("missing mode is an error", () => {
    const config = parseLoadTestArgs([])
    expect("error" in config && config.error).toContain("--mode is required")
  })

  test("substrate mode without a database url is an error", () => {
    const config = parseLoadTestArgs(["--mode", "substrate"])
    expect("error" in config && config.error).toContain("--database-url")
  })

  test("http mode without a token is an error (token is env-only)", () => {
    const config = parseLoadTestArgs([
      "--mode",
      "http",
      "--base-url",
      "https://x",
    ])
    expect("error" in config && config.error).toContain("KHALA_LOAD_TOKEN")
  })

  test("invalid values are typed errors, not throws", () => {
    expect(
      parseLoadTestArgs(["--mode", "nope"]),
    ).toEqual({ error: "--mode must be substrate or http, got nope" })
    expect("error" in parseLoadTestArgs(["--mode=substrate", "--workers=0"])).toBe(
      true,
    )
    expect(
      "error" in
        parseLoadTestArgs(["--mode=substrate", "--run-id=bad/run"]),
    ).toBe(true)
    expect("error" in parseLoadTestArgs(["--unknown-flag=1"])).toBe(true)
    expect("error" in parseLoadTestArgs(["positional"])).toBe(true)
    expect("error" in parseLoadTestArgs(["--workers"])).toBe(true)
  })

  test("default run id is a safe namespace token", () => {
    expect(defaultRunId(new Date("2026-07-04T12:34:56.789Z"))).toMatch(
      /^[A-Za-z0-9-]+$/,
    )
  })

  test("cleanup patterns are anchored on the run id", () => {
    expect(loadScopeLikePattern("r-1")).toBe("scope.user.loadtest.r-1.%")
    expect(loadClientGroupLikePattern("r-1")).toBe("cg-loadtest-r-1-%")
  })
})

// ---------------------------------------------------------------------------
// Percentiles + metrics (pure)
// ---------------------------------------------------------------------------

describe("percentile", () => {
  test("nearest-rank on a known distribution", () => {
    const sorted = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    expect(percentile(sorted, 50)).toBe(5)
    expect(percentile(sorted, 95)).toBe(10)
    expect(percentile(sorted, 99)).toBe(10)
    expect(percentile(sorted, 100)).toBe(10)
    expect(percentile([], 50)).toBe(0)
    expect(percentile([42], 99)).toBe(42)
  })
})

describe("MetricsRecorder", () => {
  test("separates successes from the error taxonomy", () => {
    const metrics = new MetricsRecorder()
    metrics.record("push", 10, true)
    metrics.record("push", 20, true)
    metrics.record("push", 500, false, "storage_unavailable")
    metrics.record("push", 700, false, "storage_unavailable")
    metrics.record("log_read", 5, true)
    const push = metrics.summary("push", 10)
    expect(push.count).toBe(2)
    expect(push.errors).toBe(2)
    expect(push.p50Ms).toBe(10)
    expect(push.throughputPerSec).toBe(0.2)
    expect(metrics.taxonomy()).toEqual({ storage_unavailable: 2 })
    expect(metrics.opNames()).toEqual(["log_read", "push"])
  })
})

// ---------------------------------------------------------------------------
// Smoke: tiny substrate run against local Postgres
// ---------------------------------------------------------------------------

describe.skipIf(!hasLocalPostgres())("substrate smoke (local Postgres)", () => {
  let pg: LocalPostgres
  let sql: SQL

  beforeAll(async () => {
    pg = await startLocalPostgres()
    const admin = new SQL({ url: pg.url, max: 1 })
    await admin.unsafe("CREATE DATABASE khala_sync_loadtest")
    await admin.end()
    const url = pg.urlFor("khala_sync_loadtest")
    const result = await runMigrations({ databaseUrl: url })
    expect(result.applied).toContain("0001_khala_sync_core.sql")
    sql = new SQL({ url, max: 8 })
  })

  afterAll(async () => {
    // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
    if (sql !== undefined) await sql.end()
    // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
    if (pg !== undefined) await pg.stop()
  })

  test("2 writers x 2s push, readers observe, cleanup removes every row", async () => {
    const runId = "smoke-1"
    const report = await runSubstrateLoad(
      {
        baseUrl: undefined,
        batchSize: 2,
        cleanup: false,
        databaseUrl: undefined,
        durationSec: 2,
        jsonOut: undefined,
        mode: "substrate",
        pool: 8,
        pushesPerSecond: 4,
        readIntervalMs: 200,
        readers: 2,
        runId,
        ssl: "disable",
        token: undefined,
        workers: 2,
      },
      { sql: sql as unknown as SyncSql },
    )

    const push = report.ops["push"]
    if (push === undefined) throw new Error("no push metrics recorded")
    expect(push.count).toBeGreaterThan(0)
    expect(push.errors).toBe(0)
    expect(report.errorTaxonomy).toEqual({})
    const logRead = report.ops["log_read"]
    if (logRead === undefined) throw new Error("no log_read metrics recorded")
    expect(logRead.count).toBeGreaterThan(0)
    expect(logRead.errors).toBe(0)
    expect(report.ops["counter_read"]?.count ?? 0).toBeGreaterThan(0)
    // Readers saw the writers' entries and measured visibility lag.
    expect(report.deltaVisibility?.count ?? 0).toBeGreaterThan(0)
    // The human summary renders without throwing.
    expect(formatHumanSummary(report)).toContain("mode=substrate")

    // Rows exist before cleanup...
    const before: Array<{ n: number }> = await sql`
      SELECT count(*)::int AS n FROM khala_sync_changelog
       WHERE scope LIKE ${loadScopeLikePattern(runId)}
    `
    expect(Number(before[0]?.n ?? 0)).toBeGreaterThan(0)

    // ...and are all gone afterwards.
    const cleanup = await cleanupLoadTestRows(sql as unknown as SyncSql, runId)
    expect(cleanup.changelogRows).toBeGreaterThan(0)
    expect(cleanup.scopeRows).toBeGreaterThan(0)
    expect(cleanup.mutationRows).toBeGreaterThan(0)
    expect(cleanup.clientStateRows).toBeGreaterThan(0)

    for (const [table, column, pattern] of [
      ["khala_sync_changelog", "scope", loadScopeLikePattern(runId)],
      ["khala_sync_scopes", "scope", loadScopeLikePattern(runId)],
      ["khala_sync_mutations", "client_group_id", loadClientGroupLikePattern(runId)],
      ["khala_sync_client_state", "client_group_id", loadClientGroupLikePattern(runId)],
    ] as const) {
      const rows: Array<{ n: number }> = await sql.unsafe(
        `SELECT count(*)::int AS n FROM ${table} WHERE ${column} LIKE $1`,
        [pattern],
      )
      expect(Number(rows[0]?.n ?? 0)).toBe(0)
    }
  })
})
