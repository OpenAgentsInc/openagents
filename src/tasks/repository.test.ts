import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import * as fs from "node:fs";
import * as os from "node:os";
import * as nodePath from "node:path";
import { createTaskRepository, resolveTaskRepositoryPaths } from "./repository.js";
import type { TaskCreate } from "./schema.js";
import { runWithTestContext } from "./test-helpers.js";

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

describe("TaskRepository", () => {
  test("resolves default paths from rootDir", () => {
    const rootDir = fs.mkdtempSync(nodePath.join(os.tmpdir(), "task-repo-paths-"));
    const paths = resolveTaskRepositoryPaths({ rootDir });

    expect(paths.tasksPath).toBe(nodePath.resolve(rootDir, ".openagents", "tasks.jsonl"));
    expect(paths.projectPath).toBe(nodePath.resolve(rootDir, ".openagents", "project.json"));
  });

  test("delegates ready/pick/update through shared tasksPath and accepts string config", async () => {
    const result = await runWithTestContext(
      Effect.gen(function* () {
        const fsSvc = yield* FileSystem.FileSystem;
        const pathSvc = yield* Path.Path;
        const dir = yield* fsSvc.makeTempDirectory({ prefix: "task-repo" });
        const tasksPath = pathSvc.join(dir, "tasks.jsonl");
        const repo = createTaskRepository(tasksPath);

        const first = yield* repo.create(makeTask("First"), {
          idPrefix: "oa",
          timestamp: new Date("2025-01-01T00:00:00Z"),
        });

        yield* repo.create(makeTask("Blocked", { deps: [{ id: first.id, type: "blocks" }] }), {
          idPrefix: "oa",
          timestamp: new Date("2025-01-01T00:00:00Z"),
        });

        const ready = yield* repo.ready();
        const next = yield* repo.pickNext();
        const updated = yield* repo.update({ id: first.id, status: "in_progress" });

        return { ready, next, updated };
      }),
    );

    expect(result.ready).toHaveLength(1);
    expect(result.ready[0]?.id).toBe(result.updated.id);
    expect(result.next?.id).toBe(result.updated.id);
    expect(result.updated.status).toBe("in_progress");
  });

  test("addComment and listComments work with options config", async () => {
    const result = await runWithTestContext(
      Effect.gen(function* () {
        const fsSvc = yield* FileSystem.FileSystem;
        const pathSvc = yield* Path.Path;
        const dir = yield* fsSvc.makeTempDirectory({ prefix: "task-repo" });
        const tasksPath = pathSvc.join(dir, "tasks.jsonl");
        const repo = createTaskRepository({ tasksPath });

        const created = yield* repo.create(makeTask("Comment"), { idPrefix: "oa" });
        yield* repo.addComment({ taskId: created.id, author: "tester", text: "note" });

        const comments = yield* repo.listComments(created.id);
        return { comments };
      }),
    );

    expect(result.comments).toHaveLength(1);
    expect(result.comments[0]?.text).toBe("note");
  });
});
