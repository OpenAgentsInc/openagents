
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import { mergeTasks, mergeTaskFiles, ensureMergeDriverConfig } from "./merge.js";
import { runWithTestContext } from "./test-helpers.js";
import type { Task } from "./schema.js";


const toJsonl = (tasks: Task[]): string => tasks.map((t) => JSON.stringify(t)).join("\n") + "\n";

const baseTask = (overrides: Partial<Task> = {}): Task => ({
  id: "oa-1",
  title: "Base",
  description: "",
  status: "open",
  priority: 2,
  type: "task",
  labels: [],
  deps: [],
  commits: [],
  comments: [],
  createdAt: "2024-01-01T00:00:00.000Z",
  updatedAt: "2024-01-01T00:00:00.000Z",
  ...overrides,
});

describe("mergeTasks", () => {
  test("merges arrays with union and picks newer updatedAt on conflicts", () => {
    const base = toJsonl([baseTask()]);
    const current = toJsonl([
      baseTask({
        title: "Current",
        labels: ["a", "b"],
        updatedAt: "2024-02-01T00:00:00.000Z",
      }),
    ]);
    const incoming = toJsonl([
      baseTask({
        title: "Incoming",
        labels: ["b", "c"],
        updatedAt: "2024-03-01T00:00:00.000Z",
      }),
    ]);

    const { tasks } = mergeTasks(base, current, incoming);
    expect(tasks[0].title).toBe("Incoming"); // newer updatedAt wins
    expect([...tasks[0].labels].sort()).toEqual(["a", "b", "c"]);
    expect(tasks[0].updatedAt).toBe("2024-03-01T00:00:00.000Z"); // max timestamp
  });

  test("adds tasks that exist only in one side", () => {
    const base = toJsonl([]);
    const current = toJsonl([baseTask({ id: "oa-1", title: "Only-current" })]);
    const incoming = toJsonl([baseTask({ id: "oa-2", title: "Only-incoming" })]);

    const { tasks } = mergeTasks(base, current, incoming);
    const ids = tasks.map((t) => t.id).sort();
    expect(ids).toEqual(["oa-1", "oa-2"]);
  });

  test("merges deps and commits with deduplication", () => {
    const base = toJsonl([
      baseTask({
        deps: [{ id: "x", type: "related" }],
        commits: ["a"],
      }),
    ]);
    const current = toJsonl([
      baseTask({
        deps: [{ id: "y", type: "blocks" }],
        commits: ["b"],
        updatedAt: "2024-02-01T00:00:00.000Z",
      }),
    ]);
    const incoming = toJsonl([
      baseTask({
        deps: [{ id: "x", type: "related" }],
        commits: ["b", "c"],
        updatedAt: "2024-02-02T00:00:00.000Z",
      }),
    ]);

    const { tasks } = mergeTasks(base, current, incoming);
    expect(tasks[0].deps).toEqual([
      { id: "x", type: "related" },
      { id: "y", type: "blocks" },
    ]);
    expect([...tasks[0].commits].sort()).toEqual(["a", "b", "c"]);
  });
});

describe("mergeTaskFiles", () => {
  test("writes merged content to output file", async () => {
    const result = await runWithTestContext(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const dir = yield* fs.makeTempDirectory({ prefix: "merge-files" });
        const basePath = `${dir}/base.jsonl`;
        const currentPath = `${dir}/current.jsonl`;
        const incomingPath = `${dir}/incoming.jsonl`;
        const outputPath = `${dir}/merged.jsonl`;

        yield* fs.writeFile(basePath, new TextEncoder().encode(toJsonl([baseTask()])));
        yield* fs.writeFile(
          currentPath,
          new TextEncoder().encode(toJsonl([baseTask({ title: "Current" })])),
        );
        yield* fs.writeFile(
          incomingPath,
          new TextEncoder().encode(toJsonl([baseTask({ title: "Incoming", updatedAt: "2024-02-01T00:00:00.000Z" })])),
        );

        const mergeResult = yield* mergeTaskFiles({
          basePath,
          currentPath,
          incomingPath,
          outputPath,
        });

        const mergedContent = yield* fs.readFileString(outputPath);
        return { mergeResult, mergedContent };
      }),
    );

    expect(result.mergeResult.mergedPath.endsWith("merged.jsonl")).toBe(true);
    const parsed = result.mergedContent.trim().split("\n").map((l) => JSON.parse(l) as Task);
    expect(parsed[0].title).toBe("Incoming");
  });
});

describe("ensureMergeDriverConfig", () => {
  test("creates .gitattributes and appends merge driver config when .git exists", async () => {
    const result = await runWithTestContext(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const path = yield* Path.Path;
        const dir = yield* fs.makeTempDirectory({ prefix: "merge-config" });
        const gitDir = path.join(dir, ".git");
        const gitConfigPath = path.join(gitDir, "config");

        yield* fs.makeDirectory(gitDir, { recursive: true });
        yield* fs.writeFile(
          gitConfigPath,
          new TextEncoder().encode("[core]\n\trepositoryformatversion = 0\n"),
        );

        const resultPaths = yield* ensureMergeDriverConfig(dir);
        const gitattributes = yield* fs.readFileString(path.join(dir, ".gitattributes"));
        const gitConfig = yield* fs.readFileString(gitConfigPath);

        return { resultPaths, gitattributes, gitConfig };
      }),
    );

    expect(result.gitattributes).toContain(".openagents/tasks.jsonl merge=oa-tasks");
    expect(result.gitConfig).toContain('[merge "oa-tasks"]');
    expect(result.gitConfig).toContain("driver = bun src/tasks/cli.ts merge");
    expect(result.resultPaths.gitattributesPath).toBeTruthy();
    expect(result.resultPaths.gitConfigPath).toBeTruthy();
  });
});
