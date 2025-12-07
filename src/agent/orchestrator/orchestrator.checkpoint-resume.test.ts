import * as BunContext from "@effect/platform-bun/BunContext";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { describe, expect, test } from "bun:test";
import { Effect, Layer } from "effect";
import { execSync } from "node:child_process";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { runOrchestrator } from "./orchestrator.js";
import { createCheckpoint, writeCheckpoint, getCheckpointPath } from "./checkpoint.js";
import type { OrchestratorEvent, SubagentResult } from "./types.js";
import { runBestAvailableSubagent } from "./subagent-router.js";
import {
  OpenRouterClient,
  type OpenRouterClientShape,
} from "../../llm/openrouter.js";
import { DatabaseService } from "../../storage/database.js";
import { makeTestDatabaseLayer } from "../../tasks/test-helpers.js";

const mockOpenRouterLayer = Layer.succeed(OpenRouterClient, {
  chat: () => Effect.fail(new Error("not used")),
} satisfies OpenRouterClientShape);

const runWithBun = <A, E>(
  program: Effect.Effect<A, E, FileSystem.FileSystem | Path.Path | OpenRouterClient | DatabaseService>
): Promise<A> =>
  Effect.gen(function* () {
    const { layer: dbLayer, cleanup } = yield* makeTestDatabaseLayer();
    const testLayer = Layer.mergeAll(BunContext.layer, mockOpenRouterLayer, dbLayer);

    try {
      return yield* program.pipe(Effect.provide(testLayer));
    } finally {
      cleanup();
    }
  }).pipe(
    Effect.provide(BunContext.layer),  // Provide services for makeTestDatabaseLayer
    Effect.runPromise
  );

const runEffect = <A, E>(program: Effect.Effect<A, E, FileSystem.FileSystem>) =>
  Effect.runPromise(program.pipe(Effect.provide(BunContext.layer)));

const createCheckpointedRepo = (name: string) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), `orch-checkpoint-${name}-`));

  execSync("git init", { cwd: dir, stdio: "ignore" });
  execSync("git checkout -b main", { cwd: dir, stdio: "ignore" });
  execSync('git config user.email "mechacoder@example.com"', {
    cwd: dir,
    stdio: "ignore",
  });
  execSync('git config user.name "MechaCoder"', { cwd: dir, stdio: "ignore" });

  fs.writeFileSync(path.join(dir, "README.md"), "# Checkpoint resume test\n");

  const now = new Date().toISOString();
  const highPriorityTask = {
    id: `oa-high-${name}`,
    title: `High priority ${name}`,
    description: "Should not be picked when resuming from checkpoint",
    status: "open",
    priority: 0,
    type: "task",
    labels: [],
    deps: [],
    commits: [],
    createdAt: now,
    updatedAt: now,
    closedAt: null,
  };

  const targetTask = {
    id: `oa-target-${name}`,
    title: `Target ${name}`,
    description: "Task we should resume",
    status: "open",
    priority: 3,
    type: "task",
    labels: [],
    deps: [],
    commits: [],
    createdAt: now,
    updatedAt: now,
    closedAt: null,
  };

  const oaDir = path.join(dir, ".openagents");
  fs.mkdirSync(oaDir, { recursive: true });
  fs.writeFileSync(
    path.join(oaDir, "project.json"),
    JSON.stringify(
      {
        projectId: `proj-${name}`,
        defaultBranch: "main",
        testCommands: ["echo tests"],
        allowPush: false,
      },
      null,
      2
    )
  );
  fs.writeFileSync(
    path.join(oaDir, "tasks.jsonl"),
    `${JSON.stringify(highPriorityTask)}\n${JSON.stringify(targetTask)}\n`
  );

  const subtasksDir = path.join(oaDir, "subtasks");
  fs.mkdirSync(subtasksDir, { recursive: true });
  const subtaskList = {
    taskId: targetTask.id,
    taskTitle: targetTask.title,
    createdAt: now,
    updatedAt: now,
    subtasks: [
      {
        id: `${targetTask.id}-done`,
        description: "already finished",
        status: "done",
        completedAt: now,
      },
      {
        id: `${targetTask.id}-pending`,
        description: "still pending",
        status: "pending",
      },
    ],
  };
  fs.writeFileSync(
    path.join(subtasksDir, `${targetTask.id}.json`),
    JSON.stringify(subtaskList, null, 2)
  );

  execSync("git add -A", { cwd: dir, stdio: "ignore" });
  execSync('git commit -m "init"', { cwd: dir, stdio: "ignore" });

  const headCommit = execSync("git rev-parse HEAD", { cwd: dir, encoding: "utf-8" }).trim();
  const checkpoint = createCheckpoint({
    sessionId: `session-${name}`,
    phase: "executing_subtask",
    taskId: targetTask.id,
    taskTitle: targetTask.title,
    completedSubtaskIds: [`${targetTask.id}-done`],
    currentSubtaskId: null,
    git: {
      branch: "main",
      headCommit,
      isDirty: false,
      stagedFiles: [],
    },
  });

  return runEffect(writeCheckpoint(oaDir, checkpoint)).then(() => ({
    dir,
    openagentsDir: oaDir,
    targetTask,
    highPriorityTask,
    checkpointPath: getCheckpointPath(oaDir),
  }));
};

describe("Orchestrator checkpoint resume", () => {
  test("resumes from checkpoint and skips other ready tasks", async () => {
    const { dir, openagentsDir, targetTask, highPriorityTask, checkpointPath } =
      await createCheckpointedRepo("resume");
    const events: OrchestratorEvent[] = [];
    const executedSubtasks: string[] = [];

    const stubSubagent: typeof runBestAvailableSubagent = ({ subtask }) =>
      Effect.sync(() => {
        executedSubtasks.push(subtask.id);
        return {
          success: true,
          subtaskId: subtask.id,
          filesModified: [],
          turns: 1,
          agent: "minimal",
        } satisfies SubagentResult;
      });

    const state = await runWithBun(
      runOrchestrator(
        {
          cwd: dir,
          openagentsDir,
          testCommands: ["echo tests"],
          allowPush: false,
          sandbox: { enabled: false, backend: "none", timeoutMs: 300_000 },
        },
        (event) => events.push(event),
        { runSubagent: stubSubagent }
      )
    );

    const selected = events.find((e) => e.type === "task_selected") as
      | { type: "task_selected"; task: { id: string } }
      | undefined;

    expect(selected?.task.id).toBe(targetTask.id);
    expect(executedSubtasks).toEqual([`${targetTask.id}-pending`]);
    expect(fs.existsSync(checkpointPath)).toBe(false);

    const tasks = fs
      .readFileSync(path.join(openagentsDir, "tasks.jsonl"), "utf-8")
      .trim()
      .split("\n")
      .map((line) => JSON.parse(line));

    const highPriority = tasks.find((t: any) => t.id === highPriorityTask.id);
    expect(highPriority.status).toBe("open");
    expect(state.phase).toBe("done");
  });
});
