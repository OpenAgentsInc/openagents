import * as BunContext from "@effect/platform-bun/BunContext";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { describe, expect, test } from "bun:test";
import { execSync } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";
import { Effect, Layer } from "effect";
import { runOrchestrator } from "./orchestrator.js";
import type { SubagentResult } from "./types.js";
import { OpenRouterClient, type OpenRouterClientShape } from "../../llm/openrouter.js";
import { DatabaseService } from "../../storage/database.js";
import { makeTestDatabaseLayer } from "../../tasks/test-helpers.ts";

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

const writeTasksFile = (dir: string, taskId: string, labels: string[] = []) => {
  const now = new Date().toISOString();
  const task = {
    id: taskId,
    title: "Claude session resume test",
    description: "ensure session IDs persist",
    status: "open",
    priority: 1,
    type: "task",
    labels,
    deps: [],
    commits: [],
    createdAt: now,
    updatedAt: now,
    closedAt: null,
  };

  fs.writeFileSync(path.join(dir, "tasks.jsonl"), `${JSON.stringify(task)}\n`);
};

describe("runOrchestrator Claude Code session persistence", () => {
  test("persists Claude Code session ids to subtasks and progress", async () => {
    const tmp = fs.mkdtempSync(path.join(process.cwd(), "tmp-cc-session-"));
    const openagentsDir = path.join(tmp, ".openagents");
    fs.mkdirSync(path.join(openagentsDir, "subtasks"), { recursive: true });
    writeTasksFile(openagentsDir, "oa-session");

    const resultPath = path.join(openagentsDir, "progress.md");
    const claudeResult: SubagentResult = {
      success: false,
      subtaskId: "oa-session-sub-001",
      filesModified: [],
      turns: 1,
      claudeCodeSessionId: "sess-new",
      claudeCodeForkedFromSessionId: "sess-old",
      sessionMetadata: {
        sessionId: "sess-new",
        forkedFromSessionId: "sess-old",
      },
    };

    try {
      await runWithBun(
        runOrchestrator(
          {
            cwd: tmp,
            openagentsDir,
            allowPush: false,
            testCommands: [],
            typecheckCommands: [],
            e2eCommands: [],
            claudeCode: { enabled: true },
          },
          () => {},
          {
            runSubagent: () => Effect.succeed(claudeResult),
          }
        )
      );

      const subtasksPath = path.join(openagentsDir, "subtasks", "oa-session.json");
      const subtasks = JSON.parse(fs.readFileSync(subtasksPath, "utf-8"));
      expect(subtasks.subtasks[0].claudeCode.sessionId).toBe("sess-new");
      expect(subtasks.subtasks[0].claudeCode.forkedFromSessionId).toBe("sess-old");

      const progress = fs.readFileSync(resultPath, "utf-8");
      expect(progress).toContain("Claude Code Session");
      expect(progress).toContain("sess-new");
      expect(progress).toContain("sess-old");
    } finally {
      fs.rmSync(tmp, { recursive: true, force: true });
    }
  });
});

