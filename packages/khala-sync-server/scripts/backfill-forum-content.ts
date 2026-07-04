#!/usr/bin/env bun
/**
 * KS-8.10 (#8321): forum content backfill CLI — D1 → Postgres.
 *
 * Reads D1 rows through `wrangler d1 execute <db> --remote --json` (the
 * KS-8.1/8.2/8.5 pattern: reuses the repo's existing wrangler auth — no
 * new admin-API surface) in bounded rowid-keyset pages, and converges
 * them into the Postgres twins from khala-sync migration
 * `0014_forum_content.sql` over a DIRECT connection (never Hyperdrive).
 * Every table converges on its PK to the D1 snapshot value — idempotent;
 * a re-run converges to the same state, and the runbook's sequence runs
 * the backfill TWICE (the second sweep is the catch-up pass after
 * dual-write has been on across the whole window).
 *
 * `forum_post_bodies` is the long pole (big body_text rows) — pages are
 * kept small by default and the rowid cursor persists in a local state
 * file after EVERY page, so an interrupted sweep resumes exactly where it
 * stopped. Delete the state file (or pass --restart) to sweep from the
 * beginning again.
 *
 * Verify mode (`--verify`): exact row counts, per-table domain tallies
 * (forum/topic counter sums, post state tallies, body byte totals,
 * report/revision tallies), the PER-TOPIC POST-CHAIN comparison (count /
 * distinct / min / max post_number per topic — the thread-page shape),
 * newest-N full row hashes, and PER-THREAD SPOT HASHES over the N most
 * recently bumped topics (sha256 over each thread's ordered
 * (post_number, id, state, sha256(body)) chain — the post-body content
 * checksum acceptance). Exits non-zero on ANY mismatch. Output references
 * row keys and sha256 hashes only — never body text.
 *
 * Usage (from packages/khala-sync-server/):
 *   bun scripts/backfill-forum-content.ts \
 *     [--database-url <postgres-url>]   # default $KHALA_SYNC_DATABASE_URL
 *     [--d1-database <name>]            # default openagents-autopilot
 *     [--wrangler-cwd <dir>]            # default ../../apps/openagents.com/workers/api
 *     [--batch-size <n>]                # default 200
 *     [--state-file <path>]             # default .forum-content-backfill-state.json
 *     [--table <name>]                  # limit to one table
 *     [--restart]                       # ignore saved cursor
 *     [--local]                         # wrangler --local (dev smoke)
 *     [--verify] [--verify-newest <n>]  # verify mode (default N=50)
 *     [--verify-threads <n>]            # sampled thread spot hashes (default 25)
 */
import { spawnSync } from "node:child_process"
import { existsSync, readFileSync, writeFileSync } from "node:fs"
import * as path from "node:path"
import { SQL } from "bun"
import {
  buildForumContentVerifyReport,
  d1ForumContentNewestHashes,
  FORUM_CONTENT_SCALAR_TALLIES,
  FORUM_CONTENT_TABLE_ORDER,
  FORUM_CONTENT_TABLE_PK,
  FORUM_CONTENT_TABLES,
  forumContentVerifyReportClean,
  postChainSql,
  postChainTallyFromRows,
  postgresForumContentNewestHashes,
  postgresForumContentRowCount,
  postgresForumContentScalar,
  postgresPostChainTally,
  postgresThreadSpotHash,
  THREAD_SPOT_HASH_SQL_D1,
  threadSpotHashFromRows,
  upsertForumContentRows,
  type D1SourceRow,
  type ForumContentTable,
  type ForumContentVerifyReport,
  type ThreadSpotHashMismatch,
} from "../src/forum-content-backfill.js"
import type { SyncSql } from "../src/sql.js"

const USAGE = `Usage: bun scripts/backfill-forum-content.ts [options]   (see file header)`

type Options = {
  batchSize: number
  d1Database: string
  databaseUrl: string | undefined
  local: boolean
  restart: boolean
  stateFile: string
  table: ForumContentTable | undefined
  verify: boolean
  verifyNewest: number
  verifyThreads: number
  wranglerCwd: string
}

