#!/usr/bin/env bun
/**
 * KS-8.4 (#8315): Pylon control-plane remainder backfill CLI -- D1 to Postgres.
 *
 * Reads source rows via `wrangler d1 execute <db> --remote --json`, copies them
 * into the migration-0009 Postgres twins over a DIRECT connection, and verifies
 * exact counts, domain tallies, and newest-N row hashes. Payload bodies for
 * raw Codex events remain in R2; this copies metadata index rows only.
 *
 * Usage (from packages/khala-sync-server/):
 *   bun scripts/backfill-pylon-control-plane.ts \
 *     [--database-url <postgres-url>]   # default $KHALA_SYNC_DATABASE_URL
 *     [--d1-database <name>]            # default openagents-autopilot
 *     [--wrangler-cwd <dir>]            # default ../../apps/openagents.com/workers/api
 *     [--batch-size <n>]                # default 500
 *     [--state-file <path>]             # default .pylon-control-plane-backfill-state.json
 *     [--table <name>]                  # limit to one target table
 *     [--restart]                       # ignore saved cursor
 *     [--local]                         # wrangler --local (dev smoke)
 *     [--verify] [--verify-newest <n>]  # verify mode (default N=50)
 *     [--raw-event-reconcile]           # raw Codex metadata aggregate + chunk-chain proof
 */
import { spawnSync } from "node:child_process"
import { existsSync, readFileSync, writeFileSync } from "node:fs"
import * as path from "node:path"
import { SQL } from "bun"
import {
  comparePylonControlPlaneTallies,
  D1_SOURCE_TABLES,
  d1PylonControlPlaneNewestHashes,
  postgresPylonCodexRawEventChunkAggregates,
  postgresPylonCodexRawEventTurnAggregates,
  postgresPylonControlPlaneNewestHashes,
  postgresPylonControlPlaneTally,
  PYLON_CONTROL_PLANE_TABLES,
  reconcilePylonCodexRawEventMetadata,
  TABLE_CONFLICT_KEY,
  TABLE_ORDER_COLUMN,
  TABLE_TALLY_COLUMN,
  tallyFromRows,
  upsertPylonControlPlaneRows,
  type D1SourceRow,
  type PylonCodexRawEventAggregateMismatch,
  type PylonCodexRawEventAggregateRow,
  type PylonCodexRawEventChunkAggregateRow,
  type PylonCodexRawEventChunkChainGap,
  type PylonCodexRawEventMetadataReconcileReport,
  type PylonControlPlaneBackfillTable,
  type PylonControlPlaneVerifyReport,
} from "../src/pylon-control-plane-backfill.js"
import type { SyncSql } from "../src/sql.js"

const USAGE =
  "Usage: bun scripts/backfill-pylon-control-plane.ts [options] (see file header)"

type Options = {
  batchSize: number
  d1Database: string
  databaseUrl: string | undefined
  local: boolean
  rawEventReconcile: boolean
  restart: boolean
  stateFile: string
  table: PylonControlPlaneBackfillTable | undefined
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
    rawEventReconcile: false,
    restart: false,
    stateFile: ".pylon-control-plane-backfill-state.json",
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
      if (!PYLON_CONTROL_PLANE_TABLES.includes(table as PylonControlPlaneBackfillTable)) {
        console.error(`error: unknown table ${table}`)
        return undefined
      }
      options.table = table as PylonControlPlaneBackfillTable
    } else if (arg === "--restart") options.restart = true
    else if (arg === "--local") options.local = true
    else if (arg === "--verify") options.verify = true
    else if (arg === "--verify-newest") options.verifyNewest = Number(next())
    else if (arg === "--raw-event-reconcile") options.rawEventReconcile = true
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

type CursorState = Partial<Record<PylonControlPlaneBackfillTable, number>>

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
  table: PylonControlPlaneBackfillTable,
): Promise<void> => {
  const source = D1_SOURCE_TABLES[table]
  const total = Number(
    d1Query(options, `SELECT COUNT(*) AS total_rows FROM ${source}`)[0]?.[
      "total_rows"
    ] ?? 0,
  )
  let cursor = state[table] ?? 0
  let scanned = 0
  let inserted = 0
  const startedAtMs = Date.now()
  for (;;) {
    const rows = d1Query(
      options,
      `SELECT rowid AS d1_rowid, * FROM ${source} WHERE rowid > ${cursor} ORDER BY rowid ASC LIMIT ${options.batchSize}`,
    ) as Array<D1SourceRow & { d1_rowid: number }>
    if (rows.length === 0) break
    inserted += await upsertPylonControlPlaneRows(sql, table, rows)
    scanned += rows.length
    cursor = Number(rows[rows.length - 1]?.d1_rowid ?? cursor)
    state[table] = cursor
    saveState(options, state)
    const elapsedS = Math.max(1, (Date.now() - startedAtMs) / 1000)
    const rate = Math.round(scanned / elapsedS)
    console.log(
      `${table}: page done (source=${source}, cursor rowid=${cursor}, scanned=${scanned}${total > 0 ? `/${total}` : ""}, inserted=${inserted}, ~${rate} rows/s)`,
    )
  }
  console.log(`${table}: complete -- scanned ${scanned}, inserted ${inserted}`)
}

