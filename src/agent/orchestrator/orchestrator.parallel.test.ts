/**
 * Tests for parallel runner features:
 * - Pre-assigned task (config.task) - skip pickNextTask
 * - Force new subtasks (config.forceNewSubtasks) - ignore existing subtask files
 *
 * These features prevent bugs where multiple parallel agents in worktrees
 * would all pick the same first ready task and reuse stale subtask files.
 */
import * as BunContext from "@effect/platform-bun/BunContext";
import { describe, expect, test } from "bun:test";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect, Layer } from "effect";
import { execSync } from "node:child_process";
import * as fs from "node:fs";
import { tmpdir } from "node:os";
import * as path from "node:path";
import { runOrchestrator } from "./orchestrator.js";
import { runBestAvailableSubagent } from "./subagent-router.js";
import type { OrchestratorEvent, SubagentResult, Subtask } from "./types.js";
import { OpenRouterClient, type OpenRouterClientShape } from "../../llm/openrouter.js";
import type { Task } from "../../tasks/index.js";
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
  }).pipe(Effect.runPromise);

const createTestRepo = (name: string, options?: { extraTasks?: Task[] }) => {
  const dir = fs.mkdtempSync(path.join(tmpdir(), `parallel-test-${name}-`));

  execSync("git init", { cwd: dir, stdio: "ignore" });
  execSync("git checkout -b main", { cwd: dir, stdio: "ignore" });
  execSync('git config user.email "mechacoder@example.com"', { cwd: dir, stdio: "ignore" });
  execSync('git config user.name "MechaCoder"', { cwd: dir, stdio: "ignore" });

  fs.writeFileSync(path.join(dir, "README.md"), "# Test Repo\n");

  const now = new Date().toISOString();

  // Create multiple tasks to test that pre-assigned task is used
  const task1: Task = {
    id: `oa-first-${name}`,
    title: `First Task ${name}`,
    description: "This is the first task in priority order",
    status: "open",
    priority: 0, // Highest priority
    type: "task",
    labels: [],
    deps: [],
    commits: [],
    comments: [],
    createdAt: now,
    updatedAt: now,
    closedAt: null,
  };

  const task2: Task = {
    id: `oa-second-${name}`,
    title: `Second Task ${name}`,
    description: "This is the second task in priority order",
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
  };

  const task3: Task = {
    id: `oa-third-${name}`,
    title: `Third Task ${name}`,
    description: "This is the third task in priority order",
    status: "open",
    priority: 2,
    type: "task",
    labels: [],
    deps: [],
    commits: [],
    comments: [],
    createdAt: now,
    updatedAt: now,
    closedAt: null,
  };

  const allTasks = [task1, task2, task3, ...(options?.extraTasks ?? [])];

  const oaDir = path.join(dir, ".openagents");
  fs.mkdirSync(oaDir, { recursive: true });
  fs.writeFileSync(path.join(oaDir, "project.json"), JSON.stringify({
    projectId: `proj-${name}`,
    defaultBranch: "main",
    testCommands: ["echo tests"],
    allowPush: false,
  }, null, 2));
  fs.writeFileSync(
    path.join(oaDir, "tasks.jsonl"),
    allTasks.map(t => JSON.stringify(t)).join("\n") + "\n"
  );

  execSync("git add -A", { cwd: dir, stdio: "ignore" });
  execSync('git commit -m "init"', { cwd: dir, stdio: "ignore" });

  return { dir, task1, task2, task3, openagentsDir: oaDir };
};

const readTasks = (tasksPath: string) =>
  fs.readFileSync(tasksPath, "utf-8").trim().split("\n").map((line) => JSON.parse(line));

