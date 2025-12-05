import { describe, expect, test } from "bun:test";
import { BunContext } from "@effect/platform-bun";
import { Effect } from "effect";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import * as path from "node:path";
import {
  STEP_RESULTS_FILENAME,
  createStepResultsManager,
  durableStep,
} from "../step-results.js";

const runWithBun = <A, E>(
  program: Effect.Effect<A, E, import("@effect/platform/FileSystem").FileSystem | import("@effect/platform/Path").Path>
) => Effect.runPromise(program.pipe(Effect.provide(BunContext.layer)));

const makeTempOpenagentsDir = async () => {
  const baseDir = await mkdtemp(path.join(tmpdir(), "step-results-"));
  const openagentsDir = path.join(baseDir, ".openagents");
  await mkdir(openagentsDir, { recursive: true });
  return { baseDir, openagentsDir };
};

describe("step-results memoization", () => {
  test("replays cached result when replay mode is active", async () => {
    const { baseDir, openagentsDir } = await makeTempOpenagentsDir();
    try {
      const store = {
        sessionId: "session-existing",
        steps: [
          {
            stepId: "init_script",
            sessionId: "session-existing",
            timestamp: new Date().toISOString(),
            result: { ran: true },
            inputHash: "hash-1",
          },
        ],
      };

      await writeFile(
        path.join(openagentsDir, STEP_RESULTS_FILENAME),
        JSON.stringify(store, null, 2)
      );

      const manager = await runWithBun(
        createStepResultsManager(openagentsDir, "session-new")
      );

      let executed = false;
      const result = await runWithBun(
        durableStep(
          manager,
          "init_script",
          () => {
            executed = true;
            return Effect.succeed({ ran: false });
          },
          { inputHash: "hash-1" }
        )
      );

      expect(executed).toBe(false);
      expect(result).toEqual({ ran: true });
      expect(manager.replayMode).toBe(true);
      expect(manager.sessionId).toBe("session-existing");
    } finally {
      await rm(baseDir, { recursive: true, force: true });
    }
  });

  test("records fresh step results when not cached", async () => {
    const { baseDir, openagentsDir } = await makeTempOpenagentsDir();
    try {
      const manager = await runWithBun(
        createStepResultsManager(openagentsDir, "session-new")
      );

      const output = await runWithBun(
        durableStep(manager, "select_task", () =>
          Effect.succeed({ id: "task-1" })
        )
      );

      expect(output).toEqual({ id: "task-1" });
      expect(manager.replayMode).toBe(false);

      const content = await readFile(
        path.join(openagentsDir, STEP_RESULTS_FILENAME),
        "utf8"
      );
      const parsed = JSON.parse(content);
      expect(parsed.sessionId).toBe("session-new");
      expect(parsed.steps[0].stepId).toBe("select_task");
      expect(parsed.steps[0].result).toEqual({ id: "task-1" });
    } finally {
      await rm(baseDir, { recursive: true, force: true });
    }
  });

  test("clear removes persisted step results", async () => {
    const { baseDir, openagentsDir } = await makeTempOpenagentsDir();
    try {
      const store = { sessionId: "session-clear", steps: [] };
      await writeFile(
        path.join(openagentsDir, STEP_RESULTS_FILENAME),
        JSON.stringify(store, null, 2)
      );

      const manager = await runWithBun(
        createStepResultsManager(openagentsDir, "session-clear")
      );

      await runWithBun(manager.clear());

      let removed = false;
      try {
        await readFile(path.join(openagentsDir, STEP_RESULTS_FILENAME), "utf8");
      } catch {
        removed = true;
      }

      expect(removed).toBe(true);
    } finally {
      await rm(baseDir, { recursive: true, force: true });
    }
  });
});
