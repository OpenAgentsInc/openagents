/**
 * Tests for Safe Mode self-healing functionality in the orchestrator.
 *
 * Safe mode enables the orchestrator to automatically recover from certain
 * init script failures by spawning Claude Code to fix the underlying issues.
 *
 * @see docs/mechacoder/GOLDEN-LOOP-v2.md Section 2.2.1 (Init Script)
 */
import * as BunContext from "@effect/platform-bun/BunContext";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { describe, expect, test } from "bun:test";
import { Effect, Layer } from "effect";
import { execSync } from "node:child_process";
import * as fs from "node:fs";
import { tmpdir } from "node:os";
import * as path from "node:path";
import { runOrchestrator } from "./orchestrator.js";
import type { OrchestratorEvent, SubagentResult } from "./types.js";
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

/**
 * Creates a test git repository with OpenAgents task configuration
 */
const createTestRepo = (name: string) => {
  const dir = fs.mkdtempSync(path.join(tmpdir(), `safemode-${name}-`));

  execSync("git init", { cwd: dir, stdio: "ignore" });
  execSync("git checkout -b main", { cwd: dir, stdio: "ignore" });
  execSync('git config user.email "mechacoder@example.com"', {
    cwd: dir,
    stdio: "ignore",
  });
  execSync('git config user.name "MechaCoder"', { cwd: dir, stdio: "ignore" });

  fs.writeFileSync(path.join(dir, "README.md"), "# Test Repo\n");

  const now = new Date().toISOString();
  const task = {
    id: `oa-${name}`,
    title: `Safe mode test: ${name}`,
    description: "Test safe mode self-healing",
    status: "open",
    priority: 1,
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
    `${JSON.stringify(task)}\n`
  );

  execSync("git add -A", { cwd: dir, stdio: "ignore" });
  execSync('git commit -m "init"', { cwd: dir, stdio: "ignore" });

  return { dir, taskId: task.id, openagentsDir: oaDir };
};

