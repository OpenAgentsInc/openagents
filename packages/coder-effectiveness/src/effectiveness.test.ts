/**
 * Grading and aggregation over fixture Harbor jobs.
 *
 * Every case reads a checked-in job directory under `fixtures/`. No model is
 * called, no Docker image runs, and no clock is read, so the suite produces the
 * same numbers in CI that it produces on a laptop.
 */

import { fileURLToPath } from "node:url";
import { describe, expect, test } from "vite-plus/test";

import { summarizeRun } from "./effectiveness.ts";
import { readHarborJob } from "./harbor-job.ts";
import { CODER_RATE_CATALOG_VERSION } from "./pricing.ts";

const fixture = (name: string): string =>
  fileURLToPath(new URL(`../fixtures/${name}`, import.meta.url));

const report = (name: string, lane = "proxy") =>
  summarizeRun(
    readHarborJob(fixture(name), {
      suite: "tb2-cross-section",
      lane,
      rateCatalogVersion: CODER_RATE_CATALOG_VERSION,
    }),
  );

describe("grading a Harbor job", () => {
  test("counts an accepted outcome only where a verifier returned a positive reward", () => {
    const result = report("priced-lane");

    expect(result.trialsTotal).toBe(4);
    expect(result.accepted).toBe(2);
    expect(result.rejected).toBe(2);
    expect(result.ungraded).toBe(0);
    expect(result.successRate).toBe(0.5);
  });

  test("keeps a trial whose verifier never ran out of both buckets", () => {
    const result = report("crashed-verifier");

    expect(result.trialsTotal).toBe(3);
    expect(result.accepted).toBe(1);
    expect(result.rejected).toBe(0);
    expect(result.ungraded).toBe(2);
    // One accepted over one graded trial. The two ungraded trials do not
    // deflate this into 33%, and they do not silently vanish either.
    expect(result.successRate).toBe(1);
    expect(result.ungradedRatio).toBeCloseTo(2 / 3, 10);
  });

  test("reads the task name, model, cli version, and thread from the trial", () => {
    const result = report("priced-lane");

    expect(result.perTrial.map((trial) => trial.task).toSorted()).toEqual([
      "build-cmake",
      "fix-git",
      "parse-log",
      "port-forward",
    ]);
    expect(result.models).toEqual(["gemini-3.7-flash"]);
    expect(result.agentVersions).toEqual(["0.4.0"]);
  });

  test("sums cached reads from the ATIF steps, which carry no total", () => {
    // The coder's exporter writes cache_read_input_tokens per step and never
    // totals it, so the reader has to add the steps up itself.
    const result = report("priced-lane");

    expect(result.cachedInputTokens).toBe(8000 + 6000 + 20000 + 4000 + 2000);
    expect(result.promptTokens).toBe(20000 + 30000 + 15000 + 10000);
    expect(result.completionTokens).toBe(600 + 1200 + 800 + 500);
  });

  test("counts tool calls and wall clock", () => {
    const result = report("priced-lane");

    expect(result.toolCalls).toBe(1 + 2 + 1 + 1);
    expect(result.wallClockSeconds).toBe(120 + 210 + 90 + 120);
  });

  test("refuses a directory that holds no Harbor result", () => {
    expect(() => report("does-not-exist")).toThrow(/not a Harbor job directory/u);
  });

  test("pins a digest that changes when the run changes", () => {
    expect(report("priced-lane").runDigest).toBe(report("priced-lane").runDigest);
    expect(report("priced-lane").runDigest).not.toBe(report("regressed-lane").runDigest);
    // The lane is part of what makes two rows comparable.
    expect(report("priced-lane").runDigest).not.toBe(report("priced-lane", "local").runDigest);
  });
});

/** The gemini-3.7-flash rates, spelled out rather than read from the catalog. */
const perMtok = (uncached: number, cached: number, output: number): number =>
  (uncached / 1e6) * 1.25 + (cached / 1e6) * 0.1 + (output / 1e6) * 10;

