
import * as BunContext from "@effect/platform-bun/BunContext";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { makeDatabaseLive } from "../storage/database.js";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import { loadMechaCoderState } from "./mechacoder-state.js";

const runWithBun = <A, E>(
  program: Effect.Effect<A, E, FileSystem.FileSystem | Path.Path>,
) => Effect.runPromise(program.pipe(Effect.provide(BunContext.layer)));

const writeFile = (
  fs: FileSystem.FileSystem,
  filePath: string,
  content: string,
) => fs.writeFile(filePath, new TextEncoder().encode(content));

describe("loadMechaCoderState", () => {
  test("loads tasks and run logs into a snapshot", async () => {
    const state = await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const path = yield* Path.Path;
        const root = yield* fs.makeTempDirectory({ prefix: "mechacoder-state" });
        const oaDir = path.join(root, ".openagents");
        const runLogDayDir = path.join(oaDir, "run-logs", "20250101");
        yield* fs.makeDirectory(runLogDayDir, { recursive: true });

        const projectConfig = { projectId: "demo-project" };
        yield* writeFile(
          fs,
          path.join(oaDir, "project.json"),
          JSON.stringify(projectConfig, null, 2),
        );

        const baseTs = "2025-01-01T00:00:00.000Z";
        const tasks = [
          {
            id: "oa-inprog",
            title: "In progress task",
            description: "",
            status: "in_progress",
            priority: 1,
            type: "feature",
            labels: ["hud"],
            deps: [],
            commits: [],
            createdAt: baseTs,
            updatedAt: baseTs,
            closedAt: null,
          },
          {
            id: "oa-blocked",
            title: "Blocked task",
            description: "",
            status: "blocked",
            priority: 2,
            type: "task",
            labels: [],
            deps: [{ id: "oa-inprog", type: "blocks" }],
            commits: [],
            createdAt: baseTs,
            updatedAt: baseTs,
            closedAt: null,
          },
        ];

        const tasksContent = tasks.map((t) => JSON.stringify(t)).join("\n") + "\n";
        yield* writeFile(fs, path.join(oaDir, "tasks.jsonl"), tasksContent);

        const run1 = {
          id: "run-early",
          taskId: "oa-inprog",
          taskTitle: "In progress task",
          status: "success" as const,
          startedAt: "2025-01-01T00:00:00.000Z",
          finishedAt: "2025-01-01T00:05:00.000Z",
          workDir: root,
          logFilePath: null,
          sessionFilePath: null,
          commits: ["abc123"],
          totalTurns: 3,
          finalMessage: "TASK_COMPLETED: oa-inprog",
          error: null,
        };

        const run2 = {
          id: "run-late",
          taskId: "oa-blocked",
          taskTitle: "Blocked task",
          status: "incomplete" as const,
          startedAt: "2025-01-01T01:00:00.000Z",
          finishedAt: "2025-01-01T01:10:00.000Z",
          workDir: root,
          logFilePath: null,
          sessionFilePath: null,
          commits: [],
          totalTurns: 5,
          finalMessage: "Missing TASK_COMPLETED",
          error: "incomplete",
        };

        yield* writeFile(
          fs,
          path.join(runLogDayDir, "000500-oa-inprog.json"),
          JSON.stringify(run1, null, 2),
        );
        yield* writeFile(
          fs,
          path.join(runLogDayDir, "011000-oa-blocked.json"),
          JSON.stringify(run2, null, 2),
        );

        const dbLayer = makeDatabaseLive(path.join(root, "test.db"));

        return yield* loadMechaCoderState({
          rootDir: root,
          maxRunLogs: 5,
        }).pipe(Effect.provide(dbLayer));
      }),
    );

    expect(state.repos).toHaveLength(1);
    expect(state.repos[0].name).toBe("demo-project");
    expect(state.repos[0].tasks).toHaveLength(2);
    expect(state.repos[0].rollup?.total).toBe(2);
    expect(state.repos[0].rollup?.blocked).toBe(1);
    expect(state.activeTaskId).toBe("oa-inprog");
    expect(state.currentPhase).toBe("edit");
    expect(state.recentRuns).toHaveLength(2);
    expect(state.recentRuns[0].id).toBe("run-late");
    expect(state.recentRuns[1].id).toBe("run-early");

    const blockedTask = state.repos[0].tasks.find((t) => t.id === "oa-blocked");
    expect(blockedTask?.deps?.[0].status).toBe("in_progress");
  });

  test("handles missing run logs gracefully", async () => {
    const state = await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const path = yield* Path.Path;
        const root = yield* fs.makeTempDirectory({ prefix: "mechacoder-state-empty" });
        const oaDir = path.join(root, ".openagents");
        yield* fs.makeDirectory(oaDir, { recursive: true });

        const projectConfig = { projectId: "no-logs" };
        yield* writeFile(
          fs,
          path.join(oaDir, "project.json"),
          JSON.stringify(projectConfig, null, 2),
        );

        const tasks = [
          {
            id: "oa-closed",
            title: "Closed task",
            description: "",
            status: "closed",
            priority: 2,
            type: "task",
            labels: [],
            deps: [],
            commits: [],
            createdAt: "2025-01-01T00:00:00.000Z",
            updatedAt: "2025-01-01T00:00:00.000Z",
            closedAt: "2025-01-01T00:10:00.000Z",
          },
        ];

        const tasksContent = tasks.map((t) => JSON.stringify(t)).join("\n") + "\n";
        yield* writeFile(fs, path.join(oaDir, "tasks.jsonl"), tasksContent);

        return yield* loadMechaCoderState({ rootDir: root, maxRunLogs: 3 });
      }),
    );

    expect(state.recentRuns).toHaveLength(0);
    expect(state.repos[0].rollup?.closed).toBe(1);
    expect(state.currentPhase).toBe("idle");
  });
});
