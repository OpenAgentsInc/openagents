#!/usr/bin/env bun
/**
 * KS-8.15 remainder (#8355): gym / mullet / blueprint / replay-clip /
 * mirrorcode eval domain backfill CLI — D1 → Postgres.
 *
 * Cursor-resumable (rowid cursor per table, persisted after every batch) and
 * idempotent (converge upserts / insert-if-absent over the exact D1 arbiter
 * keys — rerunning any range is a no-op). Same machinery as
 * scripts/backfill-training.ts.
 *
 * Usage (from packages/khala-sync-server/):
 *   bun scripts/backfill-gym-evals.ts \
 *     [--database-url <postgres-url>]   # default $KHALA_SYNC_DATABASE_URL
 *     [--d1-database <name>]            # default openagents-autopilot
 *     [--wrangler-cwd <dir>]            # default ../../apps/openagents.com/workers/api
 *     [--batch-size <n>]                # default 300
 *     [--state-file <path>]             # default .gym-evals-backfill-state.json
 *     [--table <name>]                  # limit to one table
 *     [--restart]                       # ignore saved cursor
 *     [--local]                         # wrangler --local
 *     [--verify] [--verify-newest <n>]  # verify mode (default N=50)
 *
 * Verify checks: exact per-table counts, newest-N full-row hashes (byte-exact
 * round-trip; the leaderboard-recomputation-equality acceptance for the
 * derived snapshot tables), and lifecycle state tallies.
 */

import { spawnSync } from "node:child_process"
import { existsSync, readFileSync, writeFileSync } from "node:fs"
import * as path from "node:path"
import { SQL } from "bun"
import {
  GYM_EVALS_DOMAIN_TABLE_SPECS,
  GYM_EVALS_DOMAIN_TABLES,
  GYM_EVALS_TALLY_TABLES,
  compareGymEvalsNewestHashes,
  compareGymEvalsStateTallies,
  gymEvalsDomainNewestHashesFromRows,
  gymEvalsDomainNewestOrderSql,
  gymEvalsDomainVerifyReportOk,
  gymEvalsStateTallyFromRows,
  postgresGymEvalsDomainNewestHashes,
  postgresGymEvalsDomainRowCount,
  postgresGymEvalsStateTally,
  upsertGymEvalsDomainRows,
  type GymEvalsDomainSourceRow,
  type GymEvalsDomainTable,
  type GymEvalsDomainVerifyReport,
} from "../src/gym-evals-domain-backfill.js"
import type { SyncSql } from "../src/sql.js"

type Options = {
  batchSize: number
  d1Database: string
  databaseUrl: string | undefined
  local: boolean
  restart: boolean
  stateFile: string
  table: GymEvalsDomainTable | undefined
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
    stateFile: ".gym-evals-backfill-state.json",
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
      if (!GYM_EVALS_DOMAIN_TABLES.includes(table as GymEvalsDomainTable)) {
        console.error(`error: unknown table ${table}`)
        return undefined
      }
      options.table = table as GymEvalsDomainTable
    } else if (arg === "--restart") options.restart = true
    else if (arg === "--local") options.local = true
    else if (arg === "--verify") options.verify = true
    else if (arg === "--verify-newest") options.verifyNewest = Number(next())
    else if (arg === "--help" || arg === "-h") {
      console.log("Usage: bun scripts/backfill-gym-evals.ts [options]")
      return undefined
    } else {
      console.error(`error: unknown argument ${JSON.stringify(arg)}`)
      return undefined
    }
  }

  if (
    !Number.isSafeInteger(options.batchSize) ||
    options.batchSize < 1 ||
    options.batchSize > 1000
  ) {
    console.error("error: --batch-size must be 1..1000")
    return undefined
  }
  if (
    !Number.isSafeInteger(options.verifyNewest) ||
    options.verifyNewest < 1 ||
    options.verifyNewest > 500
  ) {
    console.error("error: --verify-newest must be 1..500")
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
      `wrangler d1 execute failed (${String(result.status)}): ${result.stderr.slice(0, 500)}`,
    )
  }
  const parsed = JSON.parse(result.stdout) as Array<{
    results: Array<Record<string, unknown>>
  }>
  return parsed[0]?.results ?? []
}

