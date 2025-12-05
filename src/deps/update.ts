import { spawnSync } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";

export interface UpdateOptions {
  packages?: string[]; // specific packages to update, or all if empty
  dryRun?: boolean;
  backupPath?: string;
}

export interface UpdateResult {
  success: boolean;
  packagesUpdated: string[];
  backupPath?: string;
  error?: string;
}

const runCommand = (
  cmd: string,
  args: string[],
  opts?: { cwd?: string },
): { success: boolean; stdout?: string; error?: string } => {
  const proc = spawnSync(cmd, args, { encoding: "utf8", shell: false, cwd: opts?.cwd });
  if (proc.error) return { success: false, error: proc.error.message };
  if (proc.status !== 0) return { success: false, error: proc.stderr || "Command failed" };
  return { success: true, stdout: proc.stdout };
};

/**
 * Backup the lockfile before making changes
 */
export const backupLockfile = (backupPath?: string): string => {
  const lockfile = "bun.lockb";
  const timestamp = new Date().toISOString().replace(/[:.]/g, "-").split("T")[0];
  const defaultBackup = `.openagents/deps/backups/bun.lockb.${timestamp}.backup`;
  const targetPath = backupPath || defaultBackup;

  fs.mkdirSync(path.dirname(targetPath), { recursive: true });
  fs.copyFileSync(lockfile, targetPath);
  return targetPath;
};

/**
 * Restore lockfile from backup
 */
export const restoreLockfile = (backupPath: string): void => {
  if (!fs.existsSync(backupPath)) {
    throw new Error(`Backup file not found: ${backupPath}`);
  }
  fs.copyFileSync(backupPath, "bun.lockb");
};

/**
 * Safely update dependencies
 */
export const updateDependencies = (options: UpdateOptions = {}): UpdateResult => {
  const { packages = [], dryRun = false, backupPath } = options;

  // Backup lockfile first
  let backup: string | undefined;
  if (!dryRun) {
    try {
      backup = backupLockfile(backupPath);
    } catch (err) {
      return {
        success: false,
        packagesUpdated: [],
        error: `Failed to backup lockfile: ${(err as Error).message}`,
      };
    }
  }

  // Run bun update
  const args = ["update"];
  if (packages.length > 0) {
    args.push(...packages);
  }
  if (dryRun) {
    args.push("--dry");
  }

  const result = runCommand("bun", args);

  if (!result.success) {
    // Rollback on failure
    if (backup) {
      try {
        restoreLockfile(backup);
      } catch (rollbackErr) {
        return {
          success: false,
          packagesUpdated: [],
          backupPath: backup,
          error: `Update failed and rollback failed: ${result.error}. Backup at: ${backup}`,
        };
      }
    }
    return {
      success: false,
      packagesUpdated: [],
      backupPath: backup,
      error: result.error,
    };
  }

  // Parse output to determine what was updated
  const updatedPackages = packages.length > 0 ? packages : ["all"];

  return {
    success: true,
    packagesUpdated: updatedPackages,
    backupPath: backup,
  };
};

/**
 * Verify installation after update
 */
export const verifyInstall = (): boolean => {
  const result = runCommand("bun", ["install", "--frozen-lockfile"]);
  return result.success;
};
