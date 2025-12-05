/**
 * Git hooks management for .openagents task system
 *
 * Provides install/uninstall for hooks that:
 * - post-merge: Validate tasks.jsonl after pulls
 * - post-checkout: Validate tasks.jsonl after branch switches
 * - pre-commit: Validate tasks.jsonl before commits
 */
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect } from "effect";

export class HooksError extends Error {
  readonly _tag = "HooksError";
  constructor(
    readonly reason: "not_git_repo" | "install_failed" | "uninstall_failed" | "read_error",
    message: string,
  ) {
    super(message);
    this.name = "HooksError";
  }
}

export interface HooksConfig {
  /** Repository root directory */
  rootDir: string;
  /** Path to .openagents directory (default: .openagents) */
  openagentsDir?: string;
}

export interface HooksResult {
  installed: string[];
  skipped: string[];
  errors: string[];
}

const HOOK_NAMES = ["post-merge", "post-checkout", "pre-commit"] as const;
type HookName = typeof HOOK_NAMES[number];

const POST_MERGE_HOOK = `#!/bin/sh
# openagents-hooks-version: 1.0.0
#
# OpenAgents post-merge hook
#
# Validates tasks.jsonl after git pull or merge

# Skip during rebase
if [ -d .git/rebase-merge ] || [ -d .git/rebase-apply ]; then
    exit 0
fi

# Check if tasks.jsonl exists
if [ ! -f .openagents/tasks.jsonl ]; then
    exit 0
fi

# Check for conflict markers
if grep -q "^<<<<<<< " .openagents/tasks.jsonl || grep -q "^======= " .openagents/tasks.jsonl || grep -q "^>>>>>>> " .openagents/tasks.jsonl; then
    echo "⚠️  WARNING: .openagents/tasks.jsonl has conflict markers" >&2
    echo "   Run: bun src/tasks/cli.ts validate --check-conflicts" >&2
    echo "   Or resolve manually and run: bun src/tasks/cli.ts merge" >&2
fi

exit 0
`;

const POST_CHECKOUT_HOOK = `#!/bin/sh
# openagents-hooks-version: 1.0.0
#
# OpenAgents post-checkout hook
#
# Validates tasks.jsonl after branch switch

# Skip if checking out individual files (not a branch switch)
# Args: $1 = prev HEAD, $2 = new HEAD, $3 = 1 for branch checkout, 0 for file checkout
if [ "$3" = "0" ]; then
    exit 0
fi

# Check if tasks.jsonl exists
if [ ! -f .openagents/tasks.jsonl ]; then
    exit 0
fi

# Check for conflict markers
if grep -q "^<<<<<<< " .openagents/tasks.jsonl || grep -q "^======= " .openagents/tasks.jsonl || grep -q "^>>>>>>> " .openagents/tasks.jsonl; then
    echo "⚠️  WARNING: .openagents/tasks.jsonl has conflict markers" >&2
    echo "   Run: bun src/tasks/cli.ts validate --check-conflicts" >&2
fi

exit 0
`;

const PRE_COMMIT_HOOK = `#!/bin/sh
# openagents-hooks-version: 1.0.0
#
# OpenAgents pre-commit hook
#
# Validates tasks.jsonl before committing

# Check if tasks.jsonl is being committed
if ! git diff --cached --name-only | grep -q "^.openagents/tasks.jsonl$"; then
    exit 0
fi

# Check for conflict markers
if git diff --cached .openagents/tasks.jsonl | grep -q "^+<<<<<<< " || git diff --cached .openagents/tasks.jsonl | grep -q "^+======= " || git diff --cached .openagents/tasks.jsonl | grep -q "^+>>>>>>> "; then
    echo "❌ ERROR: Cannot commit .openagents/tasks.jsonl with conflict markers" >&2
    echo "   Run: bun src/tasks/cli.ts validate --check-conflicts" >&2
    echo "   Or resolve manually and run: bun src/tasks/cli.ts merge" >&2
    exit 1
fi

# Validate JSONL format
if command -v bun >/dev/null 2>&1; then
    if ! bun src/tasks/cli.ts validate 2>&1 | grep -q "ok.*true"; then
        echo "❌ ERROR: .openagents/tasks.jsonl validation failed" >&2
        echo "   Run: bun src/tasks/cli.ts validate" >&2
        exit 1
    fi
fi

exit 0
`;

const HOOK_CONTENT: Record<HookName, string> = {
  "post-merge": POST_MERGE_HOOK,
  "post-checkout": POST_CHECKOUT_HOOK,
  "pre-commit": PRE_COMMIT_HOOK,
};