type CursorState = Partial<Record<GymEvalsDomainTable, number>>

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
  table: GymEvalsDomainTable,
): Promise<void> => {
  let cursor = state[table] ?? 0
  let scanned = 0
  let touched = 0
  const startedAtMs = Date.now()

  for (;;) {
    const rows = d1Query(
      options,
      `SELECT rowid AS __rowid, * FROM ${table} WHERE rowid > ${cursor} ORDER BY rowid ASC LIMIT ${options.batchSize}`,
    ) as Array<GymEvalsDomainSourceRow & { __rowid?: number }>
    if (rows.length === 0) {
      break
    }
    cursor = Math.max(...rows.map((row) => Number(row.__rowid ?? 0)))
    state[table] = cursor
    const cleanRows = rows.map(({ __rowid: _rowid, ...row }) => row)
    touched += await upsertGymEvalsDomainRows(sql, table, cleanRows)
    scanned += rows.length
    saveState(options, state)
    console.log(
      JSON.stringify({
        table,
        cursor,
        scanned,
        touched,
        elapsedMs: Date.now() - startedAtMs,
      }),
    )
  }
}

const d1RowCount = (options: Options, table: GymEvalsDomainTable): number =>
  Number(
    d1Query(options, `SELECT COUNT(*) AS total_rows FROM ${table}`)[0]?.[
      "total_rows"
    ] ?? 0,
  )

const d1NewestHashes = (
  options: Options,
  table: GymEvalsDomainTable,
): ReadonlyArray<{ key: string; hash: string }> =>
  gymEvalsDomainNewestHashesFromRows(
    table,
    d1Query(
      options,
      `SELECT * FROM ${table} ORDER BY ${gymEvalsDomainNewestOrderSql(table)} LIMIT ${options.verifyNewest}`,
    ),
  )

const buildVerifyReport = async (
  sql: SyncSql,
  options: Options,
  tables: ReadonlyArray<GymEvalsDomainTable>,
): Promise<GymEvalsDomainVerifyReport> => {
  const countMismatches: Array<
    GymEvalsDomainVerifyReport["countMismatches"][number]
  > = []
  const newestHashMismatches: Array<
    GymEvalsDomainVerifyReport["newestHashMismatches"][number]
  > = []
  const stateTallyMismatches: Array<
    GymEvalsDomainVerifyReport["stateTallyMismatches"][number]
  > = []

  for (const table of tables) {
    const d1 = d1RowCount(options, table)
    const postgres = await postgresGymEvalsDomainRowCount(sql, table)
    if (d1 !== postgres) {
      countMismatches.push({ d1, postgres, table })
    }

    const mismatches = compareGymEvalsNewestHashes(
      d1NewestHashes(options, table),
      await postgresGymEvalsDomainNewestHashes(
        sql,
        table,
        options.verifyNewest,
      ),
    )
    if (mismatches.length > 0) {
      newestHashMismatches.push({ mismatches, table })
    }

    const tallyColumn = GYM_EVALS_TALLY_TABLES[table]
    if (tallyColumn !== undefined) {
      const d1Tally = gymEvalsStateTallyFromRows(
        tallyColumn,
        d1Query(options, `SELECT ${tallyColumn} FROM ${table}`),
      )
      const postgresTally = await postgresGymEvalsStateTally(
        sql,
        table,
        tallyColumn,
      )
      if (!compareGymEvalsStateTallies(d1Tally, postgresTally)) {
        stateTallyMismatches.push({
          d1: d1Tally,
          postgres: postgresTally,
          table,
        })
      }
    }
  }

  return { countMismatches, newestHashMismatches, stateTallyMismatches }
}

const main = async (): Promise<void> => {
  const options = parseArgs(process.argv.slice(2))
  if (options === undefined) {
    process.exitCode = 1
    return
  }
  if (options.databaseUrl === undefined || options.databaseUrl.length === 0) {
    throw new Error(
      "--database-url or KHALA_SYNC_DATABASE_URL is required (direct Postgres URL)",
    )
  }

  const tables =
    options.table === undefined ? GYM_EVALS_DOMAIN_TABLES : [options.table]

  const sql = new SQL(options.databaseUrl, {
    max: 1,
  }) as unknown as SyncSql & { end: () => Promise<void> }
  try {
    if (options.verify) {
      const report = await buildVerifyReport(sql, options, tables)
      console.log(JSON.stringify(report, null, 2))
      if (!gymEvalsDomainVerifyReportOk(report)) {
        process.exitCode = 1
      }
      return
    }

    const state = loadState(options)
    for (const table of tables) {
      if (GYM_EVALS_DOMAIN_TABLE_SPECS[table] === undefined) {
        throw new Error(`unknown table ${table}`)
      }
      await backfillTable(sql, options, state, table)
    }
  } finally {
    await sql.end()
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.stack : error)
  process.exitCode = 1
})
