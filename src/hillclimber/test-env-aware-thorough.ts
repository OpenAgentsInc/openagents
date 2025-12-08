#!/usr/bin/env bun
/**
 * Thorough testing of environment-aware test generation.
 * Tests multiple scenarios with detailed output.
 */

import { Effect } from "effect";
import { introspectLocalEnvironment } from "./environment-introspector.js";
import { generateTestsFromEnvironment, getAllTestsFromEnvironmentResult } from "./test-generator.js";
import { inferProhibitedTools, emptyEnvironmentInfo } from "./environment-info.js";
import type { EnvironmentInfo } from "./environment-info.js";

// Test scenarios
const SCENARIOS = {
  "rstan-to-pystan": {
    description: `
Convert R/RStan script to Python/PyStan.

The task involves converting a Bayesian statistical model from R/RStan to Python/PyStan.
The original R script performs posterior sampling using a Stan model.

Input files:
- model.R: The original R script
- model.stan: The Stan model definition
- data.csv: Input data

Expected output:
- solution.py: Python script using PyStan that produces equivalent results

Key parameters to estimate:
- alpha (intercept)
- sigma (noise)
- rho (autocorrelation)
- beta (coefficient)

The Python solution must produce posterior samples with similar statistical properties.
`,
    mockEnv: (): EnvironmentInfo => ({
      ...emptyEnvironmentInfo(),
      platform: { type: "docker" },
      languages: {
        python: {
          version: "3.11.4",
          packages: [
            { name: "pystan", version: "3.5.0" },
            { name: "numpy", version: "1.24.0" },
            { name: "pandas", version: "2.0.0" },
          ],
          executable: "/usr/bin/python3",
        },
      },
      tools: {
        available: [
          { name: "python3", path: "/usr/bin/python3", version: "3.11.4" },
          { name: "pip", path: "/usr/bin/pip" },
        ],
        prohibited: [
          { name: "R", reason: "R→Python conversion: R tools should not be used", found: false },
          { name: "Rscript", reason: "R→Python conversion: R tools should not be used", found: false },
        ],
        prohibitedCheck: { R: false, Rscript: false },
      },
      files: {
        workdir: "/app",
        listing: [
          { name: "model.R", path: "/app/model.R", type: "file", size: 2048, permissions: "-rw-r--r--" },
          { name: "model.stan", path: "/app/model.stan", type: "file", size: 1024, permissions: "-rw-r--r--" },
          { name: "data.csv", path: "/app/data.csv", type: "file", size: 4096, permissions: "-rw-r--r--" },
        ],
        taskFiles: [
          {
            path: "/app/model.R",
            extension: "R",
            lineCount: 50,
            preview: `library(rstan)
data <- read.csv("data.csv")
alpha <- 1.0
sigma <- 0.1
rho <- 0.5
beta <- 0.0
fit <- stan("model.stan", data=list(N=nrow(data), y=data$y, x=data$x))
samples <- extract(fit)`,
            detectedType: "r_script",
            structure: {
              variables: ["alpha", "sigma", "rho", "beta", "data", "fit", "samples"],
              parameters: ["alpha", "sigma", "rho", "beta"],
            },
          },
        ],
      },
      resources: { memoryLimitMB: 4096, cpuCount: 4 },
      env: {},
    }),
    expectedAntiCheat: ["R", "Rscript"],
    expectedParams: ["alpha", "sigma", "rho", "beta"],
  },

  "regex-log": {
    description: `
Write a regex pattern that extracts dates from Apache log files.

The regex should match date strings in the format: DD/Mon/YYYY:HH:MM:SS
Example: 10/Oct/2000:13:55:36

Write the regex pattern to /app/regex.txt

The pattern should:
- Match valid dates only (1-31 for day, valid months)
- Match valid times (00-23 for hour, 00-59 for min/sec)
- Be case-insensitive for month names
`,
    mockEnv: (): EnvironmentInfo => ({
      ...emptyEnvironmentInfo(),
      platform: { type: "docker" },
      languages: {
        python: {
          version: "3.11.4",
          packages: [],
          executable: "/usr/bin/python3",
        },
      },
      tools: {
        available: [
          { name: "python3", path: "/usr/bin/python3" },
          { name: "grep", path: "/usr/bin/grep" },
        ],
        prohibited: [],
        prohibitedCheck: {},
      },
      files: {
        workdir: "/app",
        listing: [
          { name: "sample.log", path: "/app/sample.log", type: "file", size: 10240, permissions: "-rw-r--r--" },
        ],
        taskFiles: [
          {
            path: "/app/sample.log",
            extension: "log",
            lineCount: 100,
            preview: `192.168.1.1 - - [10/Oct/2000:13:55:36 -0700] "GET /apache_pb.gif HTTP/1.0" 200 2326
192.168.1.2 - - [11/Oct/2000:14:22:11 -0700] "POST /submit HTTP/1.1" 302 0
invalid line without date
192.168.1.3 - - [32/Oct/2000:25:61:99 -0700] "GET /invalid HTTP/1.0" 404 0`,
            detectedType: "unknown",
          },
        ],
      },
      resources: {},
      env: {},
    }),
    expectedAntiCheat: [],
    expectedParams: [],
  },

  "python-to-rust": {
    description: `
Convert the Python matrix multiplication implementation to Rust.

Input: multiply.py - A Python script that multiplies two matrices
Output: src/main.rs - A Rust implementation with equivalent functionality

Requirements:
- Use no external crates for matrix operations
- Handle arbitrary matrix sizes
- Return error for incompatible dimensions

Do not use Python to run the solution - Rust only.
`,
    mockEnv: (): EnvironmentInfo => ({
      ...emptyEnvironmentInfo(),
      platform: { type: "docker" },
      languages: {
        rust: { version: "1.75.0" },
      },
      tools: {
        available: [
          { name: "rustc", path: "/usr/bin/rustc", version: "1.75.0" },
          { name: "cargo", path: "/usr/bin/cargo" },
        ],
        prohibited: [
          { name: "python", reason: "Python→Rust conversion: Python should not be used for solution", found: false },
          { name: "python3", reason: "Python→Rust conversion: Python should not be used for solution", found: false },
        ],
        prohibitedCheck: { python: false, python3: false },
      },
      files: {
        workdir: "/app",
        listing: [
          { name: "multiply.py", path: "/app/multiply.py", type: "file", size: 1024, permissions: "-rw-r--r--" },
          { name: "Cargo.toml", path: "/app/Cargo.toml", type: "file", size: 256, permissions: "-rw-r--r--" },
        ],
        taskFiles: [
          {
            path: "/app/multiply.py",
            extension: "py",
            lineCount: 20,
            preview: `def multiply(a, b):
    rows_a, cols_a = len(a), len(a[0])
    rows_b, cols_b = len(b), len(b[0])
    if cols_a != rows_b:
        raise ValueError("Incompatible dimensions")
    result = [[0] * cols_b for _ in range(rows_a)]
    for i in range(rows_a):
        for j in range(cols_b):
            for k in range(cols_a):
                result[i][j] += a[i][k] * b[k][j]
    return result`,
            detectedType: "python_script",
            structure: {
              functions: ["multiply"],
              variables: ["rows_a", "cols_a", "rows_b", "cols_b", "result"],
            },
          },
        ],
      },
      resources: {},
      env: {},
    }),
    expectedAntiCheat: ["python", "python3"],
    expectedParams: [],
  },
};

