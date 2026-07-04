#!/usr/bin/env bun
/**
 * KS-8.5 follow-up (#8334): agent runtime remainder backfill CLI.
 *
 * Usage (from packages/khala-sync-server/):
 *   bun scripts/backfill-agent-runtime-remainder.ts \
 *     [--database-url <postgres-url>]   # default $KHALA_SYNC_DATABASE_URL
 *     [--d1-database <name>]            # default openagents-autopilot
 *     [--wrangler-cwd <dir>]            # default ../../apps/openagents.com/workers/api
 *     [--batch-size <n>]                # default 300
 *     [--state-file <path>]             # default .agent-runtime-remainder-backfill-state.json
 *     [--table <name>]                  # limit to one table
 *     [--restart]                       # ignore saved cursor
 *     [--local]                         # wrangler --local
 *     [--verify] [--verify-newest <n>]  # verify mode, default N=50
 */

import { spawnSync } from "node:child_process"
import { existsSync, readFileSync, writeFileSync } from "node:fs"
import * as path from "node:path"
import { SQL } from "bun"
import {
  AGENT_RUNTIME_REMAINDER_SCALAR_TALLIES,
  AGENT_RUNTIME_REMAINDER_TABLE_KEY,
  AGENT_RUNTIME_REMAINDER_TABLE_ORDER,
  AGENT_RUNTIME_REMAINDER_TABLES,
  agentRuntimeRemainderVerifyReportClean,
  buildAgentRuntimeRemainderVerifyReport,
  d1AgentRuntimeRemainderNewestHashes,
  eventLedgerOrderingDensitySql,
  orderingDensityFromRows,
  postgresAgentRuntimeRemainderNewestHashes,
  postgresAgentRuntimeRemainderRowCount,
  postgresAgentRuntimeRemainderScalar,
  postgresEventLedgerOrderingDensity,
  upsertAgentRuntimeRemainderRows,
  type AgentRuntimeRemainderTable,
  type AgentRuntimeRemainderVerifyReport,
  type D1SourceRow,
} from "../src/agent-runtime-remainder-backfill.js"
import type { SyncSql } from "../src/sql.js"

const USAGE =
  "Usage: bun scripts/backfill-agent-runtime-remainder.ts [options]   (see file header)"

type Options = {
  batchSize: number
  d1Database: string
  databaseUrl: string | undefined
  local: boolean
  restart: boolean
  stateFile: string
  table: AgentRuntimeRemainderTable | undefined
  verify: boolean
  verifyNewest: number
  wranglerCwd: string
}

const parseArgs = (argv: ReadonlyArray<string>): Options | undefined => {
  const options: Options = {
    batchSize: 300,
    d1Database: "openagents-autopilot",
    databaseUrl: process.env["KHALA_SYNC_DATABASE_URL"],
    local: false,
    restart: false,
    stateFile: ".agent-runtime-remainder-backfill-state.json",
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
      if (
        !AGENT_RUNTIME_REMAINDER_TABLES.includes(
          table as AgentRuntimeRemainderTable,
        )
      ) {
        console.error(`error: unknown table ${table}`)
        return undefined
      }
      options.table = table as AgentRuntimeRemainderTable
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
    console.error("error: --batch-size must be 1..1500")
    return undefined
  }
  return options
}

const d1Query = (
  options: Options,
  command: string,
): Array<Record<string, unknown>> => {
  const result = spawnSync(
    "bunx",
    [
      "wrangler",
      "d1",
      "execute",
      options.d1Database,
      options.local ? "--local" : "--remote",
      "--json",
      "--command",
      command,
    ],
    {
      cwd: options.wranglerCwd,
      encoding: "utf8",
      maxBuffer: 256 * 1024 * 1024,
    },
  )
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

type CursorState = Partial<Record<AgentRuntimeRemainderTable, number>>

const loadState = (options: Options): CursorState =>
  options.restart || !existsSync(options.stateFile)
    ? {}
    : (JSON.parse(readFileSync(options.stateFile, "utf8")) as CursorState)

const saveState = (options: Options, state: CursorState): void => {
  writeFileSync(options.stateFile, `${JSON.stringify(state, null, 2)}\n`)
}

const backfillTable = async (
  sql: SyncSql,
  options: Options,
  state: CursorState,
  table: AgentRuntimeRemainderTable,
): Promise<void> => {
  const totalRow = d1Query(options, `SELECT COUNT(*) AS total_rows FROM ${table}`)[0]
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
    touched += await upsertAgentRuntimeRemainderRows(sql, table, rows)
    scanned += rows.length
    cursor = Number(rows[rows.length - 1]?.d1_rowid ?? cursor)
    state[table] = cursor
    saveState(options, state)
    const elapsedS = Math.max(1, (Date.now() - startedAtMs) / 1000)
    const rate = Math.round(scanned / elapsedS)
    console.log(
      `${table}: page done (cursor rowid=${cursor}, scanned=${scanned}${totalRows > 0 ? `/${totalRows}` : ""}, touched=${touched}, ~${rate} rows/s)`,
    )
  }
  console.log(`${table}: complete — scanned ${scanned} row(s), touched ${touched}`)
}

