import * as BunContext from "@effect/platform-bun/BunContext";
import { Effect, Layer } from "effect";
import * as fs from "node:fs";
import * as nodePath from "node:path";
import { readTasks } from "./service.js";
import { makeDatabaseLive } from "../storage/database.js";

const DATABASE_REL_PATH = nodePath.join(".openagents", "openagents.db");

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

export const checkTasksIntegrity = async (
  options: IntegrityOptions = {},
): Promise<IntegrityResult> => {
  const rootDir = options.rootDir ?? process.cwd();
  const dbPath = nodePath.join(rootDir, DATABASE_REL_PATH);
  const issues: IntegrityIssue[] = [];
  const warnings: IntegrityIssue[] = [];

  if (!fs.existsSync(dbPath)) {
    return {
      ok: false,
      issues: [
        {
          type: "missing",
          message: `${DATABASE_REL_PATH} is missing; agents rely on the SQLite task database.`,
        },
      ],
      warnings,
    };
  }

  try {
    const databaseLayer = Layer.mergeAll(
      makeDatabaseLive(dbPath),
      BunContext.layer,
    );
    await Effect.runPromise(
      readTasks().pipe(Effect.provide(databaseLayer)),
    );
  } catch (error) {
    issues.push({
      type: "schema_invalid",
      message: `Failed to read tasks: ${(error as Error).message}`,
      hint: "Ensure the SQLite database is accessible and valid.",
    });
  }

  return {
    ok: issues.length === 0,
    issues,
    warnings,
  };
};