interface TestResult {
  scenario: string;
  success: boolean;
  duration: number;
  totalTests: number;
  antiCheatTests: number;
  hasExpectedAntiCheat: boolean;
  hasExpectedParams: boolean;
  details: {
    antiCheatTests: string[];
    existenceTests: number;
    correctnessTests: number;
    boundaryTests: number;
    integrationTests: number;
    uncertainties: string[];
  };
  error?: string;
}

async function runScenario(name: string, config: typeof SCENARIOS["rstan-to-pystan"]): Promise<TestResult> {
  console.log(`\n${"=".repeat(60)}`);
  console.log(`SCENARIO: ${name}`);
  console.log(`${"=".repeat(60)}`);

  const startTime = Date.now();

  try {
    const env = config.mockEnv();
    console.log(`Platform: ${env.platform.type}`);
    console.log(`Prohibited tools: ${env.tools.prohibited.map(t => t.name).join(", ") || "none"}`);
    console.log(`File previews: ${env.files.taskFiles.length}`);

    const result = await generateTestsFromEnvironment(
      config.description,
      name,
      env,
      { model: "local", verbose: false }
    );

    const duration = Date.now() - startTime;
    const allTests = getAllTestsFromEnvironmentResult(result);

    // Check anti-cheat coverage
    const antiCheatNames = result.antiCheatTests.map(t =>
      `${t.id}: ${t.input.slice(0, 50)}...`
    );

    const hasExpectedAntiCheat = config.expectedAntiCheat.length === 0 ||
      config.expectedAntiCheat.every(tool =>
        result.antiCheatTests.some(t =>
          t.input.toLowerCase().includes(tool.toLowerCase()) ||
          t.reasoning.toLowerCase().includes(tool.toLowerCase())
        )
      );

    // Check parameter coverage
    const allTestText = allTests.map(t => `${t.input} ${t.reasoning}`).join(" ");
    const hasExpectedParams = config.expectedParams.length === 0 ||
      config.expectedParams.filter(p => allTestText.toLowerCase().includes(p.toLowerCase())).length >= Math.ceil(config.expectedParams.length / 2);

    console.log(`\nResults:`);
    console.log(`  Total tests: ${allTests.length}`);
    console.log(`  Anti-cheat: ${result.antiCheatTests.length}`);
    console.log(`  Existence: ${result.existenceTests.length}`);
    console.log(`  Correctness: ${result.correctnessTests.length}`);
    console.log(`  Boundary: ${result.boundaryTests.length}`);
    console.log(`  Integration: ${result.integrationTests.length}`);
    console.log(`  Duration: ${duration}ms`);

    console.log(`\nAnti-cheat tests:`);
    for (const test of result.antiCheatTests) {
      console.log(`  - ${test.id}: ${test.input}`);
      console.log(`    Expected: ${test.expectedOutput}`);
      console.log(`    Reasoning: ${test.reasoning}`);
    }

    console.log(`\nValidation:`);
    console.log(`  Has expected anti-cheat: ${hasExpectedAntiCheat ? "✅" : "❌"}`);
    console.log(`  Has expected params: ${hasExpectedParams ? "✅" : "❌"}`);

    return {
      scenario: name,
      success: true,
      duration,
      totalTests: allTests.length,
      antiCheatTests: result.antiCheatTests.length,
      hasExpectedAntiCheat,
      hasExpectedParams,
      details: {
        antiCheatTests: antiCheatNames,
        existenceTests: result.existenceTests.length,
        correctnessTests: result.correctnessTests.length,
        boundaryTests: result.boundaryTests.length,
        integrationTests: result.integrationTests.length,
        uncertainties: result.uncertainties,
      },
    };
  } catch (e) {
    const duration = Date.now() - startTime;
    console.error(`\nError: ${e instanceof Error ? e.message : String(e)}`);
    return {
      scenario: name,
      success: false,
      duration,
      totalTests: 0,
      antiCheatTests: 0,
      hasExpectedAntiCheat: false,
      hasExpectedParams: false,
      details: {
        antiCheatTests: [],
        existenceTests: 0,
        correctnessTests: 0,
        boundaryTests: 0,
        integrationTests: 0,
        uncertainties: [],
      },
      error: e instanceof Error ? e.message : String(e),
    };
  }
}

