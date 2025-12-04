import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import { execSync } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";
import { tmpdir } from "node:os";
import { mergeAgentBranchForTests } from "./parallel-runner.js";

const runGit = (repo: string, args: string | string[]) => {
  const parts = Array.isArray(args) ? args : args.split(" ");
  return execSync(["git", ...parts].join(" "), { cwd: repo, stdio: "pipe" })
    .toString()
    .trim();
};

const createConflictRepo = () => {
  const repoPath = fs.mkdtempSync(path.join(tmpdir(), "parallel-merge-conflict-"));
  runGit(repoPath, "init");
  runGit(repoPath, 'config user.email "mechacoder@example.com"');
  runGit(repoPath, 'config user.name "MechaCoder"');
  runGit(repoPath, "checkout -b main");

  const filePath = path.join(repoPath, "file.txt");
  fs.writeFileSync(filePath, "base\n");
  runGit(repoPath, "add file.txt");
  runGit(repoPath, 'commit -m "main base"');

  // Create agent branch with change
  runGit(repoPath, "checkout -b agent/conflict");
  fs.writeFileSync(filePath, "agent change\n");
  runGit(repoPath, "add file.txt");
  runGit(repoPath, 'commit -m "agent change"');

  // Return to main and introduce conflicting change
  runGit(repoPath, "checkout main");
  fs.writeFileSync(filePath, "main change\n");
  runGit(repoPath, "add file.txt");
  runGit(repoPath, 'commit -m "main change"');

  return { repoPath, branch: "agent/conflict", filePath };
};

describe("mergeAgentBranch cleanup", () => {
  test("cleans merge conflicts and restores clean working tree on failure", async () => {
    const { repoPath, branch, filePath } = createConflictRepo();

    const result = await Effect.runPromiseExit(mergeAgentBranchForTests(repoPath, branch));

    expect(result._tag).toBe("Failure");

    // Working tree should remain clean (no conflict markers or staged files)
    const status = runGit(repoPath, "status --porcelain");
    expect(status).toBe("");

    const contents = fs.readFileSync(filePath, "utf-8");
    expect(contents).toContain("main change");
    expect(contents).not.toContain(">>>>>>");
    expect(contents).not.toContain("<<<<<<<");
  });
});
