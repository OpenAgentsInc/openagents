import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import { execSync } from "node:child_process";
import * as fs from "node:fs";
import { tmpdir } from "node:os";
import * as path from "node:path";
import { runParallelFromConfig } from "./parallel-runner.js";
import type { OrchestratorConfig, OrchestratorState } from "./types.js";
import type { Task } from "../../tasks/index.js";

const readTasksFile = (tasksPath: string): Task[] =>
  fs
    .readFileSync(tasksPath, "utf-8")
    .trim()
    .split("\n")
    .map((line) => JSON.parse(line) as Task);

const writeTasksFile = (tasksPath: string, tasks: Task[]) => {
  fs.writeFileSync(tasksPath, tasks.map((t) => JSON.stringify(t)).join("\n") + "\n");
};

const git = (repoPath: string, args: string): string =>
  execSync(`git ${args}`, { cwd: repoPath, stdio: "pipe" }).toString().trim();

const createParallelFixture = () => {
  const repoPath = fs.mkdtempSync(path.join(tmpdir(), "parallel-regression-"));
  const remotePath = fs.mkdtempSync(path.join(tmpdir(), "parallel-remote-")) + ".git";

  execSync(`git init --bare ${remotePath}`);
  execSync("git init", { cwd: repoPath, stdio: "ignore" });
  execSync("git checkout -b main", { cwd: repoPath, stdio: "ignore" });
  execSync('git config user.email "mechacoder@example.com"', { cwd: repoPath, stdio: "ignore" });
  execSync('git config user.name "MechaCoder"', { cwd: repoPath, stdio: "ignore" });
  execSync(`git remote add origin ${remotePath}`, { cwd: repoPath, stdio: "ignore" });

  const oaDir = path.join(repoPath, ".openagents");
  fs.mkdirSync(oaDir, { recursive: true });
  fs.writeFileSync(path.join(oaDir, ".gitignore"), "locks/\n");

  fs.writeFileSync(
    path.join(repoPath, "package.json"),
    JSON.stringify({ name: "parallel-regression", version: "0.0.0" }, null, 2),
  );
  fs.writeFileSync(path.join(repoPath, ".gitignore"), ".worktrees/\n");

  const projectConfig = {
    projectId: "parallel-regression",
    defaultBranch: "main",
    testCommands: ["echo tests"],
    allowPush: false,
    parallelExecution: {
      enabled: true,
      maxAgents: 2,
      mergeStrategy: "direct",
    },
  };
  fs.writeFileSync(path.join(oaDir, "project.json"), JSON.stringify(projectConfig, null, 2));

  const now = new Date().toISOString();
  const tasks: Task[] = [
    {
      id: "oa-parallel-1",
      title: "Parallel Task 1",
      description: "stub task one",
      status: "open",
      priority: 1,
      type: "task",
      labels: [],
      deps: [],
      commits: [],
      createdAt: now,
      updatedAt: now,
      closedAt: null,
    },
    {
      id: "oa-parallel-2",
      title: "Parallel Task 2",
      description: "stub task two",
      status: "open",
      priority: 1,
      type: "task",
      labels: [],
      deps: [],
      commits: [],
      createdAt: now,
      updatedAt: now,
      closedAt: null,
    },
  ];

  writeTasksFile(path.join(oaDir, "tasks.jsonl"), tasks);

  execSync("git add -A", { cwd: repoPath, stdio: "ignore" });
  execSync('git commit -m "init parallel regression fixture"', { cwd: repoPath, stdio: "ignore" });
  execSync("git push origin main", { cwd: repoPath, stdio: "ignore" });

  return { repoPath, openagentsDir: oaDir, tasksPath: path.join(oaDir, "tasks.jsonl") };
};

const stubOrchestrator = (config: OrchestratorConfig): Effect.Effect<OrchestratorState> =>
  Effect.sync(() => {
    const openagentsDir = config.openagentsDir ?? path.join(config.cwd, ".openagents");
    const tasksPath = path.join(openagentsDir, "tasks.jsonl");
    const tasks = readTasksFile(tasksPath);
    const taskId = config.task?.id ?? tasks[0]?.id ?? "unknown";
    const updatedTasks = tasks.map((task) =>
      task.id === taskId ? { ...task, status: "closed", updatedAt: new Date().toISOString() } : task,
    ) as Task[];
    writeTasksFile(tasksPath, updatedTasks);

    const markerPath = path.join(config.cwd, `${taskId}.txt`);
    fs.writeFileSync(markerPath, `done ${taskId}`);

    execSync("git add -A", { cwd: config.cwd, stdio: "ignore" });
    execSync(`git commit -m "${taskId}: stub complete"`, { cwd: config.cwd, stdio: "ignore" });

    return {
      sessionId: `stub-${taskId}`,
      task: config.task ?? null,
      subtasks: null,
      progress: null,
      phase: "done",
    } satisfies OrchestratorState;
  });

describe("parallel runner regression", () => {
  test("runs parallel agents on worktrees and keeps main clean", async () => {
    const { repoPath, openagentsDir, tasksPath } = createParallelFixture();
    const tasks = readTasksFile(tasksPath);

    const slots = await Effect.runPromise(
      runParallelFromConfig({
        repoPath,
        openagentsDir,
        baseBranch: "main",
        tasks,
        parallelConfig: {
          enabled: true,
          maxAgents: 2,
          mergeStrategy: "direct",
          worktreeTimeout: 30 * 60 * 1000,
          installTimeoutMs: 15 * 60 * 1000,
          installArgs: ["--frozen-lockfile"],
          useContainers: false,
          mergeThreshold: 4,
          prThreshold: 50,
        },
        testCommands: ["echo tests"],
        ccOnly: true,
        runOrchestratorFn: stubOrchestrator,
      }),
    );

    const completed = slots.filter((slot) => slot.status === "completed");
    expect(completed).toHaveLength(2);

    const updatedTasks = readTasksFile(tasksPath);
    expect(updatedTasks.every((t) => t.status === "closed")).toBe(true);

    const status = git(repoPath, "status --porcelain");
    expect(status).toBe("");

    const worktreesDir = path.join(repoPath, ".worktrees");
    expect(fs.existsSync(worktreesDir)).toBe(true);
    expect(fs.readdirSync(worktreesDir)).toHaveLength(0);

    const agentBranches = git(repoPath, 'branch --list "agent/*"');
    expect(agentBranches).toBe("");
  }, 15000);
});
