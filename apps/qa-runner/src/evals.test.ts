// Chill-evals tests — fakes-in-CI, deterministic, NO network / NO spend (#6183).
//
// Proves the full eval path with a fake chromium across >= 2 variants:
//   - a comparison over 2 variants is produced (pass-rate / p50 / p90 / deltas);
//   - the persisted eval.json round-trips and is public-safe;
//   - the comparison is HONEST: a broken variant fails (no fabricated green);
//   - the markdown / console renderers reflect deltas + the /pro link + the
//     `not_measured` sentinel.

import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { Effect } from "effect";

import { localBackend } from "./backend";
import { scriptedBrain } from "./brain";
import {
  EVAL_SCHEMA_VERSION,
  isMeasured,
  runEval,
  type EvalVariant,
  variantDeltas,
  variantMetrics,
} from "./evals";
import { renderEvalConsole, renderEvalMarkdown } from "./evals-report";
import { makeFakeChromium } from "./fake-chromium";
import { assertPublicSafeResult } from "./result";
import { loginRegressionSteps, loginRegressionStepsWrong } from "./scenarios";
import { makeTarget } from "./target";

let dir: string;
beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "qa-evals-test-"));
});
afterEach(() => {
  rmSync(dir, { recursive: true, force: true });
});

const target = () =>
  makeTarget({ name: "fake-target", baseUrl: "https://example.test" });

const passingChromium = () =>
  makeFakeChromium({
    pages: {
      "/login": {
        text: "Log in to OpenAgents",
        html: "<form>Log in to OpenAgents</form>",
      },
    },
  });

// Two variants over the SAME scenario surface: a "good" agent config (passes)
// and a "regressed" agent config (the deliberately-wrong assertion fails). This
// is the canonical before/after MCP comparison expressed via the brain seam.
const goodVariant = (): EvalVariant => ({
  id: "mcp-on",
  label: "MCP on",
  note: "scenario passes",
  brain: () => scriptedBrain(loginRegressionSteps()),
  backend: () => localBackend({ chromium: passingChromium() }),
});

const regressedVariant = (): EvalVariant => ({
  id: "mcp-off",
  label: "MCP off",
  note: "regressed: asserts a redirect that does not happen",
  brain: () => scriptedBrain(loginRegressionStepsWrong()),
  backend: () => localBackend({ chromium: passingChromium() }),
});

const fixedNow = () => new Date("2026-06-24T00:00:00.000Z");

describe("runEval (fake chromium, deterministic)", () => {
  test("produces a comparison over 2 variants with pass-rate + latency + deltas", async () => {
    const outcome = await Effect.runPromise(
      runEval({
        id: "login-mcp-compare",
        title: "Login under MCP on vs off",
        target: target(),
        scenario: { id: "login-regression", label: "/login renders sign-in" },
        variants: [goodVariant(), regressedVariant()],
        artifactDir: dir,
        now: fixedNow,
      }),
    );

    const r = outcome.result;
    expect(r.schemaVersion).toBe(EVAL_SCHEMA_VERSION);
    expect(r.variants.length).toBe(2);
    expect(r.baselineVariantId).toBe("mcp-on");

    const good = r.variants.find((v) => v.variantId === "mcp-on")!;
    const bad = r.variants.find((v) => v.variantId === "mcp-off")!;
    // HONEST: good passes, regressed fails (no fabricated green).
    expect(good.passRate).toBe(1);
    expect(bad.passRate).toBe(0);
    expect(bad.runs[0]!.failure).toBeDefined();

    // latency percentiles are measured numbers (a real run took real time).
    expect(isMeasured(good.latencyP50Ms)).toBe(true);
    expect(isMeasured(good.latencyP90Ms)).toBe(true);

    // deltas relative to the baseline: the regressed variant has a negative
    // pass-rate delta.
    const badDelta = r.deltas.find((d) => d.variantId === "mcp-off")!;
    expect(badDelta.passRateDelta).toBe(-1);

    // each variant run carries a dereferenceable artifact dir + video.
    expect(good.runs[0]!.artifactDir).toBe("mcp-on.0");
    expect(good.runs[0]!.video).toBeDefined();

    // CI/fixture path is illustrative, never decision-grade.
    expect(r.decisionGrade).toBe(false);
  });

  test("persists eval.json that is public-safe and round-trips", async () => {
    const outcome = await Effect.runPromise(
      runEval({
        id: "login-mcp-compare",
        title: "Login under MCP on vs off",
        target: target(),
        scenario: { id: "login-regression", label: "/login renders sign-in" },
        variants: [goodVariant(), regressedVariant()],
        artifactDir: dir,
        now: fixedNow,
      }),
    );
    expect(existsSync(outcome.resultPath)).toBe(true);
    const parsed = JSON.parse(readFileSync(outcome.resultPath, "utf8"));
    expect(parsed.schemaVersion).toBe(EVAL_SCHEMA_VERSION);
    expect(parsed.variants.length).toBe(2);
    // tripwire passes on the persisted JSON.
    expect(() => assertPublicSafeResult(parsed)).not.toThrow();
  });

  test("supports repetitions and computes pass-rate over total runs", async () => {
    const outcome = await Effect.runPromise(
      runEval({
        id: "login-reps",
        title: "Login reps",
        target: target(),
        scenario: { id: "login-regression", label: "/login renders sign-in" },
        variants: [goodVariant(), regressedVariant()],
        repetitions: 3,
        artifactDir: dir,
        now: fixedNow,
      }),
    );
    const good = outcome.result.variants.find((v) => v.variantId === "mcp-on")!;
    expect(good.runCount).toBe(3);
    expect(good.passCount).toBe(3);
    expect(good.passRate).toBe(1);
    expect(outcome.result.repetitions).toBe(3);
  });

  test("rejects an eval with fewer than 2 variants (it must compare)", async () => {
    const exit = await Effect.runPromiseExit(
      runEval({
        id: "single",
        title: "single",
        target: target(),
        scenario: { id: "login-regression", label: "/login" },
        variants: [goodVariant()],
        artifactDir: dir,
        now: fixedNow,
      }),
    );
    expect(exit._tag).toBe("Failure");
  });
});