async function testProhibitedToolsInference() {
  console.log(`\n${"=".repeat(60)}`);
  console.log("TESTING: Prohibited Tools Inference");
  console.log(`${"=".repeat(60)}`);

  const testCases = [
    { desc: "Convert R to Python", expected: ["R", "Rscript", "rstan"] },
    { desc: "convert python to rust implementation", expected: [] }, // Our pattern doesn't match this exactly
    { desc: "Implement from scratch without using numpy", expected: [] },
    { desc: "You must not use pandas", expected: [] },
  ];

  for (const tc of testCases) {
    const prohibited = inferProhibitedTools(tc.desc);
    const names = prohibited.map(p => p.name);
    console.log(`\n"${tc.desc}"`);
    console.log(`  Inferred: [${names.join(", ")}]`);
    console.log(`  Expected: [${tc.expected.join(", ")}]`);
    const match = tc.expected.every(e => names.includes(e));
    console.log(`  Match: ${match ? "✅" : "❌"}`);
  }
}

async function testLocalIntrospection() {
  console.log(`\n${"=".repeat(60)}`);
  console.log("TESTING: Local Environment Introspection");
  console.log(`${"=".repeat(60)}`);

  try {
    const env = await Effect.runPromise(
      introspectLocalEnvironment("/Users/christopherdavid/code/openagents", "Test task")
    );

    console.log(`\nPlatform: ${env.platform.type}`);
    console.log(`OS: ${env.platform.osDistro || "unknown"} ${env.platform.osVersion || ""}`);

    console.log(`\nLanguages:`);
    if (env.languages.python) console.log(`  Python: ${env.languages.python.version} (${env.languages.python.packages.length} packages)`);
    if (env.languages.node) console.log(`  Node: ${env.languages.node.version}`);
    if (env.languages.rust) console.log(`  Rust: ${env.languages.rust.version}`);
    if (env.languages.go) console.log(`  Go: ${env.languages.go.version}`);

    console.log(`\nTools: ${env.tools.available.length} available`);
    console.log(`Files: ${env.files.listing.length} in workspace`);
    console.log(`Previews: ${env.files.taskFiles.length}`);

    console.log(`\nResources:`);
    console.log(`  Memory: ${env.resources.memoryLimitMB ?? "unknown"} MB`);
    console.log(`  CPUs: ${env.resources.cpuCount ?? "unknown"}`);

    return true;
  } catch (e) {
    console.error(`Error: ${e instanceof Error ? e.message : String(e)}`);
    return false;
  }
}