describe("Safe Mode Self-Healing", () => {
  describe("Init script failure detection", () => {
    test("detects typecheck_failed failure type from init script", async () => {
      const { dir, openagentsDir } = createTestRepo("typecheck-detect");
      const events: OrchestratorEvent[] = [];

      // Create init script that reports typecheck failure
      const initScript = path.join(openagentsDir, "init.sh");
      fs.writeFileSync(
        initScript,
        `#!/bin/bash
echo "error TS2322: Type 'string' is not assignable to type 'number'"
exit 1
`
      );

      const state = await runWithBun(
        runOrchestrator(
          {
            cwd: dir,
            openagentsDir,
            testCommands: ["echo tests"],
            allowPush: false,
            safeMode: false, // Disabled safe mode - just test detection
          },
          (event) => events.push(event),
          { runSubagent: () => Effect.fail(new Error("should not be called")) }
        )
      );

      // Should fail during orientation due to init script failure
      expect(state.phase).toBe("failed");
      expect(state.error).toContain("Init script failed");
      expect(state.error).toContain("typecheck_failed");

      // Check the init_script_complete event has failure type info
      const initCompleteEvent = events.find(
        (e) => e.type === "init_script_complete"
      ) as { type: "init_script_complete"; result: any } | undefined;

      expect(initCompleteEvent).toBeDefined();
      expect(initCompleteEvent?.result.failureType).toBe("typecheck_failed");
      expect(initCompleteEvent?.result.canSelfHeal).toBe(true);
    });

    test("detects test_failed failure type from init script", async () => {
      const { dir, openagentsDir } = createTestRepo("test-detect");
      const events: OrchestratorEvent[] = [];

      const initScript = path.join(openagentsDir, "init.sh");
      fs.writeFileSync(
        initScript,
        `#!/bin/bash
echo "5 tests failed"
echo "expect(received).toBe(expected)"
exit 1
`
      );

      const state = await runWithBun(
        runOrchestrator(
          {
            cwd: dir,
            openagentsDir,
            testCommands: ["echo tests"],
            allowPush: false,
            safeMode: false,
          },
          (event) => events.push(event),
          { runSubagent: () => Effect.fail(new Error("should not be called")) }
        )
      );

      expect(state.phase).toBe("failed");
      expect(state.error).toContain("test_failed");

      const initCompleteEvent = events.find(
        (e) => e.type === "init_script_complete"
      ) as { type: "init_script_complete"; result: any } | undefined;

      expect(initCompleteEvent?.result.failureType).toBe("test_failed");
      expect(initCompleteEvent?.result.canSelfHeal).toBe(true);
    });

    test("detects non-healable failure types", async () => {
      const { dir, openagentsDir } = createTestRepo("non-healable");
      const events: OrchestratorEvent[] = [];

      const initScript = path.join(openagentsDir, "init.sh");
      fs.writeFileSync(
        initScript,
        `#!/bin/bash
echo "ENOSPC: no space left on device"
exit 1
`
      );

      const state = await runWithBun(
        runOrchestrator(
          {
            cwd: dir,
            openagentsDir,
            testCommands: ["echo tests"],
            allowPush: false,
            safeMode: true, // Safe mode enabled but shouldn't try to heal disk_full
          },
          (event) => events.push(event),
          { runSubagent: () => Effect.fail(new Error("should not be called")) }
        )
      );

      // Should fail without attempting self-heal
      expect(state.phase).toBe("failed");
      expect(state.error).toContain("disk_full");
      expect(state.error).not.toContain("self-heal attempted");
    });
  });

  describe("Safe mode disabled behavior", () => {
    test("does not attempt self-healing when safeMode is false", async () => {
      const { dir, openagentsDir } = createTestRepo("no-heal");
      const events: OrchestratorEvent[] = [];
      let subagentCalled = false;

      const initScript = path.join(openagentsDir, "init.sh");
      fs.writeFileSync(
        initScript,
        `#!/bin/bash
echo "error TS2322: Type error"
exit 1
`
      );

      const state = await runWithBun(
        runOrchestrator(
          {
            cwd: dir,
            openagentsDir,
            testCommands: ["echo tests"],
            allowPush: false,
            safeMode: false, // Disabled
          },
          (event) => events.push(event),
          {
            runSubagent: () => {
              subagentCalled = true;
              return Effect.succeed({
                success: true,
                subtaskId: "test",
                filesModified: [],
                turns: 1,
              } as SubagentResult);
            },
          }
        )
      );

      // Should fail without calling subagent for healing
      expect(state.phase).toBe("failed");
      expect(subagentCalled).toBe(false);

      // No emergency subtask events
      const subtaskStartEvents = events.filter((e) => e.type === "subtask_start");
      expect(subtaskStartEvents).toHaveLength(0);
    });

    test("does not attempt self-healing when safeMode is undefined", async () => {
      const { dir, openagentsDir } = createTestRepo("no-safe");
      const events: OrchestratorEvent[] = [];

      const initScript = path.join(openagentsDir, "init.sh");
      fs.writeFileSync(
        initScript,
        `#!/bin/bash
echo "error TS2322: Type error"
exit 1
`
      );

      const state = await runWithBun(
        runOrchestrator(
          {
            cwd: dir,
            openagentsDir,
            testCommands: ["echo tests"],
            allowPush: false,
            // safeMode not set (undefined)
          },
          (event) => events.push(event),
          {
            runSubagent: () =>
              Effect.fail(new Error("should not be called for healing")),
          }
        )
      );

      expect(state.phase).toBe("failed");
      expect(state.error).toContain("Init script failed");
    });
  });

  describe("Progress file records init script results", () => {
    test("records failureType in progress when init script fails", async () => {
      const { dir, openagentsDir } = createTestRepo("progress-fail");

      const initScript = path.join(openagentsDir, "init.sh");
      fs.writeFileSync(
        initScript,
        `#!/bin/bash
echo "error TS2322: Type 'string' is not assignable"
exit 1
`
      );

      await runWithBun(
        runOrchestrator(
          {
            cwd: dir,
            openagentsDir,
            testCommands: ["echo tests"],
            allowPush: false,
            safeMode: false,
          },
          () => {},
          { runSubagent: () => Effect.fail(new Error("should not run")) }
        )
      );

      const progressPath = path.join(openagentsDir, "progress.md");
      const progress = fs.readFileSync(progressPath, "utf-8");

      expect(progress).toContain("Blockers");
      expect(progress).toContain("Init script failed");
    });

    test("records hasWarnings when init script exits with 2", async () => {
      const { dir, taskId, openagentsDir } = createTestRepo("progress-warn");
      const events: OrchestratorEvent[] = [];

      const initScript = path.join(openagentsDir, "init.sh");
      fs.writeFileSync(
        initScript,
        `#!/bin/bash
echo "Warning: uncommitted changes detected"
exit 2
`
      );

      // Create a mock subagent that succeeds
      const subagentRunner = () =>
        Effect.sync(() => {
          fs.writeFileSync(path.join(dir, "feature.txt"), "test");
          return {
            success: true,
            subtaskId: `${taskId}-sub-001`,
            filesModified: ["feature.txt"],
            turns: 1,
          } as SubagentResult;
        });

      const state = await runWithBun(
        runOrchestrator(
          {
            cwd: dir,
            openagentsDir,
            testCommands: ["echo tests"],
            allowPush: false,
            maxSubtasksPerTask: 1,
          },
          (event) => events.push(event),
          { runSubagent: subagentRunner }
        )
      );

      // Should continue despite warnings (exit 2)
      expect(state.phase).toBe("done");

      const initCompleteEvent = events.find(
        (e) => e.type === "init_script_complete"
      ) as { type: "init_script_complete"; result: any } | undefined;

      expect(initCompleteEvent?.result.success).toBe(true);
      expect(initCompleteEvent?.result.hasWarnings).toBe(true);
    });
  });

  describe("Suggested next steps based on failure type", () => {
    test("suggests inspecting init script on failure", async () => {
      const { dir, openagentsDir } = createTestRepo("suggest-steps");

      const initScript = path.join(openagentsDir, "init.sh");
      fs.writeFileSync(
        initScript,
        `#!/bin/bash
echo "Unknown failure"
exit 1
`
      );

      await runWithBun(
        runOrchestrator(
          {
            cwd: dir,
            openagentsDir,
            testCommands: ["echo tests"],
            allowPush: false,
            safeMode: false,
          },
          () => {},
          { runSubagent: () => Effect.fail(new Error("should not run")) }
        )
      );

      const progressPath = path.join(openagentsDir, "progress.md");
      const progress = fs.readFileSync(progressPath, "utf-8");

      expect(progress).toContain("Next Session Should");
      expect(progress).toMatch(/Inspect.*init\.sh/i);
    });
  });

  describe("Event emission during init script processing", () => {
    test("emits init_script_start and init_script_complete events", async () => {
      const { dir, taskId, openagentsDir } = createTestRepo("init-events");
      const events: OrchestratorEvent[] = [];

      const initScript = path.join(openagentsDir, "init.sh");
      fs.writeFileSync(
        initScript,
        `#!/bin/bash
echo "Init complete"
exit 0
`
      );

      const subagentRunner = () =>
        Effect.sync(() => {
          fs.writeFileSync(path.join(dir, "feature.txt"), "test");
          return {
            success: true,
            subtaskId: `${taskId}-sub-001`,
            filesModified: ["feature.txt"],
            turns: 1,
          } as SubagentResult;
        });

      await runWithBun(
        runOrchestrator(
          {
            cwd: dir,
            openagentsDir,
            testCommands: ["echo tests"],
            allowPush: false,
            maxSubtasksPerTask: 1,
          },
          (event) => events.push(event),
          { runSubagent: subagentRunner }
        )
      );

      const initStartEvent = events.find((e) => e.type === "init_script_start");
      const initCompleteEvent = events.find((e) => e.type === "init_script_complete");

      expect(initStartEvent).toBeDefined();
      expect(initCompleteEvent).toBeDefined();

      // Events should be in order
      const startIdx = events.findIndex((e) => e.type === "init_script_start");
      const completeIdx = events.findIndex((e) => e.type === "init_script_complete");
      expect(startIdx).toBeLessThan(completeIdx);
    });

    test("orientation_complete includes init script result", async () => {
      const { dir, taskId, openagentsDir } = createTestRepo("orient-init");
      const events: OrchestratorEvent[] = [];

      const initScript = path.join(openagentsDir, "init.sh");
      fs.writeFileSync(
        initScript,
        `#!/bin/bash
echo "Preflight OK"
exit 0
`
      );

      const subagentRunner = () =>
        Effect.sync(() => {
          fs.writeFileSync(path.join(dir, "feature.txt"), "test");
          return {
            success: true,
            subtaskId: `${taskId}-sub-001`,
            filesModified: ["feature.txt"],
            turns: 1,
          } as SubagentResult;
        });

      await runWithBun(
        runOrchestrator(
          {
            cwd: dir,
            openagentsDir,
            testCommands: ["echo tests"],
            allowPush: false,
            maxSubtasksPerTask: 1,
          },
          (event) => events.push(event),
          { runSubagent: subagentRunner }
        )
      );

      const orientCompleteEvent = events.find(
        (e) => e.type === "orientation_complete"
      ) as { type: "orientation_complete"; initScript?: any } | undefined;

      expect(orientCompleteEvent).toBeDefined();
      expect(orientCompleteEvent?.initScript).toBeDefined();
      expect(orientCompleteEvent?.initScript.ran).toBe(true);
      expect(orientCompleteEvent?.initScript.success).toBe(true);
    });
  });
});
