#!/usr/bin/env bun
/**
 * KS-8.2 (#8308): token ledger backfill CLI — D1 → Postgres.
 *
 * Reads D1 rows through `wrangler d1 execute <db> --remote --json` (the
 * KS-8.1 pattern: reuses the repo's existing wrangler auth — no new
 * admin-API surface) in bounded rowid-keyset pages, and copies them into
 * the Postgres twins from khala-sync migration 0008 over a DIRECT
 * connection (never Hyperdrive):
 *
 *   - `token_usage_events`: `ON CONFLICT DO NOTHING`, one multi-row
 *     INSERT per page. This is the big table (hundreds of thousands of
 *     rows / billions of tokens): pages default to 400 rows (bounded by
 *     wrangler's JSON output and Postgres's 65535-parameter statement
 *     cap at 35 columns) with per-page progress output.
 *   - `public_khala_tokens_served_*` rollups + leaderboard preferences:
 *     CONVERGE upserts to the D1 snapshot value.
 *
 * Resumable: progress (last rowid per table) persists in a local state
 * file; delete it (or pass --restart) to sweep from the beginning again.
 * The runbook's sequence runs the backfill TWICE — the second sweep is
 * the catch-up pass after dual-write has been on across the whole window
 * (it also closes the today-row rollup converge race).
 *
 * Verify mode (`--verify`): exact row counts, exact SUM(total_tokens),
 * the exact public tokens-served SUM, per-provider row/token tallies,
 * rollup token/event sums, and newest-N row-hash comparison. Exits
 * non-zero on ANY mismatch.
 *
 * Usage (from packages/khala-sync-server/):
 *   bun scripts/backfill-token-ledger.ts \
 *     [--database-url <postgres-url>]   # default $KHALA_SYNC_DATABASE_URL
 *     [--d1-database <name>]            # default openagents-autopilot
 *     [--wrangler-cwd <dir>]            # default ../../apps/openagents.com/workers/api
 *     [--batch-size <n>]                # default 400
 *     [--state-file <path>]             # default .token-ledger-backfill-state.json
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
  compareTokenLedgerEventsTallies,
  compareTokenLedgerRollupTallies,
  compareNewestHashes,
  d1TokenLedgerNewestHashes,
  postgresTokenLedgerEventsTally,
  postgresTokenLedgerNewestHashes,
  postgresTokenLedgerRollupTally,
  postgresTokenLedgerRowCount,
  PUBLIC_TOKENS_SERVED_SQL,
  TOKEN_LEDGER_TABLE_KEY,
  TOKEN_LEDGER_TABLE_ORDER,
  TOKEN_LEDGER_TABLES,
  tallyFromRows,
  upsertTokenLedgerRows,
  type D1SourceRow,
  type TokenLedgerBackfillTable,
  type TokenLedgerVerifyReport,
} from "../src/token-ledger-backfill.js"
import type { SyncSql } from "../src/sql.js"

const USAGE = `Usage: bun scripts/backfill-token-ledger.ts [options]   (see file header)`

type Options = {
  batchSize: number
  d1Database: string
  databaseUrl: string | undefined
  local: boolean
  restart: boolean
  stateFile: string
  table: TokenLedgerBackfillTable | undefined
  verify: boolean
  verifyNewest: number
  wranglerCwd: string
}

const parseArgs = (argv: ReadonlyArray<string>): Options | undefined => {
  const options: Options = {
    batchSize: 400,
    d1Database: "openagents-autopilot",
    databaseUrl: process.env["KHALA_SYNC_DATABASE_URL"],
    local: false,
    restart: false,
    stateFile: ".token-ledger-backfill-state.json",
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
      if (!TOKEN_LEDGER_TABLES.includes(table as TokenLedgerBackfillTable)) {
        console.error(`error: unknown table ${table}`)
        return undefined
      }
      options.table = table as TokenLedgerBackfillTable
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
    // 1500 × 35 columns = 52,500 bind params — under the 65,535 cap.
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

type CursorState = Partial<Record<TokenLedgerBackfillTable, number>>

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
  table: TokenLedgerBackfillTable,
): Promise<void> => {
  // Total once up front so the big events sweep reports honest progress.
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
    touched += await upsertTokenLedgerRows(sql, table, rows)
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
  label: string,
  report: TokenLedgerVerifyReport,
  newest: number,
): boolean => {
  console.log(`\n== ${label} ==`)
  console.log(
    `  rows: d1=${report.d1Total} postgres=${report.postgresTotal}${report.countsMatch ? "" : "  ROW-COUNT MISMATCH"}`,
  )
  for (const mismatch of report.scalarMismatches) {
    console.log(
      `  SCALAR MISMATCH ${mismatch.metric}: d1=${mismatch.d1} postgres=${mismatch.postgres}`,
    )
  }
  for (const mismatch of report.providerMismatches) {
    console.log(
      `  PROVIDER MISMATCH ${mismatch.provider}: rows d1=${mismatch.d1Rows}/pg=${mismatch.postgresRows} tokens d1=${mismatch.d1TotalTokens}/pg=${mismatch.postgresTotalTokens}`,
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
    report.scalarMismatches.length === 0 &&
    report.providerMismatches.length === 0 &&
    report.newestHashMismatches.length === 0
  )
}

const d1NewestRows = (
  options: Options,
  table: TokenLedgerBackfillTable,
): Array<Record<string, unknown>> => {
  const orderColumn = TOKEN_LEDGER_TABLE_ORDER[table]
  const keyColumns = TOKEN_LEDGER_TABLE_KEY[table]
  return d1Query(
    options,
    `SELECT * FROM ${table} ORDER BY ${orderColumn} DESC, ${keyColumns.join(" DESC, ")} DESC LIMIT ${options.verifyNewest}`,
  )
}

const verifyTable = async (
  sql: SyncSql,
  options: Options,
  table: TokenLedgerBackfillTable,
): Promise<boolean> => {
  const d1Newest = d1TokenLedgerNewestHashes(table, d1NewestRows(options, table))
  const postgresNewest = await postgresTokenLedgerNewestHashes(
    sql,
    table,
    options.verifyNewest,
  )

  if (table === "token_usage_events") {
    const d1Totals = d1Query(
      options,
      `SELECT COUNT(*) AS total_rows,
              COALESCE(SUM(total_tokens), 0) AS sum_total_tokens,
              COALESCE(SUM(${PUBLIC_TOKENS_SERVED_SQL}), 0) AS sum_public
         FROM token_usage_events`,
    )[0]
    const d1Providers = d1Query(
      options,
      `SELECT COALESCE(provider, '<null>') AS provider_key,
              COUNT(*) AS row_count,
              COALESCE(SUM(total_tokens), 0) AS total_tokens
         FROM token_usage_events
        GROUP BY COALESCE(provider, '<null>')
        ORDER BY provider_key`,
    )
    const report = compareTokenLedgerEventsTallies(
      tallyFromRows(d1Totals, d1Providers),
      await postgresTokenLedgerEventsTally(sql),
      d1Newest,
      postgresNewest,
    )
    return printReport(
      "token_usage_events (exact counts + token sums + per-provider)",
      report,
      options.verifyNewest,
    )
  }

  if (table === "token_usage_leaderboard_preferences") {
    const d1Count = Number(
      d1Query(options, `SELECT COUNT(*) AS total_rows FROM ${table}`)[0]?.[
        "total_rows"
      ] ?? 0,
    )
    const postgresCount = await postgresTokenLedgerRowCount(sql, table)
    const report: TokenLedgerVerifyReport = {
      countsMatch: d1Count === postgresCount,
      d1Total: d1Count,
      newestHashMismatches: compareNewestHashes(d1Newest, postgresNewest),
      postgresTotal: postgresCount,
      providerMismatches: [],
      scalarMismatches: [],
      table,
    }
    return printReport(table, report, options.verifyNewest)
  }

  const d1Rollup = d1Query(
    options,
    `SELECT COUNT(*) AS total_rows,
            COALESCE(SUM(tokens_served), 0) AS sum_tokens_served,
            COALESCE(SUM(usage_events), 0) AS sum_usage_events
       FROM ${table}`,
  )[0]
  const report = compareTokenLedgerRollupTallies(
    table,
    {
      sumTokensServed: Number(d1Rollup?.["sum_tokens_served"] ?? 0),
      sumUsageEvents: Number(d1Rollup?.["sum_usage_events"] ?? 0),
      totalRows: Number(d1Rollup?.["total_rows"] ?? 0),
    },
    await postgresTokenLedgerRollupTally(sql, table),
    d1Newest,
    postgresNewest,
  )
  return printReport(table, report, options.verifyNewest)
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
    options.table === undefined ? TOKEN_LEDGER_TABLES : [options.table]
  try {
    if (options.verify) {
      let allGood = true
      for (const table of tables) {
        allGood = (await verifyTable(sql, options, table)) && allGood
      }
      console.log(
        allGood
          ? "\nVERIFY OK: exact counts, token sums, provider tallies, and newest-N hashes match."
          : "\nVERIFY FAILED: mismatches above — investigate before any read cutover.",
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
