#!/usr/bin/env bun
/**
 * KS-8.10 remainder (#8338): forum remainder backfill CLI — D1 → Postgres.
 *
 * Reads D1 rows through `wrangler d1 execute <db> --remote --json` (the
 * KS-8 pattern: reuses the repo's existing wrangler auth — no new admin-API
 * surface) in bounded rowid-keyset pages, and converges them into the
 * Postgres twins from khala-sync migration `0027_forum_remainder.sql` over
 * a DIRECT connection (never Hyperdrive). Every table converges on its PK
 * to the D1 snapshot value — idempotent; a re-run converges to the same
 * state, and the runbook's sequence runs the backfill TWICE (the second
 * sweep is the catch-up pass after dual-write has been on across the whole
 * window).
 *
 * The rowid cursor persists in a local state file after EVERY page, so an
 * interrupted sweep resumes exactly where it stopped. Delete the state file
 * (or pass --restart) to sweep from the beginning again.
 *
 * Verify mode (`--verify`): exact row counts, per-table domain tallies,
 * newest-N full row hashes, plus two whole-domain checks —
 *   - TRUST RECOMPUTE-AND-COMPARE: `forum_trust_edges` grouped aggregate
 *     (per target_actor / forum / kind: count + weight sum) computed
 *     identically on both stores and compared for equality;
 *   - WORK-REQUEST SET-MEMBERSHIP: within-store referential orphan counts
 *     (both stores must be 0 and equal) and cross-store equality of the
 *     distinct cross-domain reference sets (escrow_id, reserve_receipt_ref,
 *     quote_ref, receipt_ref) into KS-8.1 assignments / KS-8.8 tips.
 * Exits non-zero on ANY mismatch. PRIVACY: output references row keys and
 * sha256 hashes only — never message subjects, participants, or content.
 *
 * Usage (from packages/khala-sync-server/):
 *   bun scripts/backfill-forum-remainder.ts \
 *     [--database-url <postgres-url>]   # default $KHALA_SYNC_DATABASE_URL
 *     [--d1-database <name>]            # default openagents-autopilot
 *     [--wrangler-cwd <dir>]            # default ../../apps/openagents.com/workers/api
 *     [--batch-size <n>]                # default 200
 *     [--state-file <path>]             # default .forum-remainder-backfill-state.json
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
  buildForumRemainderVerifyReport,
  compareTrustEdgeAggregates,
  d1ForumRemainderNewestHashes,
  FORUM_REMAINDER_SCALAR_TALLIES,
  FORUM_REMAINDER_TABLE_ORDER,
  FORUM_REMAINDER_TABLE_PK,
  FORUM_REMAINDER_TABLES,
  FORUM_WORK_REQUEST_CROSS_DOMAIN_REF_SETS,
  FORUM_WORK_REQUEST_REFERENTIAL_CHECKS,
  forumRemainderVerifyReportClean,
  postgresForumRemainderNewestHashes,
  postgresForumRemainderRowCount,
  postgresForumRemainderScalar,
  postgresRefSetDigest,
  postgresScalarValue,
  postgresTrustEdgeAggregate,
  refSetDigest,
  trustEdgeAggregateFromRows,
  trustEdgeRecomputeSql,
  upsertForumRemainderRows,
  type D1RemainderRow,
  type ForumRemainderTable,
  type ForumRemainderVerifyReport,
} from "../src/forum-remainder-backfill.js"
import type { SyncSql } from "../src/sql.js"

const USAGE = `Usage: bun scripts/backfill-forum-remainder.ts [options]   (see file header)`

type Options = {
  batchSize: number
  d1Database: string
  databaseUrl: string | undefined
  local: boolean
  restart: boolean
  stateFile: string
  table: ForumRemainderTable | undefined
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
    stateFile: ".forum-remainder-backfill-state.json",
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
      if (!FORUM_REMAINDER_TABLES.includes(table as ForumRemainderTable)) {
        console.error(`error: unknown table ${table}`)
        return undefined
      }
      options.table = table as ForumRemainderTable
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

const d1Scalar = (options: Options, command: string): number =>
  Number(d1Query(options, command)[0]?.["value"] ?? 0)

// ---------------------------------------------------------------------------
// Cursor state
// ---------------------------------------------------------------------------

type CursorState = Partial<Record<ForumRemainderTable, number>>

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
  table: ForumRemainderTable,
): Promise<void> => {
  const totalRows = d1Scalar(
    options,
    `SELECT COUNT(*) AS value FROM ${table}`,
  )

  let cursor = state[table] ?? 0
  let scanned = 0
  let touched = 0
  const startedAtMs = Date.now()
  for (;;) {
    const rows = d1Query(
      options,
      `SELECT rowid AS d1_rowid, * FROM ${table} WHERE rowid > ${cursor} ORDER BY rowid ASC LIMIT ${options.batchSize}`,
    ) as Array<D1RemainderRow & { d1_rowid: number }>
    if (rows.length === 0) break
    touched += await upsertForumRemainderRows(sql, table, rows)
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
  report: ForumRemainderVerifyReport,
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
  return forumRemainderVerifyReportClean(report)
}

const d1NewestRows = (
  options: Options,
  table: ForumRemainderTable,
): Array<Record<string, unknown>> => {
  const orderColumn = FORUM_REMAINDER_TABLE_ORDER[table]
  const pk = FORUM_REMAINDER_TABLE_PK[table]
  return d1Query(
    options,
    `SELECT * FROM ${table} ORDER BY ${orderColumn} DESC, ${pk} DESC LIMIT ${options.verifyNewest}`,
  )
}

const verifyTable = async (
  sql: SyncSql,
  options: Options,
  table: ForumRemainderTable,
): Promise<boolean> => {
  const d1Total = d1Scalar(options, `SELECT COUNT(*) AS value FROM ${table}`)
  const postgresTotal = await postgresForumRemainderRowCount(sql, table)

  const scalars: Array<{ metric: string; d1: number; postgres: number }> = []
  for (const tally of FORUM_REMAINDER_SCALAR_TALLIES[table]) {
    scalars.push({
      d1: d1Scalar(options, tally.sql),
      metric: tally.metric,
      postgres: await postgresForumRemainderScalar(sql, tally.sql),
    })
  }

  const report = buildForumRemainderVerifyReport({
    d1Newest: d1ForumRemainderNewestHashes(
      table,
      d1NewestRows(options, table),
    ),
    d1Total,
    postgresNewest: await postgresForumRemainderNewestHashes(
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

/**
 * Trust recompute-and-compare: grouped edge aggregate on both stores.
 */
