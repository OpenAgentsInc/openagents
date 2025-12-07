import * as BunContext from "@effect/platform-bun/BunContext";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { describe, expect, test } from "bun:test";
import { Effect, Layer } from "effect";
import { execSync, spawnSync } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";
import { OpenRouterClient, type OpenRouterClientShape } from "../../llm/openrouter.js";
import { DatabaseService } from "../../storage/database.js";
import { makeTestDatabaseLayer } from "../../tasks/test-helpers.js";
import { runOrchestrator } from "./orchestrator.js";
import { createGoldenLoopFixture } from "./golden-loop-fixture.js";
import type { OrchestratorEvent, SubagentResult } from "./types.js";
import { runBestAvailableSubagent } from "./subagent-router.js";

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

const withEnv = async <T>(env: Record<string, string | undefined>, fn: () => Promise<T>): Promise<T> => {
  const previous: Record<string, string | undefined> = {};
  for (const [key, value] of Object.entries(env)) {
    previous[key] = process.env[key];
    if (value === undefined) {
      delete process.env[key];
    } else {
      process.env[key] = value;
    }
  }

  try {
    return await fn();
  } finally {
    for (const [key, value] of Object.entries(previous)) {
      if (value === undefined) {
        delete process.env[key];
      } else {
        process.env[key] = value;
      }
    }
  }
};

const readTasks = (tasksPath: string) =>
  fs.readFileSync(tasksPath, "utf-8").trim().split("\n").map((line) => JSON.parse(line));

