import * as BunContext from "@effect/platform-bun/BunContext";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import * as nodePath from "node:path";
import { createTaskRepository } from "./repository.js";
import { createTask, readTasks } from "./service.js";
import type { TaskCreate } from "./schema.js";

const runWithBun = <A, E>(
  program: Effect.Effect<A, E, FileSystem.FileSystem | Path.Path>,
) => Effect.runPromise(program.pipe(Effect.provide(BunContext.layer)));

const makeTask = (title: string, overrides: Partial<TaskCreate> = {}): TaskCreate => ({
  title,
  description: "",
  status: "open",
  priority: 2,
  type: "task",
  labels: [],
  deps: [],
  comments: [],
  ...overrides,
});

const setupRepo = () =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const path = yield* Path.Path;
    const dir = yield* fs.makeTempDirectory({ prefix: "task-repo" });
    const repo = createTaskRepository({ rootDir: dir });
    const tasksPath = path.join(dir, ".openagents", "tasks.jsonl");
    return { repo, tasksPath, dir };
  });

describe("TaskRepository", () => {
  test("resolves tasksPath from rootDir", async () => {
    const result = await runWithBun(setupRepo());
    expect(result.repo.tasksPath).toBe(nodePath.join(result.dir, ".openagents", "tasks.jsonl"));
  });

  test("ready and claimNext mark tasks in_progress", async () => {
    const { claimed, savedStatus, readyFirstId, highId } = await runWithBun(
      Effect.gen(function* () {
        const { repo, tasksPath } = yield* setupRepo();
        const high = yield* createTask({ tasksPath, task: makeTask("high", { priority: 1 }) });
        yield* createTask({ tasksPath, task: makeTask("low", { priority: 3 }) });

        const ready = yield* repo.ready();
        const claimedTask = yield* repo.claimNext();
        const saved = yield* readTasks(tasksPath);
        const savedClaimed = saved.find((t) => t.id === claimedTask?.id);

        return {
          claimed: claimedTask,
          readyFirstId: ready[0]?.id,
          savedStatus: savedClaimed?.status,
          highId: high.id,
        };
      }),
    );

    expect(readyFirstId).toBe(highId);
    expect(claimed?.id).toBeDefined();
    expect(claimed?.status).toBe("in_progress");
    expect(savedStatus).toBe("in_progress");
  });

  test("update and close proxy to service helpers", async () => {
    const { updated, closed } = await runWithBun(
      Effect.gen(function* () {
        const { repo, tasksPath } = yield* setupRepo();
        const task = yield* createTask({ tasksPath, task: makeTask("to-close") });

        const updatedTask = yield* repo.update(task.id, { labels: ["focus"] }, { appendCommits: ["abc123"] });
        const closedTask = yield* repo.close(task.id, {
          reason: "Done",
          commits: ["def456"],
        });

        return { updated: updatedTask, closed: closedTask };
      }),
    );

    expect(updated.labels).toContain("focus");
    expect(updated.commits).toContain("abc123");
    expect(closed.status).toBe("closed");
    expect(closed.closeReason).toBe("Done");
    expect(closed.commits).toEqual(expect.arrayContaining(["abc123", "def456"]));
  });

  test("ready/pick/close flows share the same tasksPath", async () => {
    const result = await runWithBun(
      Effect.gen(function* () {
        const { repo } = yield* setupRepo();
        const first = yield* repo.createTask({
          task: makeTask("first", { priority: 3 }),
          idPrefix: "oa",
          timestamp: new Date("2025-01-01T00:00:00Z"),
        });
        const second = yield* repo.createTask({
          task: makeTask("second", { priority: 1 }),
          idPrefix: "oa",
          timestamp: new Date("2025-01-02T00:00:00Z"),
        });

        const ready = yield* repo.readyTasks({ sortPolicy: "priority" });
        const picked = yield* repo.pickNextTask({ sortPolicy: "priority" });
        const closed = yield* repo.closeTask({
          id: first.id,
          reason: "done",
          commits: ["abc123"],
          timestamp: new Date("2025-01-03T00:00:00Z"),
        });

        const all = yield* repo.listTasks();
        return { ready, picked, closed, all, second };
      }),
    );

    expect(result.ready.map((t) => t.id)).toHaveLength(2);
    expect(result.picked?.id).toEqual(result.second.id); // lower priority number first
    expect(result.closed.status).toBe("closed");
    expect(result.closed.commits).toContain("abc123");
    expect(result.all.some((t) => t.id === result.closed.id && t.status === "closed")).toBe(true);
  });
});