const verifyTrustRecompute = async (
  sql: SyncSql,
  options: Options,
): Promise<boolean> => {
  const d1 = trustEdgeAggregateFromRows(
    d1Query(options, trustEdgeRecomputeSql()),
  )
  const postgres = await postgresTrustEdgeAggregate(sql)
  const mismatches = compareTrustEdgeAggregates(d1, postgres)
  console.log(`\n== trust recompute (${d1.length} edge groups) ==`)
  console.log(
    mismatches.length === 0
      ? "  trust-edge aggregates match across stores"
      : `  ${mismatches.length} TRUST-RECOMPUTE MISMATCH(ES)`,
  )
  for (const mismatch of mismatches.slice(0, 20)) {
    console.log(
      `    ${mismatch.key}: d1=${mismatch.d1 ? `${mismatch.d1.edgeCount}/${mismatch.d1.weightSum}` : "<missing>"} pg=${mismatch.postgres ? `${mismatch.postgres.edgeCount}/${mismatch.postgres.weightSum}` : "<missing>"}`,
    )
  }
  return mismatches.length === 0
}

/**
 * Work-request set-membership: within-store orphan counts (both 0 + equal)
 * and cross-store equality of the cross-domain reference sets.
 */
const verifyWorkRequestReferential = async (
  sql: SyncSql,
  options: Options,
): Promise<boolean> => {
  let clean = true
  console.log(`\n== work-request set-membership referential checks ==`)
  for (const check of FORUM_WORK_REQUEST_REFERENTIAL_CHECKS) {
    const d1Orphans = d1Scalar(options, check.sql)
    const pgOrphans = await postgresScalarValue(sql, check.sql)
    const ok = d1Orphans === 0 && pgOrphans === 0 && d1Orphans === pgOrphans
    if (!ok) clean = false
    console.log(
      `  ${check.name}: d1_orphans=${d1Orphans} pg_orphans=${pgOrphans}${ok ? "" : "  ORPHAN/MISMATCH"}`,
    )
  }
  for (const set of FORUM_WORK_REQUEST_CROSS_DOMAIN_REF_SETS) {
    const d1Digest = refSetDigest(d1Query(options, set.sql))
    const pgDigest = await postgresRefSetDigest(sql, set.sql)
    const ok = d1Digest === pgDigest
    if (!ok) clean = false
    console.log(
      `  ref-set ${set.name}: ${ok ? `match (${d1Digest.split(":")[0]} ids)` : `MISMATCH d1=${d1Digest} pg=${pgDigest}`}`,
    )
  }
  return clean
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
    options.table === undefined ? FORUM_REMAINDER_TABLES : [options.table]
  try {
    if (options.verify) {
      let allGood = true
      for (const table of tables) {
        allGood = (await verifyTable(sql, options, table)) && allGood
      }
      if (options.table === undefined) {
        allGood = (await verifyTrustRecompute(sql, options)) && allGood
        allGood =
          (await verifyWorkRequestReferential(sql, options)) && allGood
      }
      console.log(
        allGood
          ? "\nVERIFY OK: exact counts, domain tallies, newest-N hashes, trust recomputation equality, and work-request set-membership referential checks match."
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
