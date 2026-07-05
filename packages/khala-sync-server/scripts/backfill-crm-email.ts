#!/usr/bin/env bun
/**
 * KS-8.11 (#8322): CRM / email / enrichment domain backfill CLI — D1 →
 * Postgres.
 *
 * Reads D1 rows through `wrangler d1 execute <db> --remote --json` (the
 * workable path: it needs only the repo's existing wrangler auth — no new
 * admin-API token surface) in bounded rowid-keyset pages, and upserts them
 * into the Postgres twins (migration 0022_crm_email_domain.sql; identical
 * table names) over a DIRECT connection (never Hyperdrive).
 * `ON CONFLICT ... DO NOTHING`, so:
 *   - re-running is idempotent (safe to interrupt + resume);
 *   - it never fights the live dual-write mirror (mirror rows win).
 *
 * Resumable: progress (last rowid per table) persists in a local state
 * file; delete it (or pass --restart) to sweep from the beginning again.
 * The runbook's sequence runs the backfill TWICE — the second sweep is the
 * catch-up pass after dual-write has been on across the whole window.
 *
 * PRIVACY (the KS-8.11 gate): these rows carry names/emails/notes — PII.
 * This CLI NEVER prints row contents: progress lines are counts and rowid
 * cursors; verify output is exact counts, per-status tallies over NON-PII
 * status columns, opaque sha256 row hashes, PII-safed keys (email-shaped
 * keys become sha256 prefixes), and whole-set digests for the
 * compliance-bearing tables (contacts, preferences, suppression entries,
 * list subscribers, outreach suppressions) — the issue's "suppression set
 * equality (exact)" acceptance without emitting a single address.
 *
 * Verify mode (`--verify`): exits non-zero on ANY mismatch.
 *
 * Usage (from packages/khala-sync-server/):
 *   bun scripts/backfill-crm-email.ts \
 *     [--database-url <postgres-url>]   # default $KHALA_SYNC_DATABASE_URL
 *     [--d1-database <name>]            # default openagents-autopilot
 *     [--wrangler-cwd <dir>]            # default ../../apps/openagents.com/workers/api
 *     [--batch-size <n>]                # default 500
 *     [--state-file <path>]             # default .crm-email-backfill-state.json
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
  CRM_EMAIL_TABLE_SPECS,
  compareCrmEmailTallies,
  d1CrmEmailNewestRowHashes,
  d1CrmEmailSetDigest,
  postgresCrmEmailNewestRowHashes,
  postgresCrmEmailSetDigest,
  postgresCrmEmailTally,
  upsertCrmEmailRows,
  type CrmEmailBackfillTable,
  type CrmEmailVerifyTally,
  type D1SourceRow,
} from "../src/crm-email-backfill.js"
import type { SyncSql } from "../src/sql.js"

// Parents before children where refs matter mid-run (campaigns → steps →
// enrollments → sends; lists → subscribers; runs → queries → sources;
// drafts/approvals → sends). Integrity is verified by set-membership, not
// FKs, but ordered fill keeps partial states sensible mid-run.
const TABLES: ReadonlyArray<CrmEmailBackfillTable> = [
  "crm_accounts",
  "crm_contacts",
  "crm_contact_lists",
  "crm_contact_list_memberships",
  "crm_activities",
  "crm_engagement_snapshots",
  "crm_opportunities",
  "crm_opportunity_contact_roles",
  "crm_source_import_runs",
  "crm_email_templates",
  "crm_email_messages",
  "crm_contact_commands",
  "crm_mcp_grants",
  "email_templates",
  "email_messages",
  "email_deliveries",
  "email_drafts",
  "email_provider_events",
  "email_campaigns",
  "email_campaign_steps",
  "email_campaign_enrollments",
  "email_campaign_sends",
  "email_preferences",
  "email_suppression_entries",
  "subscriber_lists",
  "list_subscribers",
  "business_outreach_template_approvals",
  "business_outreach_suppressions",
  "business_outreach_drafts",
  "business_outreach_sends",
  "exa_enrichment_runs",
  "exa_enrichment_queries",
  "exa_enrichment_sources",
  "exa_enrichment_budget_events",
  "exa_enrichment_cache_entries",
  "exa_enrichment_metric_events",
]

const USAGE = `Usage: bun scripts/backfill-crm-email.ts [options]   (see file header)`

type Options = {
  batchSize: number
  d1Database: string
  databaseUrl: string | undefined
  local: boolean
  restart: boolean
  stateFile: string
  table: CrmEmailBackfillTable | undefined
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
    stateFile: ".crm-email-backfill-state.json",
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
      if (!TABLES.includes(table as CrmEmailBackfillTable)) {
        console.error(`error: unknown table ${table}`)
        return undefined
      }
      options.table = table as CrmEmailBackfillTable
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

type CursorState = Partial<Record<CrmEmailBackfillTable, number>>

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
  table: CrmEmailBackfillTable,
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
    inserted += await upsertCrmEmailRows(sql, table, rows)
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
// Verify (counts / tallies / hashes ONLY — never row contents)
// ---------------------------------------------------------------------------

const d1Tally = (
  options: Options,
  table: CrmEmailBackfillTable,
): CrmEmailVerifyTally => {
  const statusColumn = CRM_EMAIL_TABLE_SPECS[table].statusColumn
  const rows = d1Query(
    options,
    `SELECT ${statusColumn} AS status_value, COUNT(*) AS row_count FROM ${table} GROUP BY ${statusColumn} ORDER BY ${statusColumn}`,
  ) as Array<{ status_value: string | number | null; row_count: number }>
  const byStatus: Record<string, number> = {}
  let total = 0
  for (const row of rows) {
    const count = Number(row.row_count)
    byStatus[row.status_value === null ? "<null>" : String(row.status_value)] =
      count
    total += count
  }
  return { byStatus, total }
}

const verifyTable = async (
  sql: SyncSql,
  options: Options,
  table: CrmEmailBackfillTable,
): Promise<boolean> => {
  const spec = CRM_EMAIL_TABLE_SPECS[table]
  const d1 = d1Tally(options, table)
  const postgres = await postgresCrmEmailTally(sql, table)
  const d1Newest = d1CrmEmailNewestRowHashes(
    table,
    d1Query(
      options,
      `SELECT * FROM ${table} ORDER BY ${spec.orderColumn} DESC, ${spec.conflictKey} DESC LIMIT ${options.verifyNewest}`,
    ),
  )
  const postgresNewest = await postgresCrmEmailNewestRowHashes(
    sql,
    table,
    options.verifyNewest,
  )
  const setDigests =
    spec.fullSetDigest === true
      ? {
          d1: d1CrmEmailSetDigest(
            table,
            d1Query(options, `SELECT * FROM ${table}`),
          ).digest,
          postgres: (await postgresCrmEmailSetDigest(sql, table)).digest,
        }
      : undefined
  const report = compareCrmEmailTallies(
    table,
    d1,
    postgres,
    d1Newest,
    postgresNewest,
    setDigests,
  )

  console.log(`\n== ${table} ==`)
  console.log(`  rows: d1=${report.d1Total} postgres=${report.postgresTotal}`)
  console.log(`  d1 tallies:       ${JSON.stringify(d1.byStatus)}`)
  console.log(`  postgres tallies: ${JSON.stringify(postgres.byStatus)}`)
  if (report.statusMismatches.length > 0) {
    console.log(
      `  STATUS MISMATCHES: ${JSON.stringify(report.statusMismatches)}`,
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
  if (report.setDigestsMatch !== undefined) {
    console.log(
      `  FULL-SET digest (${setDigests?.d1.slice(0, 16)}…): ${
        report.setDigestsMatch
          ? "sets EQUAL"
          : "sets DIFFER — compliance gate, do not cut over"
      }`,
    )
  }
  return (
    report.countsMatch &&
    report.statusMismatches.length === 0 &&
    report.newestHashMismatches.length === 0 &&
    report.setDigestsMatch !== false
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
          ? "\nVERIFY OK: exact counts, tallies, newest-N hashes, and compliance set digests match."
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
