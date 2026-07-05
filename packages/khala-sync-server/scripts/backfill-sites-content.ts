#!/usr/bin/env bun
/**
 * KS-8.12 (#8323): sites content backfill CLI — D1 → Postgres.
 *
 * Reads D1 rows through `wrangler d1 execute <db> --remote --json` (the
 * KS-8.1/8.2/8.5/8.10 pattern: reuses the repo's existing wrangler auth —
 * no new admin-API surface) in bounded rowid-keyset pages, and converges
 * them into the Postgres twins from khala-sync migrations
 * `0020_sites_core.sql` (core) + `0025_sites_remainder.sql` (the KS-8.12
 * remainder, #8357: satellites, secret-safe `site_environment_values`,
 * commerce/money, targeted sites, hostnames, legacy deployments) over a
 * DIRECT connection (never Hyperdrive). Default table set is the full
 * core+remainder `ALL_SITES_CONTENT_TABLES`.
 * Every table converges on its PK to the D1 snapshot value — idempotent;
 * a re-run converges to the same state, and the runbook's sequence runs
 * the backfill TWICE (the second sweep is the catch-up pass after
 * dual-write has been on across the whole window).
 *
 * `site_builder_messages` / `site_builder_file_snapshots` /
 * `site_versions` carry the largest rows (message bodies, 4000-char
 * preview text, static-asset manifests) — pages are kept small by
 * default and the rowid cursor persists in a local state file after
 * EVERY page, so an interrupted sweep resumes exactly where it stopped.
 * Delete the state file (or pass --restart) to sweep from the beginning
 * again.
 *
 * Verify mode (`--verify`): exact row counts, per-table domain tallies
 * (project/deployment status tallies, builder sequence sums, snapshot
 * byte totals), PER-PROJECT VERSION CHAINS (count / distinct ids /
 * min / max created_at per site), the DEPLOYMENT STATE-MACHINE census
 * (per-site per-status counts — the state-machine equality acceptance),
 * BUILDER SEQUENCE CHAINS per session (messages / events / phase runs),
 * and newest-N full row hashes per table. Exits non-zero on ANY
 * mismatch. Output references row keys and sha256 hashes only — never
 * prompts, bodies, or preview text.
 *
 * Usage (from packages/khala-sync-server/):
 *   bun scripts/backfill-sites-content.ts \
 *     [--database-url <postgres-url>]   # default $KHALA_SYNC_DATABASE_URL
 *     [--d1-database <name>]            # default openagents-autopilot
 *     [--wrangler-cwd <dir>]            # default ../../apps/openagents.com/workers/api
 *     [--batch-size <n>]                # default 200
 *     [--state-file <path>]             # default .sites-content-backfill-state.json
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
  ALL_SITES_CONTENT_TABLES,
  buildSitesContentVerifyReport,
  d1SitesContentNewestHashes,
  deploymentStateRowsFromRaw,
  deploymentStateSql,
  groupChainRowsFromRaw,
  missingReferences,
  postgresDeploymentStates,
  postgresDistinctColumn,
  postgresGroupChains,
  postgresSitesContentNewestHashes,
  postgresSitesContentRowCount,
  postgresSitesContentScalar,
  SITES_CONTENT_CHAINS,
  SITES_CONTENT_SCALAR_TALLIES,
  SITES_CONTENT_TABLE_ORDER,
  SITES_CONTENT_TABLE_PK,
  SITES_REMAINDER_REFERENTIAL_CHECKS,
  sitesContentVerifyReportClean,
  upsertSitesContentRows,
  type D1SourceRow,
  type SitesContentTable,
  type SitesContentVerifyReport,
} from "../src/sites-content-backfill.js"
import type { SyncSql } from "../src/sql.js"

const USAGE = `Usage: bun scripts/backfill-sites-content.ts [options]   (see file header)`

type Options = {
  batchSize: number
  d1Database: string
  databaseUrl: string | undefined
  local: boolean
  restart: boolean
  stateFile: string
  table: SitesContentTable | undefined
  verify: boolean
  verifyNewest: number
  wranglerCwd: string
}

const parseArgs = (argv: ReadonlyArray<string>): Options | undefined => {
  const options: Options = {
    // builder message bodies / snapshot preview text / version manifests
    // are the biggest rows — keep pages small enough for wrangler's JSON
    // output.
    batchSize: 200,
    d1Database: "openagents-autopilot",
    databaseUrl: process.env["KHALA_SYNC_DATABASE_URL"],
    local: false,
    restart: false,
    stateFile: ".sites-content-backfill-state.json",
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
      if (!ALL_SITES_CONTENT_TABLES.includes(table as SitesContentTable)) {
        console.error(`error: unknown table ${table}`)
        return undefined
      }
      options.table = table as SitesContentTable
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
    options.batchSize > 1200
  ) {
    // 1200 × 20 columns max = 24,000 bind params — well under the cap.
    console.error("error: --batch-size must be 1..1200")
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

type CursorState = Partial<Record<SitesContentTable, number>>

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
  table: SitesContentTable,
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
    touched += await upsertSitesContentRows(sql, table, rows)
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
  report: SitesContentVerifyReport,
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
    console.log(`  CHAIN MISMATCHES: ${report.chainMismatches.length}`)
    for (const mismatch of report.chainMismatches.slice(0, 20)) {
      const shape = (chain: typeof mismatch.d1) =>
        chain === undefined
          ? "<missing>"
          : `members=${chain.members} distinct=${chain.distinctOrders} min=${chain.minOrder} max=${chain.maxOrder}`
      console.log(
        `    ${mismatch.groupKey}: d1 ${shape(mismatch.d1)} | pg ${shape(mismatch.postgres)}`,
      )
    }
  }
  if (report.stateMismatches.length > 0) {
    console.log(
      `  DEPLOYMENT-STATE MISMATCHES: ${report.stateMismatches.length}`,
    )
    for (const mismatch of report.stateMismatches.slice(0, 20)) {
      console.log(
        `    ${mismatch.siteId} ${mismatch.status}: d1=${mismatch.d1 ?? "<missing>"} pg=${mismatch.postgres ?? "<missing>"}`,
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
  return sitesContentVerifyReportClean(report)
}

const d1NewestRows = (
  options: Options,
  table: SitesContentTable,
): Array<Record<string, unknown>> => {
  const orderColumn = SITES_CONTENT_TABLE_ORDER[table]
  const pk = SITES_CONTENT_TABLE_PK[table]
  return d1Query(
    options,
    `SELECT * FROM ${table} ORDER BY ${orderColumn} DESC, ${pk} DESC LIMIT ${options.verifyNewest}`,
  )
}

const verifyTable = async (
  sql: SyncSql,
  options: Options,
  table: SitesContentTable,
): Promise<boolean> => {
  const d1Total = Number(
    d1Query(options, `SELECT COUNT(*) AS total_rows FROM ${table}`)[0]?.[
      "total_rows"
    ] ?? 0,
  )
  const postgresTotal = await postgresSitesContentRowCount(sql, table)

  const scalars: Array<{ metric: string; d1: number; postgres: number }> = []
  for (const tally of SITES_CONTENT_SCALAR_TALLIES[table]) {
    scalars.push({
      d1: Number(d1Query(options, tally.sql)[0]?.["value"] ?? 0),
      metric: tally.metric,
      postgres: await postgresSitesContentScalar(sql, tally.sql),
    })
  }

  const chain = SITES_CONTENT_CHAINS.find((entry) => entry.table === table)
  const d1Chains =
    chain === undefined
      ? undefined
      : groupChainRowsFromRaw(d1Query(options, chain.sql))
  const postgresChains =
    chain === undefined ? undefined : await postgresGroupChains(sql, chain.sql)

  const isStateTable = table === "site_deployments"
  const d1States = isStateTable
    ? deploymentStateRowsFromRaw(d1Query(options, deploymentStateSql()))
    : undefined
  const postgresStates = isStateTable
    ? await postgresDeploymentStates(sql)
    : undefined

  const report = buildSitesContentVerifyReport({
    d1Chains,
    d1Newest: d1SitesContentNewestHashes(table, d1NewestRows(options, table)),
    d1States,
    d1Total,
    postgresChains,
    postgresNewest: await postgresSitesContentNewestHashes(
      sql,
      table,
      options.verifyNewest,
    ),
    postgresStates,
    postgresTotal,
    scalars,
    table,
  })
  return printReport(report, options.verifyNewest)
}

const verifyReferentialChecks = async (
  sql: SyncSql,
  options: Options,
): Promise<boolean> => {
  console.log(`\n== referential set-membership (money / referral) ==`)
  let allGood = true
  for (const check of SITES_REMAINDER_REFERENTIAL_CHECKS) {
    // D1 side: child column values vs parent ids.
    const d1Child = d1Query(
      options,
      `SELECT DISTINCT ${check.childColumn} AS value FROM ${check.childTable} WHERE ${check.childColumn} IS NOT NULL`,
    ).map((row) => row["value"])
    const d1Parent = d1Query(
      options,
      `SELECT ${check.parentColumn} AS value FROM ${check.parentTable}`,
    ).map((row) => row["value"])
    const d1Missing = missingReferences(d1Child, d1Parent)

    // Postgres side: same relation must hold on the twin.
    const pgChild = await postgresDistinctColumn(
      sql,
      check.childTable,
      check.childColumn,
    )
    const pgParent = await postgresDistinctColumn(
      sql,
      check.parentTable,
      check.parentColumn,
    )
    const pgMissing = missingReferences([...pgChild], [...pgParent])

    const ok = d1Missing.length === 0 && pgMissing.length === 0
    allGood = allGood && ok
    console.log(
      `  ${check.name}: ${
        ok
          ? "ok"
          : `MISSING d1=${d1Missing.length} postgres=${pgMissing.length}`
      }`,
    )
    for (const key of [...d1Missing, ...pgMissing].slice(0, 10)) {
      console.log(`    orphan key: ${key}`)
    }
  }
  return allGood
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
    options.table === undefined ? ALL_SITES_CONTENT_TABLES : [options.table]
  try {
    if (options.verify) {
      let allGood = true
      for (const table of tables) {
        allGood = (await verifyTable(sql, options, table)) && allGood
      }
      // Money/referral set-membership: child.column values must be a subset
      // of parent ids WITHIN each store (no cross-store joins). Only run when
      // the whole table set is in scope (a single --table run skips it).
      if (options.table === undefined) {
        allGood = (await verifyReferentialChecks(sql, options)) && allGood
      }
      console.log(
        allGood
          ? "\nVERIFY OK: exact counts, domain tallies (incl. commerce totals), version chains, deployment states, builder sequence chains, referential set-membership, and newest-N hashes match."
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
