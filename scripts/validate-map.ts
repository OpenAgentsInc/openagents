#!/usr/bin/env bun
/**
 * Quick MAP Orchestrator Validation Script
 *
 * Runs unit tests + basic integration checks in <30 seconds.
 * Use this before running full integration tests.
 *
 * Usage: bun scripts/validate-map.ts
 */

import { spawn } from "bun";
import { existsSync } from "node:fs";

const startTime = Date.now();

console.log("=".repeat(60));
console.log("MAP Orchestrator Validation");
console.log("=".repeat(60));

// Step 1: Unit tests
console.log("\n[1/3] Running unit tests...");
const testProc = spawn({
  cmd: ["bun", "test", "src/hillclimber/map-orchestrator.test.ts"],
  stdout: "pipe",
  stderr: "pipe",
});

const testExit = await testProc.exited;
const testOutput = await new Response(testProc.stdout).text();
const testStderr = await new Response(testProc.stderr).text();

if (testExit !== 0) {
  console.log("❌ Unit tests FAILED");
  console.log(testOutput);
  console.log(testStderr);
  process.exit(1);
}

// Parse test results from both stdout and stderr
const fullOutput = testOutput + testStderr;
const passMatch = fullOutput.match(/(\d+) pass/);
const failMatch = fullOutput.match(/(\d+) fail/);
const passCount = passMatch ? parseInt(passMatch[1]) : 0;
const failCount = failMatch ? parseInt(failMatch[1]) : 0;

console.log(`✅ Unit tests passed: ${passCount}/${passCount + failCount}`);

// Step 2: Module import check
console.log("\n[2/3] Verifying module imports...");
try {
  const { formatFMPrompt, runMAPOrchestrator } = await import("../src/hillclimber/map-orchestrator.js");
  const { parseToolCalls } = await import("../src/bench/model-adapter.js");
  const { decomposeTask } = await import("../src/hillclimber/decomposer.js");

  if (!formatFMPrompt || !runMAPOrchestrator || !parseToolCalls || !decomposeTask) {
    throw new Error("Missing exports");
  }
  console.log("✅ All critical modules import successfully");
} catch (error) {
  console.log("❌ Module import FAILED:", error);
  process.exit(1);
}

// Step 3: Context preservation check (the main bug we fixed)
console.log("\n[3/3] Verifying context preservation fix...");
try {
  const { formatFMPrompt } = await import("../src/hillclimber/map-orchestrator.js");

  // Test that fileContents appears in prompt
  const contextWithFiles = {
    taskDescription: "Test",
    currentSubtask: {
      id: 1,
      name: "test",
      goal: "Test",
      checkpoint: "Test",
      expectedArtifacts: [],
      dependsOn: [],
      hints: [],
      maxTurns: 5,
    },
    previousActions: [],
    hints: [],
    globalHints: [],
    fileContents: {
      "/app/regex.txt": "(?=.*\\d{1,3}).*test",
    },
  };

  const prompt = formatFMPrompt(contextWithFiles);

  if (!prompt.includes("## Current File Contents")) {
    throw new Error("fileContents section not in prompt");
  }
  if (!prompt.includes("/app/regex.txt")) {
    throw new Error("file path not in prompt");
  }
  if (!prompt.includes("(?=.*\\d{1,3}).*test")) {
    throw new Error("file content not in prompt");
  }

  console.log("✅ Context preservation fix verified");
} catch (error) {
  console.log("❌ Context preservation check FAILED:", error);
  process.exit(1);
}

// Summary
const duration = ((Date.now() - startTime) / 1000).toFixed(1);
console.log("\n" + "=".repeat(60));
console.log(`✅ ALL VALIDATIONS PASSED in ${duration}s`);
console.log("=".repeat(60));
console.log("\nReady for integration testing:");
console.log("  bun scripts/test-progress-fix.ts         # Quick 3-turn test");
console.log("  bun scripts/test-progress-fix.ts --standard  # Full 10-turn test");
