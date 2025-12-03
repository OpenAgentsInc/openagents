import * as BunContext from "@effect/platform-bun/BunContext";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { describe, expect, test } from "bun:test";
import * as fs from "node:fs";
import * as path from "node:path";
import { Effect, Layer } from "effect";
import { runOrchestrator } from "./orchestrator.js";
import type { SubagentResult } from "./types.js";
import { OpenRouterClient, type OpenRouterClientShape } from "../../llm/openrouter.js";

const mockOpenRouterLayer = Layer.succeed(OpenRouterClient, {
  chat: () => Effect.fail(new Error("not used")),
} satisfies OpenRouterClientShape);
const testLayer = Layer.mergeAll(BunContext.layer, mockOpenRouterLayer);

const runWithBun = <A, E>(
  program: Effect.Effect<A, E, FileSystem.FileSystem | Path.Path | OpenRouterClient>
) => Effect.runPromise(program.pipe(Effect.provide(testLayer)));

const writeTasksFile = (dir: string, taskId: string) => {
  const now = new Date().toISOString();
  const task = {
    id: taskId,
    title: "Claude session resume test",
    description: "ensure session IDs persist",
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
