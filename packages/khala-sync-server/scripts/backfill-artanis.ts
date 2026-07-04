#!/usr/bin/env bun
/**
 * KS-8.6 (#8317): Artanis supervision domain backfill CLI — D1 → Postgres.
 *
 * Reads D1 rows through `wrangler d1 execute <db> --remote --json` (the
 * workable path: it needs only the repo's existing wrangler auth — no new
 * admin-API token surface) in bounded rowid-keyset pages, and upserts them
 * into the Postgres twins (migration 0011_artanis_domain.sql; identical
 * table names) over a DIRECT connection (never Hyperdrive).
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
 *   bun scripts/backfill-artanis.ts \
 *     [--database-url <postgres-url>]   # default $KHALA_SYNC_DATABASE_URL
 *     [--d1-database <name>]            # default openagents-autopilot
 *     [--wrangler-cwd <dir>]            # default ../../apps/openagents.com/workers/api
 *     [--batch-size <n>]                # default 500
 *     [--state-file <path>]             # default .artanis-backfill-state.json
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
  ARTANIS_TABLE_SPECS,
  compareArtanisTallies,
  d1ArtanisNewestRowHashes,
  postgresArtanisNewestRowHashes,
  postgresArtanisTally,
  upsertArtanisRows,
  type ArtanisBackfillTable,
  type ArtanisVerifyTally,
  type D1SourceRow,
} from "../src/artanis-backfill.js"
import type { SyncSql } from "../src/sql.js"

// Threads before messages (messages reference thread_ref; integrity is
// verified by set-membership, not FKs, but ordered fill keeps partial
// states sensible mid-run). Grants before spend decisions for the same
// reason. Everything else is independent.
const TABLES: ReadonlyArray<ArtanisBackfillTable> = [
  "artanis_runtime_snapshots",
  "artanis_loop_records",
  "artanis_loop_ticks",
  "artanis_approval_gates",
  "artanis_health_snapshots",
  "artanis_work_routing_proposals",
  "artanis_forum_publication_intents",
  "artanis_nexus_pylon_adapter_dispatches",
  "artanis_responder_state",
  "artanis_responder_actions",
  "artanis_responder_ticks",
  "artanis_admin_tick_decisions",
  "artanis_closeout_verdicts",
  "artanis_fleet_overseer_decisions",
  "artanis_standing_spend_grants",
  "artanis_spend_decisions",
  "artanis_labor_unattended_receipts",
  "artanis_owner_memory",
  "artanis_threads",
  "artanis_messages",
]

const USAGE = `Usage: bun scripts/backfill-artanis.ts [options]   (see file header)`

type Options = {
  batchSize: number
  d1Database: string
  databaseUrl: string | undefined
  local: boolean
  restart: boolean
  stateFile: string
  table: ArtanisBackfillTable | undefined
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
    stateFile: ".artanis-backfill-state.json",
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
      if (!TABLES.includes(table as ArtanisBackfillTable)) {
        console.error(`error: unknown table ${table}`)
        return undefined
      }
      options.table = table as ArtanisBackfillTable
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

type CursorState = Partial<Record<ArtanisBackfillTable, number>>

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
  table: ArtanisBackfillTable,
): Promise<void> => {
  let cursor = state[table] ?? 0
  let copied = 0
  let inserted = 0
  for (;;) {
    const rows = d1Query(
      options,
      `SELECT rowid AS d1_rowid, * FROM ${table} WHERE rowid > ${cursor} ORDER BY rowid ASC LIMIT ${options.batchSize}`,
    ) as Array<D1SourceRow & { d1_rowid: number }>
    if (rows.length === 0) break
    inserted += await upsertArtanisRows(sql, table, rows)
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
  table: ArtanisBackfillTable,
): ArtanisVerifyTally => {
  const statusColumn = ARTANIS_TABLE_SPECS[table].statusColumn
  const rows = d1Query(
    options,
    `SELECT ${statusColumn} AS status_value, COUNT(*) AS row_count FROM ${table} GROUP BY ${statusColumn} ORDER BY ${statusColumn}`,
  ) as Array<{ status_value: string | number | null; row_count: number }>
  const byStatus: Record<string, number> = {}
  let total = 0
  for (const row of rows) {
    const count = Number(row.row_count)
    byStatus[row.status_value === null ? "<null>" : String(row.status_value)] =
      count
    total += count
  }
  return { byStatus, total }
}

const verifyTable = async (
  sql: SyncSql,
  options: Options,
  table: ArtanisBackfillTable,
): Promise<boolean> => {
  const spec = ARTANIS_TABLE_SPECS[table]
  const d1 = d1Tally(options, table)
  const postgres = await postgresArtanisTally(sql, table)
  const d1Newest = d1ArtanisNewestRowHashes(
    table,
    d1Query(
      options,
      `SELECT * FROM ${table} ORDER BY ${spec.orderColumn} DESC, ${spec.conflictKey} DESC LIMIT ${options.verifyNewest}`,
    ),
  )
  const postgresNewest = await postgresArtanisNewestRowHashes(
    sql,
    table,
    options.verifyNewest,
  )
  const report = compareArtanisTallies(
    table,
    d1,
    postgres,
    d1Newest,
    postgresNewest,
  )

  console.log(`\n== ${table} ==`)
  console.log(`  rows: d1=${report.d1Total} postgres=${report.postgresTotal}`)
  console.log(`  d1 tallies:       ${JSON.stringify(d1.byStatus)}`)
  console.log(`  postgres tallies: ${JSON.stringify(postgres.byStatus)}`)
  if (report.statusMismatches.length > 0) {
    console.log(
      `  STATUS MISMATCHES: ${JSON.stringify(report.statusMismatches)}`,
    )
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
      "\nBackfill sweep complete. Run again after dual-write has covered the window, then run --verify.",
    )
    return 0
  } finally {
    await (sql as unknown as { end: () => Promise<void> }).end()
  }
}

process.exit(await main())
