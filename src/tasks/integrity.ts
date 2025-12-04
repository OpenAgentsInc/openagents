import * as BunContext from "@effect/platform-bun/BunContext";
import { Effect } from "effect";
import { spawnSync } from "node:child_process";
import * as fs from "node:fs";
import * as nodePath from "node:path";
import { readTasks } from "./service.js";

const TASKS_REL_PATH = nodePath.join(".openagents", "tasks.jsonl");

export type IntegrityIssueType =
  | "missing"
  | "not_tracked"
  | "skip_worktree"
  | "assume_unchanged"
  | "schema_invalid"
  | "git_error";

export interface IntegrityIssue {
  type: IntegrityIssueType;
  message: string;
  hint?: string;
}

export interface IntegrityResult {
  ok: boolean;
  issues: IntegrityIssue[];
  warnings: IntegrityIssue[];
}

export interface IntegrityOptions {
  rootDir?: string;
  fix?: boolean;
}

const runGit = (
  rootDir: string,
  args: string[],
): { ok: boolean; stdout: string; stderr: string; exitCode: number } => {
  const result = spawnSync("git", ["-C", rootDir, ...args], {
    encoding: "utf8",
  });
  return {
    ok: result.status === 0,
    stdout: result.stdout ?? "",
    stderr: result.stderr ?? "",
    exitCode: result.status ?? 1,
  };
};

const parseTag = (line: string): string | null => {
  const trimmed = line.trim();
  return trimmed.length > 0 ? trimmed[0] : null;
};

const isAssumeUnchangedTag = (tag: string): boolean =>
  tag.length === 1 && tag === tag.toLowerCase() && tag !== "?";

export const checkTasksIntegrity = async (
  options: IntegrityOptions = {},
): Promise<IntegrityResult> => {
  const rootDir = options.rootDir ?? process.cwd();
  const tasksPath = nodePath.join(rootDir, TASKS_REL_PATH);
  const issues: IntegrityIssue[] = [];
  const warnings: IntegrityIssue[] = [];

  if (!fs.existsSync(tasksPath)) {
    return {
      ok: false,
      issues: [
        {
          type: "missing",
          message: `${TASKS_REL_PATH} is missing; agents rely on this file.`,
        },
      ],
      warnings,
    };
  }

  const lsFilesTag = runGit(rootDir, ["ls-files", "-t", TASKS_REL_PATH]);
  if (!lsFilesTag.ok) {
    warnings.push({
      type: "git_error",
      message:
        lsFilesTag.stderr.trim() ||
        `git ls-files failed with exit code ${lsFilesTag.exitCode}`,
      hint:
        "Run this check inside a git repo so skip-worktree/assume-unchanged bits are visible.",
    });
  } else {
    const tag = parseTag(lsFilesTag.stdout);
    if (!tag) {
      issues.push({
        type: "not_tracked",
        message: `${TASKS_REL_PATH} is not tracked by git; agents may miss updates.`,
        hint: "Run `git add .openagents/tasks.jsonl` to track the file.",
      });
    } else {
      const isSkip = tag.toUpperCase() === "S";
      if (isSkip) {
        const cleared =
          options.fix &&
          runGit(rootDir, [
            "update-index",
            "--no-skip-worktree",
            TASKS_REL_PATH,
          ]).ok;
        if (cleared) {
          warnings.push({
            type: "skip_worktree",
            message: `Cleared skip-worktree bit on ${TASKS_REL_PATH}.`,
          });
        } else {
          issues.push({
            type: "skip_worktree",
            message: `${TASKS_REL_PATH} is marked skip-worktree; agent changes may be ignored.`,
            hint: `Run: git update-index --no-skip-worktree ${TASKS_REL_PATH}`,
          });
        }
      }

      const lsVerbose = runGit(rootDir, ["ls-files", "-v", TASKS_REL_PATH]);
      if (lsVerbose.ok) {
        const verboseTag = parseTag(lsVerbose.stdout);
        if (verboseTag && isAssumeUnchangedTag(verboseTag)) {
          warnings.push({
            type: "assume_unchanged",
            message: `${TASKS_REL_PATH} is marked assume-unchanged; changes may be skipped.`,
            hint: `Run: git update-index --no-assume-unchanged ${TASKS_REL_PATH}`,
          });
        }
      }
    }
  }

  try {
    await Effect.runPromise(
      readTasks(tasksPath).pipe(Effect.provide(BunContext.layer)),
    );
  } catch (error) {
    issues.push({
      type: "schema_invalid",
      message: `Schema validation failed: ${(error as Error).message}`,
      hint: "Fix the JSONL entry or regenerate the file.",
    });
  }

  return {
    ok: issues.length === 0,
    issues,
    warnings,
  };
};