describe("runOrchestrator e2e execution", () => {
  test("runs e2e commands when configured (no labels required)", async () => {
    const tmp = fs.mkdtempSync(path.join(process.cwd(), "tmp-e2e-auto-"));
    const openagentsDir = path.join(tmp, ".openagents");
    fs.mkdirSync(path.join(openagentsDir, "subtasks"), { recursive: true });
    // No labels - e2e should still run when configured
    writeTasksFile(openagentsDir, "oa-e2e-auto", []);

    // Initialize git repo for commit phase
    execSync("git init", { cwd: tmp, stdio: "ignore" });
    execSync('git config user.email "test@example.com"', { cwd: tmp, stdio: "ignore" });
    execSync('git config user.name "Test User"', { cwd: tmp, stdio: "ignore" });
    fs.writeFileSync(path.join(tmp, "README.md"), "# Test Repo\n");
    execSync("git add .", { cwd: tmp, stdio: "ignore" });
    execSync('git commit -m "init"', { cwd: tmp, stdio: "ignore" });

    const resultPath = path.join(openagentsDir, "progress.md");
    const claudeResult: SubagentResult = {
      success: true,
      subtaskId: "oa-e2e-auto-sub-001",
      filesModified: ["README.md"],
      turns: 1,
      agent: "claude-code",
    };

    try {
      await runWithBun(
        runOrchestrator(
          {
            cwd: tmp,
            openagentsDir,
            allowPush: false,
            testCommands: [],
            typecheckCommands: [],
            e2eCommands: ['node -e "console.log(\\\"e2e auto pass\\\")"'],
            claudeCode: { enabled: false },
            skipInitScript: true,
          },
          () => {},
          {
            runSubagent: () => {
              fs.appendFileSync(path.join(tmp, "README.md"), "updated\n");
              return Effect.succeed(claudeResult);
            },
          }
        )
      );

      const progress = fs.readFileSync(resultPath, "utf-8");
      expect(progress).toContain("E2E Run: Yes");
      expect(progress).toContain("E2E Passing After Work: Yes");
    } finally {
      fs.rmSync(tmp, { recursive: true, force: true });
    }
  });

  test("skips e2e commands when task has skip-e2e label", async () => {
    const tmp = fs.mkdtempSync(path.join(process.cwd(), "tmp-e2e-skip-"));
    const openagentsDir = path.join(tmp, ".openagents");
    fs.mkdirSync(path.join(openagentsDir, "subtasks"), { recursive: true });
    // skip-e2e label should skip e2e even when configured
    writeTasksFile(openagentsDir, "oa-e2e-skip", ["skip-e2e"]);

    // Initialize git repo for commit phase
    execSync("git init", { cwd: tmp, stdio: "ignore" });
    execSync('git config user.email "test@example.com"', { cwd: tmp, stdio: "ignore" });
    execSync('git config user.name "Test User"', { cwd: tmp, stdio: "ignore" });
    fs.writeFileSync(path.join(tmp, "README.md"), "# Test Repo\n");
    execSync("git add .", { cwd: tmp, stdio: "ignore" });
    execSync('git commit -m "init"', { cwd: tmp, stdio: "ignore" });

    const resultPath = path.join(openagentsDir, "progress.md");
    const claudeResult: SubagentResult = {
      success: true,
      subtaskId: "oa-e2e-skip-sub-001",
      filesModified: ["README.md"],
      turns: 1,
      agent: "claude-code",
    };

    const events: { type: string; reason?: string }[] = [];

    try {
      await runWithBun(
        runOrchestrator(
          {
            cwd: tmp,
            openagentsDir,
            allowPush: false,
            testCommands: [],
            typecheckCommands: [],
            e2eCommands: ['node -e "console.log(\\\"e2e should not run\\\")"'],
            claudeCode: { enabled: false },
            skipInitScript: true,
          },
          (event) => {
            if (event.type === "e2e_skipped") {
              events.push({ type: event.type, reason: (event as any).reason });
            }
          },
          {
            runSubagent: () => {
              fs.appendFileSync(path.join(tmp, "README.md"), "updated\n");
              return Effect.succeed(claudeResult);
            },
          }
        )
      );

      const progress = fs.readFileSync(resultPath, "utf-8");
      expect(progress).toContain("E2E Run: No");
      expect(events).toHaveLength(1);
      expect(events[0].reason).toBe("Task has skip-e2e label");
    } finally {
      fs.rmSync(tmp, { recursive: true, force: true });
    }
  });

  test("runs e2e commands when task labels require it", async () => {
    const tmp = fs.mkdtempSync(path.join(process.cwd(), "tmp-e2e-run-"));
    const openagentsDir = path.join(tmp, ".openagents");
    fs.mkdirSync(path.join(openagentsDir, "subtasks"), { recursive: true });
    writeTasksFile(openagentsDir, "oa-e2e", ["e2e"]);

    // Initialize git repo for commit phase
    execSync("git init", { cwd: tmp, stdio: "ignore" });
    execSync('git config user.email "test@example.com"', { cwd: tmp, stdio: "ignore" });
    execSync('git config user.name "Test User"', { cwd: tmp, stdio: "ignore" });
    fs.writeFileSync(path.join(tmp, "README.md"), "# Test Repo\n");
    execSync("git add .", { cwd: tmp, stdio: "ignore" });
    execSync('git commit -m "init"', { cwd: tmp, stdio: "ignore" });

    const resultPath = path.join(openagentsDir, "progress.md");
    const claudeResult: SubagentResult = {
      success: true,
      subtaskId: "oa-e2e-sub-001",
      filesModified: ["README.md"],
      turns: 1,
      agent: "claude-code",
    };

    try {
      await runWithBun(
        runOrchestrator(
          {
            cwd: tmp,
            openagentsDir,
            allowPush: false,
            testCommands: [],
            typecheckCommands: [],
            e2eCommands: ['node -e "console.log(\\\"e2e pass\\\")"'],
            claudeCode: { enabled: false },
            skipInitScript: true,
          },
          () => {},
          {
            runSubagent: () => {
              fs.appendFileSync(path.join(tmp, "README.md"), "updated\n");
              return Effect.succeed(claudeResult);
            },
          }
        )
      );

      const progress = fs.readFileSync(resultPath, "utf-8");
      expect(progress).toContain("E2E Run: Yes");
      expect(progress).toContain("E2E Passing After Work: Yes");
    } finally {
      fs.rmSync(tmp, { recursive: true, force: true });
    }
  });
});
