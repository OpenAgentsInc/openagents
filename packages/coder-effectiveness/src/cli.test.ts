/**
 * The CLI, run the way a scheduled job runs it.
 *
 * These cases spawn the entry point rather than importing it, so they prove
 * the command is invocable and that its exit codes mean what the runbook says
 * they mean. A gate that could not be measured must not exit 0.
 */

import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { describe, expect, test } from "vite-plus/test";

const cliPath = fileURLToPath(new URL("./cli.ts", import.meta.url));
const fixture = (name: string): string =>
  fileURLToPath(new URL(`../fixtures/${name}`, import.meta.url));
/**
 * Floors sized for the fixture jobs, which hold three or four trials rather
 * than the twelve a real cross-section run does. The shipped suite floors
 * would stop at the trial count before reaching the cost criterion these
 * cases are about; `thresholds.test.ts` covers the shipped file itself.
 */
const fixtureFloors = fileURLToPath(
  new URL("../fixtures/floors-fixture-scale.json", import.meta.url),
);
const placeholderOkFloors = fileURLToPath(
  new URL("../fixtures/floors-fixture-scale-placeholder-ok.json", import.meta.url),
);

const run = (args: ReadonlyArray<string>): { status: number; stdout: string; stderr: string } => {
  const result = spawnSync(process.execPath, ["--import", "tsx", cliPath, ...args], {
    encoding: "utf8",
  });
  return { status: result.status ?? -1, stdout: result.stdout, stderr: result.stderr };
};

describe("coder-effectiveness report", () => {
  test("prints the report and exits 0 without a thresholds file", () => {
    const result = run([fixture("priced-lane"), "--suite", "tb2-cross-section"]);

    expect(result.status).toBe(0);
    expect(result.stdout).toContain("Cost per accepted outcome");
    expect(result.stdout).toContain("gemini-3.7-flash");
  });

  test("skips the `--` pnpm forwards", () => {
    const result = run(["--", fixture("priced-lane")]);

    expect(result.status).toBe(0);
  });

  test("prints unknown, not $0.0000, for the unpriced luna lane", () => {
    const result = run([fixture("unpriced-lane")]);

    expect(result.stdout).toContain("unknown");
    expect(result.stdout).toContain("cost_unknown");
    expect(result.stdout).toContain("gpt-5.6-luna");
    // The line under the cost heading must never read as a free run.
    expect(result.stdout).not.toMatch(/\$0\.0000 per accepted outcome/u);
  });

  test("exits 0 once the floors opt into the provisional rates", () => {
    const result = run([fixture("priced-lane"), "--thresholds", placeholderOkFloors]);

    expect(result.status).toBe(0);
    expect(result.stdout).toContain("PASSED");
  });

  test("exits 2 on the same priced run when the floors do not opt in", () => {
    // Same run, same ceiling, one field different. The number exists; scoring
    // a dollar floor against provisional rates is what needs consent.
    const result = run([fixture("priced-lane"), "--thresholds", fixtureFloors]);

    expect(result.status).toBe(2);
    expect(result.stdout).toContain("placeholder");
  });

  test("exits 1 when a measured floor is breached", () => {
    // The regressed lane accepts 1 of 4 graded trials against a 0.4 floor.
    const result = run([fixture("regressed-lane"), "--thresholds", fixtureFloors]);

    expect(result.status).toBe(1);
    expect(result.stdout).toContain("FAILED");
    expect(result.stdout).toContain("success_rate");
  });

  test("exits 2, not 0, when the gate cannot be verified", () => {
    // The floors declare a dollar ceiling; the luna lane carries no rate. A CI
    // step that only checks for a zero exit must not read this as green.
    const result = run([fixture("unpriced-lane"), "--thresholds", fixtureFloors]);

    expect(result.status).toBe(2);
    expect(result.status).not.toBe(0);
    expect(result.stdout).toContain("UNVERIFIABLE");
    expect(result.stdout).toContain("is not a pass");
  });

  test("emits a machine-readable report under --json", () => {
    const result = run([fixture("mixed-lane"), "--json"]);
    const parsed = JSON.parse(result.stdout) as {
      report: { costPerAcceptedOutcome: { usd: number | null; disposition: string } };
      gate: null;
    };

    expect(result.status).toBe(0);
    expect(parsed.gate).toBeNull();
    expect(parsed.report.costPerAcceptedOutcome.usd).toBeNull();
    expect(parsed.report.costPerAcceptedOutcome.disposition).toBe("cost_partial");
  });

  test("prices from a served catalog when given one", () => {
    const models = fileURLToPath(new URL("../fixtures/served-models.json", import.meta.url));
    const result = run([fixture("priced-lane"), "--models", models, "--json"]);
    const parsed = JSON.parse(result.stdout) as {
      report: { costPerAcceptedOutcome: { usd: number | null } };
    };

    expect(result.status).toBe(0);
    expect(parsed.report.costPerAcceptedOutcome.usd).not.toBeNull();
  });

  test("refuses a directory that is not a Harbor job", () => {
    const result = run([fixture("nope")]);

    expect(result.status).toBe(1);
    expect(result.stderr).toContain("not a Harbor job directory");
  });

  test("rejects an unknown option instead of ignoring it", () => {
    const result = run([fixture("priced-lane"), "--lane", "moon"]);

    expect(result.status).toBe(1);
    expect(result.stderr).toContain("--lane must be proxy or local");
  });

  test("prints usage on --help", () => {
    const result = run(["--help"]);

    expect(result.status).toBe(0);
    expect(result.stdout).toContain("Exit codes");
  });
});
