#!/usr/bin/env bun
/**
 * KS-8.9 (#8320): inference entitlements backfill CLI — D1 → Postgres.
 *
 * Reads D1 rows through `wrangler d1 execute <db> --remote --json` (the
 * KS-8.1/KS-8.2 pattern: reuses the repo's existing wrangler auth — no new
 * admin-API surface) in bounded rowid-keyset pages, and copies them into
 * the Postgres twins from khala-sync migration 0013 over a DIRECT
 * connection (never Hyperdrive). The table registry in
 * `src/inference-entitlements-backfill.ts` drives all 29 tables: event
 * tables land `ON CONFLICT DO NOTHING`, state tables CONVERGE to the D1
 * snapshot.
 *
 * Resumable: progress (last rowid per table) persists in a local state
 * file; delete it (or pass --restart) to sweep from the beginning again.
 * The runbook's sequence runs the backfill TWICE — the second sweep is
 * the catch-up pass after dual-write has been on across the whole window
 * (it also closes the today's-row converge race on the counters).
 *
 * Verify mode (`--verify`): exact row counts per table, per-group
 * ("per-plan") tallies on the registry's bounded group column, newest-N
 * row-hash comparison, and the §3.6 enforcement invariant —
 * tally = SUM(events) PER KEY for the free-tier / free-usage /
 * earned-allowance families on the Postgres side. Exits non-zero on ANY
 * mismatch.
 *
 * Usage (from packages/khala-sync-server/):
 *   bun scripts/backfill-inference-entitlements.ts \
 *     [--database-url <postgres-url>]   # default $KHALA_SYNC_DATABASE_URL
 *     [--d1-database <name>]            # default openagents-autopilot
 *     [--wrangler-cwd <dir>]            # default ../../apps/openagents.com/workers/api
 *     [--batch-size <n>]                # default 200 (29-col max tables)
 *     [--state-file <path>]             # default .inference-entitlements-backfill-state.json
 *     [--table <name>]                  # limit to one table
 *     [--restart]                       # ignore saved cursor
 *     [--local]                         # wrangler --local (dev smoke)
 *     [--verify] [--verify-newest <n>]  # verify mode (default N=50)
 */
import { spawnSync } from "node:child_process"
import { existsSync, readFileSync, writeFileSync } from "node:fs"
import * as path from "node:path"
import { SQL } from "bun"
import {
  compareEntitlementsTallies,
  d1EntitlementsNewestHashes,
  entitlementsTallyFromRows,
  INFERENCE_ENTITLEMENTS_TABLE_NAMES,
  INFERENCE_ENTITLEMENTS_TABLES,
  postgresEntitlementsNewestHashes,
  postgresEntitlementsTableTally,
  postgresEntitlementsTallyInvariants,
  upsertEntitlementsRows,
  type D1SourceRow,
  type EntitlementsVerifyReport,
} from "../src/inference-entitlements-backfill.js"
import type { SyncSql } from "../src/sql.js"

const USAGE = `Usage: bun scripts/backfill-inference-entitlements.ts [options]   (see file header)`

type Options = {
  batchSize: number
  d1Database: string
  databaseUrl: string | undefined
  local: boolean
  restart: boolean
  stateFile: string
  table: string | undefined
  verify: boolean
  verifyNewest: number
  wranglerCwd: string
}

const parseArgs = (argv: ReadonlyArray<string>): Options | undefined => {
  const options: Options = {
    batchSize: 200,
    d1Database: "openagents-autopilot",
    databaseUrl: process.env["KHALA_SYNC_DATABASE_URL"],
    local: false,
    restart: false,
    stateFile: ".inference-entitlements-backfill-state.json",
    table: undefined,
    verify: false,
    verifyNewest: 50,
    wranglerCwd: path.resolve(
      import.meta.dir,
      "../../../apps/openagents.com/workers/api",
    ),
  }
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i]
    const next = () => {
      const value = argv[++i]
      if (value === undefined) throw new Error(`${arg} requires a value`)
      return value
    }
    if (arg === "--database-url") options.databaseUrl = next()
    else if (arg === "--d1-database") options.d1Database = next()
    else if (arg === "--wrangler-cwd") options.wranglerCwd = next()
    else if (arg === "--batch-size") options.batchSize = Number(next())
    else if (arg === "--state-file") options.stateFile = next()
    else if (arg === "--table") {
      const table = next()
      if (INFERENCE_ENTITLEMENTS_TABLES[table] === undefined) {
        console.error(`error: unknown table ${table}`)
        return undefined
      }
      options.table = table
    } else if (arg === "--restart") options.restart = true
    else if (arg === "--local") options.local = true
    else if (arg === "--verify") options.verify = true
    else if (arg === "--verify-newest") options.verifyNewest = Number(next())
    else if (arg === "--help" || arg === "-h") {
      console.log(USAGE)
      return undefined
    } else {
      console.error(`error: unknown argument ${JSON.stringify(arg)}`)
      return undefined
    }
  }
  if (
    !Number.isSafeInteger(options.batchSize) ||
    options.batchSize < 1 ||
    options.batchSize > 1500
  ) {
    // 1500 × 29 columns = 43,500 bind params — under the 65,535 cap.
    console.error("error: --batch-size must be 1..1500")
    return undefined
  }
  return options
}

