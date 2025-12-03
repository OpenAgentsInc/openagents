import * as BunContext from "@effect/platform-bun/BunContext";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import { execSync } from "node:child_process";
import * as fs from "node:fs";
import { tmpdir } from "node:os";
import * as path from "node:path";
import { runOrchestrator } from "./orchestrator.js";
import { runBestAvailableSubagent, type RunBestAvailableSubagentOptions } from "./subagent-router.js";
import type { OrchestratorEvent, SubagentResult } from "./types.js";

const runWithBun = <A, E>(program: Effect.Effect<A, E, any>) =>
  Effect.runPromise(program.pipe(Effect.provide(BunContext.layer)));

const createTestRepo = (name: string) => {
  const dir = fs.mkdtempSync(path.join(tmpdir(), `claude-e2e-${name}-`));

  execSync("git init", { cwd: dir, stdio: "ignore" });
  execSync("git checkout -b main", { cwd: dir, stdio: "ignore" });
  execSync('git config user.email "mechacoder@example.com"', { cwd: dir, stdio: "ignore" });
  execSync('git config user.name "MechaCoder"', { cwd: dir, stdio: "ignore" });

  fs.writeFileSync(path.join(dir, "README.md"), "# Test Repo\n");

  const now = new Date().toISOString();
  const task = {
    id: `oa-${name}`,
    title: `Claude E2E ${name}`,
    description: "Exercise Golden Loop with Claude Code path",
    status: "open",
    priority: 1,
    type: "task",
    labels: ["claude-code"],
    deps: [],
    commits: [],
    createdAt: now,
    updatedAt: now,
    closedAt: null,
  };

  const oaDir = path.join(dir, ".openagents");
  fs.mkdirSync(oaDir, { recursive: true });
  fs.writeFileSync(path.join(oaDir, "project.json"), JSON.stringify({
    projectId: `proj-${name}`,
    defaultBranch: "main",
    testCommands: ["echo tests"],
    allowPush: false,
  }, null, 2));
  fs.writeFileSync(path.join(oaDir, "tasks.jsonl"), `${JSON.stringify(task)}\n`);

  execSync("git add -A", { cwd: dir, stdio: "ignore" });
  execSync('git commit -m "init"', { cwd: dir, stdio: "ignore" });

  return { dir, taskId: task.id, openagentsDir: oaDir };
};

const readTasks = (tasksPath: string) =>
  fs.readFileSync(tasksPath, "utf-8").trim().split("\n").map((line) => JSON.parse(line));

describe("Golden Loop with Claude Code", () => {
  test("completes a task using Claude Code subagent", async () => {
    const { dir, taskId, openagentsDir } = createTestRepo("success");
    const events: OrchestratorEvent[] = [];
    const createdFile = path.join(dir, "feature.txt");

    const claudeRunner = (
      options: RunBestAvailableSubagentOptions<never>,
    ): Effect.Effect<SubagentResult, Error, never> =>
      Effect.sync(() => {
        fs.writeFileSync(createdFile, "updated by Claude Code");
        return {
          success: true,
          subtaskId: options.subtask.id,
          filesModified: [path.relative(dir, createdFile)],
          turns: 2,
          sessionMetadata: {
            toolsUsed: { Edit: 1 },
            summary: "Completed via Claude Code",
          },
        };
      });

    const state = await runWithBun(
      runOrchestrator(
        {
          cwd: dir,
          openagentsDir,
          testCommands: ["echo tests"],
          allowPush: false,
          maxSubtasksPerTask: 1,
          claudeCode: { enabled: true },
        },
        (event) => events.push(event),
        { runSubagent: claudeRunner },
      ),
    );

    expect(state.phase).toBe("done");
    expect(fs.existsSync(createdFile)).toBe(true);

    const tasks = readTasks(path.join(openagentsDir, "tasks.jsonl"));
    const closedTask = tasks.find((t) => t.id === taskId);
    expect(closedTask?.status).toBe("closed");
    expect(closedTask?.commits?.length ?? 0).toBeGreaterThan(0);

    const progress = fs.readFileSync(path.join(openagentsDir, "progress.md"), "utf-8");
    expect(progress).toContain("Claude Code Session");
    expect(progress).toContain("Completed via Claude Code");

    const log = execSync("git log --oneline -1", { cwd: dir, encoding: "utf-8" });
    expect(log).toContain(taskId);
  });

  test("falls back to minimal subagent when Claude Code fails", async () => {
    const { dir, taskId, openagentsDir } = createTestRepo("fallback");
    const events: OrchestratorEvent[] = [];
    const fallbackFile = path.join(dir, "fallback.txt");

    const subagentRunner = (options: RunBestAvailableSubagentOptions<never>) =>
      runBestAvailableSubagent({
        ...options,
        claudeCode: { enabled: true, fallbackToMinimal: true },
        detectClaudeCodeFn: async () => ({ available: true }),
        runClaudeCodeFn: async () => ({
          success: false,
          subtaskId: options.subtask.id,
          filesModified: [],
          turns: 1,
          error: "rate_limit",
        }),
        runMinimalSubagent: () =>
          Effect.sync(() => {
            fs.writeFileSync(fallbackFile, "handled by minimal subagent");
            return {
              success: true,
              subtaskId: options.subtask.id,
              filesModified: [path.relative(dir, fallbackFile)],
              turns: 1,
              sessionMetadata: { summary: "fallback to minimal subagent" },
            };
          }),
      });

    const state = await runWithBun(
      runOrchestrator(
        {
          cwd: dir,
          openagentsDir,
          testCommands: ["echo tests"],
          allowPush: false,
          maxSubtasksPerTask: 1,
          claudeCode: { enabled: true, fallbackToMinimal: true },
        },
        (event) => events.push(event),
        { runSubagent: subagentRunner },
      ),
    );

    expect(state.phase).toBe("done");
    expect(fs.existsSync(fallbackFile)).toBe(true);

    const tasks = readTasks(path.join(openagentsDir, "tasks.jsonl"));
    const closedTask = tasks.find((t) => t.id === taskId);
    expect(closedTask?.status).toBe("closed");
    expect(closedTask?.commits?.length ?? 0).toBeGreaterThan(0);

    const progress = fs.readFileSync(path.join(openagentsDir, "progress.md"), "utf-8");
    expect(progress).toContain("fallback to minimal subagent");

    const log = execSync("git log --oneline -1", { cwd: dir, encoding: "utf-8" });
    expect(log).toContain(taskId);
  });
});
