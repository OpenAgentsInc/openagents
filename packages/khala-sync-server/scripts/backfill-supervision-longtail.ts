#!/usr/bin/env bun
/**
 * KS-8.17 (#8328): supervision long-tail (Adjutant / Omni / Autopilot / ops)
 * backfill CLI — D1 → Postgres.
 *
 * Reads D1 rows through `wrangler d1 execute <db> --remote --json` (the
 * KS-8.1/8.2/8.16 pattern: reuses the repo's existing wrangler auth — no new
 * admin-API surface) in bounded rowid-keyset pages, and converges them into
 * the Postgres twins from khala-sync migration `0024_supervision_longtail.sql`
 * over a DIRECT connection (never Hyperdrive). Every table converges on its
 * COMPOSITE PK to the D1 snapshot value — idempotent; a re-run converges to
 * the same state, and the runbook's sequence runs the backfill TWICE (the
 * second sweep is the catch-up pass after dual-write has been on across the
 * whole window).
 *
 * The rowid cursor persists in a local state file after EVERY page, so an
 * interrupted sweep resumes exactly where it stopped. Delete the state file
 * (or pass --restart) to sweep from the beginning again.
 *
 * Verify mode (`--verify`): exact row counts, per-table domain scalar tallies,
 * IDEMPOTENCY KEY SET EQUALITY (`omni_idempotency_keys`), PUBLIC PROOF-BUNDLE
 * DIGESTS (`omni_public_proof_bundles`), and newest-N full row hashes per
 * table. Exits non-zero on ANY mismatch.
 *
 * SECRETS (SPEC invariant 9): output references row KEYS and sha256 hashes
 * only — never a custody column value (transcript/metadata/entries/result/
 * receipt JSON).
 *
 * Usage (from packages/khala-sync-server/):
 *   bun scripts/backfill-supervision-longtail.ts \
 *     [--database-url <postgres-url>]   # default $KHALA_SYNC_DATABASE_URL
 *     [--d1-database <name>]            # default openagents-autopilot
 *     [--wrangler-cwd <dir>]            # default ../../apps/openagents.com/workers/api
 *     [--batch-size <n>]                # default 400
 *     [--state-file <path>]             # default .supervision-longtail-backfill-state.json
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
  buildSupervisionLongtailVerifyReport,
  compareIdempotencyKeySets,
  compareProofBundleDigests,
  d1SupervisionLongtailNewestHashes,
  idempotencyKeySetFromRows,
  omniIdempotencyKeySql,
  postgresOmniIdempotencyKeySet,
  postgresProofBundleDigest,
  postgresSupervisionLongtailNewestHashes,
  postgresSupervisionLongtailRowCount,
  postgresSupervisionLongtailScalar,
  proofBundleDigestFromRows,
  proofBundleProjectionSql,
  SUPERVISION_LONGTAIL_SCALAR_TALLIES,
  SUPERVISION_LONGTAIL_TABLES,
  supervisionLongtailNewestOrderSql,
  supervisionLongtailVerifyReportClean,
  upsertSupervisionLongtailRows,
  type D1SupervisionLongtailSourceRow,
  type SupervisionLongtailTable,
  type SupervisionLongtailVerifyReport,
} from "../src/supervision-longtail-backfill.js"
import type { SyncSql } from "../src/sql.js"

const USAGE = `Usage: bun scripts/backfill-supervision-longtail.ts [options]   (see file header)`

type Options = {
  batchSize: number
  d1Database: string
  databaseUrl: string | undefined
  local: boolean
  restart: boolean
  stateFile: string
  table: SupervisionLongtailTable | undefined
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
    stateFile: ".supervision-longtail-backfill-state.json",
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
        !SUPERVISION_LONGTAIL_TABLES.includes(
          table as SupervisionLongtailTable,
        )
      ) {
        console.error(`error: unknown table ${table}`)
        return undefined
      }
      options.table = table as SupervisionLongtailTable
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

// ---------------------------------------------------------------------------
// Cursor state
// ---------------------------------------------------------------------------

type CursorState = Partial<Record<SupervisionLongtailTable, number>>

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
  table: SupervisionLongtailTable,
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
    ) as Array<D1SupervisionLongtailSourceRow & { d1_rowid: number }>
    if (rows.length === 0) break
    touched += await upsertSupervisionLongtailRows(sql, table, rows)
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
  report: SupervisionLongtailVerifyReport,
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
  return supervisionLongtailVerifyReportClean(report)
}

const verifyTable = async (
  sql: SyncSql,
  options: Options,
  table: SupervisionLongtailTable,
): Promise<boolean> => {
  const d1Total = Number(
    d1Query(options, `SELECT COUNT(*) AS total_rows FROM ${table}`)[0]?.[
      "total_rows"
    ] ?? 0,
  )
  const postgresTotal = await postgresSupervisionLongtailRowCount(sql, table)

  const scalars: Array<{ metric: string; d1: number; postgres: number }> = []
  for (const tally of SUPERVISION_LONGTAIL_SCALAR_TALLIES[table] ?? []) {
    scalars.push({
      d1: Number(d1Query(options, tally.sql)[0]?.["value"] ?? 0),
      metric: tally.metric,
      postgres: await postgresSupervisionLongtailScalar(sql, tally.sql),
    })
  }

  const report = buildSupervisionLongtailVerifyReport({
    d1Newest: d1SupervisionLongtailNewestHashes(
      table,
      d1Query(
        options,
        `SELECT * FROM ${table} ORDER BY ${supervisionLongtailNewestOrderSql(table)} LIMIT ${options.verifyNewest}`,
      ),
    ),
    d1Total,
    postgresNewest: await postgresSupervisionLongtailNewestHashes(
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

const verifyIdempotencyKeys = async (
  sql: SyncSql,
  options: Options,
): Promise<boolean> => {
  const d1 = idempotencyKeySetFromRows(d1Query(options, omniIdempotencyKeySql()))
  const postgres = await postgresOmniIdempotencyKeySet(sql)
  const comparison = compareIdempotencyKeySets(d1, postgres)
  console.log(`\n== omni_idempotency_keys — key set equality ==`)
  console.log(
    `  keys: d1=${comparison.d1Count} postgres=${comparison.postgresCount}; ${
      comparison.equal ? "SETS EQUAL" : "SETS DIFFER"
    }`,
  )
  if (comparison.missingInPostgres.length > 0) {
    console.log(
      `  missing in postgres (${comparison.missingInPostgres.length}): ${comparison.missingInPostgres.slice(0, 20).join(", ")}`,
    )
  }
  if (comparison.extraInPostgres.length > 0) {
    console.log(
      `  extra in postgres (${comparison.extraInPostgres.length}): ${comparison.extraInPostgres.slice(0, 20).join(", ")}`,
    )
  }
  return comparison.equal
}

const verifyProofBundles = async (
  sql: SyncSql,
  options: Options,
): Promise<boolean> => {
  const d1 = proofBundleDigestFromRows(
    d1Query(options, proofBundleProjectionSql()),
  )
  const postgres = await postgresProofBundleDigest(sql)
  const mismatches = compareProofBundleDigests(d1, postgres)
  console.log(
    `\n== omni_public_proof_bundles — public proof-bundle digests (per workroom) ==`,
  )
  console.log(
    `  workrooms: d1=${d1.size} postgres=${postgres.size}; ${
      mismatches.length === 0 ? "all digests match" : `${mismatches.length} MISMATCH(ES)`
    }`,
  )
  for (const mismatch of mismatches.slice(0, 20)) {
    const shape = (digest: typeof mismatch.d1) =>
      digest === undefined
        ? "<missing>"
        : `bundles=${digest.bundles} ${digest.digest.slice(0, 16)}`
    console.log(
      `    ${mismatch.workroom}: d1 ${shape(mismatch.d1)} | pg ${shape(mismatch.postgres)}`,
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
      "error: no Postgres URL (pass --database-url or set KHALA_SYNC_DATABASE_URL)",
    )
    return 2
  }

  const sql = new SQL(options.databaseUrl) as unknown as SyncSql
  const tables =
    options.table === undefined
      ? SUPERVISION_LONGTAIL_TABLES
      : [options.table]

  try {
    if (options.verify) {
      let clean = true
      for (const table of tables) {
        clean = (await verifyTable(sql, options, table)) && clean
      }
      if (options.table === undefined) {
        clean = (await verifyIdempotencyKeys(sql, options)) && clean
        clean = (await verifyProofBundles(sql, options)) && clean
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
