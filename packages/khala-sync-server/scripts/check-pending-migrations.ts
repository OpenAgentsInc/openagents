#!/usr/bin/env bun
/**
 * Khala Sync migration deploy gate (openagents issue #8410 follow-up).
 *
 * WHY THIS EXISTS. `apps/openagents.com/workers/api`'s D1 schema has
 * `check:pending-migrations` gating `deploy:safe` so the Worker never ships
 * ahead of its D1 schema (the 2026-06-25 gateway-wide 500 AAR — see
 * `apps/openagents.com/scripts/check-pending-migrations.mjs`). Khala Sync's
 * OWN Postgres schema (a separate DIRECT-connection migration system,
 * `scripts/migrate.ts` — never Hyperdrive) had no equivalent gate:
 * migration `0032_khala_sync_runtime_control_intents_seq.sql` sat unapplied
 * on production for hours after merge until a live production debugging
 * session (2026-07-04/05, runtime dispatch consumer hardening, #8388/#8410)
 * found and applied it manually via direct `psql` inspection of
 * `khala_sync_migrations`. This script closes that gap the same way: it
 * runs the SAME dry-run migration plan `scripts/migrate.ts --dry-run` uses
 * (`runMigrations({ dryRun: true })`, `../src/migrate.ts`) and EXITS
 * NON-ZERO if any migration is pending, naming the pending files.
 *
 * Usage:
 *   bun scripts/check-pending-migrations.ts [--database-url <url>]
 *
 * The connection URL comes from --database-url or KHALA_SYNC_DATABASE_URL —
 * see docs/khala-sync/RUNBOOK.md's "Migrations runbook" for where the
 * owner's direct-connection secret lives (normally
 * ~/work/.secrets/khala-sync-cloudsql.env, sourced before running deploy
 * commands — this is deliberately NOT wired into the generic `check:deploy`
 * PR sweep, only into the owner-run `deploy:safe` path, exactly like the D1
 * check above).
 */
import { runMigrations } from "../src/migrate.js"

export interface PendingMigrationsDecision {
  readonly ok: boolean
  readonly exitCode: number
  readonly message: string
}

/**
 * Pure decision core (unit-tested in `check-pending-migrations.test.ts`
 * without any real Postgres connection) — mirrors
 * `apps/openagents.com/scripts/check-pending-migrations.mjs`'s
 * `decidePendingMigrations` for the D1 case.
 */
export const decidePendingKhalaSyncMigrations = (
  pending: ReadonlyArray<string>,
): PendingMigrationsDecision => {
  if (pending.length === 0) {
    return {
      ok: true,
      exitCode: 0,
      message: "check-pending-migrations: OK — 0 pending Khala Sync migrations (Postgres, direct connection).",
    }
  }
  const list = pending.map((name) => `  - ${name}`).join("\n")
  return {
    ok: false,
    exitCode: 1,
    message:
      `✘ check-pending-migrations: ${pending.length} Khala Sync migration(s) PENDING (Postgres):\n${list}\n` +
      "  The Worker must NOT ship ahead of the Khala Sync schema (openagents #8410,\n" +
      "  follow-up to the 2026-06-25 D1 AAR). Apply them first:\n" +
      "    cd packages/khala-sync-server && KHALA_SYNC_DATABASE_URL=<direct-url> bun run migrate\n" +
      "  then re-run the sanctioned deploy (`bun run deploy:safe`).",
  }
}

const USAGE = `Usage: bun scripts/check-pending-migrations.ts [--database-url <url>]

Options:
  --database-url <url>  Direct Postgres URL (default: $KHALA_SYNC_DATABASE_URL).
  --help                Show this help.
`

const main = async (argv: ReadonlyArray<string>): Promise<number> => {
  let databaseUrl = process.env["KHALA_SYNC_DATABASE_URL"]
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i]
    if (arg === "--database-url") {
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
      "✘ check-pending-migrations: no database URL — pass --database-url or set " +
        "KHALA_SYNC_DATABASE_URL\n  (owner secret: ~/work/.secrets/khala-sync-cloudsql.env — " +
        "see docs/khala-sync/RUNBOOK.md's \"Migrations runbook\")",
    )
    return 2
  }

  try {
    const result = await runMigrations({ databaseUrl, dryRun: true })
    const decision = decidePendingKhalaSyncMigrations(result.plan.pending.map((file) => file.filename))
    if (decision.ok) {
      console.log(decision.message)
    } else {
      console.error(decision.message)
    }
    return decision.exitCode
  } catch (error) {
    console.error(
      `✘ check-pending-migrations: ${error instanceof Error ? error.message : "unknown error"}\n` +
        "  (auth/connectivity to the direct Postgres URL, or a hash-mismatch/missing-file refusal — " +
        "see the error above)",
    )
    return 2
  }
}

if (import.meta.main) {
  process.exit(await main(process.argv.slice(2)))
}
