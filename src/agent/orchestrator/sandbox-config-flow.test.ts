/**
 * Tests for sandbox configuration flow through the agent system.
 * Verifies that sandbox config is properly passed from CLI -> overnight -> parallel-runner -> orchestrator
 */
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import * as BunContext from "@effect/platform-bun/BunContext";
import { execSync } from "node:child_process";
import * as fs from "node:fs";
import { tmpdir } from "node:os";
import * as path from "node:path";
import { runParallelFromConfig } from "./parallel-runner.js";
import type { Task } from "../../tasks/index.js";
import type { OrchestratorConfig, OrchestratorState } from "./types.js";

const readTasksFile = (tasksPath: string): Task[] =>
  fs
    .readFileSync(tasksPath, "utf-8")
    .trim()
    .split("\n")
    .map((line) => JSON.parse(line) as Task);

const writeTasksFile = (tasksPath: string, tasks: Task[]) => {
  fs.writeFileSync(tasksPath, tasks.map((t) => JSON.stringify(t)).join("\n") + "\n");
};

/**
 * Creates a minimal fixture repo for testing sandbox config flow
 */
const createSandboxConfigFixture = () => {
  const repoPath = fs.mkdtempSync(path.join(tmpdir(), "sandbox-config-test-"));
  const remotePath = fs.mkdtempSync(path.join(tmpdir(), "sandbox-config-remote-")) + ".git";

  execSync(`git init --bare ${remotePath}`);
  execSync("git init", { cwd: repoPath, stdio: "ignore" });
  execSync("git checkout -b main", { cwd: repoPath, stdio: "ignore" });
  execSync('git config user.email "test@example.com"', { cwd: repoPath, stdio: "ignore" });
  execSync('git config user.name "Test"', { cwd: repoPath, stdio: "ignore" });
  execSync(`git remote add origin ${remotePath}`, { cwd: repoPath, stdio: "ignore" });

  const oaDir = path.join(repoPath, ".openagents");
  fs.mkdirSync(oaDir, { recursive: true });
  fs.writeFileSync(path.join(oaDir, ".gitignore"), "locks/\n");

  fs.writeFileSync(
    path.join(repoPath, "package.json"),
    JSON.stringify({ name: "sandbox-config-test", version: "0.0.0" }, null, 2),
  );
  fs.writeFileSync(path.join(repoPath, ".gitignore"), ".worktrees/\n");

  const projectConfig = {
    projectId: "sandbox-config-test",
    defaultBranch: "main",
    testCommands: ["echo tests"],
    allowPush: false,
    parallelExecution: {
      enabled: true,
      maxAgents: 1,
      mergeStrategy: "direct",
    },
    sandbox: {
      enabled: true,
      backend: "macos-container" as const,
      memoryLimit: "4G",
      timeoutMs: 300000,
    },
  };
  fs.writeFileSync(path.join(oaDir, "project.json"), JSON.stringify(projectConfig, null, 2));

  const now = new Date().toISOString();
  const tasks: Task[] = [
    {
      id: "oa-sandbox-test-1",
      title: "Sandbox Config Test Task",
      description: "Test task for verifying sandbox config flow",
      status: "open",
      priority: 1,
      type: "task",
      labels: [],
      deps: [],
      commits: [],
      comments: [],
      createdAt: now,
      updatedAt: now,
      closedAt: null,
    },
  ];

  writeTasksFile(path.join(oaDir, "tasks.jsonl"), tasks);

  execSync("git add -A", { cwd: repoPath, stdio: "ignore" });
  execSync('git commit -m "init sandbox config fixture"', { cwd: repoPath, stdio: "ignore" });
  execSync("git push origin main", { cwd: repoPath, stdio: "ignore" });

  return { repoPath, openagentsDir: oaDir, tasksPath: path.join(oaDir, "tasks.jsonl"), projectConfig };
};

