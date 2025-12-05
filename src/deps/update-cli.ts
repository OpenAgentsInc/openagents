#!/usr/bin/env bun
import { updateDependencies, restoreLockfile, verifyInstall } from "./update.js";

const args = process.argv.slice(2);
const packages: string[] = [];
let dryRun = false;
let backupPath: string | undefined;
let rollbackPath: string | undefined;

for (let i = 0; i < args.length; i++) {
  const arg = args[i];
  if (arg === "--dry-run" || arg === "-n") {
    dryRun = true;
  } else if (arg === "--backup" || arg === "-b") {
    backupPath = args[i + 1];
    i++;
  } else if (arg === "--rollback" || arg === "-r") {
    rollbackPath = args[i + 1];
    i++;
  } else if (arg === "--help" || arg === "-h") {
    console.log(`Usage: bun src/deps/update-cli.ts [OPTIONS] [PACKAGES...]

Update Bun dependencies safely with automatic backup and rollback.

OPTIONS:
  --dry-run, -n         Preview updates without applying
  --backup, -b <path>   Custom backup path (default: .openagents/deps/backups/bun.lockb.YYYY-MM-DD.backup)
  --rollback, -r <path> Rollback to a previous backup
  --help, -h            Show this help

EXAMPLES:
  # Update all dependencies (with backup)
  bun run deps:update

  # Dry run to preview updates
  bun run deps:update --dry-run

  # Update specific packages
  bun run deps:update effect @effect/platform

  # Rollback to a backup
  bun run deps:update --rollback .openagents/deps/backups/bun.lockb.2025-12-05.backup

WORKFLOW:
  1. Run audit: bun run deps:audit
  2. Preview updates: bun run deps:update --dry-run
  3. Apply updates: bun run deps:update
  4. Test: bun test
  5. Rollback if needed: bun run deps:update --rollback <backup-path>
`);
    process.exit(0);
  } else if (!arg.startsWith("-")) {
    packages.push(arg);
  }
}

const main = () => {
  // Handle rollback
  if (rollbackPath) {
    try {
      console.log(`Rolling back to: ${rollbackPath}`);
      restoreLockfile(rollbackPath);
      console.log("✅ Lockfile restored");

      console.log("Verifying installation...");
      if (verifyInstall()) {
        console.log("✅ Installation verified");
        process.exit(0);
      } else {
        console.error("❌ Installation verification failed");
        process.exit(1);
      }
    } catch (err) {
      console.error(`❌ Rollback failed: ${(err as Error).message}`);
      process.exit(1);
    }
    return;
  }

  // Handle update
  console.log(dryRun ? "🔍 Previewing updates..." : "📦 Updating dependencies...");
  if (packages.length > 0) {
    console.log(`Packages: ${packages.join(", ")}`);
  } else {
    console.log("Packages: all");
  }

  const result = updateDependencies(
    backupPath ? { packages, dryRun, backupPath: backupPath } : { packages, dryRun },
  );

  if (!result.success) {
    console.error(`❌ Update failed: ${result.error}`);
    if (result.backupPath) {
      console.log(`Backup available at: ${result.backupPath}`);
    }
    process.exit(1);
  }

  if (dryRun) {
    console.log("✅ Dry run completed");
    console.log("Run without --dry-run to apply updates");
  } else {
    console.log("✅ Dependencies updated");
    if (result.backupPath) {
      console.log(`📁 Backup saved to: ${result.backupPath}`);
      console.log(`   Rollback with: bun run deps:update --rollback ${result.backupPath}`);
    }
    console.log("\nNext steps:");
    console.log("  1. Run tests: bun test");
    console.log("  2. If tests fail, rollback with the command above");
    console.log("  3. If tests pass, commit the updated bun.lockb");
  }

  process.exit(0);
};

main();
