#!/usr/bin/env bun
/**
 * KS-8.1 (#8307): pylon assignments/dispatch backfill CLI — D1 → Postgres.
 *
 * Reads D1 rows through `wrangler d1 execute <db> --remote --json` (the
 * workable path: it needs only the repo's existing wrangler auth — no new
 * admin-API token surface) in bounded rowid-keyset pages, and upserts them
 * into the Postgres twins (`pylon_registrations`, `pylon_assignments`,
 * `pylon_assignment_events`) over a DIRECT connection (never Hyperdrive).
 * `ON CONFLICT ... DO NOTHING`, so:
 *   - re-running is idempotent (safe to interrupt + resume);
 *   - it never fights the live dual-write mirror (mirror rows win).
 *
 * Resumable: progress (last rowid per table) persists in a local state
 * file; delete it (or pass --restart) to sweep from the beginning again.
 * The runbook's sequence runs the backfill TWICE — the second sweep is the
 * catch-up pass after dual-write has been on across the whole window.
 *
 * Verify mode (`--verify`): exact per-table row counts, per-status/state
 * tallies, and newest-N row-hash comparison (2026-06-29 after-action
 * reconciliation culture). Exits non-zero on ANY mismatch.
 *
 * Usage (from packages/khala-sync-server/):
 *   bun scripts/backfill-pylon.ts \
 *     [--database-url <postgres-url>]   # default $KHALA_SYNC_DATABASE_URL
 *     [--d1-database <name>]            # default openagents-autopilot
 *     [--wrangler-cwd <dir>]            # default ../../apps/openagents.com/workers/api
 *     [--batch-size <n>]                # default 500
 *     [--state-file <path>]             # default .pylon-backfill-state.json
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
  comparePylonTallies,
  D1_SOURCE_TABLES,
  d1NewestRowHashes,
  postgresNewestRowHashes,
  postgresPylonTally,
  TABLE_STATUS_COLUMN,
  upsertPylonRows,
  type D1SourceRow,
  type PylonBackfillTable,
  type PylonVerifyTally,
} from "../src/pylon-backfill.js"
import type { SyncSql } from "../src/sql.js"

const TABLES: ReadonlyArray<PylonBackfillTable> = [
  // Registrations first: assignments/events reference pylon_ref (integrity
  // is verified by set-membership, not FKs, but ordered fill keeps the
  // partial states sensible mid-run).
  "pylon_registrations",
  "pylon_assignments",
  "pylon_assignment_events",
]

const USAGE = `Usage: bun scripts/backfill-pylon.ts [options]   (see file header)`

type Options = {
  batchSize: number
  d1Database: string
  databaseUrl: string | undefined
  local: boolean
  restart: boolean
  stateFile: string
  table: PylonBackfillTable | undefined
  verify: boolean
  verifyNewest: number
  wranglerCwd: string
}

const parseArgs = (argv: ReadonlyArray<string>): Options | undefined => {
  const options: Options = {
    batchSize: 500,
    d1Database: "openagents-autopilot",
    databaseUrl: process.env["KHALA_SYNC_DATABASE_URL"],
    local: false,
    restart: false,
    stateFile: ".pylon-backfill-state.json",
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
      if (!TABLES.includes(table as PylonBackfillTable)) {
        console.error(`error: unknown table ${table}`)
        return undefined
      }
      options.table = table as PylonBackfillTable
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
    options.batchSize > 5000
  ) {
    console.error("error: --batch-size must be 1..5000")
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
  // `--json` output: [{ results: [...], success: true, meta: {...} }]
  const parsed = JSON.parse(result.stdout) as Array<{
    results: Array<Record<string, unknown>>
  }>
  return parsed[0]?.results ?? []
}

// ---------------------------------------------------------------------------
// Cursor state
// ---------------------------------------------------------------------------

type CursorState = Partial<Record<PylonBackfillTable, number>>

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
  table: PylonBackfillTable,
): Promise<void> => {
  const source = D1_SOURCE_TABLES[table]
  let cursor = state[table] ?? 0
  let copied = 0
  let inserted = 0
  for (;;) {
    const rows = d1Query(
      options,
      `SELECT rowid AS d1_rowid, * FROM ${source} WHERE rowid > ${cursor} ORDER BY rowid ASC LIMIT ${options.batchSize}`,
    ) as Array<D1SourceRow & { d1_rowid: number }>
    if (rows.length === 0) break
    inserted += await upsertPylonRows(sql, table, rows)
    copied += rows.length
    cursor = Number(rows[rows.length - 1]?.d1_rowid ?? cursor)
    state[table] = cursor
    saveState(options, state)
    console.log(
      `${table}: page done (cursor rowid=${cursor}, scanned=${copied}, newly inserted=${inserted})`,
    )
  }
  console.log(
    `${table}: complete — scanned ${copied} row(s) this run, ${inserted} newly inserted`,
  )
}

// ---------------------------------------------------------------------------
// Verify
// ---------------------------------------------------------------------------

const d1Tally = (
  options: Options,
  table: PylonBackfillTable,
): PylonVerifyTally => {
  const source = D1_SOURCE_TABLES[table]
  const statusColumn = TABLE_STATUS_COLUMN[table]
  const rows = d1Query(
    options,
    `SELECT ${statusColumn} AS status_value, COUNT(*) AS row_count FROM ${source} GROUP BY ${statusColumn} ORDER BY ${statusColumn}`,
  ) as Array<{ status_value: string | null; row_count: number }>
  const byStatus: Record<string, number> = {}
  let total = 0
  for (const row of rows) {
    const count = Number(row.row_count)
    byStatus[row.status_value ?? "<null>"] = count
    total += count
  }
  return { byStatus, total }
}

const verifyTable = async (
  sql: SyncSql,
  options: Options,
  table: PylonBackfillTable,
): Promise<boolean> => {
  const source = D1_SOURCE_TABLES[table]
  const orderColumn =
    table === "pylon_assignment_events" ? "created_at" : "updated_at"
  const keyColumn =
    table === "pylon_assignment_events"
      ? "event_ref"
      : table === "pylon_assignments"
        ? "assignment_ref"
        : "pylon_ref"
  const d1 = d1Tally(options, table)
  const postgres = await postgresPylonTally(sql, table)
  const d1Newest = d1NewestRowHashes(
    table,
    d1Query(
      options,
      `SELECT * FROM ${source} ORDER BY ${orderColumn} DESC, ${keyColumn} DESC LIMIT ${options.verifyNewest}`,
    ),
  )
  const postgresNewest = await postgresNewestRowHashes(
    sql,
    table,
    options.verifyNewest,
  )
  const report = comparePylonTallies(table, d1, postgres, d1Newest, postgresNewest)

  console.log(`\n== ${table} (D1 source: ${source}) ==`)
  console.log(`  rows: d1=${report.d1Total} postgres=${report.postgresTotal}`)
  console.log(`  d1 tallies:       ${JSON.stringify(d1.byStatus)}`)
  console.log(`  postgres tallies: ${JSON.stringify(postgres.byStatus)}`)
  if (report.statusMismatches.length > 0) {
    console.log(`  STATUS MISMATCHES: ${JSON.stringify(report.statusMismatches)}`)
  }
  console.log(
    `  newest-${options.verifyNewest} row hashes: ${
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
    report.statusMismatches.length === 0 &&
    report.newestHashMismatches.length === 0
  )
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
  const tables = options.table === undefined ? TABLES : [options.table]
  try {
    if (options.verify) {
      let allGood = true
      for (const table of tables) {
        allGood = (await verifyTable(sql, options, table)) && allGood
      }
      console.log(
        allGood
          ? "\nVERIFY OK: exact counts, tallies, and newest-N hashes match."
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
