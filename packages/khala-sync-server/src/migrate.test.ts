import { SQL } from "bun"
import { afterAll, beforeAll, describe, expect, setDefaultTimeout, test } from "bun:test"
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import * as path from "node:path"
import {
  defaultMigrationsDir,
  MigrationFilenameError,
  MigrationFileMissingError,
  MigrationHashMismatchError,
  readMigrationFiles,
  runMigrations,
} from "./migrate.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

// ---------------------------------------------------------------------------
// Pure file/plan behavior (no database needed)
// ---------------------------------------------------------------------------

describe("readMigrationFiles", () => {
  test("returns files in filename order with content hashes", async () => {
    const dir = await mkdtemp(path.join(tmpdir(), "khala-sync-migrations-"))
    try {
      await writeFile(path.join(dir, "0010_later.sql"), "SELECT 10;\n")
      await writeFile(path.join(dir, "0002_earlier.sql"), "SELECT 2;\n")
      await writeFile(path.join(dir, "notes.txt"), "not a migration")
      const files = await readMigrationFiles(dir)
      expect(files.map((f) => f.filename)).toEqual([
        "0002_earlier.sql",
        "0010_later.sql",
      ])
      expect(files[0]?.sha256).toMatch(/^[0-9a-f]{64}$/)
    } finally {
      await rm(dir, { recursive: true, force: true })
    }
  })

  test("rejects filenames outside NNNN_snake_case.sql", async () => {
    const dir = await mkdtemp(path.join(tmpdir(), "khala-sync-migrations-"))
    try {
      await writeFile(path.join(dir, "01_short-prefix.sql"), "SELECT 1;\n")
      await expect(readMigrationFiles(dir)).rejects.toBeInstanceOf(
        MigrationFilenameError,
      )
    } finally {
      await rm(dir, { recursive: true, force: true })
    }
  })

  test("the checked-in migrations directory parses", async () => {
    const files = await readMigrationFiles(defaultMigrationsDir)
    expect(files.map((f) => f.filename)).toContain("0001_khala_sync_core.sql")
  })
})

// ---------------------------------------------------------------------------
// Integration: real local Postgres (initdb + pg_ctl throwaway instance)
// ---------------------------------------------------------------------------

