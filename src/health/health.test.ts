import { describe, expect, test } from "bun:test";
import * as fs from "node:fs";
import * as path from "node:path";
import { runHealthChecks } from "./health.js";

const writeProjectConfig = (dir: string, typecheckCmds: string[], testCmds: string[], e2eCmds?: string[]) => {
  const cfg = {
    version: 1,
    projectId: "test",
    defaultBranch: "main",
    defaultModel: "x",
    rootDir: ".",
    typecheckCommands: typecheckCmds,
    testCommands: testCmds,
    e2eCommands: e2eCmds ?? [],
  };
  fs.mkdirSync(path.join(dir, ".openagents"), { recursive: true });
  fs.writeFileSync(path.join(dir, ".openagents", "project.json"), JSON.stringify(cfg, null, 2));
};

describe("runHealthChecks", () => {
  test("passes when all commands succeed", async () => {
    const dir = fs.mkdtempSync(path.join(fs.realpathSync("/tmp"), "health-ok-"));
    writeProjectConfig(dir, ["bash -c 'exit 0'"], ["bash -c 'exit 0'"], ["bash -c 'exit 0'"]);

    const result = await runHealthChecks(dir);
    expect(result.ok).toBe(true);
    expect(result.results).toHaveLength(3);
    expect(result.results.every((r) => r.exitCode === 0)).toBe(true);
  });

  test("fails when any command fails", async () => {
    const dir = fs.mkdtempSync(path.join(fs.realpathSync("/tmp"), "health-fail-"));
    writeProjectConfig(dir, ["bash -c 'exit 0'"], ["bash -c 'exit 2'"]);

    const result = await runHealthChecks(dir);
    expect(result.ok).toBe(false);
    const failing = result.results.find((r) => r.exitCode !== 0);
    expect(failing?.kind).toBe("test");
  });

  test("throws when project config missing", async () => {
    const dir = fs.mkdtempSync(path.join(fs.realpathSync("/tmp"), "health-missing-"));
    await expect(runHealthChecks(dir)).rejects.toThrow(/Project config not found/);
  });
});
