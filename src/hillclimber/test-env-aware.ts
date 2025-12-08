#!/usr/bin/env bun
/**
 * Quick test for environment-aware test generation.
 * Tests introspection and generation with a mock environment.
 */

import { Effect } from "effect";
import { introspectLocalEnvironment, localCommandExecutor } from "./environment-introspector.js";
import { generateTestsFromEnvironment, getAllTestsFromEnvironmentResult } from "./test-generator.js";
import { inferProhibitedTools, emptyEnvironmentInfo } from "./environment-info.js";
import type { EnvironmentInfo } from "./environment-info.js";

const RSTAN_TO_PYSTAN_DESCRIPTION = `
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

The Python solution must produce posterior samples with similar statistical properties
to the R/RStan solution.
`;

async function testProhibitedToolsInference() {
  console.log("=== Testing Prohibited Tools Inference ===\n");

  const prohibited = inferProhibitedTools(RSTAN_TO_PYSTAN_DESCRIPTION);

  console.log("Inferred prohibited tools:");
  for (const tool of prohibited) {
    console.log(`  - ${tool.name}: ${tool.reason}`);
  }
  console.log();
}

async function testLocalIntrospection() {
  console.log("=== Testing Local Introspection ===\n");

  try {
    const env = await Effect.runPromise(
      introspectLocalEnvironment("/Users/christopherdavid/code/openagents", RSTAN_TO_PYSTAN_DESCRIPTION)
    );

    console.log("Platform:", env.platform.type);
    console.log("Languages detected:");
    if (env.languages.python) console.log(`  - Python ${env.languages.python.version}`);
    if (env.languages.node) console.log(`  - Node ${env.languages.node.version}`);
    if (env.languages.r) console.log(`  - R ${env.languages.r.version}`);

    console.log("\nProhibited tools (should NOT be present):");
    for (const tool of env.tools.prohibited) {
      console.log(`  - ${tool.name}: found=${tool.found} (should be false)`);
    }

    console.log("\nFiles in workspace:", env.files.listing.length);
    console.log("File previews:", env.files.taskFiles.length);

    return env;
  } catch (e) {
    console.error("Introspection failed:", e);
    return null;
  }
}

