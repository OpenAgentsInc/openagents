import { tmpdir } from "node:os";
import * as fs from "node:fs";
import * as path from "node:path";
import { execSync } from "node:child_process";
import type { Task } from "../../tasks/schema.js";

export type GoldenLoopFixtureOptions = {
  name?: string;
  task?: Partial<Task>;
  testCommands?: string[];
  allowPush?: boolean;
};

export type GoldenLoopFixture = {
  dir: string;
  openagentsDir: string;
  tasksPath: string;
  taskId: string;
};

/**
 * Creates a reusable Golden Loop-ready git repo with .openagents config.
 * Useful for orchestrator/CLI/overnight regression tests.
 */
export const createGoldenLoopFixture = (options: GoldenLoopFixtureOptions = {}): GoldenLoopFixture => {
  const name = options.name ?? "fixture";
  const dir = fs.mkdtempSync(path.join(tmpdir(), `golden-loop-${name}-`));

  execSync("git init", { cwd: dir, stdio: "ignore" });
  execSync("git checkout -b main", { cwd: dir, stdio: "ignore" });
  execSync('git config user.email "mechacoder@example.com"', { cwd: dir, stdio: "ignore" });
  execSync('git config user.name "MechaCoder"', { cwd: dir, stdio: "ignore" });

  fs.writeFileSync(path.join(dir, "README.md"), "# Golden Loop Fixture Repo\n");

  const now = new Date().toISOString();
  const taskId = options.task?.id ?? `oa-${name}`;
  const task: Task = {
    id: taskId,
    title: options.task?.title ?? `Golden Loop Fixture ${name}`,
    description: options.task?.description ?? "Stub task for Golden Loop regression",
    status: options.task?.status ?? "open",
    priority: options.task?.priority ?? 1,
    type: options.task?.type ?? "task",
    labels: options.task?.labels ?? ["golden-loop"],
    deps: options.task?.deps ?? [],
    commits: options.task?.commits ?? [],
    createdAt: options.task?.createdAt ?? now,
    updatedAt: options.task?.updatedAt ?? now,
    closedAt: options.task?.closedAt ?? null,
  };

  const oaDir = path.join(dir, ".openagents");
  fs.mkdirSync(oaDir, { recursive: true });
  fs.writeFileSync(
    path.join(oaDir, "project.json"),
    JSON.stringify(
      {
        projectId: `proj-${name}`,
        defaultBranch: "main",
        testCommands: options.testCommands ?? ["echo tests"],
        allowPush: options.allowPush ?? false,
      },
      null,
      2,
    ),
  );
  fs.writeFileSync(path.join(oaDir, "tasks.jsonl"), `${JSON.stringify(task)}\n`);

  execSync("git add -A", { cwd: dir, stdio: "ignore" });
  execSync('git commit -m "init"', { cwd: dir, stdio: "ignore" });

  return { dir, openagentsDir: oaDir, tasksPath: path.join(oaDir, "tasks.jsonl"), taskId };
};
