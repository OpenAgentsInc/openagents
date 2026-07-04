#!/usr/bin/env bun
/**
 * Khala Sync migration CLI (KS-0.3).
 *
 * Applies ordered .sql files from packages/khala-sync-server/migrations/
 * to Postgres over a DIRECT connection (never Hyperdrive). Idempotent;
 * records applied migrations in khala_sync_migrations and refuses to run
 * if an applied file's hash changed on disk.
 *
 * Usage:
 *   bun scripts/migrate.ts [--dry-run] [--database-url <url>]
 *
 * The connection URL comes from --database-url or KHALA_SYNC_DATABASE_URL.
 * See the README "Migrations runbook" section.
 */
import {
  MigrationFileMissingError,
  MigrationHashMismatchError,
  runMigrations,
} from "../src/migrate.js"

const USAGE = `Usage: bun scripts/migrate.ts [--dry-run] [--database-url <url>]

Options:
  --dry-run             Print the migration plan without applying anything.
  --database-url <url>  Direct Postgres URL (default: $KHALA_SYNC_DATABASE_URL).
  --help                Show this help.
`

const main = async (argv: ReadonlyArray<string>): Promise<number> => {
  let dryRun = false
  let databaseUrl = process.env["KHALA_SYNC_DATABASE_URL"]
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

  try {
    const result = await runMigrations({
      databaseUrl,
      dryRun,
      log: (line) => console.log(line),
    })
    if (result.dryRun) {
      console.log(
        `dry run: ${result.plan.pending.length} pending, ` +
          `${result.plan.alreadyApplied.length} already applied — nothing was changed`,
      )
    } else {
      console.log(
        `done: applied ${result.applied.length}, ` +
          `already applied ${result.plan.alreadyApplied.length}`,
      )
    }
    return 0
  } catch (error) {
    if (
      error instanceof MigrationHashMismatchError ||
      error instanceof MigrationFileMissingError
    ) {
      console.error(`refused: ${error.message}`)
      return 1
    }
    throw error
  }
}

process.exit(await main(process.argv.slice(2)))