describe.skipIf(!hasLocalPostgres())("migration runner against local Postgres", () => {
  let pg: LocalPostgres
  let admin: SQL
  let dbCounter = 0

  const freshDatabaseUrl = async (): Promise<string> => {
    const name = `khala_sync_mig_${++dbCounter}`
    await admin.unsafe(`CREATE DATABASE ${name}`)
    return pg.urlFor(name)
  }

  beforeAll(async () => {
    pg = await startLocalPostgres()
    admin = new SQL({ url: pg.url, max: 1 })
  })

  afterAll(async () => {
    await admin?.end()
    await pg?.stop()
  })

  test("dry run on a fresh database plans 0001 without applying anything", async () => {
    const url = await freshDatabaseUrl()
    const lines: Array<string> = []
    const result = await runMigrations({
      databaseUrl: url,
      dryRun: true,
      log: (line) => lines.push(line),
    })
    expect(result.dryRun).toBe(true)
    expect(result.applied).toEqual([])
    expect(result.plan.pending.map((f) => f.filename)).toContain(
      "0001_khala_sync_core.sql",
    )
    expect(lines.join("\n")).toContain("would apply")

    // Nothing was created — not even the ledger table.
    const sql = new SQL({ url, max: 1 })
    try {
      const [{ ledger }] = await sql`
        SELECT to_regclass('khala_sync_migrations') IS NOT NULL AS ledger
      `
      expect(ledger).toBe(false)
    } finally {
      await sql.end()
    }
  })

  test("applies 0001, records the ledger row, and is idempotent on rerun", async () => {
    const url = await freshDatabaseUrl()

    const first = await runMigrations({ databaseUrl: url })
    expect(first.applied).toContain("0001_khala_sync_core.sql")

    const second = await runMigrations({ databaseUrl: url })
    expect(second.applied).toEqual([])
    expect(second.plan.alreadyApplied).toContain("0001_khala_sync_core.sql")

    const sql = new SQL({ url, max: 1 })
    try {
      const rows = await sql`
        SELECT filename, sha256, applied_at FROM khala_sync_migrations ORDER BY filename
      `
      expect(rows.length).toBe(first.applied.length)
      expect(rows[0].filename).toBe("0001_khala_sync_core.sql")
      expect(rows[0].sha256).toMatch(/^[0-9a-f]{64}$/)
      expect(rows[0].applied_at).toBeInstanceOf(Date)

      // 0001's objects exist.
      for (const table of [
        "khala_sync_scopes",
        "khala_sync_changelog",
        "khala_sync_mutations",
        "khala_sync_client_state",
      ]) {
        const [{ exists }] = await sql`
          SELECT to_regclass(${table}) IS NOT NULL AS exists
        `
        expect(`${table}:${exists}`).toBe(`${table}:true`)
      }
      const [{ trigger }] = await sql`
        SELECT count(*)::int AS trigger FROM pg_trigger
        WHERE tgname = 'khala_sync_changelog_append_notify'
      `
      expect(trigger).toBe(1)
    } finally {
      await sql.end()
    }
  })

  test("refuses to run when an applied migration's file hash changed", async () => {
    const url = await freshDatabaseUrl()
    const dir = await mkdtemp(path.join(tmpdir(), "khala-sync-migrations-"))
    try {
      const file = path.join(dir, "0001_fixture.sql")
      await writeFile(file, "CREATE TABLE fixture_one (id text PRIMARY KEY);\n")
      const first = await runMigrations({ databaseUrl: url, migrationsDir: dir })
      expect(first.applied).toEqual(["0001_fixture.sql"])

      await writeFile(file, "CREATE TABLE fixture_one_changed (id text PRIMARY KEY);\n")
      await expect(
        runMigrations({ databaseUrl: url, migrationsDir: dir }),
      ).rejects.toBeInstanceOf(MigrationHashMismatchError)

      // Dry run refuses on the same condition.
      await expect(
        runMigrations({ databaseUrl: url, migrationsDir: dir, dryRun: true }),
      ).rejects.toBeInstanceOf(MigrationHashMismatchError)
    } finally {
      await rm(dir, { recursive: true, force: true })
    }
  })

  test("refuses to run when an applied migration file disappeared", async () => {
    const url = await freshDatabaseUrl()
    const dir = await mkdtemp(path.join(tmpdir(), "khala-sync-migrations-"))
    try {
      const file = path.join(dir, "0001_fixture.sql")
      await writeFile(file, "CREATE TABLE fixture_two (id text PRIMARY KEY);\n")
      await runMigrations({ databaseUrl: url, migrationsDir: dir })

      await rm(file)
      await expect(
        runMigrations({ databaseUrl: url, migrationsDir: dir }),
      ).rejects.toBeInstanceOf(MigrationFileMissingError)
    } finally {
      await rm(dir, { recursive: true, force: true })
    }
  })

  test("applies later migrations added after an earlier run, in order", async () => {
    const url = await freshDatabaseUrl()
    const dir = await mkdtemp(path.join(tmpdir(), "khala-sync-migrations-"))
    try {
      await writeFile(
        path.join(dir, "0001_first.sql"),
        "CREATE TABLE ordered_a (id text PRIMARY KEY);\n",
      )
      await runMigrations({ databaseUrl: url, migrationsDir: dir })

      await writeFile(
        path.join(dir, "0002_second.sql"),
        "ALTER TABLE ordered_a ADD COLUMN note text;\n",
      )
      const result = await runMigrations({ databaseUrl: url, migrationsDir: dir })
      expect(result.applied).toEqual(["0002_second.sql"])
      expect(result.plan.alreadyApplied).toEqual(["0001_first.sql"])
    } finally {
      await rm(dir, { recursive: true, force: true })
    }
  })

  test("CLI entrypoint dry-runs and applies via --database-url", async () => {
    const url = await freshDatabaseUrl()
    const script = path.join(import.meta.dir, "..", "scripts", "migrate.ts")

    const dry = Bun.spawnSync(
      ["bun", script, "--dry-run", "--database-url", url],
      { stdout: "pipe", stderr: "pipe" },
    )
    expect(dry.exitCode).toBe(0)
    expect(dry.stdout.toString()).toContain("would apply      0001_khala_sync_core.sql")

    const apply = Bun.spawnSync(["bun", script, "--database-url", url], {
      stdout: "pipe",
      stderr: "pipe",
    })
    expect(apply.exitCode).toBe(0)
    expect(apply.stdout.toString()).toContain("applied          0001_khala_sync_core.sql")

    const again = Bun.spawnSync(["bun", script, "--database-url", url], {
      stdout: "pipe",
      stderr: "pipe",
    })
    expect(again.exitCode).toBe(0)
    expect(again.stdout.toString()).toContain("up to date — nothing to apply")
  })
})
