import * as BunContext from "@effect/platform-bun/BunContext";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  closeTask,
  createTask,
  listTasks,
  pickNextTask,
  readyTasks,
  readTasks,
  updateTask,
} from "./service.js";
import type { TaskCreate } from "./schema.js";

const runWithBun = <A, E>(
  program: Effect.Effect<A, E, FileSystem.FileSystem | Path.Path>,
) =>
  Effect.runPromise(program.pipe(Effect.provide(BunContext.layer)));

const makeTask = (title: string, overrides: Partial<TaskCreate> = {}): TaskCreate => ({
  title,
  description: "",
  status: "open",
  priority: 2,
  type: "task",
  labels: [],
  deps: [],
  ...overrides,
});

const setup = () =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const path = yield* Path.Path;
    const dir = yield* fs.makeTempDirectory({ prefix: "task-service" });
    const tasksPath = path.join(dir, "tasks.jsonl");
    return { tasksPath };
  });

describe("TaskService", () => {
  test("creates and persists a task with defaults", async () => {
    const result = await runWithBun(
      Effect.gen(function* () {
        const { tasksPath } = yield* setup();
        const created = yield* createTask({
          tasksPath,
          task: makeTask("Write TaskService"),
          idPrefix: "oa",
          timestamp: new Date("2025-01-01T00:00:00Z"),
        });

        const saved = yield* readTasks(tasksPath);
        return { created, saved };
      }),
    );

    expect(result.created.id.startsWith("oa-")).toBe(true);
    expect(result.created.status).toBe("open");
    expect(result.created.priority).toBe(2);
    expect(result.saved).toHaveLength(1);
    expect(result.saved[0]?.title).toBe("Write TaskService");
  });

  test("computes ready tasks sorted by priority and age", async () => {
    const ready = await runWithBun(
      Effect.gen(function* () {
        const { tasksPath } = yield* setup();
        const closedDep = yield* createTask({
          tasksPath,
          task: makeTask("Closed dep", { status: "closed" }),
          timestamp: new Date("2025-01-01T00:00:00Z"),
        });

        yield* createTask({
          tasksPath,
          task: makeTask("Highest priority", { priority: 0 }),
          timestamp: new Date("2025-01-01T01:00:00Z"),
        });

        yield* createTask({
          tasksPath,
          task: makeTask("Blocked task", {
            priority: 1,
            deps: [{ id: closedDep.id, type: "blocks" }],
          }),
          timestamp: new Date("2025-01-01T02:00:00Z"),
        });

        yield* createTask({
          tasksPath,
          task: makeTask("Lower priority", { priority: 2 }),
          timestamp: new Date("2025-01-01T03:00:00Z"),
        });

        return yield* readyTasks(tasksPath);
      }),
    );

    expect(ready.map((t) => t.title)).toEqual([
      "Highest priority",
      "Blocked task",
      "Lower priority",
    ]);
  });

  test("updates, closes, and appends commits", async () => {
    const finalTask = await runWithBun(
      Effect.gen(function* () {
        const { tasksPath } = yield* setup();
        const created = yield* createTask({
          tasksPath,
          task: makeTask("Implement updates"),
          timestamp: new Date("2025-01-01T00:00:00Z"),
        });

        yield* updateTask({
          tasksPath,
          id: created.id,
          update: { assignee: "mechacoder", priority: 1 },
          timestamp: new Date("2025-01-01T01:00:00Z"),
        });

        return yield* closeTask({
          tasksPath,
          id: created.id,
          reason: "Completed",
          commits: ["abc123"],
          timestamp: new Date("2025-01-01T02:00:00Z"),
        });
      }),
    );

    expect(finalTask.status).toBe("closed");
    expect(finalTask.closeReason).toBe("Completed");
    expect(finalTask.assignee).toBe("mechacoder");
    expect(finalTask.commits).toEqual(["abc123"]);
    expect(finalTask.closedAt).toBe("2025-01-01T02:00:00.000Z");
  });

  test("lists tasks with filters and picks next ready", async () => {
    const result = await runWithBun(
      Effect.gen(function* () {
        const { tasksPath } = yield* setup();
        const first = yield* createTask({
          tasksPath,
          task: makeTask("Ready A", { priority: 1 }),
          timestamp: new Date("2025-01-01T00:00:00Z"),
        });
        const second = yield* createTask({
          tasksPath,
          task: makeTask("Ready B", { priority: 0 }),
          timestamp: new Date("2025-01-01T01:00:00Z"),
        });
        yield* createTask({
          tasksPath,
          task: makeTask("Blocked", { deps: [{ id: first.id, type: "blocks" }] }),
          timestamp: new Date("2025-01-01T02:00:00Z"),
        });

        const filtered = yield* listTasks(tasksPath, { priority: 1 });
        const next = yield* pickNextTask(tasksPath);
        return { filtered, next, second };
      }),
    );

    expect(result.filtered).toHaveLength(1);
    expect(result.filtered[0]?.title).toBe("Ready A");
    expect(result.next?.id).toBe(result.second.id);
  });
});
