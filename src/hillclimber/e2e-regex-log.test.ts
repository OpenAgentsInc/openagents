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

    // Load task from suite
    const suiteResult = await loadTerminalBenchSuite(SUITE_PATH);
    if (!suiteResult || !suiteResult.tasks) {
      throw new Error(`Failed to load suite from ${SUITE_PATH}`);
    }
    regexLogTask = suiteResult.tasks.find((t) => t.id === "regex-log") || null;

    if (!regexLogTask) {
      throw new Error("regex-log task not found in suite");
    }

    // Copy TB2 task files to workspace
    const taskDir = join(TB2_ROOT, "regex-log");
    if (existsSync(taskDir)) {
      await cp(taskDir, join(workspace, "regex-log"), { recursive: true });
      // Move contents to workspace root (TB2 structure)
      // This is a simplified setup - real TB2 has more structure
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

