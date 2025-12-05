/**
 * Generate BUILD-HEALTH.md report from health check results
 */

import { writeFileSync } from "fs";
import { resolve } from "path";
import type { HealthReport } from "./health.js";

export function generateBuildHealthReport(report: HealthReport, outputPath?: string): string {
  const timestamp = new Date().toISOString();

  // Parse test results
  let testPass = 0;
  let testFail = 0;
  let testExpectCalls = 0;
  let testFiles = 0;
  let testDuration = "unknown";

  const testResults = report.results.filter((r) => r.kind === "test");
  for (const result of testResults) {
    const output = result.stdout + result.stderr;
    const passMatch = output.match(/(\d+) pass/);
    const failMatch = output.match(/(\d+) fail/);
    const expectMatch = output.match(/(\d+) expect\(\) calls/);
    const filesMatch = output.match(/Ran \d+ tests across (\d+) files/);
    const durationMatch = output.match(/\[([^\]]+)\]/);

    if (passMatch) testPass += parseInt(passMatch[1]);
    if (failMatch) testFail += parseInt(failMatch[1]);
    if (expectMatch) testExpectCalls = parseInt(expectMatch[1]);
    if (filesMatch) testFiles = parseInt(filesMatch[1]);
    if (durationMatch) testDuration = durationMatch[1];
  }

  const testStatus = testFail === 0 ? "✅ PASS" : "❌ FAIL";

  // Parse typecheck results
  const typecheckResults = report.results.filter((r) => r.kind === "typecheck");
  const typecheckSuccess = typecheckResults.every((r) => r.exitCode === 0);
  const typecheckStatus = typecheckSuccess ? "✅ PASS" : "⚠️ WARN";

  let typecheckSection = "";
  if (typecheckSuccess) {
    typecheckSection = "✅ **All type checks passing**";
  } else {
    const errors = typecheckResults
      .filter((r) => r.exitCode !== 0)
      .map((r) => r.stderr || r.stdout)
      .join("\n\n")
      .slice(0, 500);
    typecheckSection = `⚠️ **Issues Found**\n\n\`\`\`\n${errors || "Unknown error"}\n\`\`\``;
  }

  // Parse e2e results
  const e2eResults = report.results.filter((r) => r.kind === "e2e");
  let e2eSection = "";
  if (e2eResults.length > 0) {
    const e2eSuccess = e2eResults.every((r) => r.exitCode === 0);
    const e2eStatus = e2eSuccess ? "✅ PASS" : "❌ FAIL";
    e2eSection = `| **E2E Tests** | ${e2eStatus} | ${e2eResults.length} command(s) run |\n`;
  }

  const markdown = `# Build Health Status

> **Last Updated:** ${timestamp}
> **Auto-generated** by \`bun run health:report\`

## Summary

| Metric | Status | Details |
|--------|--------|---------|
| **Tests** | ${testStatus} | ${testPass} pass, ${testFail} fail |
| **TypeCheck** | ${typecheckStatus} | ${typecheckSuccess ? "All types valid" : "See issues below"} |
${e2eSection}| **Coverage** | 📊 TRACKED | Run tests with --coverage to see details |

## Test Results

\`\`\`
${testPass} pass
${testFail} fail
${testExpectCalls} expect() calls
Ran ${testPass + testFail} tests across ${testFiles} files. [${testDuration}]
\`\`\`

### Test Status: ${testStatus === "✅ PASS" ? "✅ **ALL TESTS PASSING**" : "❌ **TESTS FAILING**"}

## TypeCheck Status

${typecheckSection}

${e2eResults.length > 0 ? `## E2E Test Results\n\n${e2eResults.map((r) => `- \`${r.command}\`: ${r.exitCode === 0 ? "✅ PASS" : "❌ FAIL"}`).join("\n")}\n` : ""}
## Coverage Highlights

To see detailed coverage, run:

\`\`\`bash
bun test --coverage
\`\`\`

### Critical Modules

Coverage tracking for:
- \`src/agent/orchestrator/\` - Agent orchestration logic
- \`src/tools/\` - Tool implementations (bash, edit, read, write, etc.)
- \`src/hud/\` - HUD server and protocol
- \`src/tasks/\` - Task management system

## Trends

*Historical data will be tracked once CI automation is in place.*

## How to Update

Run the health report generator:

\`\`\`bash
bun run health:report
\`\`\`

This will:
1. Run tests, typechecks, and e2e tests (as configured in .openagents/project.json)
2. Update this file with latest results
3. Exit with non-zero code if any checks fail

## CI Integration

Add to your CI workflow:

\`\`\`yaml
- name: Update Build Health
  run: bun run health:report

- name: Commit Health Report
  if: always()
  run: |
    git config user.name "CI Bot"
    git config user.email "ci@openagents.com"
    git add docs/BUILD-HEALTH.md
    git commit -m "chore: update build health report [skip ci]" || true
    git push
\`\`\`
`;

  if (outputPath) {
    const fullPath = resolve(outputPath);
    writeFileSync(fullPath, markdown, "utf8");
    console.log(`✅ Build health report written to: ${fullPath}`);
  }

  return markdown;
}