describe("cost per accepted outcome", () => {
  test("divides the whole run's cost by the accepted outcomes, failures included", () => {
    const result = report("priced-lane");

    const expectedTotal =
      perMtok(20000 - 14000, 14000, 600) +
      perMtok(30000 - 20000, 20000, 1200) +
      perMtok(15000 - 4000, 4000, 800) +
      perMtok(10000 - 2000, 2000, 500);

    expect(result.cost.totalUsd).toBeCloseTo(expectedTotal, 10);
    expect(result.cost.coverage).toBe("known");
    // Two accepted outcomes carry the cost of all four trials.
    expect(result.costPerAcceptedOutcome.usd).toBeCloseTo(expectedTotal / 2, 10);
    expect(result.costPerAcceptedOutcome.disposition).toBe("known");
  });

  test("labels a number derived from placeholder rates as provisional", () => {
    const result = report("priced-lane");

    expect(result.costPerAcceptedOutcome.rateBasis).toBe("operator_placeholder");
    expect(result.costPerAcceptedOutcome.reason).toContain("placeholder");
  });

  // The headline constraint.
  test("reports unknown, not zero, for a run entirely on the unpriced luna lane", () => {
    const result = report("unpriced-lane");

    expect(result.accepted).toBe(2);
    expect(result.cost.coverage).toBe("unknown");
    expect(result.cost.totalUsd).toBeNull();
    expect(result.costPerAcceptedOutcome.usd).toBeNull();
    expect(result.costPerAcceptedOutcome.usd).not.toBe(0);
    expect(result.costPerAcceptedOutcome.disposition).toBe("cost_unknown");
    expect(result.costPerAcceptedOutcome.reason).toContain("gpt-5.6-luna");
  });

  test("still reports the volume it does know on an unpriced lane", () => {
    // Refusing a cost is not refusing to measure. Tokens, tool calls, and wall
    // clock are counted the same way on every lane.
    const result = report("unpriced-lane");

    expect(result.promptTokens).toBe(9000 + 14000 + 7000);
    expect(result.completionTokens).toBe(350 + 700 + 300);
    expect(result.successRate).toBeCloseTo(2 / 3, 10);
  });

  test("withholds a number when only some trials could be priced", () => {
    // Two priced trials and one unpriced. Summing the two and dividing by all
    // three accepted outcomes would produce a real-looking number that is too
    // low by whatever the third trial cost.
    const result = report("mixed-lane");

    expect(result.accepted).toBe(3);
    expect(result.cost.coverage).toBe("partial");
    expect(result.cost.pricedTrials).toBe(2);
    expect(result.cost.unpricedTrials).toBe(1);
    expect(result.costPerAcceptedOutcome.usd).toBeNull();
    expect(result.costPerAcceptedOutcome.disposition).toBe("cost_partial");
  });

  test("names why each unpriced trial went unpriced", () => {
    const result = report("mixed-lane");

    expect(result.cost.unpricedReasons.join(" ")).toContain("gpt-5.6-luna");
    expect(result.perTrial.find((trial) => trial.task === "parse-log")?.disposition).toBe(
      "unpriced_model",
    );
  });

  test("reports an undefined cost per outcome when nothing was accepted", () => {
    const zeroAccepted = summarizeRun({
      jobId: "job-none",
      suite: "tb2-cross-section",
      lane: "proxy",
      runDigest: "effectiveness:test",
      trials: [
        {
          task: "fix-git",
          outcome: "rejected",
          modelId: "gemini-3.7-flash",
          agentVersion: "0.4.0",
          promptTokens: 10_000,
          completionTokens: 500,
          cachedInputTokens: 0,
          toolCalls: 1,
          wallClockSeconds: 60,
          threadId: null,
          exception: null,
        },
      ],
    });

    // The run cost real money and bought nothing, so the cost of an accepted
    // outcome is undefined — not zero, and not infinity.
    expect(zeroAccepted.cost.totalUsd).toBeGreaterThan(0);
    expect(zeroAccepted.costPerAcceptedOutcome.usd).toBeNull();
    expect(zeroAccepted.costPerAcceptedOutcome.disposition).toBe("no_accepted_outcomes");
    expect(zeroAccepted.successRate).toBe(0);
  });

  test("a regression raises the cost per accepted outcome", () => {
    // Same suite, same model, one accepted outcome instead of two, and more
    // tokens spent getting there. This is the number the gate watches.
    const before = report("priced-lane");
    const after = report("regressed-lane");

    expect(before.costPerAcceptedOutcome.usd).not.toBeNull();
    expect(after.costPerAcceptedOutcome.usd).not.toBeNull();
    expect(after.costPerAcceptedOutcome.usd!).toBeGreaterThan(
      before.costPerAcceptedOutcome.usd! * 3,
    );
    expect(after.successRate!).toBeLessThan(before.successRate!);
  });
});
