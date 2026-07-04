#!/usr/bin/env bun
/**
 * Khala Sync changelog compaction CLI (KS-2.3 #8289).
 *
 * Advances each scope's retained-window watermark
 * (khala_sync_scopes.retained_from_version) and prunes compactable
 * khala_sync_changelog rows behind it — bounded by entry count, age, and
 * (when the capture lane's checkpoint table exists) the per-scope
 * pushed_through_version. See src/compaction.ts and the README
 * "Compaction runbook" section.
 *
 * Intended for cron / Cloud Scheduler use over a DIRECT Postgres
 * connection (never Hyperdrive).
 *
 * Usage:
 *   bun scripts/compact.ts [--dry-run] [--database-url <url>]
 *     [--max-retained-entries <n>] [--max-retained-age-ms <ms>]
 *
 * The connection URL comes from --database-url or KHALA_SYNC_DATABASE_URL.
 */
import { SQL } from "bun"
import { compactAll, type CompactScopeResult } from "../src/compaction.js"

const DEFAULT_MAX_RETAINED_ENTRIES = 10_000

const USAGE = `Usage: bun scripts/compact.ts [--dry-run] [--database-url <url>] [--max-retained-entries <n>] [--max-retained-age-ms <ms>]

Options:
  --dry-run                   Print the per-scope compaction plan without writing.
  --database-url <url>        Direct Postgres URL (default: $KHALA_SYNC_DATABASE_URL).
  --max-retained-entries <n>  Always keep the newest N version groups per scope
                              (default: ${DEFAULT_MAX_RETAINED_ENTRIES}).
  --max-retained-age-ms <ms>  Never compact entries younger than this age.
  --help                      Show this help.
`

const parsePositiveInt = (name: string, raw: string | undefined): number | null => {
  if (raw === undefined) {
    console.error(`error: ${name} requires a value\n`)
    return null
  }
  const value = Number(raw)
  if (!Number.isSafeInteger(value) || value < 0) {
    console.error(`error: ${name} must be a non-negative integer, got ${raw}\n`)
    return null
  }
  return value
}

const formatResult = (r: CompactScopeResult): string => {
  const verb = r.dryRun ? "would delete" : "deleted"
  const bounds = [
    `entry_count=${r.entryCountCandidate}`,
    ...(r.ageCandidate === null ? [] : [`age=${r.ageCandidate}`]),
    ...(r.captureCheckpointCandidate === null
      ? []
      : [`capture_checkpoint=${r.captureCheckpointCandidate}`]),
  ].join(" ")
  return (
    `${String(r.scope)}  last_version=${r.lastVersion} ` +
    `retained_from ${r.previousRetainedFromVersion} -> ${r.newRetainedFromVersion} ` +
    `(bounded_by=${r.boundedBy}; candidates: ${bounds}) ` +
    `${verb} ${r.deletedRows} rows, ${r.preservedSnapshotRows} snapshot rows retained behind the watermark`
  )
}

const main = async (argv: ReadonlyArray<string>): Promise<number> => {
  let dryRun = false
  let databaseUrl = process.env["KHALA_SYNC_DATABASE_URL"]
  let maxRetainedEntries = DEFAULT_MAX_RETAINED_ENTRIES
  let maxRetainedAgeMs: number | undefined
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i]
    if (arg === "--dry-run") {
      dryRun = true
    } else if (arg === "--database-url") {
      databaseUrl = argv[++i]
      if (databaseUrl === undefined) {
        console.error("error: --database-url requires a value\n")
        console.error(USAGE)
        return 2
      }
    } else if (arg === "--max-retained-entries") {
      const value = parsePositiveInt("--max-retained-entries", argv[++i])
      if (value === null || value < 1) {
        console.error(USAGE)
        return 2
      }
      maxRetainedEntries = value
    } else if (arg === "--max-retained-age-ms") {
      const value = parsePositiveInt("--max-retained-age-ms", argv[++i])
      if (value === null) {
        console.error(USAGE)
        return 2
      }
      maxRetainedAgeMs = value
    } else if (arg === "--help" || arg === "-h") {
      console.log(USAGE)
      return 0
    } else {
      console.error(`error: unknown argument ${JSON.stringify(arg)}\n`)
      console.error(USAGE)
      return 2
    }
  }
  if (databaseUrl === undefined || databaseUrl === "") {
    console.error(
      "error: no database URL — pass --database-url or set KHALA_SYNC_DATABASE_URL\n",
    )
    console.error(USAGE)
    return 2
  }

  const sql = new SQL({ url: databaseUrl, max: 1 })
  try {
    const summary = await compactAll(sql, {
      maxRetainedEntries,
      ...(maxRetainedAgeMs === undefined ? {} : { maxRetainedAgeMs }),
      dryRun,
    })
    for (const result of summary.results) {
      console.log(formatResult(result))
    }
    for (const failure of summary.failures) {
      console.error(`FAILED ${failure.scope}: ${failure.messageSafe}`)
    }
    const mode = summary.dryRun ? "dry run" : "done"
    console.log(
      `${mode}: ${summary.scopesExamined} scope(s) examined, ` +
        `${summary.scopesAdvanced} advanced, ` +
        `${summary.totalDeletedRows} row(s) ${summary.dryRun ? "would be " : ""}deleted, ` +
        `${summary.failures.length} failure(s)`,
    )
    return summary.failures.length > 0 ? 1 : 0
  } finally {
    await sql.end()
  }
}

process.exit(await main(process.argv.slice(2)))