const printReport = (
  report: PylonControlPlaneVerifyReport,
  newest: number,
): boolean => {
  console.log(`\n== ${report.table} ==`)
  console.log(
    `  rows: d1=${report.d1Total} postgres=${report.postgresTotal}${report.countsMatch ? "" : " ROW-COUNT MISMATCH"}`,
  )
  for (const mismatch of report.statusMismatches) {
    console.log(
      `  TALLY MISMATCH ${mismatch.status}: d1=${mismatch.d1} postgres=${mismatch.postgres}`,
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
    report.statusMismatches.length === 0 &&
    report.newestHashMismatches.length === 0
  )
}

const d1NewestRows = (
  options: Options,
  table: PylonControlPlaneBackfillTable,
): ReadonlyArray<D1SourceRow> => {
  const source = D1_SOURCE_TABLES[table]
  const orderColumn = TABLE_ORDER_COLUMN[table]
  const keyColumns = TABLE_CONFLICT_KEY[table]
    .split(",")
    .map((column) => column.trim())
  return d1Query(
    options,
    `SELECT * FROM ${source} ORDER BY ${orderColumn} DESC, ${keyColumns.join(" DESC, ")} DESC LIMIT ${options.verifyNewest}`,
  )
}

const d1TallyRows = (
  options: Options,
  table: PylonControlPlaneBackfillTable,
): ReadonlyArray<D1SourceRow> => {
  const source = D1_SOURCE_TABLES[table]
  const tallyColumn = TABLE_TALLY_COLUMN[table]
  return d1Query(
    options,
    `SELECT ${tallyColumn} FROM ${source}`,
  )
}

const verifyTable = async (
  sql: SyncSql,
  options: Options,
  table: PylonControlPlaneBackfillTable,
): Promise<boolean> => {
  const d1Tally = tallyFromRows(table, d1TallyRows(options, table))
  const pgTally = await postgresPylonControlPlaneTally(sql, table)
  const report = comparePylonControlPlaneTallies(
    table,
    d1Tally,
    pgTally,
    d1PylonControlPlaneNewestHashes(table, d1NewestRows(options, table)),
    await postgresPylonControlPlaneNewestHashes(
      sql,
      table,
      options.verifyNewest,
    ),
  )
  return printReport(report, options.verifyNewest)
}

const d1RawEventTurnAggregateRows = (
  options: Options,
): ReadonlyArray<PylonCodexRawEventAggregateRow> =>
  d1Query(
    options,
    `SELECT
       assignment_ref,
       lease_ref,
       pylon_ref,
       owner_user_id,
       turn_index,
       COUNT(*) AS row_count,
       COALESCE(SUM(event_count), 0) AS event_count,
       COALESCE(SUM(byte_length), 0) AS byte_length
     FROM ${D1_SOURCE_TABLES.pylon_codex_raw_events}
     GROUP BY assignment_ref, lease_ref, pylon_ref, owner_user_id, turn_index
     ORDER BY owner_user_id, assignment_ref, lease_ref, pylon_ref, turn_index`,
  ) as unknown as ReadonlyArray<PylonCodexRawEventAggregateRow>

const d1RawEventChunkAggregateRows = (
  options: Options,
): ReadonlyArray<PylonCodexRawEventChunkAggregateRow> =>
  d1Query(
    options,
    `SELECT
       assignment_ref,
       lease_ref,
       pylon_ref,
       owner_user_id,
       turn_index,
       COUNT(*) AS row_count,
       COUNT(DISTINCT chunk_index) AS distinct_chunk_indexes,
       MIN(chunk_index) AS min_chunk_index,
       MAX(chunk_index) AS max_chunk_index,
       COALESCE(SUM(event_count), 0) AS event_count,
       COALESCE(SUM(byte_length), 0) AS byte_length
     FROM ${D1_SOURCE_TABLES.pylon_codex_raw_event_chunks}
     GROUP BY assignment_ref, lease_ref, pylon_ref, owner_user_id, turn_index
     ORDER BY owner_user_id, assignment_ref, lease_ref, pylon_ref, turn_index`,
  ) as unknown as ReadonlyArray<PylonCodexRawEventChunkAggregateRow>

const printRawEventMismatch = (
  label: string,
  mismatch: PylonCodexRawEventAggregateMismatch,
): void => {
  console.log(
    `  ${label} ${mismatch.key}: d1=${JSON.stringify(mismatch.d1)} postgres=${JSON.stringify(mismatch.postgres)}`,
  )
}

const printRawEventChunkGap = (
  gap: PylonCodexRawEventChunkChainGap,
): void => {
  console.log(
    `  CHUNK GAP ${gap.source} ${gap.key}: min=${gap.minChunkIndex} max=${gap.maxChunkIndex} rows=${gap.chunkCount} distinct=${gap.distinctChunkIndexes} expected=${gap.expectedChunkCount}`,
  )
}

const printRawEventReconcileReport = (
  report: PylonCodexRawEventMetadataReconcileReport,
): boolean => {
  console.log("\n== pylon_codex_raw_event_metadata ==")
  console.log(
    `  turn metadata rows: d1=${report.turnEvents.d1Total} postgres=${report.turnEvents.postgresTotal}`,
  )
  console.log(
    `  chunk metadata rows: d1=${report.chunks.d1Total} postgres=${report.chunks.postgresTotal}`,
  )
  console.log(
    `  turn aggregate parity: ${
      report.turnEvents.mismatches.length === 0
        ? "all match"
        : `${report.turnEvents.mismatches.length} MISMATCH(ES)`
    }`,
  )
  for (const mismatch of report.turnEvents.mismatches.slice(0, 25)) {
    printRawEventMismatch("TURN MISMATCH", mismatch)
  }
  console.log(
    `  chunk aggregate parity: ${
      report.chunks.mismatches.length === 0
        ? "all match"
        : `${report.chunks.mismatches.length} MISMATCH(ES)`
    }`,
  )
  for (const mismatch of report.chunks.mismatches.slice(0, 25)) {
    printRawEventMismatch("CHUNK MISMATCH", mismatch)
  }
  console.log(
    `  chunk chains: ${
      report.chunks.chainGaps.length === 0
        ? "contiguous per turn"
        : `${report.chunks.chainGaps.length} GAP(S)`
    }`,
  )
  for (const gap of report.chunks.chainGaps.slice(0, 25)) {
    printRawEventChunkGap(gap)
  }
  return report.ok
}

const reconcileRawEventMetadata = async (
  sql: SyncSql,
  options: Options,
): Promise<boolean> => {
  const report = reconcilePylonCodexRawEventMetadata({
    d1Chunks: d1RawEventChunkAggregateRows(options),
    d1TurnEvents: d1RawEventTurnAggregateRows(options),
    postgresChunks: await postgresPylonCodexRawEventChunkAggregates(sql),
    postgresTurnEvents: await postgresPylonCodexRawEventTurnAggregates(sql),
  })
  return printRawEventReconcileReport(report)
}

const main = async (): Promise<number> => {
  const options = parseArgs(process.argv.slice(2))
  if (options === undefined) return 2
  if (options.databaseUrl === undefined || options.databaseUrl === "") {
    console.error(
      "error: no Postgres URL -- pass --database-url or set KHALA_SYNC_DATABASE_URL",
    )
    return 2
  }

  const sql = new SQL(options.databaseUrl) as unknown as SyncSql
  const tables =
    options.table === undefined ? PYLON_CONTROL_PLANE_TABLES : [options.table]
  try {
    if (options.verify || options.rawEventReconcile) {
      let allGood = true
      if (options.verify) {
        for (const table of tables) {
          allGood = (await verifyTable(sql, options, table)) && allGood
        }
      }
      if (options.rawEventReconcile) {
        allGood = (await reconcileRawEventMetadata(sql, options)) && allGood
      }
      console.log(
        allGood
          ? "\nVERIFY OK: exact row counts, tallies, newest-N hashes, and requested raw-event checks match."
          : "\nVERIFY FAILED: mismatches above -- investigate before read cutover.",
      )
      return allGood ? 0 : 1
    }

    const state = loadState(options)
    for (const table of tables) {
      await backfillTable(sql, options, state, table)
    }
    console.log("\nBackfill sweep complete. Run again for catch-up, then --verify.")
    return 0
  } finally {
    await (sql as unknown as { end: () => Promise<void> }).end()
  }
}

process.exit(await main())
