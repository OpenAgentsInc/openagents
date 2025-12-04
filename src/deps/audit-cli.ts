#!/usr/bin/env bun
import { runDepAudit, writeReport } from "./audit.js";

const args = process.argv.slice(2);
let output: string | null = null;
let json = false;

for (let i = 0; i < args.length; i++) {
  const arg = args[i];
  if (arg === "--output" || arg === "-o") {
    output = args[i + 1] ?? null;
    i++;
  } else if (arg === "--json") {
    json = true;
  } else if (arg === "--help" || arg === "-h") {
    console.log(`Usage: bun src/deps/audit-cli.ts [--output <file>] [--json]

Runs:
- npm audit --json --production
- bunx npm-check-updates --jsonAll

Writes a combined report (default: .openagents/deps/audit-report.json).
`);
    process.exit(0);
  }
}

const main = () => {
  const report = runDepAudit();
  if (output === null) {
    output = ".openagents/deps/audit-report.json";
  }
  writeReport(report, output);

  if (json) {
    console.log(JSON.stringify(report, null, 2));
  } else {
    console.log(`Audit status: ${report.audit.status}`);
    console.log(
      `Vulnerabilities: info=${report.audit.vulnerabilities.info ?? 0}, low=${report.audit.vulnerabilities.low ?? 0}, moderate=${report.audit.vulnerabilities.moderate ?? 0}, high=${report.audit.vulnerabilities.high ?? 0}, critical=${report.audit.vulnerabilities.critical ?? 0}`,
    );
    console.log(`Upgradeable packages: ${report.upgrades.count}`);
    if (report.upgrades.packages.length) {
      console.log(
        report.upgrades.packages
          .slice(0, 10)
          .map((p) => `- ${p.name}: ${p.current} → ${p.latest}`)
          .join("\n"),
      );
      if (report.upgrades.packages.length > 10) {
        console.log(`...and ${report.upgrades.packages.length - 10} more`);
      }
    }
    console.log(`Report written to ${output}`);
  }

  const hasCriticalIssues =
    report.audit.status === "failed" && Object.values(report.audit.vulnerabilities ?? {}).some((v) => v > 0);
  process.exit(hasCriticalIssues ? 1 : 0);
};

main();
