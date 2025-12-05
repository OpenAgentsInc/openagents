import { Effect } from "effect";
import { execFileSync, execSync } from "node:child_process";
import * as fs from "node:fs";
import * as nodePath from "node:path";

const normalizePathsForGit = (paths: readonly string[], cwd: string): string[] => {
  const normalized = new Set<string>();

  for (const raw of paths) {
    if (!raw) continue;
    const trimmed = raw.trim();
    if (!trimmed) continue;

    const resolved = nodePath.isAbsolute(trimmed) ? trimmed : nodePath.resolve(cwd, trimmed);
    const relative = nodePath.relative(cwd, resolved);

    // Ignore paths outside the repo
    if (relative.startsWith("..")) continue;

    const gitPath = relative === "" ? "." : relative.split(nodePath.sep).join("/");
    normalized.add(gitPath);
  }

  return Array.from(normalized);
};

export const getCurrentBranch = (
  cwd: string,
): Effect.Effect<string, Error, never> =>
  Effect.tryPromise({
    try: async () =>
      execSync("git rev-parse --abbrev-ref HEAD", {
        cwd,
        encoding: "utf-8",
      }).trim(),
    catch: (error: any) => new Error(`Failed to get current branch: ${error.message}`),
  });

export const pushToRemote = (
  branch: string,
  cwd: string,
): Effect.Effect<void, Error, never> =>
  Effect.tryPromise({
    try: async () => {
      execSync(`git push origin ${branch}`, { cwd, encoding: "utf-8" });
    },
    catch: (error: any) => new Error(`Failed to push: ${error.message}`),
  });

export const createCommit = (
  taskId: string,
  message: string,
  cwd: string,
  paths?: readonly string[],
  debugCommit = false
): Effect.Effect<string, Error, never> =>
  Effect.tryPromise({
    try: async () => {
      const execOptions = { cwd, encoding: "utf-8" as const };

      // If specific paths are provided, normalize them relative to cwd
      const normalizedPaths = paths ? normalizePathsForGit(paths, cwd) : [];

      if (normalizedPaths.length === 0) {
        throw new Error("No paths provided to stage for commit");
      }

      const statusOutput = execFileSync(
        "git",
        ["status", "--porcelain", "--", ...normalizedPaths],
        execOptions
      );
      if (debugCommit) {
        console.log("[createCommit] normalizedPaths:", normalizedPaths);
        console.log("[createCommit] statusOutput:", statusOutput);
      }
      const stageable = Array.from(
        new Set(
          statusOutput
            .split("\n")
            .map((line) => line)
            .filter((line) => line.trim().length > 0)
            .map((line) => {
              const status = line.slice(0, 2);
              const pathPart = line.slice(3);
              const renameParts = pathPart.split(" -> ");
              const candidate = renameParts[renameParts.length - 1]?.trim() ?? "";
              const absolutePath = nodePath.resolve(cwd, candidate);
              const exists = fs.existsSync(absolutePath);
              const isDeletion = status.includes("D");
              return !candidate || (!exists && !isDeletion) ? "" : candidate;
            })
            .filter((line) => line.length > 0)
        )
      );

      if (stageable.length === 0) {
        throw new Error(
          `No matching changes found for provided paths: ${normalizedPaths.join(", ")}`
        );
      }

      if (debugCommit) {
        console.log("[createCommit] stageable:", stageable);
      }

      execFileSync("git", ["add", "--", ...stageable], execOptions);

      const fullMessage = `${taskId}: ${message}

🤖 Generated with [OpenAgents](https://openagents.com)

Co-Authored-By: MechaCoder <noreply@openagents.com>
`;

      execFileSync("git", ["commit", "-F", "-"], {
        ...execOptions,
        input: fullMessage,
      });

      const sha = execFileSync("git", ["rev-parse", "HEAD"], execOptions).trim();
      return sha;
    },
    catch: (error: any) => new Error(`Failed to create commit: ${error.message}`),
  });