describe("sandbox config flow", () => {
  test("sandbox config is passed to orchestrator from parallel-runner", async () => {
    const { repoPath, openagentsDir, projectConfig } = createSandboxConfigFixture();
    const tasks = readTasksFile(path.join(openagentsDir, "tasks.jsonl"));

    // Capture what orchestrator receives
    let receivedConfig: OrchestratorConfig | null = null;

    const captureOrchestrator = (config: OrchestratorConfig): Effect.Effect<OrchestratorState> =>
      Effect.sync(() => {
        receivedConfig = config;
        // Mark task as done to satisfy the flow
        const tasksPath = path.join(config.openagentsDir ?? path.join(config.cwd, ".openagents"), "tasks.jsonl");
        const currentTasks = readTasksFile(tasksPath);
        const updatedTasks = currentTasks.map((t) =>
          t.id === config.task?.id ? { ...t, status: "closed", updatedAt: new Date().toISOString() } : t,
        ) as Task[];
        writeTasksFile(tasksPath, updatedTasks);

        // Create a marker file and commit
        fs.writeFileSync(path.join(config.cwd, "marker.txt"), "done");
        execSync("git add -A", { cwd: config.cwd, stdio: "ignore" });
        execSync('git commit -m "task complete"', { cwd: config.cwd, stdio: "ignore" });

        return {
          sessionId: "test-session",
          task: config.task ?? null,
          subtasks: null,
          progress: null,
          phase: "done" as const,
        } satisfies OrchestratorState;
      });

    await Effect.runPromise(
      runParallelFromConfig({
        repoPath,
        openagentsDir,
        baseBranch: "main",
        tasks,
        parallelConfig: {
          enabled: true,
          maxAgents: 1,
          mergeStrategy: "direct",
          worktreeTimeout: 30 * 60 * 1000,
          installTimeoutMs: 15 * 60 * 1000,
          installArgs: ["--skip-install"],
          mergeThreshold: 4,
          prThreshold: 50,
        },
        sandbox: projectConfig.sandbox,
        testCommands: ["echo tests"],
        ccOnly: true,
        runOrchestratorFn: captureOrchestrator,
      }),
    );

    // Verify sandbox config was received by orchestrator
    expect(receivedConfig).not.toBeNull();
    expect(receivedConfig!.sandbox).toBeDefined();
    expect(receivedConfig!.sandbox?.enabled).toBe(true);
    expect(receivedConfig!.sandbox?.backend).toBe("macos-container");
    expect(receivedConfig!.sandbox?.memoryLimit).toBe("4G");
    expect(receivedConfig!.sandbox?.timeoutMs).toBe(300000);
  }, 60000);

  test("sandbox config can be overridden via CLI flag (disabled)", async () => {
    const { repoPath, openagentsDir, projectConfig } = createSandboxConfigFixture();
    const tasks = readTasksFile(path.join(openagentsDir, "tasks.jsonl"));

    let receivedConfig: OrchestratorConfig | null = null;

    const captureOrchestrator = (config: OrchestratorConfig): Effect.Effect<OrchestratorState> =>
      Effect.sync(() => {
        receivedConfig = config;
        const tasksPath = path.join(config.openagentsDir ?? path.join(config.cwd, ".openagents"), "tasks.jsonl");
        const currentTasks = readTasksFile(tasksPath);
        const updatedTasks = currentTasks.map((t) =>
          t.id === config.task?.id ? { ...t, status: "closed", updatedAt: new Date().toISOString() } : t,
        ) as Task[];
        writeTasksFile(tasksPath, updatedTasks);

        fs.writeFileSync(path.join(config.cwd, "marker.txt"), "done");
        execSync("git add -A", { cwd: config.cwd, stdio: "ignore" });
        execSync('git commit -m "task complete"', { cwd: config.cwd, stdio: "ignore" });

        return {
          sessionId: "test-session",
          task: config.task ?? null,
          subtasks: null,
          progress: null,
          phase: "done" as const,
        } satisfies OrchestratorState;
      });

    // Pass sandbox config with enabled=false (simulating CLI override)
    const overriddenSandbox = { ...projectConfig.sandbox, enabled: false };

    await Effect.runPromise(
      runParallelFromConfig({
        repoPath,
        openagentsDir,
        baseBranch: "main",
        tasks,
        parallelConfig: {
          enabled: true,
          maxAgents: 1,
          mergeStrategy: "direct",
          worktreeTimeout: 30 * 60 * 1000,
          installTimeoutMs: 15 * 60 * 1000,
          installArgs: ["--skip-install"],
          mergeThreshold: 4,
          prThreshold: 50,
        },
        sandbox: overriddenSandbox,
        testCommands: ["echo tests"],
        ccOnly: true,
        runOrchestratorFn: captureOrchestrator,
      }),
    );

    // Verify overridden sandbox config was received
    expect(receivedConfig).not.toBeNull();
    expect(receivedConfig!.sandbox).toBeDefined();
    expect(receivedConfig!.sandbox?.enabled).toBe(false); // CLI override
    expect(receivedConfig!.sandbox?.backend).toBe("macos-container"); // Rest unchanged
  }, 60000);
});
