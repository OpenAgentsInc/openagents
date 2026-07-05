#!/usr/bin/env bun
/**
 * KS-8.8 (#8319): Treasury / payouts / tips settlement backfill CLI —
 * D1 → Postgres.
 *
 * Reads D1 rows through `wrangler d1 execute <db> --remote --json` (the
 * workable path: it needs only the repo's existing wrangler auth — no new
 * admin-API token surface) in bounded rowid-keyset pages, and upserts them
 * into the Postgres twins (migration 0016_treasury_domain.sql; identical
 * table names) over a DIRECT connection (never Hyperdrive).
 * `ON CONFLICT ... DO NOTHING`, so:
 *   - re-running is idempotent (safe to interrupt + resume);
 *   - it never fights the live dual-write mirror (mirror rows win — a
 *     settlement state the authority advanced is never regressed by a
 *     stale snapshot page).
 *
 * Resumable: progress (last rowid per table) persists in a local state
 * file; delete it (or pass --restart) to sweep from the beginning again.
 * The runbook's sequence runs the backfill TWICE — the second sweep is the
 * catch-up pass after dual-write has been on across the whole window.
 *
 * Verify mode (`--verify`) — MONEY RECONCILIATION IS THE ACCEPTANCE:
 * exact per-table row counts, per-(state, rail) row tallies WITH exact
 * money-column SUMs (millisat / sat / cent / minor-unit totals must match
 * to the unit), and newest-N row-hash comparison. Exits non-zero on ANY
 * mismatch.
 *
 * `mpp_lightning_replay` / `mpp_spt_replay` were retired from this table
 * list (see `src/treasury-backfill.ts` header) — the D1 tables no longer
 * exist (worker migration `0303_drop_mpp_replay_tables.sql`, #8387).
 *
 * Usage (from packages/khala-sync-server/):
 *   bun scripts/backfill-treasury.ts \
 *     [--database-url <postgres-url>]   # default $KHALA_SYNC_DATABASE_URL
 *     [--d1-database <name>]            # default openagents-autopilot
 *     [--wrangler-cwd <dir>]            # default ../../apps/openagents.com/workers/api
 *     [--batch-size <n>]                # default 500
 *     [--state-file <path>]             # default .treasury-backfill-state.json
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
  TREASURY_TABLE_SPECS,
  compareTreasuryTallies,
  d1TreasuryNewestRowHashes,
  d1TreasuryTallyFromGroups,
  d1TreasuryTallySql,
  postgresTreasuryNewestRowHashes,
  postgresTreasuryTally,
  upsertTreasuryRows,
  type D1SourceRow,
  type TreasuryBackfillTable,
} from "../src/treasury-backfill.js"
import type { SyncSql } from "../src/sql.js"

// Referenced-before-referencing fill order (integrity is verified by
// set-membership, not FKs, but ordered fill keeps partial states sensible
// mid-run): approvals before intents before attempts before
// reconciliation/receipts; tip attempts before webhook events; receipts
// before settlement claims; escrows before escrow receipts.
const TABLES: ReadonlyArray<TreasuryBackfillTable> = [
  "treasury_transactions",
  "nexus_payout_target_approvals",
  "nexus_treasury_payout_intents",
  "nexus_treasury_payout_attempts",
  "nexus_treasury_payout_reconciliation_events",
  "nexus_payment_authority_receipts",
  "nexus_release_gates",
  "forum_money_actions",
  "forum_payment_events",
  "forum_receipts",
  "forum_l402_challenges",
  "forum_l402_redemptions",
  "forum_direct_tip_attempts",
  "forum_direct_tip_webhook_events",
  "forum_tip_recipient_wallets",
  "forum_tip_settlement_claims",
  "x_claim_reward_ledger",
  "agent_claim_reward_ledger",
  "agent_balances",
  "labor_escrows",
  "labor_escrow_receipts",
  "partner_payout_ledger_entries",
  "partner_agreements",
  "site_referral_payout_ledger_entries",
  "revenue_event_provenance",
]

const USAGE = `Usage: bun scripts/backfill-treasury.ts [options]   (see file header)`

type Options = {
  batchSize: number
  d1Database: string
  databaseUrl: string | undefined
  local: boolean
  restart: boolean
  stateFile: string
  table: TreasuryBackfillTable | undefined
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
    restart: false,
    stateFile: ".treasury-backfill-state.json",
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
      if (!TABLES.includes(table as TreasuryBackfillTable)) {
        console.error(`error: unknown table ${table}`)
        return undefined
      }
      options.table = table as TreasuryBackfillTable
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
    options.batchSize > 5000
  ) {
    console.error("error: --batch-size must be 1..5000")
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
  // `--json` output: [{ results: [...], success: true, meta: {...} }]
  const parsed = JSON.parse(result.stdout) as Array<{
    results: Array<Record<string, unknown>>
  }>
  return parsed[0]?.results ?? []
}

// ---------------------------------------------------------------------------
// Cursor state
// ---------------------------------------------------------------------------

type CursorState = Partial<Record<TreasuryBackfillTable, number>>

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
  table: TreasuryBackfillTable,
): Promise<void> => {
  let cursor = state[table] ?? 0
  let copied = 0
  let inserted = 0
  for (;;) {
    const rows = d1Query(
      options,
      `SELECT rowid AS d1_rowid, * FROM ${table} WHERE rowid > ${cursor} ORDER BY rowid ASC LIMIT ${options.batchSize}`,
    ) as Array<D1SourceRow & { d1_rowid: number }>
    if (rows.length === 0) break
    inserted += await upsertTreasuryRows(sql, table, rows)
    copied += rows.length
    cursor = Number(rows[rows.length - 1]?.d1_rowid ?? cursor)
    state[table] = cursor
    saveState(options, state)
    console.log(
      `${table}: page done (cursor rowid=${cursor}, scanned=${copied}, newly inserted=${inserted})`,
    )
  }
  console.log(
    `${table}: complete — scanned ${copied} row(s) this run, ${inserted} newly inserted`,
  )
}

// ---------------------------------------------------------------------------
// Verify
// ---------------------------------------------------------------------------

const verifyTable = async (
  sql: SyncSql,
  options: Options,
  table: TreasuryBackfillTable,
): Promise<boolean> => {
  const spec = TREASURY_TABLE_SPECS[table]
  const d1 = d1TreasuryTallyFromGroups(
    table,
    d1Query(options, d1TreasuryTallySql(table)),
  )
  const postgres = await postgresTreasuryTally(sql, table)
  const d1Newest = d1TreasuryNewestRowHashes(
    table,
    d1Query(
      options,
      `SELECT * FROM ${table} ORDER BY ${spec.orderColumn} DESC, ${spec.conflictKey} DESC LIMIT ${options.verifyNewest}`,
    ),
  )
  const postgresNewest = await postgresTreasuryNewestRowHashes(
    sql,
    table,
    options.verifyNewest,
  )
  const report = compareTreasuryTallies(
    table,
    d1,
    postgres,
    d1Newest,
    postgresNewest,
  )

  console.log(`\n== ${table} ==`)
  console.log(`  rows: d1=${report.d1Total} postgres=${report.postgresTotal}`)
  console.log(`  d1 (state|rail → count + money sums):       ${JSON.stringify(d1.byGroup)}`)
  console.log(`  postgres (state|rail → count + money sums): ${JSON.stringify(postgres.byGroup)}`)
  if (report.groupMismatches.length > 0) {
    console.log(
      `  GROUP/AMOUNT MISMATCHES: ${JSON.stringify(report.groupMismatches)}`,
    )
  }
  console.log(
    `  newest-${options.verifyNewest} row hashes: ${
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
    report.groupMismatches.length === 0 &&
    report.newestHashMismatches.length === 0
  )
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
  const tables = options.table === undefined ? TABLES : [options.table]
  try {
    if (options.verify) {
      let allGood = true
      for (const table of tables) {
        allGood = (await verifyTable(sql, options, table)) && allGood
      }
      console.log(
        allGood
          ? "\nVERIFY OK: exact counts, per-state money sums, and newest-N hashes match."
          : "\nVERIFY FAILED: mismatches above — investigate before any read cutover.",
      )
      return allGood ? 0 : 1
    }

    const state = loadState(options)
    for (const table of tables) {
      await backfillTable(sql, options, state, table)
    }
    console.log(
      "\nBackfill sweep complete. Run again after dual-write has covered the window, then run --verify.",
    )
    return 0
  } finally {
    await (sql as unknown as { end: () => Promise<void> }).end()
  }
}

process.exit(await main())
