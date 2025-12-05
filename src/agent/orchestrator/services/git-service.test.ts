import { describe, expect, it } from "bun:test";
import { mkdtempSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { execSync } from "node:child_process";
import { Effect } from "effect";
import { getCurrentBranch, getHeadCommit, getStagedFiles, isDirty } from "./git-service.js";

const runEffect = async <A>(program: Effect.Effect<A, Error>) =>
  Effect.runPromise(program.pipe(Effect.orDie));

describe("git-service", () => {
  it("reads branch, head commit, staged files, and dirty state", async () => {
    const dir = mkdtempSync("/tmp/git-service-");
    execSync("git init -b main", { cwd: dir });

    const filePath = join(dir, "file.txt");
    writeFileSync(filePath, "hello", "utf8");
    execSync("git add file.txt", { cwd: dir });
    execSync('git commit -m "init"', { cwd: dir });

    const branch = await runEffect(getCurrentBranch(dir));
    expect(branch).toBe("main");

    const head = await runEffect(getHeadCommit(dir));
    expect(head).toMatch(/[0-9a-f]{7,40}/);

    expect(await runEffect(isDirty(dir))).toBe(false);

    writeFileSync(filePath, "hello world", "utf8");
    expect(await runEffect(isDirty(dir))).toBe(true);

    execSync("git add file.txt", { cwd: dir });
    const staged = await runEffect(getStagedFiles(dir));
    expect(staged).toContain("file.txt");
  });
});
