import { SQL } from "bun"
import { createHash } from "node:crypto"
import { readdir, readFile } from "node:fs/promises"
import * as path from "node:path"

/**
 * Khala Sync migration runner core (KS-0.3).
 *
 * Applies ordered `.sql` files from `packages/khala-sync-server/migrations/`
 * to Postgres over a DIRECT connection (never Hyperdrive — see
 * docs/khala-sync/SPEC.md §4 connectivity invariants). Applied migrations
 * are recorded in `khala_sync_migrations` (filename, sha256, applied_at);
 * re-runs are idempotent, and the runner REFUSES to proceed if an
 * already-applied file's content hash changed on disk.
 *
 * The CLI entrypoint is `scripts/migrate.ts`; this module holds the
 * testable core.
 */

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

export class MigrationHashMismatchError extends Error {
  readonly _tag = "MigrationHashMismatchError"
  constructor(
    readonly mismatches: ReadonlyArray<{
      readonly filename: string
      readonly appliedSha256: string
      readonly fileSha256: string
    }>,
  ) {
    super(
      "refusing to run: applied migration file(s) changed on disk: " +
        mismatches
          .map(
            (m) =>
              `${m.filename} (applied ${m.appliedSha256.slice(0, 12)}…, file ${m.fileSha256.slice(0, 12)}…)`,
          )
          .join(", "),
    )
  }
}

export class MigrationFileMissingError extends Error {
  readonly _tag = "MigrationFileMissingError"
  constructor(readonly filenames: ReadonlyArray<string>) {
    super(
      "refusing to run: migration(s) recorded as applied but missing from " +
        `the migrations directory: ${filenames.join(", ")}`,
    )
  }
}

export class MigrationFilenameError extends Error {
  readonly _tag = "MigrationFilenameError"
  constructor(readonly filename: string) {
    super(
      `invalid migration filename ${JSON.stringify(filename)}: expected ` +
        "NNNN_snake_case_name.sql (e.g. 0001_khala_sync_core.sql)",
    )
  }
}

// ---------------------------------------------------------------------------
// Migration files
// ---------------------------------------------------------------------------

export interface MigrationFile {
  readonly filename: string
  readonly sha256: string
  readonly sql: string
}

const MIGRATION_FILENAME_PATTERN = /^\d{4}_[a-z0-9_]+\.sql$/

export const sha256Hex = (content: string): string =>
  createHash("sha256").update(content, "utf8").digest("hex")

/**
 * Read the ordered migration set from a directory. Every `.sql` file must
 * match `NNNN_snake_case_name.sql`; ordering is lexicographic on the
 * zero-padded numeric prefix (plain filename sort).
 */
export const readMigrationFiles = async (
  dir: string,
): Promise<ReadonlyArray<MigrationFile>> => {
  const entries = (await readdir(dir)).filter((name) => name.endsWith(".sql"))
  for (const name of entries) {
    if (!MIGRATION_FILENAME_PATTERN.test(name)) {
      throw new MigrationFilenameError(name)
    }
  }
  entries.sort()
  return Promise.all(
    entries.map(async (filename) => {
      const sql = await readFile(path.join(dir, filename), "utf8")
      return { filename, sha256: sha256Hex(sql), sql }
    }),
  )
}

// ---------------------------------------------------------------------------
// Ledger table
// ---------------------------------------------------------------------------

export const MIGRATIONS_TABLE = "khala_sync_migrations"

const MIGRATIONS_TABLE_DDL = `
CREATE TABLE IF NOT EXISTS khala_sync_migrations (
  filename   text        PRIMARY KEY,
  sha256     text        NOT NULL,
  applied_at timestamptz NOT NULL DEFAULT now()
)
`

// ---------------------------------------------------------------------------
// Plan + run
// ---------------------------------------------------------------------------

export interface MigrationPlan {
  /** Already applied, hash matches — nothing to do. */
  readonly alreadyApplied: ReadonlyArray<string>
  /** Not yet applied — will be applied in this order. */
  readonly pending: ReadonlyArray<MigrationFile>
  /** Applied, but the on-disk content hash differs — refusal condition. */
  readonly mismatched: ReadonlyArray<{
    readonly filename: string
    readonly appliedSha256: string
    readonly fileSha256: string
  }>
  /** Recorded as applied but no longer on disk — refusal condition. */
  readonly missingFiles: ReadonlyArray<string>
}

