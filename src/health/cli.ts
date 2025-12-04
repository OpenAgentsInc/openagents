#!/usr/bin/env bun
import { runHealthChecks } from "./health.js";

const args = process.argv.slice(2);
let rootDir = ".";
let json = false;

for (let i = 0; i < args.length; i++) {
  const arg = args[i];
  if (arg === "--root" || arg === "-r") {
    rootDir = args[i + 1] ?? ".";
    i++;
  } else if (arg === "--json") {
    json = true;
  } else if (arg === "--help" || arg === "-h") {
    console.log(`Usage: bun src/health/cli.ts [--root <dir>] [--json]

Runs typecheck, test, and e2e commands from .openagents/project.json and reports status.
Exit code is non-zero if any command fails.
`);
    process.exit(0);
  }
}

const main = async () => {
  try {
    const report = await runHealthChecks(rootDir);
    if (json) {
      console.log(JSON.stringify(report, null, 2));
    } else {
      for (const result of report.results) {
        const status = result.exitCode === 0 ? "OK" : `FAIL (${result.exitCode})`;
        console.log(`[${result.kind}] ${status} - ${result.command}`);
      }
      console.log(`Overall: ${report.ok ? "healthy" : "unhealthy"}`);
    }
    process.exit(report.ok ? 0 : 1);
  } catch (err) {
    console.error(`Health check failed: ${err}`);
    process.exit(1);
  }
};

main();