async function testEnvAwareGeneration() {
  console.log("\n=== Testing Environment-Aware Test Generation ===\n");

  // Create a mock environment simulating the rstan-to-pystan task
  const mockEnv: EnvironmentInfo = {
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
        { name: "rstan", reason: "R→Python conversion: R tools should not be used", found: false },
      ],
      prohibitedCheck: { R: false, Rscript: false, rstan: false },
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
library(tidyverse)

# Load data
data <- read.csv("data.csv")

# Define model parameters
alpha <- 1.0  # intercept prior mean
sigma <- 0.1  # noise prior
rho <- 0.5    # autocorrelation
beta <- 0.0   # coefficient prior mean

# Compile and fit model
fit <- stan("model.stan", data=list(N=nrow(data), y=data$y, x=data$x))

# Extract posteriors
samples <- extract(fit)
print(summary(samples$alpha))
print(summary(samples$sigma))
print(summary(samples$rho))
print(summary(samples$beta))`,
          detectedType: "r_script",
          structure: {
            variables: ["alpha", "sigma", "rho", "beta", "data", "fit", "samples"],
            functions: [],
            imports: ["rstan", "tidyverse"],
            parameters: ["alpha", "sigma", "rho", "beta"],
          },
        },
        {
          path: "/app/model.stan",
          extension: "stan",
          lineCount: 30,
          preview: `data {
  int<lower=0> N;
  vector[N] y;
  vector[N] x;
}
parameters {
  real alpha;
  real<lower=0> sigma;
  real<lower=-1,upper=1> rho;
  real beta;
}
model {
  alpha ~ normal(1.0, 0.5);
  sigma ~ exponential(10);
  rho ~ uniform(-1, 1);
  beta ~ normal(0, 1);
  y ~ normal(alpha + beta * x, sigma);
}`,
          detectedType: "stan_model",
          structure: {
            parameters: ["alpha", "sigma", "rho", "beta"],
          },
        },
      ],
    },
    resources: { memoryLimitMB: 4096, cpuCount: 4 },
    env: {},
  };

  try {
    console.log("Generating environment-aware tests for rstan-to-pystan...\n");

    const result = await generateTestsFromEnvironment(
      RSTAN_TO_PYSTAN_DESCRIPTION,
      "rstan-to-pystan",
      mockEnv,
      { model: "local", verbose: true }
    );

    console.log("\n=== Results ===\n");
    console.log(`Total tests generated: ${getAllTestsFromEnvironmentResult(result).length}`);
    console.log(`Duration: ${result.durationMs}ms`);
    console.log(`Model: ${result.model}`);

    console.log("\nDescription requirements:");
    for (const req of result.descriptionRequirements.slice(0, 5)) {
      console.log(`  - ${req}`);
    }

    console.log("\nEnvironment requirements:");
    for (const req of result.environmentRequirements.slice(0, 5)) {
      console.log(`  - ${req}`);
    }

    console.log("\n=== ANTI-CHEAT TESTS (CRITICAL) ===");
    for (const test of result.antiCheatTests) {
      console.log(`\n  ${test.id}:`);
      console.log(`    Input: ${test.input}`);
      console.log(`    Expected: ${test.expectedOutput}`);
      console.log(`    Reasoning: ${test.reasoning}`);
      console.log(`    Confidence: ${test.confidence}`);
    }

    console.log("\n=== EXISTENCE TESTS ===");
    for (const test of result.existenceTests.slice(0, 3)) {
      console.log(`\n  ${test.id}: ${test.reasoning}`);
    }

    console.log("\n=== CORRECTNESS TESTS ===");
    for (const test of result.correctnessTests.slice(0, 3)) {
      console.log(`\n  ${test.id}: ${test.reasoning}`);
    }

    console.log("\n=== BOUNDARY TESTS ===");
    for (const test of result.boundaryTests.slice(0, 3)) {
      console.log(`\n  ${test.id}: ${test.reasoning}`);
    }

    if (result.uncertainties.length > 0) {
      console.log("\nUncertainties:");
      for (const u of result.uncertainties.slice(0, 3)) {
        console.log(`  - ${u}`);
      }
    }

    // Check if anti-cheat tests were generated
    const hasAntiCheat = result.antiCheatTests.length > 0;
    console.log("\n=== VALIDATION ===");
    console.log(`Anti-cheat tests generated: ${hasAntiCheat ? "✅ YES" : "❌ NO"}`);

    const hasRProhibition = result.antiCheatTests.some(t =>
      t.input.includes("R") || t.reasoning.toLowerCase().includes("r ")
    );
    console.log(`R prohibition test: ${hasRProhibition ? "✅ YES" : "❌ NO"}`);

    // Check parameter coverage
    const allTests = getAllTestsFromEnvironmentResult(result);
    const hasAlpha = allTests.some(t => t.input.includes("alpha") || t.reasoning.includes("alpha"));
    const hasSigma = allTests.some(t => t.input.includes("sigma") || t.reasoning.includes("sigma"));
    const hasRho = allTests.some(t => t.input.includes("rho") || t.reasoning.includes("rho"));
    const hasBeta = allTests.some(t => t.input.includes("beta") || t.reasoning.includes("beta"));

    console.log(`Parameter coverage:`);
    console.log(`  alpha: ${hasAlpha ? "✅" : "❌"}`);
    console.log(`  sigma: ${hasSigma ? "✅" : "❌"}`);
    console.log(`  rho: ${hasRho ? "✅" : "❌"}`);
    console.log(`  beta: ${hasBeta ? "✅" : "❌"}`);

  } catch (e) {
    console.error("Generation failed:", e);
  }
}

async function main() {
  await testProhibitedToolsInference();
  await testLocalIntrospection();
  await testEnvAwareGeneration();
}

main().catch(console.error);