// ---------------------------------------------------------------------------
// D1 access via wrangler
// ---------------------------------------------------------------------------

const d1Query = (
  options: Options,
  command: string,
): Array<Record<string, unknown>> => {
  const args = [
    "wrangler",
    "d1",
    "execute",
    options.d1Database,
    options.local ? "--local" : "--remote",
    "--json",
    "--command",
    command,
  ]
  const result = spawnSync("bunx", args, {
    cwd: options.wranglerCwd,
    encoding: "utf8",
    maxBuffer: 256 * 1024 * 1024,
  })
  if (result.status !== 0) {
    throw new Error(
      `wrangler d1 execute failed (${result.status}): ${result.stderr.slice(0, 500)}`,
    )
  }
  const parsed = JSON.parse(result.stdout) as Array<{
    results: Array<Record<string, unknown>>
  }>
  return parsed[0]?.results ?? []
}

// ---------------------------------------------------------------------------
// Cursor state
// ---------------------------------------------------------------------------

type CursorState = Partial<Record<string, number>>

const loadState = (options: Options): CursorState =>
  options.restart || !existsSync(options.stateFile)
    ? {}
    : (JSON.parse(readFileSync(options.stateFile, "utf8")) as CursorState)

const saveState = (options: Options, state: CursorState): void => {
  writeFileSync(options.stateFile, `${JSON.stringify(state, null, 2)}\n`)
}

// ---------------------------------------------------------------------------
// Backfill
// ---------------------------------------------------------------------------

const backfillTable = async (
  sql: SyncSql,
  options: Options,
  state: CursorState,
  table: string,
): Promise<void> => {
  const totalRow = d1Query(
    options,
    `SELECT COUNT(*) AS total_rows FROM ${table}`,
  )[0]
  const totalRows = Number(totalRow?.["total_rows"] ?? 0)

  let cursor = state[table] ?? 0
  let scanned = 0
  let touched = 0
  const startedAtMs = Date.now()
  for (;;) {
    const rows = d1Query(
      options,
      `SELECT rowid AS d1_rowid, * FROM ${table} WHERE rowid > ${cursor} ORDER BY rowid ASC LIMIT ${options.batchSize}`,
    ) as Array<D1SourceRow & { d1_rowid: number }>
    if (rows.length === 0) break
    touched += await upsertEntitlementsRows(sql, table, rows)
    scanned += rows.length
    cursor = Number(rows[rows.length - 1]?.d1_rowid ?? cursor)
    state[table] = cursor
    saveState(options, state)
    const elapsedS = Math.max(1, (Date.now() - startedAtMs) / 1000)
    const rate = Math.round(scanned / elapsedS)
    console.log(
      `${table}: page done (cursor rowid=${cursor}, scanned=${scanned}${totalRows > 0 ? `/${totalRows}` : ""}, fresh/converged=${touched}, ~${rate} rows/s)`,
    )
  }
  console.log(
    `${table}: complete — scanned ${scanned} row(s) this run, ${touched} fresh/converged`,
  )
}

// ---------------------------------------------------------------------------
// Verify
// ---------------------------------------------------------------------------