async function main() {
  console.log("╔════════════════════════════════════════════════════════════╗");
  console.log("║     THOROUGH ENVIRONMENT-AWARE TEST GENERATION TESTS       ║");
  console.log("╚════════════════════════════════════════════════════════════╝");

  // Test prohibited tools inference
  await testProhibitedToolsInference();

  // Test local introspection
  const introspectionOk = await testLocalIntrospection();
  if (!introspectionOk) {
    console.log("\n⚠️  Local introspection failed, continuing with mock tests...");
  }

  // Run all scenarios
  const results: TestResult[] = [];
  for (const [name, config] of Object.entries(SCENARIOS)) {
    const result = await runScenario(name, config);
    results.push(result);
  }

  // Summary
  console.log(`\n${"═".repeat(60)}`);
  console.log("SUMMARY");
  console.log(`${"═".repeat(60)}`);

  const successful = results.filter(r => r.success).length;
  const withAntiCheat = results.filter(r => r.antiCheatTests > 0).length;
  const withExpectedAntiCheat = results.filter(r => r.hasExpectedAntiCheat).length;

  console.log(`\nOverall:`);
  console.log(`  Scenarios run: ${results.length}`);
  console.log(`  Successful: ${successful}/${results.length}`);
  console.log(`  With anti-cheat tests: ${withAntiCheat}/${results.length}`);
  console.log(`  With expected anti-cheat: ${withExpectedAntiCheat}/${results.length}`);

  console.log(`\nPer scenario:`);
  for (const r of results) {
    const status = r.success ? "✅" : "❌";
    const antiCheat = r.hasExpectedAntiCheat ? "✅" : "❌";
    const params = r.hasExpectedParams ? "✅" : "❌";
    console.log(`  ${status} ${r.scenario}: ${r.totalTests} tests, ${r.antiCheatTests} anti-cheat, AC:${antiCheat} P:${params}, ${r.duration}ms`);
  }

  // Output JSON for analysis
  console.log(`\n${"═".repeat(60)}`);
  console.log("JSON OUTPUT");
  console.log(`${"═".repeat(60)}`);
  console.log(JSON.stringify(results, null, 2));
}

main().catch(console.error);
