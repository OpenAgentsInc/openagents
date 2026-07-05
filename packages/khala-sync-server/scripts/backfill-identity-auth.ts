#!/usr/bin/env bun
/**
 * KS-8.18 (#8329): Identity and auth core backfill CLI — D1 → Postgres.
 * The LAST and most sensitive KS-8 domain.
 *
 * Reads D1 rows through `wrangler d1 execute <db> --remote --json` (the
 * KS-8.1/8.2/8.16 pattern: reuses the repo's existing wrangler auth — no
 * new admin-API surface) in bounded rowid-keyset pages, and converges them
 * into the Postgres twins from khala-sync migration
 * `0028_identity_auth_domain.sql` over a DIRECT connection (never
 * Hyperdrive). Every table converges on its PRIMARY KEY to the D1 snapshot
 * value — idempotent; a re-run converges to the same state, and the
 * runbook's sequence runs the backfill TWICE (the second sweep is the
 * catch-up pass after dual-write has been on across the whole window).
 *
 * The rowid cursor persists in a local state file after EVERY page, so an
 * interrupted sweep resumes exactly where it stopped. Delete the state
 * file (or pass --restart) to sweep from the beginning again.
 *
 * Verify mode (`--verify`): exact row counts (identity SET EQUALITY over
 * users/auth_identities), per-table CUSTODY-SAFE domain scalar tallies
 * (status/health/kind counts; NO ciphertext / value_json / user_code /
 * state column is ever selected), and newest-N full row hashes per table.
 * Exits non-zero on ANY mismatch.
 *
 * SECRETS (SPEC invariant 9): output references row KEYS (ids / refs /
 * owner_user_id) and sha256 hashes only — NEVER token ciphertext, session
 * payloads, device codes, OAuth state nonces, or any custody column value.
 *
 * Usage (from packages/khala-sync-server/):
 *   bun scripts/backfill-identity-auth.ts \
 *     [--database-url <postgres-url>]   # default $KHALA_SYNC_DATABASE_URL
 *     [--d1-database <name>]            # default openagents-autopilot
 *     [--wrangler-cwd <dir>]            # default ../../apps/openagents.com/workers/api
 *     [--batch-size <n>]                # default 400
 *     [--state-file <path>]             # default .identity-auth-backfill-state.json
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
  buildIdentityAuthVerifyReport,
  d1IdentityAuthNewestHashes,
  IDENTITY_AUTH_DOMAIN_TABLES,
  IDENTITY_AUTH_SCALAR_TALLIES,
  identityAuthNewestOrderSql,
  identityAuthVerifyReportClean,
  postgresIdentityAuthNewestHashes,
  postgresIdentityAuthRowCount,
  postgresIdentityAuthScalar,
  upsertIdentityAuthRows,
  type D1IdentityAuthSourceRow,
  type IdentityAuthDomainTable,
  type IdentityAuthVerifyReport,
} from "../src/identity-auth-backfill.js"
import type { SyncSql } from "../src/sql.js"

const USAGE = `Usage: bun scripts/backfill-identity-auth.ts [options]   (see file header)`

type Options = {
  batchSize: number
  d1Database: string
  databaseUrl: string | undefined
  local: boolean
  restart: boolean
  stateFile: string
  table: IdentityAuthDomainTable | undefined
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
    stateFile: ".identity-auth-backfill-state.json",
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
      if (!IDENTITY_AUTH_DOMAIN_TABLES.includes(table as IdentityAuthDomainTable)) {
        console.error(`error: unknown table ${table}`)
        return undefined
      }
      options.table = table as IdentityAuthDomainTable
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
    // 1500 × 35 columns max = 52,500 bind params per page across many
    // single-row statements — well under any driver cap.
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

type CursorState = Partial<Record<IdentityAuthDomainTable, number>>

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
  table: IdentityAuthDomainTable,
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
    ) as Array<D1IdentityAuthSourceRow & { d1_rowid: number }>
    if (rows.length === 0) break
    touched += await upsertIdentityAuthRows(sql, table, rows)
    scanned += rows.length
    cursor = Number(rows[rows.length - 1]?.d1_rowid ?? cursor)
    state[table] = cursor
    saveState(options, state)
    const elapsedS = Math.max(1, (Date.now() - startedAtMs) / 1000)
    const rate = Math.round(scanned / elapsedS)
    console.log(
      `${table}: page done (cursor rowid=${cursor}, scanned=${scanned}${totalRows > 0 ? `/${totalRows}` : ""}, converged=${touched}, ~${rate} rows/s)`,
    )
  }
  console.log(
    `${table}: complete — scanned ${scanned} row(s) this run, ${touched} converged`,
  )
}

// ---------------------------------------------------------------------------
// Verify
// ---------------------------------------------------------------------------

const printReport = (
  report: IdentityAuthVerifyReport,
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
  return identityAuthVerifyReportClean(report)
}

const verifyTable = async (
  sql: SyncSql,
  options: Options,
  table: IdentityAuthDomainTable,
): Promise<boolean> => {
  const d1Total = Number(
    d1Query(options, `SELECT COUNT(*) AS total_rows FROM ${table}`)[0]?.[
      "total_rows"
    ] ?? 0,
  )
  const postgresTotal = await postgresIdentityAuthRowCount(sql, table)

  const scalars: Array<{ metric: string; d1: number; postgres: number }> = []
  for (const tally of IDENTITY_AUTH_SCALAR_TALLIES[table]) {
    scalars.push({
      d1: Number(d1Query(options, tally.sql)[0]?.["value"] ?? 0),
      metric: tally.metric,
      postgres: await postgresIdentityAuthScalar(sql, tally.sql),
    })
  }

  const report = buildIdentityAuthVerifyReport({
    d1Newest: d1IdentityAuthNewestHashes(
      table,
      d1Query(
        options,
        `SELECT * FROM ${table} ORDER BY ${identityAuthNewestOrderSql(table)} LIMIT ${options.verifyNewest}`,
      ),
    ),
    d1Total,
    postgresNewest: await postgresIdentityAuthNewestHashes(
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
      "error: no Postgres URL (pass --database-url or set KHALA_SYNC_DATABASE_URL)",
    )
    return 2
  }

  const sql = new SQL(options.databaseUrl) as unknown as SyncSql
  const tables =
    options.table === undefined
      ? IDENTITY_AUTH_DOMAIN_TABLES
      : [options.table]

  try {
    if (options.verify) {
      let clean = true
      for (const table of tables) {
        clean = (await verifyTable(sql, options, table)) && clean
      }
      console.log(
        clean
          ? "\nverify: CLEAN — every check matches"
          : "\nverify: MISMATCHES FOUND (see above)",
      )
      return clean ? 0 : 1
    }

    const state = loadState(options)
    for (const table of tables) {
      await backfillTable(sql, options, state, table)
    }
    return 0
  } finally {
    await (sql as unknown as { end: () => Promise<void> }).end()
  }
}

process.exit(await main())