export const planMigrations = (
  files: ReadonlyArray<MigrationFile>,
  appliedRows: ReadonlyArray<{ readonly filename: string; readonly sha256: string }>,
): MigrationPlan => {
  const appliedByName = new Map(appliedRows.map((r) => [r.filename, r.sha256]))
  const fileNames = new Set(files.map((f) => f.filename))
  const alreadyApplied: Array<string> = []
  const pending: Array<MigrationFile> = []
  const mismatched: Array<{
    filename: string
    appliedSha256: string
    fileSha256: string
  }> = []
  for (const file of files) {
    const appliedSha = appliedByName.get(file.filename)
    if (appliedSha === undefined) {
      pending.push(file)
    } else if (appliedSha === file.sha256) {
      alreadyApplied.push(file.filename)
    } else {
      mismatched.push({
        filename: file.filename,
        appliedSha256: appliedSha,
        fileSha256: file.sha256,
      })
    }
  }
  const missingFiles = appliedRows
    .map((r) => r.filename)
    .filter((name) => !fileNames.has(name))
  return { alreadyApplied, pending, mismatched, missingFiles }
}

export interface RunMigrationsOptions {
  /** Direct Postgres connection URL (never a Hyperdrive binding string). */
  readonly databaseUrl: string
  /** Defaults to the package's `migrations/` directory. */
  readonly migrationsDir?: string
  /** Print the plan without applying anything (and without DDL writes). */
  readonly dryRun?: boolean
  readonly log?: (line: string) => void
}

export interface RunMigrationsResult {
  readonly dryRun: boolean
  /** Filenames applied by THIS run, in order. Empty on dry runs. */
  readonly applied: ReadonlyArray<string>
  readonly plan: MigrationPlan
}

export const defaultMigrationsDir = path.join(import.meta.dir, "..", "migrations")

/**
 * Run (or dry-run) the migration set. Refuses with
 * `MigrationHashMismatchError` / `MigrationFileMissingError` when the ledger
 * disagrees with the on-disk files — in both dry-run and apply mode.
 *
 * Concurrency: apply mode takes a session advisory lock
 * (`hashtext('khala_sync_migrations')`) so two runners cannot interleave.
 * Each migration file runs inside ONE transaction together with its ledger
 * insert.
 */
export const runMigrations = async (
  options: RunMigrationsOptions,
): Promise<RunMigrationsResult> => {
  const dryRun = options.dryRun === true
  const log = options.log ?? (() => {})
  const migrationsDir = options.migrationsDir ?? defaultMigrationsDir
  const files = await readMigrationFiles(migrationsDir)

  // max: 1 so the session advisory lock and all statements share one
  // direct connection.
  const sql = new SQL({ url: options.databaseUrl, max: 1 })
  try {
    if (!dryRun) {
      await sql`SELECT pg_advisory_lock(hashtext(${MIGRATIONS_TABLE})::bigint)`
      await sql.unsafe(MIGRATIONS_TABLE_DDL)
    }

    const [{ ledger }] = await sql`
      SELECT to_regclass(${MIGRATIONS_TABLE}) IS NOT NULL AS ledger
    `
    const appliedRows: ReadonlyArray<{ filename: string; sha256: string }> =
      ledger === true
        ? await sql`SELECT filename, sha256 FROM khala_sync_migrations ORDER BY filename`
        : []

    const plan = planMigrations(files, appliedRows)

    if (plan.mismatched.length > 0) {
      throw new MigrationHashMismatchError(plan.mismatched)
    }
    if (plan.missingFiles.length > 0) {
      throw new MigrationFileMissingError(plan.missingFiles)
    }

    for (const name of plan.alreadyApplied) {
      log(`already applied  ${name}`)
    }

    if (dryRun) {
      for (const file of plan.pending) {
        log(`would apply      ${file.filename} (sha256 ${file.sha256.slice(0, 12)}…)`)
      }
      if (plan.pending.length === 0) {
        log("up to date — nothing to apply")
      }
      return { dryRun, applied: [], plan }
    }

    const applied: Array<string> = []
    for (const file of plan.pending) {
      await sql.begin(async (tx) => {
        await tx.unsafe(file.sql)
        await tx`
          INSERT INTO khala_sync_migrations (filename, sha256)
          VALUES (${file.filename}, ${file.sha256})
        `
      })
      applied.push(file.filename)
      log(`applied          ${file.filename} (sha256 ${file.sha256.slice(0, 12)}…)`)
    }
    if (applied.length === 0) {
      log("up to date — nothing to apply")
    }
    return { dryRun, applied, plan }
  } finally {
    await sql.end()
  }
}
