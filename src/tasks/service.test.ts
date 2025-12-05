import * as BunContext from "@effect/platform-bun/BunContext";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  closeTask,
  reopenTask,
  createTask,
  listTasks,
  pickNextTask,
  readyTasks,
  readTasks,
  updateTask,
  archiveTasks,
  readArchivedTasks,
  searchAllTasks,
  getStaleTasks,
  addComment,
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
  comments: [],
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

  test("appends comments with generated metadata", async () => {
    const result = await runWithBun(
      Effect.gen(function* () {
        const { tasksPath } = yield* setup();
        const created = yield* createTask({
          tasksPath,
          task: makeTask("Collect feedback"),
          timestamp: new Date("2025-01-01T00:00:00Z"),
        });

        const first = yield* addComment({
          tasksPath,
          taskId: created.id,
          text: "Initial note",
          author: "alice",
          commentId: "c-1",
          timestamp: new Date("2025-01-01T01:00:00Z"),
        });

        const second = yield* addComment({
          tasksPath,
          taskId: created.id,
          text: "Follow-up",
          author: "bob",
          commentId: "c-2",
          timestamp: new Date("2025-01-01T02:00:00Z"),
        });

        const saved = yield* readTasks(tasksPath);
        return { first, second, saved };
      }),
    );

    expect(result.saved[0]?.comments).toHaveLength(2);
    expect(result.saved[0]?.comments?.[0]?.id).toBe("c-1");
    expect(result.saved[0]?.comments?.[0]?.createdAt).toBe("2025-01-01T01:00:00.000Z");
    expect(result.saved[0]?.comments?.[1]?.author).toBe("bob");
    expect(result.saved[0]?.updatedAt).toBe("2025-01-01T02:00:00.000Z");
    expect(result.second.comment.text).toBe("Follow-up");
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

  test("finds stale in-progress tasks older than threshold", async () => {
    const stale = await runWithBun(
      Effect.gen(function* () {
        const { tasksPath } = yield* setup();
        // Stale in_progress updated 40 days before the reference timestamp
        yield* createTask({
          tasksPath,
          task: makeTask("Stale in-progress", { status: "in_progress" }),
          timestamp: new Date("2025-01-01T00:00:00Z"),
        });
        // Recent in_progress should be ignored
        yield* createTask({
          tasksPath,
          task: makeTask("Fresh in-progress", { status: "in_progress" }),
          timestamp: new Date("2025-02-08T00:00:00Z"),
        });
        // Old closed task should not appear when filtering status
        yield* createTask({
          tasksPath,
          task: makeTask("Closed old", { status: "closed" }),
          timestamp: new Date("2025-01-01T00:00:00Z"),
        });

        return yield* getStaleTasks({
          tasksPath,
          days: 30,
          status: "in_progress",
          timestamp: new Date("2025-02-10T00:00:00Z"), // deterministic threshold
        });
      }),
    );

    expect(stale).toHaveLength(1);
    expect(stale[0]?.title).toBe("Stale in-progress");
  });
});

