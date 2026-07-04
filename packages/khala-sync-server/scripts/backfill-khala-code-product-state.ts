#!/usr/bin/env bun
/**
 * KS-8.13 (#8324): Khala Code product-state backfill CLI — D1 → Postgres.
 *
 * Usage (from packages/khala-sync-server/):
 *   bun scripts/backfill-khala-code-product-state.ts \
 *     [--database-url <postgres-url>]   # default $KHALA_SYNC_DATABASE_URL
 *     [--d1-database <name>]            # default openagents-autopilot
 *     [--wrangler-cwd <dir>]            # default ../../apps/openagents.com/workers/api
 *     [--batch-size <n>]                # default 300
 *     [--state-file <path>]             # default .khala-code-product-state-backfill-state.json
 *     [--table <name>]                  # limit to one table
 *     [--restart]                       # ignore saved cursor
 *     [--local]                         # wrangler --local
 *     [--verify] [--verify-newest <n>]  # verify mode (default N=50)
 */

import { spawnSync } from "node:child_process"
import { existsSync, readFileSync, writeFileSync } from "node:fs"
import * as path from "node:path"
import { SQL } from "bun"
import {
  KHALA_CODE_PRODUCT_STATE_TABLE_SPECS,
  KHALA_CODE_PRODUCT_STATE_TABLES,
  compareMessageChainFingerprints,
  compareNewestHashes,
  khalaCodeProductStateNewestHashesFromRows,
  khalaCodeProductStateNewestOrderSql,
  khalaCodeProductStateVerifyReportOk,
  membershipSetFingerprintFromRows,
  messageChainFingerprintsFromRows,
  postgresKhalaCodeProductStateNewestHashes,
  postgresKhalaCodeProductStateRowCount,
  postgresMembershipSetFingerprint,
  postgresMessageChainFingerprints,
  upsertKhalaCodeProductStateRows,
  type D1SourceRow,
  type KhalaCodeProductStateTable,
  type KhalaCodeProductStateVerifyReport,
} from "../src/khala-code-product-state-backfill.js"
import type { SyncSql } from "../src/sql.js"

