import { expect, test } from "bun:test";
import { Effect } from "effect";
import { execSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import * as path from "node:path";
import { createCommit } from "./services/git-service.js";

const setupRepo = () => {
  const dir = mkdtempSync(path.join(tmpdir(), "commit-scope-"));
  execSync("git init", { cwd: dir, stdio: "pipe" });
  execSync('git config user.email "test@example.com"', { cwd: dir, stdio: "pipe" });
  execSync('git config user.name "Test User"', { cwd: dir, stdio: "pipe" });
  writeFileSync(path.join(dir, "tracked.txt"), "initial\n");
  execSync("git add tracked.txt", { cwd: dir, stdio: "pipe" });
  execSync('git commit -m "init"', { cwd: dir, stdio: "pipe" });
  return dir;
};

test("createCommit stages only provided paths", async () => {
  const dir = setupRepo();
  writeFileSync(path.join(dir, "tracked.txt"), "changed\n");
  writeFileSync(path.join(dir, "unrelated.txt"), "leave alone\n");

  const sha = await Effect.runPromise(
    createCommit("oa-f0dd88", "Limit staging to tracked", dir, ["tracked.txt"])
  );

  const committedFiles = execSync("git show --name-only --pretty=format:", {
    cwd: dir,
    encoding: "utf-8",
  })
    .split("\n")
    .filter((line) => line.trim().length > 0);

  expect(committedFiles).toEqual(["tracked.txt"]);
  expect(sha).toMatch(/^[0-9a-f]{7,}$/);

  const status = execSync("git status --porcelain", { cwd: dir, encoding: "utf-8" });
  expect(status).toContain("?? unrelated.txt");

  const lastMessage = execSync("git log -1 --pretty=%B", { cwd: dir, encoding: "utf-8" });
  expect(lastMessage).toContain("oa-f0dd88: Limit staging to tracked");
});

test("createCommit normalizes absolute and directory paths", async () => {
  const dir = setupRepo();
  const oaDir = path.join(dir, ".openagents");
  mkdirSync(oaDir, { recursive: true });
  writeFileSync(path.join(oaDir, "tasks.jsonl"), "[]");

  const sha = await Effect.runPromise(
    createCommit("oa-f0dd88", "Commit metadata only", dir, [oaDir])
  );

  const committedFiles = execSync("git show --name-only --pretty=format:", {
    cwd: dir,
    encoding: "utf-8",
  })
    .split("\n")
    .filter((line) => line.trim().length > 0);

  expect(committedFiles).toEqual([".openagents/tasks.jsonl"]);
  expect(sha).toMatch(/^[0-9a-f]{7,}$/);
});
