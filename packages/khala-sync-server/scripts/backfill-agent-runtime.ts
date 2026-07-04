#!/usr/bin/env bun
/**
 * KS-8.5 (#8316): agent runtime metadata backfill CLI — D1 → Postgres.
 *
 * Reads D1 rows through `wrangler d1 execute <db> --remote --json` (the
 * KS-8.1/8.2 pattern: reuses the repo's existing wrangler auth — no new
 * admin-API surface) in bounded rowid-keyset pages, and copies them into
 * the Postgres twins from khala-sync migration 0010 over a DIRECT
 * connection (never Hyperdrive):
 *
 *   - `agent_run_events` / `agent_goal_events`: bare `ON CONFLICT DO
 *     NOTHING`, one multi-row INSERT per page (the exact D1
 *     INSERT-OR-IGNORE key sets).
 *   - the six state tables: CONVERGE upserts to the D1 snapshot value
 *     (triggers converge on the live (owner, trigger_ref) arbiter; goal
 *     pages are ordered archived-first for the scope unique).
 *
 * Resumable: progress (last rowid per table) persists in a local state
 * file; delete it (or pass --restart) to sweep from the beginning again.
 * The runbook's sequence runs the backfill TWICE — the second sweep is
 * the catch-up pass after dual-write has been on across the whole window.
 *
 * Verify mode (`--verify`): exact row counts, per-table domain tallies
 * (run event_cursor/status sums, trace visibility + training-consent +
 * step-count + digest tallies, goal usage sums, trigger state sums),
 * per-run and per-goal EVENT-CHAIN comparison (count / distinct / min /
 * max per parent — the KS-8.5 contiguity evidence), and newest-N row-hash
 * comparison (the trace content-hash sample). Exits non-zero on ANY
 * mismatch. PRIVACY: output references row keys and sha256 hashes only —
 * never trace trajectory content.
 *
 * Usage (from packages/khala-sync-server/):
 *   bun scripts/backfill-agent-runtime.ts \
 *     [--database-url <postgres-url>]   # default $KHALA_SYNC_DATABASE_URL
 *     [--d1-database <name>]            # default openagents-autopilot
 *     [--wrangler-cwd <dir>]            # default ../../apps/openagents.com/workers/api
 *     [--batch-size <n>]                # default 200
 *     [--state-file <path>]             # default .agent-runtime-backfill-state.json
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
  AGENT_RUNTIME_SCALAR_TALLIES,
  AGENT_RUNTIME_TABLE_KEY,
  AGENT_RUNTIME_TABLE_ORDER,
  AGENT_RUNTIME_TABLES,
  agentRuntimeVerifyReportClean,
  buildAgentRuntimeVerifyReport,
  d1AgentRuntimeNewestHashes,
  eventChainSql,
  eventChainTallyFromRows,
  postgresAgentRuntimeNewestHashes,
  postgresAgentRuntimeRowCount,
  postgresAgentRuntimeScalar,
  postgresEventChainTally,
  upsertAgentRuntimeRows,
  type AgentRuntimeBackfillTable,
  type AgentRuntimeVerifyReport,
  type D1SourceRow,
} from "../src/agent-runtime-backfill.js"
import type { SyncSql } from "../src/sql.js"

const USAGE = `Usage: bun scripts/backfill-agent-runtime.ts [options]   (see file header)`

type Options = {
  batchSize: number
  d1Database: string
  databaseUrl: string | undefined
  local: boolean
  restart: boolean
  stateFile: string
  table: AgentRuntimeBackfillTable | undefined
  verify: boolean
  verifyNewest: number
  wranglerCwd: string
}

const parseArgs = (argv: ReadonlyArray<string>): Options | undefined => {
  const options: Options = {
    // agent_traces rows can carry near-1MB trajectory_json values — keep
    // pages small enough for wrangler's JSON output and the param cap.
    batchSize: 200,
    d1Database: "openagents-autopilot",
    databaseUrl: process.env["KHALA_SYNC_DATABASE_URL"],
    local: false,
    restart: false,
    stateFile: ".agent-runtime-backfill-state.json",
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
      if (!AGENT_RUNTIME_TABLES.includes(table as AgentRuntimeBackfillTable)) {
        console.error(`error: unknown table ${table}`)
        return undefined
      }
      options.table = table as AgentRuntimeBackfillTable
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
    // 1500 × 27 columns max = 40,500 bind params — under the 65,535 cap.
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

type CursorState = Partial<Record<AgentRuntimeBackfillTable, number>>

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
  table: AgentRuntimeBackfillTable,
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
    touched += await upsertAgentRuntimeRows(sql, table, rows)
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
  report: AgentRuntimeVerifyReport,
  newest: number,
): boolean => {
  console.log(`\n== ${report.table} ==`)
  console.log(
    `  rows: d1=${report.d1Total} postgres=${report.postgresTotal}${report.countsMatch ? "" : "  ROW-COUNT MISMATCH"}`,
  )
  for (const mismatch of report.scalarMismatches) {
    console.log(
      `  SCALAR MISMATCH ${mismatch.metric}: d1=${mismatch.d1} postgres=${mismatch.postgres}`,
    )
  }
  if (report.chainMismatches.length > 0) {
    console.log(`  EVENT-CHAIN MISMATCHES: ${report.chainMismatches.length}`)
    for (const mismatch of report.chainMismatches.slice(0, 20)) {
      const shape = (chain: typeof mismatch.d1) =>
        chain === undefined
          ? "<missing>"
          : `events=${chain.events} distinct=${chain.distinctSequences} min=${chain.minSequence} max=${chain.maxSequence}`
      console.log(
        `    ${mismatch.parentId}: d1 ${shape(mismatch.d1)} | pg ${shape(mismatch.postgres)}`,
      )
    }
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
  return agentRuntimeVerifyReportClean(report)
}

const d1NewestRows = (
  options: Options,
  table: AgentRuntimeBackfillTable,
): Array<Record<string, unknown>> => {
  const orderColumn = AGENT_RUNTIME_TABLE_ORDER[table]
  const keyColumns = AGENT_RUNTIME_TABLE_KEY[table]
  return d1Query(
    options,
    `SELECT * FROM ${table} ORDER BY ${orderColumn} DESC, ${keyColumns.join(" DESC, ")} DESC LIMIT ${options.verifyNewest}`,
  )
}

const verifyTable = async (
  sql: SyncSql,
  options: Options,
  table: AgentRuntimeBackfillTable,
): Promise<boolean> => {
  const d1Total = Number(
    d1Query(options, `SELECT COUNT(*) AS total_rows FROM ${table}`)[0]?.[
      "total_rows"
    ] ?? 0,
  )
  const postgresTotal = await postgresAgentRuntimeRowCount(sql, table)

  const scalars: Array<{ metric: string; d1: number; postgres: number }> = []
  for (const tally of AGENT_RUNTIME_SCALAR_TALLIES[table]) {
    scalars.push({
      d1: Number(d1Query(options, tally.sql)[0]?.["value"] ?? 0),
      metric: tally.metric,
      postgres: await postgresAgentRuntimeScalar(sql, tally.sql),
    })
  }

  const isChainTable =
    table === "agent_run_events" || table === "agent_goal_events"
  const d1Chains = isChainTable
    ? eventChainTallyFromRows(d1Query(options, eventChainSql(table)))
    : undefined
  const postgresChains = isChainTable
    ? await postgresEventChainTally(sql, table)
    : undefined

  const report = buildAgentRuntimeVerifyReport({
    d1Chains,
    d1Newest: d1AgentRuntimeNewestHashes(table, d1NewestRows(options, table)),
    d1Total,
    postgresChains,
    postgresNewest: await postgresAgentRuntimeNewestHashes(
      sql,
      table,
      options.verifyNewest,
    ),
    postgresTotal,
    scalars,
    table,
  })
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
    options.table === undefined ? AGENT_RUNTIME_TABLES : [options.table]
  try {
    if (options.verify) {
      let allGood = true
      for (const table of tables) {
        allGood = (await verifyTable(sql, options, table)) && allGood
      }
      console.log(
        allGood
          ? "\nVERIFY OK: exact counts, domain tallies, event chains, and newest-N hashes match."
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