const getHooksDir = (
  rootDir: string,
): Effect.Effect<string, HooksError, FileSystem.FileSystem | Path.Path> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const path = yield* Path.Path;
    const hooksDir = path.join(rootDir, ".git", "hooks");

    const exists = yield* fs.exists(hooksDir).pipe(
      Effect.mapError(
        (e) =>
          new HooksError(
            "read_error",
            `Failed to check hooks directory: ${e.message}`,
          ),
      ),
    );

    if (!exists) {
      return yield* Effect.fail(
        new HooksError(
          "not_git_repo",
          `Not a git repository: ${rootDir}`,
        ),
      );
    }

    return hooksDir;
  });

/**
 * Install git hooks for .openagents task management
 */
export const installHooks = (
  config: HooksConfig,
): Effect.Effect<HooksResult, HooksError, FileSystem.FileSystem | Path.Path> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const path = yield* Path.Path;
    const hooksDir = yield* getHooksDir(config.rootDir);

    const installed: string[] = [];
    const skipped: string[] = [];
    const errors: string[] = [];

    for (const hookName of HOOK_NAMES) {
      const hookPath = path.join(hooksDir, hookName);
      const content = HOOK_CONTENT[hookName];

      try {
        // Check if hook already exists
        const exists = yield* fs.exists(hookPath).pipe(
          Effect.catchAll(() => Effect.succeed(false)),
        );

        if (exists) {
          // Read existing hook to check if it's our version
          const existing = yield* fs.readFileString(hookPath).pipe(
            Effect.catchAll(() => Effect.succeed("")),
          );

          if (existing.includes("openagents-hooks-version")) {
            skipped.push(hookName);
            continue;
          }

          // Backup existing hook
          const backupPath = `${hookPath}.backup`;
          yield* fs.writeFileString(backupPath, existing).pipe(
            Effect.catchAll(() => Effect.succeed(undefined)),
          );
        }

        // Write new hook
        yield* fs.writeFileString(hookPath, content).pipe(
          Effect.mapError(
            (e) =>
              new HooksError(
                "install_failed",
                `Failed to write ${hookName}: ${e.message}`,
              ),
          ),
        );

        // Make executable (chmod +x)
        // Note: Effect Platform doesn't have chmod, so we rely on git hooks being executable by default
        // or the user setting permissions manually

        installed.push(hookName);
      } catch (err) {
        errors.push(`${hookName}: ${(err as Error).message}`);
      }
    }

    return { installed, skipped, errors };
  });

/**
 * Uninstall git hooks for .openagents task management
 */
export const uninstallHooks = (
  config: HooksConfig,
): Effect.Effect<HooksResult, HooksError, FileSystem.FileSystem | Path.Path> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const path = yield* Path.Path;
    const hooksDir = yield* getHooksDir(config.rootDir);

    const installed: string[] = [];
    const skipped: string[] = [];
    const errors: string[] = [];

    for (const hookName of HOOK_NAMES) {
      const hookPath = path.join(hooksDir, hookName);

      try {
        const exists = yield* fs.exists(hookPath).pipe(
          Effect.catchAll(() => Effect.succeed(false)),
        );

        if (!exists) {
          skipped.push(`${hookName} (not installed)`);
          continue;
        }

        // Read hook to verify it's our version
        const content = yield* fs.readFileString(hookPath).pipe(
          Effect.catchAll(() => Effect.succeed("")),
        );

        if (!content.includes("openagents-hooks-version")) {
          skipped.push(`${hookName} (not openagents hook)`);
          continue;
        }

        // Remove hook
        yield* fs.remove(hookPath).pipe(
          Effect.mapError(
            (e) =>
              new HooksError(
                "uninstall_failed",
                `Failed to remove ${hookName}: ${e.message}`,
              ),
          ),
        );

        // Restore backup if it exists
        const backupPath = `${hookPath}.backup`;
        const backupExists = yield* fs.exists(backupPath).pipe(
          Effect.catchAll(() => Effect.succeed(false)),
        );

        if (backupExists) {
          const backupContent = yield* fs.readFileString(backupPath).pipe(
            Effect.catchAll(() => Effect.succeed("")),
          );
          yield* fs.writeFileString(hookPath, backupContent).pipe(
            Effect.catchAll(() => Effect.succeed(undefined)),
          );
          yield* fs.remove(backupPath).pipe(
            Effect.catchAll(() => Effect.succeed(undefined)),
          );
        }

        installed.push(hookName);
      } catch (err) {
        errors.push(`${hookName}: ${(err as Error).message}`);
      }
    }

    return { installed, skipped, errors };
  });
