/**
 * End-to-end integration test for regex-log task.
 * 
 * Tests that HillClimber can solve the regex-log task using MAP orchestrator
 * with real FM integration.
 * 
 * Success criteria:
 * - Task passes (1/1 test, extracting 9/9 dates) in < 15 turns
 * - FM receives specific failure feedback
 * - System iterates toward solution
 */

import { describe, test, expect, beforeAll, afterAll } from "bun:test";
import { mkdtemp, rm } from "fs/promises";
import { join } from "path";
import { tmpdir } from "os";
import { cp } from "fs/promises";
import { existsSync } from "fs";
import { Effect } from "effect";
import { BunContext } from "@effect/platform-bun";
import { runTaskWithMAP } from "./map-orchestrator.js";
import { loadTerminalBenchSuite, type TerminalBenchTask } from "../bench/terminal-bench.js";
import type { HillClimberConfig } from "./types.js";

const SUITE_PATH = "tasks/terminal-bench-2.json";
const TB2_ROOT = process.env.TB2_ROOT || "tasks/terminal-bench-2";

describe("E2E: regex-log task", () => {
  let workspace: string;
  let regexLogTask: TerminalBenchTask | null = null;

  beforeAll(async () => {
    // Create temporary workspace
    workspace = await mkdtemp(join(tmpdir(), "hillclimber-e2e-"));
    console.log(`[E2E] Workspace: ${workspace}`);

    // Load task from suite (using Effect)
    const suiteResult = await Effect.runPromise(
      loadTerminalBenchSuite(SUITE_PATH).pipe(Effect.provide(BunContext.layer))
    );
    if (!suiteResult || !suiteResult.tasks) {
      throw new Error(`Failed to load suite from ${SUITE_PATH}`);
    }
    regexLogTask = suiteResult.tasks.find((t) => t.id === "regex-log") || null;

    if (!regexLogTask) {
      throw new Error("regex-log task not found in suite");
    }

    // Copy TB2 environment files to workspace (but NOT Dockerfile)
    // Tests will be run in Docker, so we don't need to copy them here
    const sourcePath = regexLogTask.source_path || join(TB2_ROOT, "regex-log");
    if (existsSync(sourcePath)) {
      const { readdirSync, statSync, cpSync } = await import("fs");

      // Copy environment files to workspace root (these become /app/ contents in Docker)
      const envDir = join(sourcePath, "environment");
      if (existsSync(envDir)) {
        const entries = readdirSync(envDir);
        for (const entry of entries) {
          // Skip Dockerfile - it's used for image building, not workspace content
          if (entry === "Dockerfile") continue;

          const srcPath = join(envDir, entry);
          const destPath = join(workspace, entry);
          if (statSync(srcPath).isDirectory()) {
            cpSync(srcPath, destPath, { recursive: true });
          } else {
            cpSync(srcPath, destPath);
          }
        }
      }

      // Tests stay in source_path - Docker runner will copy them
      // DO NOT copy or modify test files - that would be gaming the benchmark

      console.log(`[E2E] Copied TB2 environment files from ${sourcePath} to ${workspace}`);
    } else {
      console.warn(`[E2E] TB2 source path not found: ${sourcePath}`);
      console.warn(`[E2E] Tests may fail - workspace not properly set up`);
    }
  });

  afterAll(async () => {
    // Cleanup
    if (workspace && existsSync(workspace)) {
      await rm(workspace, { recursive: true, force: true });
    }
  });

  test("regex-log solves in < 15 turns", async () => {
    if (!regexLogTask) {
      throw new Error("regex-log task not loaded");
    }

    const config: HillClimberConfig = {
      id: 0,
      taskId: "regex-log",
      hint: null,
      useSkills: false,
      maxTurnsOverride: 15,
      configHash: "",
      isCurrent: true,
      createdAt: new Date().toISOString(),
    };

    const output: string[] = [];
    const onOutput = (text: string) => {
      output.push(text);
      console.log(`[E2E] ${text}`);
    };

    const result = await runTaskWithMAP(
      regexLogTask,
      config,
      workspace,
      120, // timeout
      15, // maxTurns
      onOutput,
    );

    console.log(`[E2E] Result: passed=${result.passed}, turns=${result.turns}, progress=${result.progress}`);

    // Assertions
    expect(result.passed).toBe(true);
    expect(result.turns).toBeLessThan(15);
    expect(result.progress).toBeGreaterThanOrEqual(1.0);

    // Verify output contains verification feedback
    const outputText = output.join("\n");
    expect(outputText).toContain("Verification");
    expect(outputText).toContain("tests passing");
  }, 180000); // 3 minute timeout

  test("FM receives specific failure feedback", async () => {
    if (!regexLogTask) {
      throw new Error("regex-log task not loaded");
    }

    const config: HillClimberConfig = {
      id: 0,
      taskId: "regex-log",
      hint: null,
      useSkills: false,
      maxTurnsOverride: 5,
      configHash: "",
      isCurrent: true,
      createdAt: new Date().toISOString(),
    };

    const output: string[] = [];
    const onOutput = (text: string) => {
      output.push(text);
    };

    // Run for a few turns to see feedback
    await runTaskWithMAP(
      regexLogTask,
      config,
      workspace,
      60,
      5,
      onOutput,
    );

    const outputText = output.join("\n");
    
    // Should see verification feedback with specific failures
    expect(outputText).toMatch(/Verification|FAILED|test_/i);
  }, 90000);
});

