#!/usr/bin/env bun
/**
 * KS-8.7 (#8318): billing / Stripe / pay-ins backfill CLI — D1 → Postgres.
 *
 * Reads D1 rows through `wrangler d1 execute <db> --remote --json` (the
 * KS-8.1/8.2 pattern: reuses the repo's existing wrangler auth) in bounded
 * rowid-keyset pages and CONVERGE-upserts them into the Postgres twins from
 * khala-sync migration 0015 over a DIRECT connection (never Hyperdrive).
 * Converge is the correct mode for this domain: several tables are UPDATEd
 * in place on D1 (webhook status, checkout fulfillment, pay-in states), so
 * a sweep must bring stale mirror rows forward to the D1 snapshot. Amounts
 * and idempotency keys are copied byte-exact and never recomputed.
 *
 * Resumable: progress (last rowid per table) persists in a local state
 * file; delete it (or pass --restart) to sweep from the beginning again.
 * The runbook's sequence runs the backfill TWICE — the second sweep
 * (--restart) is the catch-up pass after dual-write has been on across the
 * whole window; it also re-converges any rows UPDATEd on D1 after the
 * first sweep copied them.
 *
 * Verify mode (`--verify`) — the §3.4 money acceptance, exact or explain:
 *   - exact row counts per table
 *   - billing_ledger_entries: SUM(amount_cents) per (currency, source) AND
 *     the full PER-USER balance map (balance = SUM(ledger) to the cent)
 *   - pay_ins / pay_in_legs: msat sums per (type, state) / (direction, kind)
 *   - stripe_webhook_events: exact event-id SET equality (sorted digest)
 *   - buyer receipts/debits/challenges: minor-unit sums per (asset, status)
 *   - khala_code paid-plan intents: cents/sats sums per (rail, status)
 *   - newest-N row-hash comparison for every table
 * Exits non-zero on ANY mismatch.
 *
 * Usage (from packages/khala-sync-server/):
 *   bun scripts/backfill-billing.ts \
 *     [--database-url <postgres-url>]   # default $KHALA_SYNC_DATABASE_URL
 *     [--d1-database <name>]            # default openagents-autopilot
 *     [--wrangler-cwd <dir>]            # default ../../apps/openagents.com/workers/api
 *     [--batch-size <n>]                # default 400 (max 1500)
 *     [--state-file <path>]             # default .billing-backfill-state.json
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
  BILLING_DOMAIN_TABLES,
  BILLING_GROUPED_TALLIES,
  billingNewestHashesFromRows,
  billingNewestOrderSql,
  billingVerifyReportOk,
  compareGroupedTallies,
  compareNewestHashes,
  groupedTallyFromRows,
  groupedTallySql,
  keySetDigestFromKeys,
  postgresBillingNewestHashes,
  postgresBillingRowCount,
  postgresGroupedTally,
  postgresKeySetDigest,
  upsertBillingRows,
  type BillingDomainTable,
  type BillingVerifyReport,
  type D1SourceRow,
} from "../src/billing-backfill.js"
import { BILLING_DOMAIN_TABLE_SPECS } from "../src/billing-domain-tables.js"
import type { SyncSql } from "../src/sql.js"

const USAGE = `Usage: bun scripts/backfill-billing.ts [options]   (see file header)`

/** Tables whose full natural-key SET equality is part of the acceptance. */
const KEY_SET_TABLES: ReadonlySet<BillingDomainTable> = new Set([
  "stripe_webhook_events",
])

type Options = {
  batchSize: number
  d1Database: string
  databaseUrl: string | undefined
  local: boolean
  restart: boolean
  stateFile: string
  table: BillingDomainTable | undefined
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
    stateFile: ".billing-backfill-state.json",
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
      if (!BILLING_DOMAIN_TABLES.includes(table as BillingDomainTable)) {
        console.error(`error: unknown table ${table}`)
        return undefined
      }
      options.table = table as BillingDomainTable
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
    // 1500 × 22 columns (widest table) = 33,000 bind params — under 65,535.
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

type CursorState = Partial<Record<BillingDomainTable, number>>

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
  table: BillingDomainTable,
): Promise<void> => {
  const totalRow = d1Query(
    options,
    `SELECT COUNT(*) AS total_rows FROM ${table}`,
  )[0]
  const totalRows = Number(totalRow?.["total_rows"] ?? 0)

  let cursor = state[table] ?? 0
  let scanned = 0
  let converged = 0
  const startedAtMs = Date.now()
  for (;;) {
    const rows = d1Query(
      options,
      `SELECT rowid AS d1_rowid, * FROM ${table} WHERE rowid > ${cursor} ORDER BY rowid ASC LIMIT ${options.batchSize}`,
    ) as Array<D1SourceRow & { d1_rowid: number }>
    if (rows.length === 0) break
    converged += await upsertBillingRows(sql, table, rows)
    scanned += rows.length
    cursor = Number(rows[rows.length - 1]?.d1_rowid ?? cursor)
    state[table] = cursor
    saveState(options, state)
    const elapsedS = Math.max(1, (Date.now() - startedAtMs) / 1000)
    const rate = Math.round(scanned / elapsedS)
    console.log(
      `${table}: page done (cursor rowid=${cursor}, scanned=${scanned}${totalRows > 0 ? `/${totalRows}` : ""}, converged=${converged}, ~${rate} rows/s)`,
    )
  }
  console.log(
    `${table}: complete — scanned ${scanned} row(s) this run, ${converged} converged`,
  )
}