describe("Pre-assigned task (config.task)", () => {
  test("uses config.task instead of pickNextTask when provided", async () => {
    const { dir, task2, openagentsDir } = createTestRepo("preassigned");
    const events: OrchestratorEvent[] = [];
    const createdFile = path.join(dir, "feature.txt");

    // Mock subagent that creates a file
    const subagentRunner: typeof runBestAvailableSubagent = (options) =>
      Effect.sync(() => {
        fs.writeFileSync(createdFile, `worked on ${options.subtask.id}`);
        return {
          success: true,
          subtaskId: options.subtask.id,
          filesModified: [path.relative(dir, createdFile)],
          turns: 1,
          agent: "claude-code",
        } satisfies SubagentResult;
      });

    // Pass task2 as the pre-assigned task (not task1 which would be picked by priority)
    const state = await runWithBun(
      runOrchestrator(
        {
          cwd: dir,
          openagentsDir,
          testCommands: ["echo tests"],
          allowPush: false,
          maxSubtasksPerTask: 1,
          claudeCode: { enabled: true },
          task: task2, // Pre-assigned task (priority 1, not 0)
        },
        (event) => events.push(event),
        { runSubagent: subagentRunner },
      ),
    );

    expect(state.phase).toBe("done");

    // Verify the correct task was used (task2, not task1)
    const taskSelectedEvent = events.find((e) => e.type === "task_selected") as {
      type: "task_selected";
      task: Task;
    };
    expect(taskSelectedEvent).toBeDefined();
    expect(taskSelectedEvent.task.id).toBe(task2.id);
    expect(taskSelectedEvent.task.title).toBe(task2.title);

    // Verify task2 was closed (not task1)
    const tasks = readTasks(path.join(openagentsDir, "tasks.jsonl"));
    const closedTask = tasks.find((t: Task) => t.id === task2.id);
    expect(closedTask?.status).toBe("closed");

    // Verify task1 is still open
    const firstTask = tasks.find((t: Task) => t.id.includes("first"));
    expect(firstTask?.status).toBe("open");
  });

  test("pickNextTask is used when config.task is not provided", async () => {
    const { dir, task1, openagentsDir } = createTestRepo("no-preassign");
    const events: OrchestratorEvent[] = [];
    const createdFile = path.join(dir, "feature.txt");

    const subagentRunner: typeof runBestAvailableSubagent = (options) =>
      Effect.sync(() => {
        fs.writeFileSync(createdFile, `worked on ${options.subtask.id}`);
        return {
          success: true,
          subtaskId: options.subtask.id,
          filesModified: [path.relative(dir, createdFile)],
          turns: 1,
          agent: "claude-code",
        } satisfies SubagentResult;
      });

    // No task provided - should pick task1 (highest priority)
    const state = await runWithBun(
      runOrchestrator(
        {
          cwd: dir,
          openagentsDir,
          testCommands: ["echo tests"],
          allowPush: false,
          maxSubtasksPerTask: 1,
          claudeCode: { enabled: true },
          // No task property - will use pickNextTask
        },
        (event) => events.push(event),
        { runSubagent: subagentRunner },
      ),
    );

    expect(state.phase).toBe("done");

    // Verify task1 was picked (highest priority)
    const taskSelectedEvent = events.find((e) => e.type === "task_selected") as {
      type: "task_selected";
      task: Task;
    };
    expect(taskSelectedEvent).toBeDefined();
    expect(taskSelectedEvent.task.id).toBe(task1.id);

    // Verify task1 was closed
    const tasks = readTasks(path.join(openagentsDir, "tasks.jsonl"));
    const closedTask = tasks.find((t: Task) => t.id === task1.id);
    expect(closedTask?.status).toBe("closed");
  });

  test("simulates parallel runners with different pre-assigned tasks", async () => {
    // This test simulates what happens with the parallel runner:
    // Two agents get different tasks pre-assigned, should work on different tasks
    const { dir, task1, task2 } = createTestRepo("parallel-sim");

    // Create separate openagents dirs to simulate worktrees
    const worktree1Dir = fs.mkdtempSync(path.join(tmpdir(), "worktree1-"));
    const worktree2Dir = fs.mkdtempSync(path.join(tmpdir(), "worktree2-"));

    // Copy repo to both worktrees
    execSync(`cp -r ${dir}/. ${worktree1Dir}/`);
    execSync(`cp -r ${dir}/. ${worktree2Dir}/`);

    const oaDir1 = path.join(worktree1Dir, ".openagents");
    const oaDir2 = path.join(worktree2Dir, ".openagents");

    const events1: OrchestratorEvent[] = [];
    const events2: OrchestratorEvent[] = [];

    const createSubagentRunner = (worktreeDir: string) =>
      ((options) =>
        Effect.sync(() => {
          fs.writeFileSync(path.join(worktreeDir, "feature.txt"), `worked on ${options.subtask.id}`);
          return {
            success: true,
            subtaskId: options.subtask.id,
            filesModified: ["feature.txt"],
            turns: 1,
            agent: "claude-code",
          } satisfies SubagentResult;
        })) as typeof runBestAvailableSubagent;

    // Run both orchestrators "in parallel" (sequentially in test, but with different tasks)
    const [state1, state2] = await Promise.all([
      runWithBun(
        runOrchestrator(
          {
            cwd: worktree1Dir,
            openagentsDir: oaDir1,
            testCommands: ["echo tests"],
            allowPush: false,
            maxSubtasksPerTask: 1,
            claudeCode: { enabled: true },
            task: task1, // Agent 1 gets task1
          },
          (event) => events1.push(event),
          { runSubagent: createSubagentRunner(worktree1Dir) },
        ),
      ),
      runWithBun(
        runOrchestrator(
          {
            cwd: worktree2Dir,
            openagentsDir: oaDir2,
            testCommands: ["echo tests"],
            allowPush: false,
            maxSubtasksPerTask: 1,
            claudeCode: { enabled: true },
            task: task2, // Agent 2 gets task2
          },
          (event) => events2.push(event),
          { runSubagent: createSubagentRunner(worktree2Dir) },
        ),
      ),
    ]);

    expect(state1.phase).toBe("done");
    expect(state2.phase).toBe("done");

    // Verify each agent worked on their assigned task
    const task1Selected = events1.find((e) => e.type === "task_selected") as {
      type: "task_selected";
      task: Task;
    };
    const task2Selected = events2.find((e) => e.type === "task_selected") as {
      type: "task_selected";
      task: Task;
    };

    expect(task1Selected.task.id).toBe(task1.id);
    expect(task2Selected.task.id).toBe(task2.id);

    // Verify subtasks were created with correct task IDs
    const decompose1 = events1.find((e) => e.type === "task_decomposed") as {
      type: "task_decomposed";
      subtasks: Subtask[];
    };
    const decompose2 = events2.find((e) => e.type === "task_decomposed") as {
      type: "task_decomposed";
      subtasks: Subtask[];
    };

    expect(decompose1.subtasks[0].id).toContain(task1.id);
    expect(decompose2.subtasks[0].id).toContain(task2.id);

    // Cleanup
    fs.rmSync(worktree1Dir, { recursive: true, force: true });
    fs.rmSync(worktree2Dir, { recursive: true, force: true });
  });
});