type Options = {
  batchSize: number
  d1Database: string
  databaseUrl: string | undefined
  local: boolean
  restart: boolean
  stateFile: string
  table: KhalaCodeProductStateTable | undefined
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
    stateFile: ".khala-code-product-state-backfill-state.json",
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
        !KHALA_CODE_PRODUCT_STATE_TABLES.includes(
          table as KhalaCodeProductStateTable,
        )
      ) {
        console.error(`error: unknown table ${table}`)
        return undefined
      }
      options.table = table as KhalaCodeProductStateTable
    } else if (arg === "--restart") options.restart = true
    else if (arg === "--local") options.local = true
    else if (arg === "--verify") options.verify = true
    else if (arg === "--verify-newest") options.verifyNewest = Number(next())
    else if (arg === "--help" || arg === "-h") {
      console.log("Usage: bun scripts/backfill-khala-code-product-state.ts [options]")
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

type CursorState = Partial<Record<KhalaCodeProductStateTable, number>>

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
  table: KhalaCodeProductStateTable,
): Promise<void> => {
  let cursor = state[table] ?? 0
  let scanned = 0
  let touched = 0
  const startedAtMs = Date.now()

  for (;;) {
    const rows = d1Query(
      options,
      `SELECT rowid AS __rowid, * FROM ${table} WHERE rowid > ${cursor} ORDER BY rowid ASC LIMIT ${options.batchSize}`,
    ) as Array<D1SourceRow & { __rowid?: number }>
    if (rows.length === 0) {
      break
    }
    cursor = Math.max(...rows.map((row) => Number(row.__rowid ?? 0)))
    state[table] = cursor
    const cleanRows = rows.map(({ __rowid: _rowid, ...row }) => row)
    touched += await upsertKhalaCodeProductStateRows(sql, table, cleanRows)
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

const d1RowCount = (
  options: Options,
  table: KhalaCodeProductStateTable,
): number =>
  Number(
    d1Query(options, `SELECT COUNT(*) AS total_rows FROM ${table}`)[0]?.[
      "total_rows"
    ] ?? 0,
  )

const d1NewestHashes = (
  options: Options,
  table: KhalaCodeProductStateTable,
): ReadonlyArray<{ key: string; hash: string }> =>
  khalaCodeProductStateNewestHashesFromRows(
    table,
    d1Query(
      options,
      `SELECT * FROM ${table} ORDER BY ${khalaCodeProductStateNewestOrderSql(table)} LIMIT ${options.verifyNewest}`,
    ),
  )

const buildVerifyReport = async (
  sql: SyncSql,
  options: Options,
  tables: ReadonlyArray<KhalaCodeProductStateTable>,
): Promise<KhalaCodeProductStateVerifyReport> => {
  const countMismatches: Array<
    KhalaCodeProductStateVerifyReport["countMismatches"][number]
  > = []
  const newestHashMismatches: Array<
    KhalaCodeProductStateVerifyReport["newestHashMismatches"][number]
  > = []

  for (const table of tables) {
    const d1 = d1RowCount(options, table)
    const postgres = await postgresKhalaCodeProductStateRowCount(sql, table)
    if (d1 !== postgres) {
      countMismatches.push({ d1, postgres, table })
    }

    const mismatches = compareNewestHashes(
      d1NewestHashes(options, table),
      await postgresKhalaCodeProductStateNewestHashes(
        sql,
        table,
        options.verifyNewest,
      ),
    )
    if (mismatches.length > 0) {
      newestHashMismatches.push({ mismatches, table })
    }
  }

  const d1Membership = membershipSetFingerprintFromRows(
    d1Query(
      options,
      `SELECT team_id, user_id, role, status, removed_at
         FROM team_memberships
        ORDER BY team_id ASC, user_id ASC`,
    ),
  )
  const postgresMembership = await postgresMembershipSetFingerprint(sql)
  const membershipSetMismatch =
    d1Membership.count === postgresMembership.count &&
    d1Membership.digest === postgresMembership.digest
      ? undefined
      : { d1: d1Membership, postgres: postgresMembership }

  const messageChainMismatches: Array<
    KhalaCodeProductStateVerifyReport["messageChainMismatches"][number]
  > = []
  for (const table of ["team_chat_messages", "thread_messages"] as const) {
    const d1Rows =
      table === "team_chat_messages"
        ? d1Query(
            options,
            `SELECT id, team_id, project_id, autopilot_thread_id, created_at
               FROM team_chat_messages
              WHERE deleted_at IS NULL AND archived_at IS NULL
              ORDER BY team_id ASC, project_id ASC, autopilot_thread_id ASC, created_at ASC, id ASC`,
          )
        : d1Query(
            options,
            `SELECT id, thread_id, created_at, version
               FROM thread_messages
              WHERE deleted_at IS NULL AND archived_at IS NULL
              ORDER BY thread_id ASC, created_at ASC, id ASC`,
          )
    const mismatches = compareMessageChainFingerprints(
      messageChainFingerprintsFromRows(table, d1Rows),
      await postgresMessageChainFingerprints(sql, table),
    )
    if (mismatches.length > 0) {
      messageChainMismatches.push({ mismatches, table })
    }
  }

  return {
    countMismatches,
    membershipSetMismatch,
    messageChainMismatches,
    newestHashMismatches,
  }
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
    options.table === undefined
      ? KHALA_CODE_PRODUCT_STATE_TABLES
      : [options.table]

  const sql = new SQL(options.databaseUrl, {
    max: 1,
  }) as unknown as SyncSql & { end: () => Promise<void> }
  try {
    if (options.verify) {
      const report = await buildVerifyReport(sql, options, tables)
      console.log(JSON.stringify(report, null, 2))
      if (!khalaCodeProductStateVerifyReportOk(report)) {
        process.exitCode = 1
      }
      return
    }

    const state = loadState(options)
    for (const table of tables) {
      if (KHALA_CODE_PRODUCT_STATE_TABLE_SPECS[table] === undefined) {
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