// ---------------------------------------------------------------------------
// Verify
// ---------------------------------------------------------------------------

const printReport = (
  report: BillingVerifyReport,
  newest: number,
): boolean => {
  console.log(`\n== ${report.table} ==`)
  console.log(
    `  rows: d1=${report.d1Total} postgres=${report.postgresTotal}${report.countsMatch ? "" : "  ROW-COUNT MISMATCH"}`,
  )
  if (report.keySetMatch !== undefined) {
    console.log(
      `  key-set digest: ${report.keySetMatch ? "identical sets" : "SET MISMATCH — the dedupe key sets differ"}`,
    )
  }
  for (const group of report.groupedMismatches) {
    if (group.mismatches.length === 0) {
      console.log(`  ${group.label}: exact match`)
      continue
    }
    console.log(`  ${group.label}: ${group.mismatches.length} MISMATCH(ES)`)
    for (const mismatch of group.mismatches.slice(0, 25)) {
      console.log(
        `    ${mismatch.groupKey}: rows d1=${mismatch.d1Rows}/pg=${mismatch.postgresRows} sums d1=[${mismatch.d1Sums.join(",")}] pg=[${mismatch.postgresSums.join(",")}]`,
      )
    }
    if (group.mismatches.length > 25) {
      console.log(`    ... and ${group.mismatches.length - 25} more`)
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
  return billingVerifyReportOk(report)
}

const verifyTable = async (
  sql: SyncSql,
  options: Options,
  table: BillingDomainTable,
): Promise<boolean> => {
  const d1Count = Number(
    d1Query(options, `SELECT COUNT(*) AS total_rows FROM ${table}`)[0]?.[
      "total_rows"
    ] ?? 0,
  )
  const postgresCount = await postgresBillingRowCount(sql, table)

  const d1Newest = billingNewestHashesFromRows(
    table,
    d1Query(
      options,
      `SELECT * FROM ${table} ORDER BY ${billingNewestOrderSql(table)} LIMIT ${options.verifyNewest}`,
    ),
  )
  const postgresNewest = await postgresBillingNewestHashes(
    sql,
    table,
    options.verifyNewest,
  )

  const groupedMismatches: Array<{
    label: string
    mismatches: ReturnType<typeof compareGroupedTallies>
  }> = []
  for (const spec of BILLING_GROUPED_TALLIES[table] ?? []) {
    const d1Tally = groupedTallyFromRows(
      spec,
      d1Query(options, groupedTallySql(table, spec)),
    )
    const postgresTally = await postgresGroupedTally(sql, table, spec)
    groupedMismatches.push({
      label: spec.label,
      mismatches: compareGroupedTallies(d1Tally, postgresTally),
    })
  }

  let keySetMatch: boolean | undefined
  if (KEY_SET_TABLES.has(table)) {
    const keyColumns = BILLING_DOMAIN_TABLE_SPECS[table].keyColumns
    const keyExpr = keyColumns
      .map((column) => `COALESCE(${column}, '<null>')`)
      .join(" || ':' || ")
    const d1Keys = d1Query(
      options,
      `SELECT ${keyExpr} AS row_key FROM ${table} ORDER BY row_key ASC`,
    ).map((row) => String(row["row_key"] ?? ""))
    const d1Digest = keySetDigestFromKeys(d1Keys)
    const postgresDigest = await postgresKeySetDigest(sql, table)
    keySetMatch =
      d1Digest.count === postgresDigest.count &&
      d1Digest.digest === postgresDigest.digest
  }

  const report: BillingVerifyReport = {
    countsMatch: d1Count === postgresCount,
    d1Total: d1Count,
    groupedMismatches,
    keySetMatch,
    newestHashMismatches: compareNewestHashes(d1Newest, postgresNewest),
    postgresTotal: postgresCount,
    table,
  }
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
  // CFG-4 (#8519) HARD CUTOVER GUARD: `pay_ins` / `pay_in_legs` are
  // Postgres-AUTHORITATIVE once the credits cutover deploy is live. This
  // script's converge upsert copies D1 -> Postgres and would CLOBBER newer
  // Postgres rows with stale D1 state after that deploy. They are therefore
  // excluded from the default all-tables sweep; converging them requires an
  // explicit `--table pay_ins` / `--table pay_in_legs`, which is ONLY valid
  // as the final pre-deploy catch-up sweep while the old D1-writing Worker
  // is still serving. NEVER run it after the cutover deploy.
  const creditsCutoverTables: ReadonlyArray<BillingDomainTable> = [
    "pay_ins",
    "pay_in_legs",
  ]
  const tables =
    options.table === undefined
      ? BILLING_DOMAIN_TABLES.filter(
          table => !creditsCutoverTables.includes(table),
        )
      : [options.table]
  try {
    if (options.verify) {
      let allGood = true
      for (const table of tables) {
        allGood = (await verifyTable(sql, options, table)) && allGood
      }
      console.log(
        allGood
          ? "\nVERIFY OK: exact counts, per-account balances, money sums, key sets, and newest-N hashes match."
          : "\nVERIFY FAILED: mismatches above — investigate before any read cutover. Money reads NEVER cut on a red verify.",
      )
      return allGood ? 0 : 1
    }

    const state = loadState(options)
    for (const table of tables) {
      await backfillTable(sql, options, state, table)
    }
    console.log(
      "\nBackfill sweep complete. Run again with --restart for the catch-up pass, then --verify.",
    )
    return 0
  } finally {
    await (sql as unknown as { end: () => Promise<void> }).end()
  }
}

process.exit(await main())