describe("orchestrator stress harness", () => {
  test("recovers from chaos scenarios and leaves the repo clean", async () => {
    const fixture = createGoldenLoopFixture({
      name: "stress-harness",
      testCommands: ["bash ./scripts/flaky-test.sh"],
      setup: (dir, openagentsDir) => {
        const scriptsDir = path.join(dir, "scripts");
        fs.mkdirSync(scriptsDir, { recursive: true });

        const testScript = [
          "#!/usr/bin/env bash",
          "set -euo pipefail",
          "mkdir -p .tmp",
          'flag=".tmp/fail-once-hit"',
          "",
          'if [[ -f "$flag" ]]; then',
          '  echo "flaky failure" >&2',
          "  exit 1",
          "fi",
          "",
          'echo "tests passed"',
          "",
        ].join("\n");
        const testPath = path.join(scriptsDir, "flaky-test.sh");
        fs.writeFileSync(testPath, testScript, "utf-8");
        fs.chmodSync(testPath, 0o755);

        const initScript = [
          "#!/usr/bin/env bash",
          "set -euo pipefail",
          'oa_dir=$(cd "$(dirname "$0")" && pwd)',
          "",
          'if [[ -f "${oa_dir}/require-token" && -z "${DUMMY_TOKEN:-}" ]]; then',
          '  echo "missing required token"',
          "  exit 1",
          "fi",
          "",
          'if [[ "${CHAOS_REQUIRE_TOKEN:-0}" == "1" && -z "${DUMMY_TOKEN:-}" ]]; then',
          '  echo "missing required token"',
          "  exit 1",
          "fi",
          "",
          'if [[ "${CHAOS_ALLOW_DIRTY:-0}" != "1" ]]; then',
          '  filtered=""',
          '  while IFS= read -r line; do',
          '    case "$line" in',
          '      "?? .openagents/progress.md"|" M .openagents/progress.md") continue ;;',
          '      "?? .openagents/subtasks"*|" M .openagents/subtasks"*) continue ;;',
          '      "?? .openagents/checkpoint.json"*|" M .openagents/checkpoint.json"*|" D .openagents/checkpoint.json"*) continue ;;',
          '      "?? docs/logs/"*|" M docs/logs/"*) continue ;;',
          '      "?? .tmp/"*|" M .tmp/"*) continue ;;',
          "    esac",
          '    filtered+="${line}\n"',
          "  done < <(git status --porcelain)",
          '  if [[ -n "${filtered}" ]]; then',
          '    echo "workspace dirty"',
          '    printf "%b" "${filtered}"',
          "    exit 1",
          "  fi",
          "fi",
          "",
          'echo "init ok"',
          "",
        ].join("\n");
        const initPath = path.join(openagentsDir, "init.sh");
        fs.writeFileSync(initPath, initScript, "utf-8");
        fs.chmodSync(initPath, 0o755);

        fs.writeFileSync(path.join(dir, ".gitignore"), ".tmp/\n", "utf-8");
      },
    });

    const { dir, openagentsDir, taskId, tasksPath } = fixture;
    const tokenGuardPath = path.join(openagentsDir, "require-token");
    const flakyFlagPath = path.join(dir, ".tmp", "fail-once-hit");
    const events: { label: string; event: OrchestratorEvent }[] = [];
    const baseConfig = {
      cwd: dir,
      openagentsDir,
      testCommands: ["bash ./scripts/flaky-test.sh"],
      allowPush: false,
      maxSubtasksPerTask: 1,
      claudeCode: { enabled: false },
    };

    const subagentRunner: typeof runBestAvailableSubagent = (options) =>
      Effect.sync(() => {
        const tmpDir = path.join(options.cwd, ".tmp");
        fs.mkdirSync(tmpDir, { recursive: true });
        const outputFile = path.join(tmpDir, `artifact-${options.subtask.id}.txt`);
        fs.writeFileSync(outputFile, `completed ${options.subtask.id}`);
        return {
          success: true,
          subtaskId: options.subtask.id,
          filesModified: [path.relative(options.cwd, outputFile)],
          turns: 1,
          agent: "minimal",
        } satisfies SubagentResult;
      });

    const runScenario = (label: string, env: Record<string, string | undefined>, prepare?: () => void) =>
      withEnv(env, () => {
        prepare?.();
        return runWithBun(
          runOrchestrator(
            baseConfig,
            (event) => events.push({ label, event }),
            { runSubagent: subagentRunner },
          )
        );
      });

    const dirtyFile = path.join(dir, "dirty.txt");
    const dirtyState = await runScenario(
      "dirty-working-tree",
      {},
      () => fs.writeFileSync(dirtyFile, "leftover dirt", "utf-8"),
    );
    expect(dirtyState.phase).toBe("failed");
    expect(dirtyState.error).toContain("Init script failed");
    expect(readTasks(tasksPath)[0].status).toBe("open");
    fs.unlinkSync(dirtyFile);

    const missingEnvState = await runScenario(
      "missing-env",
      {
        DUMMY_TOKEN: "",
      },
      () => fs.writeFileSync(tokenGuardPath, "required", "utf-8"),
    );
    expect(missingEnvState.phase).toBe("failed");
    const missingProbe = spawnSync("bash", [path.join(openagentsDir, "init.sh")], {
      cwd: dir,
      env: { ...process.env, DUMMY_TOKEN: "" },
      encoding: "utf-8",
    });
    expect(missingProbe.status).toBe(1);
    fs.rmSync(tokenGuardPath, { force: true });
    expect(readTasks(tasksPath)[0].status).toBe("open");

    const flakyFailure = await runScenario(
      "flaky-tests-first",
      {
        DUMMY_TOKEN: "ok",
      },
      () => {
        fs.mkdirSync(path.dirname(flakyFlagPath), { recursive: true });
        fs.writeFileSync(flakyFlagPath, "fail", "utf-8");
      },
    );
    expect(flakyFailure.phase).toBe("failed");
    const flakyVerifications = events
      .filter((e) => e.label === "flaky-tests-first")
      .map((entry) => entry.event)
      .filter(
        (event): event is Extract<OrchestratorEvent, { type: "verification_complete" }> =>
          event.type === "verification_complete"
      );
    expect(flakyVerifications.some((event) => event.passed === false)).toBe(true);
    const flakyProbe = spawnSync("bash", [path.join(dir, "scripts", "flaky-test.sh")], {
      cwd: dir,
      env: { ...process.env, DUMMY_TOKEN: "ok" },
      encoding: "utf-8",
    });
    expect(flakyProbe.status).toBe(1);
    fs.rmSync(flakyFlagPath, { force: true });
    expect(readTasks(tasksPath)[0].status).toBe("open");

    const successState = await runScenario("flaky-tests-recover", { DUMMY_TOKEN: "ok" });
    expect(successState.phase).toBe("done");

    const closedTask = readTasks(tasksPath)[0];
    expect(closedTask.status).toBe("closed");
    expect(closedTask.commits?.length ?? 0).toBeGreaterThan(0);

    const completed = events.filter(
      (e) => e.event.type === "session_complete" && e.event.success === true
    );
    const failures = events.filter(
      (e) => e.event.type === "session_complete" && e.event.success === false
    );
    expect(completed.length).toBe(1);
    expect(failures.length).toBeGreaterThanOrEqual(2);

    const status = execSync("git status --porcelain", { cwd: dir, encoding: "utf-8" });
    const dirtyLines = status
      .split("\n")
      .map((line) => line.trim())
      .filter(Boolean);
    expect(
      dirtyLines.every(
        (line) =>
          line.endsWith(".openagents/progress.md") ||
          line.endsWith(".openagents/tasks.jsonl") ||
          line.includes(".openagents/subtasks/") ||
          line.includes(".openagents/checkpoint.json")
      ),
    ).toBe(true);

    const log = execSync("git log --oneline -1", { cwd: dir, encoding: "utf-8" });
    expect(log).toContain(taskId);
  });
});