describe("pure comparison math", () => {
  test("variantMetrics computes pass-rate + percentiles; not_measured on empty", () => {
    const v = goodVariant();
    const empty = variantMetrics(v, []);
    expect(empty.passRate).toBe(0);
    expect(empty.runCount).toBe(0);
    expect(isMeasured(empty.latencyP50Ms)).toBe(false); // not_measured, not 0

    const withRuns = variantMetrics(v, [
      { status: "pass", durationMs: 100, artifactDir: "a.0" },
      { status: "fail", durationMs: 300, artifactDir: "a.1", failure: "x" },
    ]);
    expect(withRuns.passRate).toBe(0.5);
    expect(withRuns.passCount).toBe(1);
    expect(isMeasured(withRuns.latencyP50Ms)).toBe(true);
  });

  test("variantDeltas are relative to the first (baseline) variant", () => {
    const a = variantMetrics(goodVariant(), [
      { status: "pass", durationMs: 100, artifactDir: "a.0" },
    ]);
    const b = variantMetrics(regressedVariant(), [
      { status: "fail", durationMs: 200, artifactDir: "b.0", failure: "x" },
    ]);
    const deltas = variantDeltas([a, b]);
    expect(deltas[0]!.passRateDelta).toBe(0); // baseline vs itself
    expect(deltas[1]!.passRateDelta).toBe(-1);
    expect(deltas[1]!.latencyP50DeltaMs).toBe(100); // 200 - 100, slower
  });
});

describe("renderers", () => {
  const sampleResult = async () => {
    const outcome = await Effect.runPromise(
      runEval({
        id: "login-mcp-compare",
        title: "Login under MCP on vs off",
        target: target(),
        scenario: { id: "login-regression", label: "/login renders sign-in" },
        variants: [goodVariant(), regressedVariant()],
        artifactDir: dir,
        now: fixedNow,
      }),
    );
    return outcome.result;
  };

  test("console report shows both variants + baseline marker + illustrative label", async () => {
    const out = renderEvalConsole(await sampleResult());
    expect(out).toContain("MCP on");
    expect(out).toContain("MCP off");
    expect(out).toContain("ILLUSTRATIVE");
    expect(out).toContain("baseline");
  });

  test("markdown PR body has a table, the /pro link, and an honest headline", async () => {
    const md = renderEvalMarkdown(await sampleResult(), {
      proBaseUrl: "https://openagents.com",
    });
    expect(md).toContain("| variant | pass-rate | p50 | p90 | Δpass | Δp50 |");
    expect(md).toContain("https://openagents.com/pro/evals/login-mcp-compare");
    // one variant failed -> honest warning headline, never fake green.
    expect(md).toContain("some variants failed");
    expect(md).toContain("Illustrative");
  });

  test("markdown embeds gh-attach video markdown when supplied", async () => {
    const md = renderEvalMarkdown(await sampleResult(), {
      proBaseUrl: "https://openagents.com",
      variantVideoMarkdown: {
        "mcp-on": "https://github.com/u/r/assets/1/video.mp4",
      },
    });
    expect(md).toContain("https://github.com/u/r/assets/1/video.mp4");
  });
});
