import { describe, expect, test, afterEach } from "bun:test";
import * as fs from "node:fs";
import * as os from "node:os";
import * as nodePath from "node:path";
import { spawnSync } from "node:child_process";
import { checkTasksIntegrity } from "./integrity.js";

const git = (cwd: string, args: string[]) => {
  const result = spawnSync("git", ["-C", cwd, ...args], { encoding: "utf8" });
  if (result.status !== 0) {
    throw new Error(
      `git ${args.join(" ")} failed: ${result.stderr || result.stdout}`,
    );
  }
  return result;
};

const makeTaskPayload = (): string =>
  JSON.stringify({
    id: "oa-test",
    title: "Test task",
    description: "",
    status: "open",
    priority: 2,
    type: "task",
    labels: [],
    deps: [],
    commits: [],
    createdAt: "2024-01-01T00:00:00.000Z",
    updatedAt: "2024-01-01T00:00:00.000Z",
    closedAt: null,
  });

const setupRepo = (): string => {
  const dir = fs.mkdtempSync(nodePath.join(os.tmpdir(), "tasks-integrity-"));
  git(dir, ["init", "-q"]);
  fs.mkdirSync(nodePath.join(dir, ".openagents"), { recursive: true });
  fs.writeFileSync(
    nodePath.join(dir, ".openagents", "tasks.jsonl"),
    `${makeTaskPayload()}\n`,
  );
  git(dir, ["add", ".openagents/tasks.jsonl"]);
  return dir;
};

const cleanupDirs: string[] = [];
afterEach(() => {
  while (cleanupDirs.length > 0) {
    const dir = cleanupDirs.pop();
    if (dir) {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  }
});

describe("checkTasksIntegrity", () => {
  test("passes when tasks file is tracked and valid", async () => {
    const repo = setupRepo();
    cleanupDirs.push(repo);

    const result = await checkTasksIntegrity({ rootDir: repo });

    expect(result.ok).toBe(true);
    expect(result.issues).toHaveLength(0);
  });

  test("flags skip-worktree and can clear it with --fix", async () => {
    const repo = setupRepo();
    cleanupDirs.push(repo);
    git(repo, [
      "update-index",
      "--skip-worktree",
      ".openagents/tasks.jsonl",
    ]);

    const result = await checkTasksIntegrity({ rootDir: repo });
    expect(result.ok).toBe(false);
    expect(result.issues.some((issue) => issue.type === "skip_worktree")).toBe(
      true,
    );

    const fixed = await checkTasksIntegrity({ rootDir: repo, fix: true });
    expect(fixed.ok).toBe(true);
    expect(
      fixed.warnings.some((warning) => warning.type === "skip_worktree"),
    ).toBe(true);
  });

  test("fails on schema errors", async () => {
    const repo = setupRepo();
    cleanupDirs.push(repo);
    fs.writeFileSync(
      nodePath.join(repo, ".openagents", "tasks.jsonl"),
      '{"id":123}\n',
    );
    git(repo, ["add", ".openagents/tasks.jsonl"]);

    const result = await checkTasksIntegrity({ rootDir: repo });

    expect(result.ok).toBe(false);
    expect(
      result.issues.some((issue) => issue.type === "schema_invalid"),
    ).toBe(true);
  });
});
