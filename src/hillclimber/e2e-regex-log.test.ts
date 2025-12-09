/**
 * End-to-end integration test for regex-log task.
 * 
 * Tests that HillClimber can solve the regex-log task using MAP orchestrator
 * with real FM integration.
 * 
 * Success criteria:
 * - Task passes (9/9 tests) in < 15 turns
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

    // Copy TB2 task files to workspace using source_path from task
    const sourcePath = regexLogTask.source_path || join(TB2_ROOT, "regex-log");
    if (existsSync(sourcePath)) {
      const { readdirSync, statSync, cpSync, readFileSync, writeFileSync, mkdirSync } = await import("fs");
      
      // Copy environment files to workspace root
      const envDir = join(sourcePath, "environment");
      if (existsSync(envDir)) {
        const entries = readdirSync(envDir);
        for (const entry of entries) {
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
      
      // Copy test files, replacing /app/ paths with workspace path
      const testsDir = join(sourcePath, "tests");
      const destTestsDir = join(workspace, "tests");
      if (existsSync(testsDir)) {
        mkdirSync(destTestsDir, { recursive: true });
        const testFiles = readdirSync(testsDir);
        for (const file of testFiles) {
          const srcFile = join(testsDir, file);
          const destFile = join(destTestsDir, file);
          if (statSync(srcFile).isFile()) {
            let content = readFileSync(srcFile, "utf-8");
            // Replace /app/ with workspace path for local execution
            content = content.replace(/\/app\//g, `${workspace}/`);
            content = content.replace(/\/app(?=["'])/g, workspace);
            writeFileSync(destFile, content);
          }
        }
      }
      
      console.log(`[E2E] Copied TB2 files from ${sourcePath} to ${workspace}`);
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
      maxTurns: 15,
      timeout: 120,
      enableReflection: false,
      enableMemory: false,
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
      maxTurns: 5, // Just enough to see feedback
      timeout: 60,
      enableReflection: false,
      enableMemory: false,
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

