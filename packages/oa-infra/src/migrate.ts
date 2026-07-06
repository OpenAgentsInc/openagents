/**
 * oa-infra migration runner (CFG-2, issue #8517).
 *
 * Same approach as packages/khala-sync-server/src/migrate.ts: ordered
 * `NNNN_snake_case.sql` files applied over a DIRECT Bun `SQL` connection,
 * recorded in `oa_infra_migrations` (filename, sha256, applied_at).
 * Idempotent; REFUSES to run when an applied file's on-disk hash changed or
 * a recorded file is missing. Apply mode holds a session advisory lock so
 * two runners cannot interleave, and each file + its ledger insert commit
 * in one transaction.
 *
 * Kept separate from khala-sync's runner on purpose: oa-infra tables can be
 * installed into ANY Postgres database (including app-owned ones) without
 * dragging khala-sync's ledger along.
 */
import { SQL } from "bun"
import { createHash } from "node:crypto"
import { readdir, readFile } from "node:fs/promises"
import * as path from "node:path"

export class OaInfraMigrationHashMismatchError extends Error {
  readonly _tag = "OaInfraMigrationHashMismatchError"
  constructor(
    readonly mismatches: ReadonlyArray<{
      readonly filename: string
      readonly appliedSha256: string
      readonly fileSha256: string
    }>,
  ) {
    super(
      "refusing to run: applied migration file(s) changed on disk: " +
        mismatches.map((m) => m.filename).join(", "),
    )
  }
}

export class OaInfraMigrationFileMissingError extends Error {
  readonly _tag = "OaInfraMigrationFileMissingError"
  constructor(readonly filenames: ReadonlyArray<string>) {
    super(
      "refusing to run: migration(s) recorded as applied but missing from " +
        `the migrations directory: ${filenames.join(", ")}`,
    )
  }
}

export class OaInfraMigrationFilenameError extends Error {
  readonly _tag = "OaInfraMigrationFilenameError"
  constructor(readonly filename: string) {
    super(
      `invalid migration filename ${JSON.stringify(filename)}: expected ` +
        "NNNN_snake_case_name.sql (e.g. 0001_oa_infra_kv.sql)",
    )
  }
}

export interface OaInfraMigrationFile {
  readonly filename: string
  readonly sha256: string
  readonly sql: string
}

const MIGRATION_FILENAME_PATTERN = /^\d{4}_[a-z0-9_]+\.sql$/

const sha256Hex = (content: string): string =>
  createHash("sha256").update(content, "utf8").digest("hex")

export const readMigrationFiles = async (
  dir: string,
): Promise<ReadonlyArray<OaInfraMigrationFile>> => {
  const entries = (await readdir(dir)).filter((name) => name.endsWith(".sql"))
  for (const name of entries) {
    if (!MIGRATION_FILENAME_PATTERN.test(name)) {
      throw new OaInfraMigrationFilenameError(name)
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

export const MIGRATIONS_TABLE = "oa_infra_migrations"

const MIGRATIONS_TABLE_DDL = `
CREATE TABLE IF NOT EXISTS oa_infra_migrations (
  filename   text        PRIMARY KEY,
  sha256     text        NOT NULL,
  applied_at timestamptz NOT NULL DEFAULT now()
)
`

export interface RunOaInfraMigrationsOptions {
  /** Direct Postgres connection URL (never a vendor pooler binding). */
  readonly databaseUrl: string
  /** Defaults to this package's `migrations/` directory. */
  readonly migrationsDir?: string
  /** Print the plan without applying anything. */
  readonly dryRun?: boolean
  readonly log?: (line: string) => void
}

export interface RunOaInfraMigrationsResult {
  readonly dryRun: boolean
  /** Filenames applied by THIS run, in order. Empty on dry runs. */
  readonly applied: ReadonlyArray<string>
  readonly alreadyApplied: ReadonlyArray<string>
}

export const defaultMigrationsDir = path.join(import.meta.dir, "..", "migrations")

export const runOaInfraMigrations = async (
  options: RunOaInfraMigrationsOptions,
): Promise<RunOaInfraMigrationsResult> => {
  const dryRun = options.dryRun === true
  const log = options.log ?? (() => {})
  const files = await readMigrationFiles(options.migrationsDir ?? defaultMigrationsDir)

  // max: 1 so the advisory lock and all statements share one connection.
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
        ? await sql`SELECT filename, sha256 FROM oa_infra_migrations ORDER BY filename`
        : []

    const appliedByName = new Map(appliedRows.map((row) => [row.filename, row.sha256]))
    const fileNames = new Set(files.map((file) => file.filename))

    const mismatched = files.flatMap((file) => {
      const appliedSha = appliedByName.get(file.filename)
      return appliedSha !== undefined && appliedSha !== file.sha256
        ? [{ filename: file.filename, appliedSha256: appliedSha, fileSha256: file.sha256 }]
        : []
    })
    if (mismatched.length > 0) throw new OaInfraMigrationHashMismatchError(mismatched)

    const missingFiles = appliedRows
      .map((row) => row.filename)
      .filter((name) => !fileNames.has(name))
    if (missingFiles.length > 0) throw new OaInfraMigrationFileMissingError(missingFiles)

    const alreadyApplied = files
      .filter((file) => appliedByName.has(file.filename))
      .map((file) => file.filename)
    const pending = files.filter((file) => !appliedByName.has(file.filename))

    for (const name of alreadyApplied) log(`already applied  ${name}`)

    if (dryRun) {
      for (const file of pending) log(`would apply      ${file.filename}`)
      return { dryRun, applied: [], alreadyApplied }
    }

    const applied: Array<string> = []
    for (const file of pending) {
      await sql.begin(async (tx) => {
        await tx.unsafe(file.sql)
        await tx`
          INSERT INTO oa_infra_migrations (filename, sha256)
          VALUES (${file.filename}, ${file.sha256})
        `
      })
      applied.push(file.filename)
      log(`applied          ${file.filename}`)
    }
    if (applied.length === 0) log("up to date — nothing to apply")
    return { dryRun, applied, alreadyApplied }
  } finally {
    await sql.end()
  }
}
