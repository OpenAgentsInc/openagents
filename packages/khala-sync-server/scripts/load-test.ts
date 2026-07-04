#!/usr/bin/env bun
/**
 * Khala Sync fleet-burst load harness CLI (KS-9.1, #8310).
 *
 * Simulates the June 28-29 failure shape (N concurrent writers x paced
 * mutator pushes + M concurrent readers + public-counter-style reads)
 * against either the substrate directly (a Postgres URL — local throwaway
 * or staging Cloud SQL over TLS) or the deployed Worker routes.
 *
 * Usage:
 *   bun scripts/load-test.ts --mode substrate --database-url <url> \
 *     [--workers 40] [--pushes-per-second 2] [--batch 2] [--readers 10] \
 *     [--read-interval-ms 1000] [--duration-sec 300] [--pool 16] \
 *     [--ssl require|verify|disable] [--run-id <id>] [--no-cleanup] \
 *     [--json-out <file>]
 *
 *   KHALA_LOAD_TOKEN=<agent bearer> bun scripts/load-test.ts --mode http \
 *     --base-url https://openagents-staging.openagents.workers.dev \
 *     [--database-url <url>]   # optional: enables post-run cleanup
 *
 * Secrets are env-only (`KHALA_LOAD_DATABASE_URL`, `KHALA_LOAD_TOKEN`) or
 * flag-provided URLs; the tool never prints credentials. Every synthetic
 * row is namespaced by --run-id and deleted afterwards unless
 * --no-cleanup is passed (plain DELETEs keyed on the runId prefixes; see
 * `cleanupLoadTestRows`).
 *
 * SAFETY: substrate mode checks the server's max_connections and refuses
 * to open a pool larger than 25% of it — staging and prod share the Cloud
 * SQL instance, so a load test must never exhaust backend slots.
 */
import { writeFile } from "node:fs/promises"
import postgres from "postgres"
import type { LoadTestReport } from "../src/load-test.js"
import {
  cleanupLoadTestRows,
  formatHumanSummary,
  parseLoadTestArgs,
  runHttpLoad,
  runSubstrateLoad,
} from "../src/load-test.js"
import type { SyncSql } from "../src/sql.js"

const USAGE = `Usage:
  bun scripts/load-test.ts --mode substrate --database-url <url> [options]
  KHALA_LOAD_TOKEN=<bearer> bun scripts/load-test.ts --mode http --base-url <origin> [options]

Options:
  --mode substrate|http     Direct engine against Postgres, or deployed Worker routes.
  --database-url <url>      Direct Postgres URL (substrate mode; enables cleanup).
                            Env: KHALA_LOAD_DATABASE_URL.
  --base-url <origin>       Worker origin (http mode). Env: KHALA_LOAD_BASE_URL.
  --ssl require|verify|disable
                            TLS posture for the direct connection (default require:
                            encrypted, cert NOT verified — Cloud SQL server certs are
                            not in the default trust store).
  --workers <n>             Concurrent writer client groups (default 40).
  --pushes-per-second <x>   Target pushes/sec per worker, paced closed-loop (default 2).
  --batch <n>               Mutations per push (default 2).
  --readers <n>             Concurrent readers (default 10).
  --read-interval-ms <ms>   Reader poll interval (default 1000).
  --duration-sec <s>        Run duration (default 300).
  --pool <n>                Max pooled connections, substrate mode (default 16).
  --run-id <id>             Row namespace [A-Za-z0-9-]+ (default: timestamp).
  --no-cleanup              Keep synthetic rows after the run.
  --json-out <file>         Also write the JSON report to this file.
  --help                    Show this help.

The agent bearer token is env-only: KHALA_LOAD_TOKEN. Never pass it as a flag.
`

const makeDirectSql = async (
  databaseUrl: string,
  pool: number,
  ssl: "require" | "verify" | "disable",
): Promise<{ sql: SyncSql; end: () => Promise<void> }> => {
  // Same driver as the Worker path (postgres.js), pooled. `prepare: false`
  // mirrors the Hyperdrive transaction-mode discipline so measured
  // statement shapes match production.
  const client = postgres(databaseUrl, {
    connect_timeout: 15,
    max: pool,
    prepare: false,
    ...(ssl === "disable"
      ? {}
      : ssl === "verify"
        ? { ssl: true }
        : { ssl: { rejectUnauthorized: false } }),
  })
  return {
    end: () => client.end({ timeout: 10 }),
    sql: client as unknown as SyncSql,
  }
}

/** Refuse pools that could eat the shared instance's backend slots. */
const assertPoolIsSafe = async (sql: SyncSql, pool: number): Promise<void> => {
  const rows: Array<{ max_connections: string }> =
    await sql`SELECT current_setting('max_connections') AS max_connections`
  const maxConnections = Number(rows[0]?.max_connections ?? 0)
  if (maxConnections > 0 && pool > Math.floor(maxConnections / 4)) {
    throw new Error(
      `--pool ${pool} exceeds 25% of the server's max_connections ` +
        `(${maxConnections}); staging and prod share this instance — ` +
        `use a smaller pool`,
    )
  }
}

const main = async (argv: ReadonlyArray<string>): Promise<number> => {
  if (argv.includes("--help") || argv.includes("-h")) {
    console.log(USAGE)
    return 0
  }
  const config = parseLoadTestArgs(argv, process.env)
  if ("error" in config) {
    console.error(`error: ${config.error}\n`)
    console.error(USAGE)
    return 2
  }

  console.log(
    `khala-sync load test starting: mode=${config.mode} runId=${config.runId} ` +
      `workers=${config.workers} readers=${config.readers} ` +
      `batch=${config.batchSize} target=${config.pushesPerSecond} push/s/worker ` +
      `duration=${config.durationSec}s`,
  )

  let report: LoadTestReport
  let direct: { sql: SyncSql; end: () => Promise<void> } | undefined
  try {
    if (config.mode === "substrate") {
      direct = await makeDirectSql(
        config.databaseUrl as string,
        config.pool,
        config.ssl,
      )
      await assertPoolIsSafe(direct.sql, config.pool)
      report = await runSubstrateLoad(config, { sql: direct.sql })
    } else {
      report = await runHttpLoad(config)
      if (config.cleanup && config.databaseUrl !== undefined) {
        direct = await makeDirectSql(config.databaseUrl, 2, config.ssl)
      }
    }

    if (config.cleanup && direct !== undefined) {
      const cleanup = await cleanupLoadTestRows(direct.sql, config.runId)
      report = { ...report, cleanup }
    } else if (config.cleanup) {
      report = {
        ...report,
        notes: [
          ...report.notes,
          "cleanup skipped: no --database-url available in http mode",
        ],
      }
    }
  } finally {
    if (direct !== undefined) {
      try {
        await direct.end()
      } catch {
        // best-effort teardown
      }
    }
  }

  console.log(formatHumanSummary(report))
  if (config.jsonOut !== undefined) {
    await writeFile(config.jsonOut, `${JSON.stringify(report, null, 2)}\n`)
    console.log(`json report written to ${config.jsonOut}`)
  }

  const pushErrors = report.ops["push"]?.errors ?? 0
  return pushErrors > 0 ? 1 : 0
}

process.exitCode = await main(process.argv.slice(2))
