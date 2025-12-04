import { describe, expect, test } from "bun:test";
import * as fs from "node:fs";
import * as path from "node:path";
import { execSync } from "node:child_process";
import { createGoldenLoopFixture } from "./golden-loop-fixture.js";

const readJson = (filePath: string) => JSON.parse(fs.readFileSync(filePath, "utf-8"));

describe("createGoldenLoopFixture", () => {
  test("creates a git-backed repo with .openagents config and open task", () => {
    const fixture = createGoldenLoopFixture({
      name: "regression",
      testCommands: ["echo ok"],
      allowPush: true,
    });

    // project.json is present with requested settings
    const projectPath = path.join(fixture.openagentsDir, "project.json");
    const project = readJson(projectPath);
    expect(project.defaultBranch).toBe("main");
    expect(project.testCommands).toEqual(["echo ok"]);
    expect(project.allowPush).toBe(true);

    // tasks.jsonl contains a single open task
    const tasks = fs
      .readFileSync(fixture.tasksPath, "utf-8")
      .trim()
      .split("\n")
      .map((line) => JSON.parse(line));
    expect(tasks).toHaveLength(1);
    expect(tasks[0].id).toBe(fixture.taskId);
    expect(tasks[0].status).toBe("open");

    // repository has an initial commit
    const log = execSync("git log --oneline -1", { cwd: fixture.dir, encoding: "utf-8" });
    expect(log).toContain("init");
  });

  test("respects task overrides", () => {
    const fixture = createGoldenLoopFixture({
      name: "custom-task",
      task: {
        id: "oa-custom",
        priority: 0,
        labels: ["golden-loop", "custom"],
      },
    });

    const tasks = fs
      .readFileSync(fixture.tasksPath, "utf-8")
      .trim()
      .split("\n")
      .map((line) => JSON.parse(line));

    expect(tasks[0].id).toBe("oa-custom");
    expect(tasks[0].priority).toBe(0);
    expect(tasks[0].labels).toEqual(["golden-loop", "custom"]);
  });

  test("runs setup hook before initial commit", () => {
    const fixture = createGoldenLoopFixture({
      name: "with-setup",
      setup: (dir) => {
        const scriptsDir = path.join(dir, "scripts");
        fs.mkdirSync(scriptsDir, { recursive: true });
        fs.writeFileSync(path.join(scriptsDir, "custom.sh"), "echo setup", "utf-8");
      },
    });

    const files = execSync("git ls-files", { cwd: fixture.dir, encoding: "utf-8" });
    expect(files).toContain("scripts/custom.sh");
  });
});