const parseArgs = (argv: ReadonlyArray<string>): Options | undefined => {
  const options: Options = {
    // forum_post_bodies rows carry full post bodies — keep pages small
    // enough for wrangler's JSON output.
    batchSize: 200,
    d1Database: "openagents-autopilot",
    databaseUrl: process.env["KHALA_SYNC_DATABASE_URL"],
    local: false,
    restart: false,
    stateFile: ".forum-content-backfill-state.json",
    table: undefined,
    verify: false,
    verifyNewest: 50,
    verifyThreads: 25,
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
      if (!FORUM_CONTENT_TABLES.includes(table as ForumContentTable)) {
        console.error(`error: unknown table ${table}`)
        return undefined
      }
      options.table = table as ForumContentTable
    } else if (arg === "--restart") options.restart = true
    else if (arg === "--local") options.local = true
    else if (arg === "--verify") options.verify = true
    else if (arg === "--verify-newest") options.verifyNewest = Number(next())
    else if (arg === "--verify-threads") options.verifyThreads = Number(next())
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
    // 1500 × 17 columns max = 25,500 bind params — well under the cap.
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

type CursorState = Partial<Record<ForumContentTable, number>>

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
  table: ForumContentTable,
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
    touched += await upsertForumContentRows(sql, table, rows)
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
  report: ForumContentVerifyReport,
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
    console.log(`  POST-CHAIN MISMATCHES: ${report.chainMismatches.length}`)
    for (const mismatch of report.chainMismatches.slice(0, 20)) {
      const shape = (chain: typeof mismatch.d1) =>
        chain === undefined
          ? "<missing>"
          : `posts=${chain.posts} distinct=${chain.distinctNumbers} min=${chain.minNumber} max=${chain.maxNumber}`
      console.log(
        `    ${mismatch.topicId}: d1 ${shape(mismatch.d1)} | pg ${shape(mismatch.postgres)}`,
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
  return forumContentVerifyReportClean(report)
}

const d1NewestRows = (
  options: Options,
  table: ForumContentTable,
): Array<Record<string, unknown>> => {
  const orderColumn = FORUM_CONTENT_TABLE_ORDER[table]
  const pk = FORUM_CONTENT_TABLE_PK[table]
  return d1Query(
    options,
    `SELECT * FROM ${table} ORDER BY ${orderColumn} DESC, ${pk} DESC LIMIT ${options.verifyNewest}`,
  )
}

const verifyTable = async (
  sql: SyncSql,
  options: Options,
  table: ForumContentTable,
): Promise<boolean> => {
  const d1Total = Number(
    d1Query(options, `SELECT COUNT(*) AS total_rows FROM ${table}`)[0]?.[
      "total_rows"
    ] ?? 0,
  )
  const postgresTotal = await postgresForumContentRowCount(sql, table)

  const scalars: Array<{ metric: string; d1: number; postgres: number }> = []
  for (const tally of FORUM_CONTENT_SCALAR_TALLIES[table]) {
    scalars.push({
      d1: Number(d1Query(options, tally.sql)[0]?.["value"] ?? 0),
      metric: tally.metric,
      postgres: await postgresForumContentScalar(sql, tally.sql),
    })
  }

  const isChainTable = table === "forum_posts"
  const d1Chains = isChainTable
    ? postChainTallyFromRows(d1Query(options, postChainSql()))
    : undefined
  const postgresChains = isChainTable
    ? await postgresPostChainTally(sql)
    : undefined

  const report = buildForumContentVerifyReport({
    d1Chains,
    d1Newest: d1ForumContentNewestHashes(table, d1NewestRows(options, table)),
    d1Total,
    postgresChains,
    postgresNewest: await postgresForumContentNewestHashes(
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
 * Sampled per-thread spot hashes: the N most recently bumped topics'
 * ordered (post_number, id, state, sha256(body)) chains, hashed on both
 * stores. This is the post-body content checksum the KS-8.10 acceptance
 * asks for — body bytes participate, output stays hashes-only.
 */
const verifyThreadSpotHashes = async (
  sql: SyncSql,
  options: Options,
): Promise<boolean> => {
  const topics = d1Query(
    options,
    `SELECT id FROM forum_topics ORDER BY updated_at DESC, id DESC LIMIT ${options.verifyThreads}`,
  )
  const mismatches: Array<ThreadSpotHashMismatch> = []
  for (const topic of topics) {
    const topicId = String(topic["id"] ?? "")
    if (topicId === "") continue
    const d1Rows = d1Query(
      options,
      THREAD_SPOT_HASH_SQL_D1.replace("?", `'${topicId.replaceAll("'", "''")}'`),
    )
    const d1Hash = threadSpotHashFromRows(d1Rows)
    const postgresHash = await postgresThreadSpotHash(sql, topicId)
    if (d1Hash !== postgresHash) {
      mismatches.push({ d1Hash, postgresHash, topicId })
    }
  }
  console.log(
    `\n== thread spot hashes (${topics.length} sampled topics) ==`,
  )
  console.log(
    mismatches.length === 0
      ? "  all sampled thread chains match"
      : `  ${mismatches.length} THREAD-CHAIN MISMATCH(ES)`,
  )
  for (const mismatch of mismatches) {
    console.log(
      `    ${mismatch.topicId}: d1=${mismatch.d1Hash.slice(0, 16)} postgres=${mismatch.postgresHash.slice(0, 16)}`,
    )
  }
  return mismatches.length === 0
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
    options.table === undefined ? FORUM_CONTENT_TABLES : [options.table]
  try {
    if (options.verify) {
      let allGood = true
      for (const table of tables) {
        allGood = (await verifyTable(sql, options, table)) && allGood
      }
      if (options.table === undefined || options.table === "forum_posts") {
        allGood = (await verifyThreadSpotHashes(sql, options)) && allGood
      }
      console.log(
        allGood
          ? "\nVERIFY OK: exact counts, domain tallies, post chains, thread spot hashes, and newest-N hashes match."
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