describe("Archive functionality", () => {
  test("archives old closed tasks and preserves active tasks", async () => {
    const result = await runWithBun(
      Effect.gen(function* () {
        const { tasksPath } = yield* setup();
        const archivePath = tasksPath.replace("tasks.jsonl", "tasks-archive.jsonl");

        // Create tasks: 2 old closed, 1 recent closed, 1 open
        const oldClosed1 = yield* createTask({
          tasksPath,
          task: makeTask("Old closed 1", { status: "open" }),
          timestamp: new Date("2025-01-01T00:00:00Z"),
        });
        yield* closeTask({
          tasksPath,
          id: oldClosed1.id,
          reason: "Done",
          timestamp: new Date("2025-01-01T01:00:00Z"),
        });

        const oldClosed2 = yield* createTask({
          tasksPath,
          task: makeTask("Old closed 2", { status: "open" }),
          timestamp: new Date("2025-01-02T00:00:00Z"),
        });
        yield* closeTask({
          tasksPath,
          id: oldClosed2.id,
          reason: "Done",
          timestamp: new Date("2025-01-02T01:00:00Z"),
        });

        const recentClosed = yield* createTask({
          tasksPath,
          task: makeTask("Recent closed", { status: "open" }),
          timestamp: new Date("2025-06-01T00:00:00Z"),
        });
        yield* closeTask({
          tasksPath,
          id: recentClosed.id,
          reason: "Done",
          timestamp: new Date("2025-06-01T01:00:00Z"),
        });

        yield* createTask({
          tasksPath,
          task: makeTask("Still open"),
          timestamp: new Date("2025-01-03T00:00:00Z"),
        });

        // Archive tasks older than 30 days from reference date
        const archiveResult = yield* archiveTasks({
          tasksPath,
          daysOld: 30,
          timestamp: new Date("2025-06-15T00:00:00Z"),
        });

        const remaining = yield* readTasks(tasksPath);
        const archived = yield* readArchivedTasks(archivePath);

        return { archiveResult, remaining, archived, oldClosed1, oldClosed2, recentClosed };
      }),
    );

    expect(result.archiveResult.dryRun).toBe(false);
    expect(result.archiveResult.archived).toHaveLength(2);
    expect(result.archiveResult.archived.map((t) => t.id)).toContain(result.oldClosed1.id);
    expect(result.archiveResult.archived.map((t) => t.id)).toContain(result.oldClosed2.id);
    expect(result.remaining).toHaveLength(2); // recent closed + open
    expect(result.archived).toHaveLength(2);
  });

  test("dry-run mode shows what would be archived without changes", async () => {
    const result = await runWithBun(
      Effect.gen(function* () {
        const { tasksPath } = yield* setup();
        const archivePath = tasksPath.replace("tasks.jsonl", "tasks-archive.jsonl");

        // Create an old closed task
        const oldClosed = yield* createTask({
          tasksPath,
          task: makeTask("Old closed"),
          timestamp: new Date("2025-01-01T00:00:00Z"),
        });
        yield* closeTask({
          tasksPath,
          id: oldClosed.id,
          reason: "Done",
          timestamp: new Date("2025-01-01T01:00:00Z"),
        });

        // Dry run
        const archiveResult = yield* archiveTasks({
          tasksPath,
          daysOld: 30,
          dryRun: true,
          timestamp: new Date("2025-06-15T00:00:00Z"),
        });

        const remaining = yield* readTasks(tasksPath);
        const archived = yield* readArchivedTasks(archivePath);

        return { archiveResult, remaining, archived };
      }),
    );

    expect(result.archiveResult.dryRun).toBe(true);
    expect(result.archiveResult.archived).toHaveLength(1);
    // Original file unchanged
    expect(result.remaining).toHaveLength(1);
    // No archive file created
    expect(result.archived).toHaveLength(0);
  });

  test("readArchivedTasks returns empty array when archive does not exist", async () => {
    const result = await runWithBun(
      Effect.gen(function* () {
        const { tasksPath } = yield* setup();
        const archivePath = tasksPath.replace("tasks.jsonl", "tasks-archive.jsonl");
        return yield* readArchivedTasks(archivePath);
      }),
    );

    expect(result).toEqual([]);
  });

  test("searchAllTasks searches both active and archived tasks", async () => {
    const result = await runWithBun(
      Effect.gen(function* () {
        const { tasksPath } = yield* setup();

        // Create and archive a task
        const toArchive = yield* createTask({
          tasksPath,
          task: makeTask("Archived feature"),
          timestamp: new Date("2025-01-01T00:00:00Z"),
        });
        yield* closeTask({
          tasksPath,
          id: toArchive.id,
          reason: "Done",
          timestamp: new Date("2025-01-01T01:00:00Z"),
        });

        yield* archiveTasks({
          tasksPath,
          daysOld: 30,
          timestamp: new Date("2025-06-15T00:00:00Z"),
        });

        // Create an active task
        yield* createTask({
          tasksPath,
          task: makeTask("Active feature"),
          timestamp: new Date("2025-06-10T00:00:00Z"),
        });

        const searchResult = yield* searchAllTasks({ tasksPath });

        return { searchResult, toArchive };
      }),
    );

    expect(result.searchResult.active).toHaveLength(1);
    expect(result.searchResult.active[0]?.title).toBe("Active feature");
    expect(result.searchResult.archived).toHaveLength(1);
    expect(result.searchResult.archived[0]?.title).toBe("Archived feature");
  });

  test("archive appends to existing archive file", async () => {
    const result = await runWithBun(
      Effect.gen(function* () {
        const { tasksPath } = yield* setup();
        const archivePath = tasksPath.replace("tasks.jsonl", "tasks-archive.jsonl");

        // Create and archive first task
        const first = yield* createTask({
          tasksPath,
          task: makeTask("First archived"),
          timestamp: new Date("2025-01-01T00:00:00Z"),
        });
        yield* closeTask({
          tasksPath,
          id: first.id,
          reason: "Done",
          timestamp: new Date("2025-01-01T01:00:00Z"),
        });

        yield* archiveTasks({
          tasksPath,
          daysOld: 30,
          timestamp: new Date("2025-06-01T00:00:00Z"),
        });

        // Create and archive second task
        const second = yield* createTask({
          tasksPath,
          task: makeTask("Second archived"),
          timestamp: new Date("2025-02-01T00:00:00Z"),
        });
        yield* closeTask({
          tasksPath,
          id: second.id,
          reason: "Done",
          timestamp: new Date("2025-02-01T01:00:00Z"),
        });

        yield* archiveTasks({
          tasksPath,
          daysOld: 30,
          timestamp: new Date("2025-07-01T00:00:00Z"),
        });

        const archived = yield* readArchivedTasks(archivePath);
        return { archived, first, second };
      }),
    );

    expect(result.archived).toHaveLength(2);
    expect(result.archived.map((t) => t.id)).toContain(result.first.id);
    expect(result.archived.map((t) => t.id)).toContain(result.second.id);
  });

  test("reopenTask sets closed task back to open", async () => {
    const result = await runWithBun(
      Effect.gen(function* () {
        const { tasksPath } = yield* setup();

        // Create and close a task
        const task = yield* createTask({
          tasksPath,
          task: makeTask("To be closed"),
          timestamp: new Date("2025-01-01T00:00:00Z"),
        });
        const closed = yield* closeTask({
          tasksPath,
          id: task.id,
          reason: "Initial close",
          commits: ["abc123"],
          timestamp: new Date("2025-01-01T01:00:00Z"),
        });

        // Reopen the task
        const reopened = yield* reopenTask({
          tasksPath,
          id: task.id,
          timestamp: new Date("2025-01-01T02:00:00Z"),
        });

        const tasks = yield* readTasks(tasksPath);
        const taskAfter = tasks.find((t) => t.id === task.id);

        return { closed, reopened, taskAfter };
      }),
    );

    expect(result.closed.status).toBe("closed");
    expect(result.closed.closeReason).toBe("Initial close");
    expect(result.closed.closedAt).toBe("2025-01-01T01:00:00.000Z");

    expect(result.reopened.status).toBe("open");
    expect(result.reopened.closeReason).toBeUndefined();
    expect(result.reopened.closedAt).toBeNull();
    expect(result.reopened.updatedAt).toBe("2025-01-01T02:00:00.000Z");
    // Commits should be preserved
    expect(result.reopened.commits).toEqual(["abc123"]);

    expect(result.taskAfter?.status).toBe("open");
  });

  test("reopenTask fails on non-existent task", async () => {
    const result = await runWithBun(
      Effect.gen(function* () {
        const { tasksPath } = yield* setup();

        return yield* reopenTask({
          tasksPath,
          id: "oa-nonexistent",
        }).pipe(
          Effect.map(() => ({ success: true, error: null })),
          Effect.catchAll((e) => Effect.succeed({ success: false, error: e })),
        );
      }),
    );

    expect(result.success).toBe(false);
    expect(result.error?.reason).toBe("not_found");
  });

  test("reopenTask fails on non-closed task", async () => {
    const result = await runWithBun(
      Effect.gen(function* () {
        const { tasksPath } = yield* setup();

        // Create an open task
        const task = yield* createTask({
          tasksPath,
          task: makeTask("Open task"),
        });

        return yield* reopenTask({
          tasksPath,
          id: task.id,
        }).pipe(
          Effect.map(() => ({ success: true, error: null })),
          Effect.catchAll((e) => Effect.succeed({ success: false, error: e })),
        );
      }),
    );

    expect(result.success).toBe(false);
    expect(result.error?.reason).toBe("conflict");
  });

  test("readTasks fails when conflict markers are present", async () => {
    await expect(
      runWithBun(
        Effect.gen(function* () {
          const { tasksPath } = yield* setup();
          const fs = yield* FileSystem.FileSystem;
          const content = `<<<<<<< ours
{"id":"oa-1","title":"A","description":"","status":"open","priority":2,"type":"task","labels":[],"deps":[],"commits":[],"createdAt":"2024-01-01T00:00:00.000Z","updatedAt":"2024-01-01T00:00:00.000Z","closedAt":null}
=======
{"id":"oa-1","title":"B","description":"","status":"open","priority":2,"type":"task","labels":[],"deps":[],"commits":[],"createdAt":"2024-01-01T00:00:00.000Z","updatedAt":"2024-01-01T00:00:00.000Z","closedAt":null}
>>>>>>> theirs
`;
          yield* fs.writeFile(tasksPath, new TextEncoder().encode(content));
          yield* readTasks(tasksPath);
        }),
      ),
    ).rejects.toThrow("Merge conflict markers detected");
  });
});