describe("Force new subtasks (config.forceNewSubtasks)", () => {
  test("ignores existing subtask file when forceNewSubtasks is true", async () => {
    const { dir, task1, openagentsDir } = createTestRepo("force-new");

    // Pre-create a stale subtask file with a "done" subtask
    const subtasksDir = path.join(openagentsDir, "subtasks");
    fs.mkdirSync(subtasksDir, { recursive: true });
    const staleSubtaskList = {
      taskId: task1.id,
      taskTitle: task1.title,
      subtasks: [
        {
          id: `${task1.id}-sub-001`,
          description: "Stale subtask from previous run",
          status: "done", // Already marked as done
          completedAt: new Date().toISOString(),
        },
      ],
      createdAt: new Date(Date.now() - 86400000).toISOString(), // 1 day ago
      updatedAt: new Date(Date.now() - 86400000).toISOString(),
    };
    fs.writeFileSync(
      path.join(subtasksDir, `${task1.id}.json`),
      JSON.stringify(staleSubtaskList, null, 2)
    );

    const events: OrchestratorEvent[] = [];
    let subtaskWorkedOn = "";

    const subagentRunner: typeof runBestAvailableSubagent = (options) =>
      Effect.sync(() => {
        subtaskWorkedOn = options.subtask.id;
        fs.writeFileSync(path.join(dir, "feature.txt"), "worked");
        return {
          success: true,
          subtaskId: options.subtask.id,
          filesModified: ["feature.txt"],
          turns: 1,
          agent: "claude-code",
        } satisfies SubagentResult;
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
          task: task1,
          forceNewSubtasks: true, // Force creating new subtasks
        },
        (event) => events.push(event),
        { runSubagent: subagentRunner },
      ),
    );

    expect(state.phase).toBe("done");

    // Verify a NEW subtask was created and worked on (not the stale "done" one)
    expect(subtaskWorkedOn).toContain(task1.id);
    expect(subtaskWorkedOn).not.toBe(""); // Subtask was actually executed

    // The key verification is that the subagent was called at all.
    // If we had used the stale subtask file (status: "done"), the subagent
    // would have been skipped entirely. The fact that subtaskWorkedOn is set
    // proves that forceNewSubtasks created a fresh pending subtask.
    const subtaskStartEvent = events.find((e) => e.type === "subtask_start");
    expect(subtaskStartEvent).toBeDefined();
    // Subtask complete event should also exist
    const subtaskCompleteEvent = events.find((e) => e.type === "subtask_complete");
    expect(subtaskCompleteEvent).toBeDefined();
  });

  test("uses existing subtask file when forceNewSubtasks is false", async () => {
    const { dir, task1, openagentsDir } = createTestRepo("use-existing");

    // Pre-create a subtask file with a pending subtask
    const subtasksDir = path.join(openagentsDir, "subtasks");
    fs.mkdirSync(subtasksDir, { recursive: true });
    const existingSubtaskId = `${task1.id}-existing-subtask`;
    const existingSubtaskList = {
      taskId: task1.id,
      taskTitle: task1.title,
      subtasks: [
        {
          id: existingSubtaskId,
          description: "Existing subtask from file",
          status: "pending",
        },
      ],
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    };
    fs.writeFileSync(
      path.join(subtasksDir, `${task1.id}.json`),
      JSON.stringify(existingSubtaskList, null, 2)
    );

    const events: OrchestratorEvent[] = [];
    let subtaskWorkedOn = "";

    const subagentRunner: typeof runBestAvailableSubagent = (options) =>
      Effect.sync(() => {
        subtaskWorkedOn = options.subtask.id;
        fs.writeFileSync(path.join(dir, "feature.txt"), "worked");
        return {
          success: true,
          subtaskId: options.subtask.id,
          filesModified: ["feature.txt"],
          turns: 1,
          agent: "claude-code",
        } satisfies SubagentResult;
      });

    const state = await runWithBun(
      runOrchestrator(
        {
          cwd: dir,
          openagentsDir,
          testCommands: ["echo tests"],
          allowPush: false,
          claudeCode: { enabled: true },
          task: task1,
          forceNewSubtasks: false, // Use existing subtasks
        },
        (event) => events.push(event),
        { runSubagent: subagentRunner },
      ),
    );

    expect(state.phase).toBe("done");

    // Verify the EXISTING subtask was used
    expect(subtaskWorkedOn).toBe(existingSubtaskId);
  });

  test("skips already-done subtasks when forceNewSubtasks is false", async () => {
    const { dir, task1, openagentsDir } = createTestRepo("skip-done");

    // Pre-create a subtask file where all subtasks are done
    const subtasksDir = path.join(openagentsDir, "subtasks");
    fs.mkdirSync(subtasksDir, { recursive: true });
    const allDoneSubtaskList = {
      taskId: task1.id,
      taskTitle: task1.title,
      subtasks: [
        {
          id: `${task1.id}-sub-001`,
          description: "Already done subtask",
          status: "done",
          completedAt: new Date().toISOString(),
        },
      ],
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    };
    fs.writeFileSync(
      path.join(subtasksDir, `${task1.id}.json`),
      JSON.stringify(allDoneSubtaskList, null, 2)
    );

    const events: OrchestratorEvent[] = [];
    let subagentCalled = false;

    const subagentRunner: typeof runBestAvailableSubagent = (options) =>
      Effect.sync(() => {
        subagentCalled = true;
        return {
          success: true,
          subtaskId: options.subtask.id,
          filesModified: [],
          turns: 1,
          agent: "claude-code",
        } satisfies SubagentResult;
      });

    const state = await runWithBun(
      runOrchestrator(
        {
          cwd: dir,
          openagentsDir,
          testCommands: ["echo tests"],
          allowPush: false,
          claudeCode: { enabled: true },
          task: task1,
          forceNewSubtasks: false, // Use existing subtasks (all done)
        },
        (event) => events.push(event),
        { runSubagent: subagentRunner },
      ),
    );

    // Should complete without calling subagent (all subtasks already done)
    expect(state.phase).toBe("done");
    expect(subagentCalled).toBe(false);
  });

  test("forceNewSubtasks creates fresh subtasks even if file exists", async () => {
    const { dir, task1, openagentsDir } = createTestRepo("fresh-subtasks");

    // Pre-create a subtask file with custom description
    const subtasksDir = path.join(openagentsDir, "subtasks");
    fs.mkdirSync(subtasksDir, { recursive: true });
    const customDescription = "CUSTOM_DESCRIPTION_FROM_FILE";
    const existingSubtaskList = {
      taskId: task1.id,
      taskTitle: task1.title,
      subtasks: [
        {
          id: `${task1.id}-custom`,
          description: customDescription,
          status: "pending",
        },
      ],
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    };
    fs.writeFileSync(
      path.join(subtasksDir, `${task1.id}.json`),
      JSON.stringify(existingSubtaskList, null, 2)
    );

    const events: OrchestratorEvent[] = [];

    const subagentRunner: typeof runBestAvailableSubagent = (options) =>
      Effect.sync(() => {
        fs.writeFileSync(path.join(dir, "feature.txt"), "worked");
        return {
          success: true,
          subtaskId: options.subtask.id,
          filesModified: ["feature.txt"],
          turns: 1,
          agent: "claude-code",
        } satisfies SubagentResult;
      });

    await runWithBun(
      runOrchestrator(
        {
          cwd: dir,
          openagentsDir,
          testCommands: ["echo tests"],
          allowPush: false,
          maxSubtasksPerTask: 1,
          claudeCode: { enabled: true },
          task: task1,
          forceNewSubtasks: true,
        },
        (event) => events.push(event),
        { runSubagent: subagentRunner },
      ),
    );

    // Verify the subtask does NOT have the custom description (fresh subtask created)
    const decomposeEvent = events.find((e) => e.type === "task_decomposed") as {
      type: "task_decomposed";
      subtasks: Subtask[];
    };
    expect(decomposeEvent.subtasks[0].description).not.toContain(customDescription);
    // Should have description from the task itself
    expect(decomposeEvent.subtasks[0].description).toContain(task1.title);
  });
});

