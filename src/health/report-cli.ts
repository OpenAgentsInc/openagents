#!/usr/bin/env bun
/**
 * Generate BUILD-HEALTH.md report
 *
 * Usage: bun src/health/report-cli.ts [--root <dir>] [--output <path>]
 */

import { runHealthChecks } from "./health.js";
import { generateBuildHealthReport } from "./report.js";
import { resolve } from "path";

const args = process.argv.slice(2);
let rootDir = ".";
let outputPath = resolve("docs/BUILD-HEALTH.md");

for (let i = 0; i < args.length; i++) {
  const arg = args[i];
  if (arg === "--root" || arg === "-r") {
    rootDir = args[i + 1] ?? ".";
    i++;
  } else if (arg === "--output" || arg === "-o") {
    outputPath = args[i + 1] ?? outputPath;
    i++;
  } else if (arg === "--help" || arg === "-h") {
    console.log(`Usage: bun src/health/report-cli.ts [--root <dir>] [--output <path>]

Runs health checks and generates a BUILD-HEALTH.md report.

Options:
  --root, -r <dir>      Project root directory (default: ".")
  --output, -o <path>   Output file path (default: "docs/BUILD-HEALTH.md")
  --help, -h            Show this help message
`);
    process.exit(0);
  }
}

const main = async () => {
  try {
    console.log("Running health checks...");
    const report = await runHealthChecks(rootDir);

    console.log("\nGenerating BUILD-HEALTH.md report...");
    generateBuildHealthReport(report, outputPath);

    console.log("\nHealth check summary:");
    for (const result of report.results) {
      const status = result.exitCode === 0 ? "✅ OK" : `❌ FAIL (${result.exitCode})`;
      console.log(`  [${result.kind}] ${status} - ${result.command}`);
    }

    console.log(`\nOverall: ${report.ok ? "✅ healthy" : "❌ unhealthy"}`);
    process.exit(report.ok ? 0 : 1);
  } catch (err) {
    console.error(`Health report generation failed: ${err}`);
    process.exit(1);
  }
};

main();
