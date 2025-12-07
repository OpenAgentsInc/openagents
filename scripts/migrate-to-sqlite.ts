#!/usr/bin/env bun
/**
 * Migration Script: JSONL → SQLite
 *
 * One-time migration to convert existing tasks.jsonl to SQLite database.
 *
 * Usage:
 *   bun scripts/migrate-to-sqlite.ts [--dry-run] [--force]
 *
 * Flags:
 *   --dry-run: Preview what would be migrated without making changes
 *   --force: Overwrite existing database if it exists
 */

import * as BunContext from "@effect/platform-bun/BunContext";
import { Effect } from "effect";
import { importTasksFromJsonl, dryRunImport } from "../src/storage/import-jsonl.js";
import * as fs from "node:fs";
import * as path from "node:path";

const migrate = Effect.gen(function* () {
  const args = process.argv.slice(2);
  const isDryRun = args.includes("--dry-run");
  const force = args.includes("--force");

  const rootDir = process.cwd();
  const jsonlPath = path.join(rootDir, ".openagents", "tasks.jsonl");
  const dbPath = path.join(rootDir, ".openagents", "openagents.db");
  const backupPath = path.join(rootDir, ".openagents", "tasks.jsonl.backup");

  console.log("=".repeat(70));
  console.log("JSONL to SQLite Migration Script");
  console.log("=".repeat(70));
  console.log(`Root directory: ${rootDir}`);
  console.log(`Source: ${jsonlPath}`);
  console.log(`Target: ${dbPath}`);
  console.log(`Mode: ${isDryRun ? "DRY RUN" : "LIVE MIGRATION"}`);
  console.log("");

  // Check if source exists
  if (!fs.existsSync(jsonlPath)) {
    console.error(`❌ Error: Source file not found: ${jsonlPath}`);
    console.error("\nPlease ensure .openagents/tasks.jsonl exists before running migration.");
    process.exit(1);
  }

  // Check if target exists (and force not specified)
  if (fs.existsSync(dbPath) && !force && !isDryRun) {
    console.error(`❌ Error: Database already exists: ${dbPath}`);
    console.error("\nOptions:");
    console.error("  1. Delete the existing database manually");
    console.error("  2. Run with --force to overwrite");
    console.error("  3. Run with --dry-run to preview migration");
    process.exit(1);
  }

  // Dry run mode
  if (isDryRun) {
    console.log("Running dry run analysis...\n");

    const analysis = yield* dryRunImport(jsonlPath);

    console.log("\nDry Run Results:");
    console.log(`  Tasks to import: ${analysis.taskCount}`);
    console.log(`  Dependencies to import: ${analysis.depCount}`);
    console.log(`  Deletions file found: ${analysis.hasDeleteions ? "Yes" : "No"}`);

    console.log("\nTo proceed with actual migration:");
    console.log(`  bun scripts/migrate-to-sqlite.ts`);

    return;
  }

  // Live migration
  console.log("Step 1: Creating backup...");
  try {
    fs.copyFileSync(jsonlPath, backupPath);
    console.log(`✓ Backup created: ${backupPath}`);
  } catch (e) {
    console.error(`❌ Failed to create backup: ${e}`);
    process.exit(1);
  }

  // If database exists and force is true, delete it
  if (fs.existsSync(dbPath) && force) {
    console.log("\nStep 2: Removing existing database (--force)...");
    fs.unlinkSync(dbPath);
    console.log("✓ Existing database removed");
  }

  console.log("\nStep 3: Running import...");
  const result = yield* importTasksFromJsonl(jsonlPath, dbPath);

  if (!result.validationPassed) {
    console.error("\n❌ Migration validation FAILED!");
    console.error("\nErrors:");
    for (const error of result.errors) {
      console.error(`  - ${error}`);
    }

    console.error("\nRollback instructions:");
    console.error(`  mv ${backupPath} ${jsonlPath}`);
    console.error(`  rm ${dbPath}`);

    process.exit(1);
  }

  console.log("\nStep 4: Renaming original JSONL...");
  const migratedPath = jsonlPath + ".migrated";
  fs.renameSync(jsonlPath, migratedPath);
  console.log(`✓ Renamed to ${migratedPath}`);

  console.log("\n" + "=".repeat(70));
  console.log("✅ MIGRATION COMPLETED SUCCESSFULLY!");
  console.log("=".repeat(70));
  console.log(`\nImported: ${result.tasksImported} tasks, ${result.dependenciesImported} dependencies`);

  console.log("\nNext steps:");
  console.log("  1. Run tests: bun test src/tasks/");
  console.log("  2. Verify tasks: bun src/tasks/cli.ts list");
  console.log("  3. Try ready tasks: bun src/tasks/cli.ts ready");

  console.log("\nBackup files (keep these for safety):");
  console.log(`  ${backupPath}`);
  console.log(`  ${migratedPath}`);

  console.log("\nRollback (if needed):");
  console.log(`  mv ${backupPath} ${jsonlPath}`);
  console.log(`  rm ${dbPath}`);
});

Effect.runPromise(migrate.pipe(Effect.provide(BunContext.layer)))
  .then(() => process.exit(0))
  .catch((err) => {
    console.error("\n❌ Migration failed:", err);
    console.error("\nStack trace:");
    console.error(err.stack);
    process.exit(1);
  });