describe("Combined parallel runner simulation", () => {
  test("multiple agents work on different tasks with fresh subtasks", async () => {
    const { dir, task1, task2, task3, openagentsDir } = createTestRepo("full-parallel");

    // Pre-create stale subtask files for all tasks (simulating files in git)
    const subtasksDir = path.join(openagentsDir, "subtasks");
    fs.mkdirSync(subtasksDir, { recursive: true });

    for (const task of [task1, task2, task3]) {
      const staleSubtasks = {
        taskId: task.id,
        taskTitle: task.title,
        subtasks: [
          {
            id: `${task.id}-stale`,
            description: "Stale subtask that should be ignored",
            status: "done",
          },
        ],
        createdAt: new Date(Date.now() - 86400000).toISOString(),
        updatedAt: new Date(Date.now() - 86400000).toISOString(),
      };
      fs.writeFileSync(
        path.join(subtasksDir, `${task.id}.json`),
        JSON.stringify(staleSubtasks, null, 2)
      );
    }

    // Create separate worktree directories
    const worktreeDirs = [1, 2, 3].map((i) =>
      fs.mkdtempSync(path.join(tmpdir(), `worktree-${i}-`))
    );

    // Copy repo to all worktrees
    for (const wtDir of worktreeDirs) {
      execSync(`cp -r ${dir}/. ${wtDir}/`);
    }

    const allEvents: OrchestratorEvent[][] = [[], [], []];
    const tasksWorkedOn: string[] = [];

    const createRunner = (i: number, wtDir: string) =>
      ((options) =>
        Effect.sync(() => {
          tasksWorkedOn.push(options.subtask.id);
          fs.writeFileSync(path.join(wtDir, `feature-${i}.txt`), "worked");
          return {
            success: true,
            subtaskId: options.subtask.id,
            filesModified: [`feature-${i}.txt`],
            turns: 1,
            agent: "claude-code",
          } satisfies SubagentResult;
        })) as typeof runBestAvailableSubagent;

    const tasks = [task1, task2, task3];

    // Run all three agents "in parallel"
    const states = await Promise.all(
      worktreeDirs.map((wtDir, i) =>
        runWithBun(
          runOrchestrator(
            {
              cwd: wtDir,
              openagentsDir: path.join(wtDir, ".openagents"),
              testCommands: ["echo tests"],
              allowPush: false,
              maxSubtasksPerTask: 1,
              claudeCode: { enabled: true },
              task: tasks[i], // Each agent gets different task
              forceNewSubtasks: true, // Ignore stale subtask files
            },
            (event) => allEvents[i].push(event),
            { runSubagent: createRunner(i, wtDir) },
          ),
        )
      )
    );

    // All should complete successfully
    expect(states.every((s) => s.phase === "done")).toBe(true);

    // Each agent should have worked on a different task
    expect(tasksWorkedOn.length).toBe(3);
    expect(tasksWorkedOn.some((id) => id.includes(task1.id))).toBe(true);
    expect(tasksWorkedOn.some((id) => id.includes(task2.id))).toBe(true);
    expect(tasksWorkedOn.some((id) => id.includes(task3.id))).toBe(true);

    // Verify none of them used the stale subtask IDs
    expect(tasksWorkedOn.every((id) => !id.includes("stale"))).toBe(true);

    // Cleanup
    for (const wtDir of worktreeDirs) {
      fs.rmSync(wtDir, { recursive: true, force: true });
    }
  });
});