const printReport = (
  report: AgentRuntimeRemainderVerifyReport,
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
  if (report.orderingDensityMismatches.length > 0) {
    console.log(
      `  ORDERING-DENSITY MISMATCHES: ${report.orderingDensityMismatches.length}`,
    )
    for (const mismatch of report.orderingDensityMismatches.slice(0, 20)) {
      const shape = (row: typeof mismatch.d1) =>
        row === undefined
          ? "<missing>"
          : `entries=${row.entries} distinct=${row.distinctSequences} min=${row.minSequence} max=${row.maxSequence}`
      console.log(
        `    ${mismatch.ownerAgentUserId}: d1=${shape(mismatch.d1)} postgres=${shape(mismatch.postgres)}`,
      )
    }
  }
  if (
    report.d1OrderingGappedOwners > 0 ||
    report.postgresOrderingGappedOwners > 0
  ) {
    console.log(
      `  ORDERING GAPS: d1=${report.d1OrderingGappedOwners} postgres=${report.postgresOrderingGappedOwners}`,
    )
  }
  if (report.newestHashMismatches.length > 0) {
    console.log(
      `  NEWEST-${newest} HASH MISMATCHES: ${report.newestHashMismatches.length}`,
    )
    for (const mismatch of report.newestHashMismatches.slice(0, 20)) {
      console.log(
        `    ${mismatch.key}: d1=${mismatch.d1Hash ?? "<missing>"} postgres=${mismatch.postgresHash ?? "<missing>"}`,
      )
    }
  }
  const clean = agentRuntimeRemainderVerifyReportClean(report)
  console.log(`  status: ${clean ? "clean" : "DRIFT"}`)
  return clean
}

const artifactNextRows = (options: Options): number => {
  const tables = d1Query(
    options,
    "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'event_ledger_entries_next'",
  )
  if (tables.length === 0) return 0
  const row = d1Query(options, "SELECT COUNT(*) AS total_rows FROM event_ledger_entries_next")[0]
  return Number(row?.["total_rows"] ?? 0)
}

const verifyTable = async (
  sql: SyncSql,
  options: Options,
  table: AgentRuntimeRemainderTable,
): Promise<boolean> => {
  const d1Total = Number(
    d1Query(options, `SELECT COUNT(*) AS total_rows FROM ${table}`)[0]?.[
      "total_rows"
    ] ?? 0,
  )
  const postgresTotal = await postgresAgentRuntimeRemainderRowCount(sql, table)
  const scalars = await Promise.all(
    AGENT_RUNTIME_REMAINDER_SCALAR_TALLIES[table].map(async (tally) => ({
      d1: Number(d1Query(options, tally.sql)[0]?.["value"] ?? 0),
      metric: tally.metric,
      postgres: await postgresAgentRuntimeRemainderScalar(sql, tally.sql),
    })),
  )
  const d1OrderingDensity =
    table === "event_ledger_entries"
      ? orderingDensityFromRows(d1Query(options, eventLedgerOrderingDensitySql))
      : undefined
  const postgresOrderingDensity =
    table === "event_ledger_entries"
      ? await postgresEventLedgerOrderingDensity(sql)
      : undefined
  const orderColumn = AGENT_RUNTIME_REMAINDER_TABLE_ORDER[table]
  const keyColumns = AGENT_RUNTIME_REMAINDER_TABLE_KEY[table]
  const d1Newest = d1AgentRuntimeRemainderNewestHashes(
    table,
    d1Query(
      options,
      `SELECT * FROM ${table} ORDER BY ${orderColumn} DESC, ${keyColumns.join(" DESC, ")} DESC LIMIT ${options.verifyNewest}`,
    ),
  )
  const postgresNewest = await postgresAgentRuntimeRemainderNewestHashes(
    sql,
    table,
    options.verifyNewest,
  )
  return printReport(
    buildAgentRuntimeRemainderVerifyReport({
      d1Newest,
      d1OrderingDensity,
      d1Total,
      postgresNewest,
      postgresOrderingDensity,
      postgresTotal,
      scalars,
      table,
    }),
    options.verifyNewest,
  )
}

const main = async (): Promise<void> => {
  const options = parseArgs(process.argv.slice(2))
  if (options === undefined) process.exit(2)
  if (options.databaseUrl === undefined || options.databaseUrl.trim() === "") {
    throw new Error("missing --database-url or KHALA_SYNC_DATABASE_URL")
  }

  const sql = new SQL({ url: options.databaseUrl, max: 4 }) as unknown as SyncSql
  try {
    const tables =
      options.table === undefined ? AGENT_RUNTIME_REMAINDER_TABLES : [options.table]
    if (options.verify) {
      let clean = true
      for (const table of tables) clean = (await verifyTable(sql, options, table)) && clean
      const artifactRows = artifactNextRows(options)
      if (artifactRows > 0) {
        console.log(
          `\nARTIFACT DRIFT: event_ledger_entries_next still exists with ${artifactRows} row(s)`,
        )
        clean = false
      }
      console.log(
        `\nevent_ledger_entries_next artifact: ${artifactRows === 0 ? "absent-or-empty" : "NOT EMPTY"}`,
      )
      if (!clean) process.exit(1)
      return
    }

    const state = loadState(options)
    for (const table of tables) await backfillTable(sql, options, state, table)
  } finally {
    await (sql as unknown as SQL).end()
  }
}

await main()