const printReport = (
  report: EntitlementsVerifyReport,
  newest: number,
): boolean => {
  console.log(`\n== ${report.table} ==`)
  console.log(
    `  rows: d1=${report.d1Total} postgres=${report.postgresTotal}${report.countsMatch ? "" : "  ROW-COUNT MISMATCH"}`,
  )
  for (const mismatch of report.groupMismatches) {
    console.log(
      `  GROUP MISMATCH ${mismatch.group}: d1=${mismatch.d1Rows} postgres=${mismatch.postgresRows}`,
    )
  }
  console.log(
    `  newest-${newest} row hashes: ${
      report.newestHashMismatches.length === 0
        ? "all match"
        : `${report.newestHashMismatches.length} MISMATCH(ES)`
    }`,
  )
  for (const mismatch of report.newestHashMismatches) {
    console.log(
      `    ${mismatch.key}: d1=${mismatch.d1Hash?.slice(0, 16) ?? "<missing>"} postgres=${mismatch.postgresHash?.slice(0, 16) ?? "<missing>"}`,
    )
  }
  return (
    report.countsMatch &&
    report.groupMismatches.length === 0 &&
    report.newestHashMismatches.length === 0
  )
}

const verifyTable = async (
  sql: SyncSql,
  options: Options,
  table: string,
): Promise<boolean> => {
  const spec = INFERENCE_ENTITLEMENTS_TABLES[table]
  if (spec === undefined) throw new Error(`unknown table ${table}`)

  const d1NewestRows = d1Query(
    options,
    `SELECT * FROM ${table} ORDER BY ${spec.orderColumn} DESC, ${spec.keyColumns.join(" DESC, ")} DESC LIMIT ${options.verifyNewest}`,
  )
  const d1Newest = d1EntitlementsNewestHashes(table, d1NewestRows)
  const postgresNewest = await postgresEntitlementsNewestHashes(
    sql,
    table,
    options.verifyNewest,
  )

  const d1Totals = d1Query(
    options,
    `SELECT COUNT(*) AS total_rows FROM ${table}`,
  )[0]
  const d1Groups =
    spec.groupColumn === undefined
      ? []
      : d1Query(
          options,
          `SELECT COALESCE(CAST(${spec.groupColumn} AS TEXT), '<null>') AS group_key,
                  COUNT(*) AS row_count
             FROM ${table}
            GROUP BY COALESCE(CAST(${spec.groupColumn} AS TEXT), '<null>')
            ORDER BY group_key`,
        )
  const report = compareEntitlementsTallies(
    table,
    entitlementsTallyFromRows(d1Totals, d1Groups),
    await postgresEntitlementsTableTally(sql, table),
    d1Newest,
    postgresNewest,
  )
  return printReport(report, options.verifyNewest)
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

const main = async (): Promise<number> => {
  const options = parseArgs(process.argv.slice(2))
  if (options === undefined) return 2
  if (options.databaseUrl === undefined || options.databaseUrl === "") {
    console.error(
      "error: no Postgres URL — pass --database-url or set KHALA_SYNC_DATABASE_URL",
    )
    return 2
  }

  const sql = new SQL(options.databaseUrl) as unknown as SyncSql
  const tables =
    options.table === undefined
      ? INFERENCE_ENTITLEMENTS_TABLE_NAMES
      : [options.table]
  try {
    if (options.verify) {
      let allGood = true
      for (const table of tables) {
        allGood = (await verifyTable(sql, options, table)) && allGood
      }
      // The §3.6 enforcement invariant: tally = SUM(events) per key.
      console.log(`\n== tally = SUM(events) enforcement invariants ==`)
      for (const invariant of await postgresEntitlementsTallyInvariants(
        sql,
      )) {
        if (invariant.mismatches.length === 0) {
          console.log(`  ${invariant.family}: OK (exact per-key equality)`)
          continue
        }
        allGood = false
        console.log(
          `  ${invariant.family}: ${invariant.mismatches.length} PER-KEY MISMATCH(ES)`,
        )
        for (const mismatch of invariant.mismatches.slice(0, 20)) {
          console.log(
            `    ${mismatch.key}: tally count=${mismatch.tallyCount}/amount=${mismatch.tallyAmount} events count=${mismatch.eventsCount}/amount=${mismatch.eventsAmount}`,
          )
        }
      }
      console.log(
        allGood
          ? "\nVERIFY OK: exact counts, per-group tallies, newest-N hashes, and tally=SUM(events) invariants match."
          : "\nVERIFY FAILED: mismatches above — investigate before any read cutover (this store ENFORCES allow/deny after cutover).",
      )
      return allGood ? 0 : 1
    }

    const state = loadState(options)
    for (const table of tables) {
      await backfillTable(sql, options, state, table)
    }
    console.log(
      "\nBackfill sweep complete. Run again for the catch-up pass, then --verify.",
    )
    return 0
  } finally {
    await (sql as unknown as { end: () => Promise<void> }).end()
  }
}

process.exit(await main())
