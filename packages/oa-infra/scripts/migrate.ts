#!/usr/bin/env bun
/**
 * oa-infra migration CLI (CFG-2, issue #8517).
 *
 * Usage:
 *   bun scripts/migrate.ts [--dry-run] [--database-url <url>]
 *
 * The connection URL comes from --database-url or OA_INFRA_DATABASE_URL.
 */
import { runOaInfraMigrations } from "../src/migrate.ts"

const USAGE = `Usage: bun scripts/migrate.ts [--dry-run] [--database-url <url>]

Options:
  --dry-run             Print the migration plan without applying anything.
  --database-url <url>  Direct Postgres URL (default: $OA_INFRA_DATABASE_URL).
  --help                Show this help.
`

const main = async (argv: ReadonlyArray<string>): Promise<number> => {
  let dryRun = false
  let databaseUrl = process.env["OA_INFRA_DATABASE_URL"]
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
      "error: no database URL — pass --database-url or set OA_INFRA_DATABASE_URL\n",
    )
    console.error(USAGE)
    return 2
  }
  const result = await runOaInfraMigrations({
    databaseUrl,
    dryRun,
    log: (line) => console.log(line),
  })
  console.log(
    result.dryRun
      ? `dry run complete (${result.alreadyApplied.length} already applied)`
      : `done — applied ${result.applied.length} migration(s)`,
  )
  return 0
}

process.exit(await main(process.argv.slice(2)))
