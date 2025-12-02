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

  test("computes ready tasks with hybrid priority/age sorting", async () => {
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
          task: makeTask("Recent high", { priority: 0 }),
          timestamp: new Date(Date.now() - 1 * 60 * 60 * 1000), // recent
        });

        yield* createTask({
          tasksPath,
          task: makeTask("Recent blocked resolved", {
            priority: 1,
            deps: [{ id: closedDep.id, type: "blocks" }],
          }),
          timestamp: new Date(Date.now() - 2 * 60 * 60 * 1000), // recent
        });

        yield* createTask({
          tasksPath,
          task: makeTask("Lower priority", { priority: 2 }),
          timestamp: new Date("2024-12-01T00:00:00Z"), // older
        });

        return yield* readyTasks(tasksPath);
      }),
    );

    expect(ready.map((t) => t.title)).toEqual([
      "Recent high",
      "Recent blocked resolved",
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

  test("lists tasks with filters, sort policy, and picks next ready", async () => {
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

        const filtered = yield* listTasks(tasksPath, { priority: 1, sortPolicy: "priority" });
        const next = yield* pickNextTask(tasksPath, { sortPolicy: "priority" });
        return { filtered, next, second };
      }),
    );

    expect(result.filtered).toHaveLength(1);
    expect(result.filtered[0]?.title).toBe("Ready A");
    expect(result.next?.id).toBe(result.second.id);
  });

  test("applies labelsAny/unassigned filters, limits, and sortPolicy oldest", async () => {
    const result = await runWithBun(
      Effect.gen(function* () {
        const { tasksPath } = yield* setup();
        yield* createTask({
          tasksPath,
          task: makeTask("Assigned recent", { assignee: "alice", labels: ["frontend"], priority: 2 }),
          timestamp: new Date(Date.now() - 1 * 60 * 60 * 1000),
        });
        const unassigned = yield* createTask({
          tasksPath,
          task: makeTask("Unassigned older", { labels: ["backend", "cli"], priority: 2 }),
          timestamp: new Date("2025-01-01T00:00:00Z"),
        });
        const alsoOlder = yield* createTask({
          tasksPath,
          task: makeTask("Unassigned also older", { labels: ["cli"], priority: 2 }),
          timestamp: new Date("2025-01-02T00:00:00Z"),
        });
        // Blocked should be filtered out even if it matches labels
        yield* createTask({
          tasksPath,
          task: makeTask("Blocked task", {
            status: "blocked",
            labels: ["cli"],
            deps: [{ id: unassigned.id, type: "blocks" }],
          }),
          timestamp: new Date("2025-01-03T00:00:00Z"),
        });

        const ready = yield* readyTasks(tasksPath, {
          labelsAny: ["cli"],
          unassigned: true,
          sortPolicy: "oldest",
          limit: 1,
        });

        const listed = yield* listTasks(tasksPath, {
          labelsAny: ["cli"],
          unassigned: true,
          sortPolicy: "oldest",
          status: "open",
        });

        return { ready, listed, unassigned, alsoOlder };
      }),
    );

    expect(result.ready).toHaveLength(1);
    expect(result.ready[0]?.title).toBe("Unassigned older");
    expect(result.listed.map((t) => t.id)).toEqual([result.unassigned.id, result.alsoOlder.id]);
  });
});
