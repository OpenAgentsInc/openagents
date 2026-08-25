/**
 * Comparison: trends over time, lanes against each other, and the deltas this
 * suite refuses to state.
 *
 * Rows are built from the same checked-in fixture Harbor jobs the rest of the
 * suite reads, with the clock injected. No model is called and no Docker image
 * runs.
 */

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, test } from "vite-plus/test";

import { compareRuns } from "./compare.ts";
import { summarizeRun } from "./effectiveness.ts";
import { readHarborJob } from "./harbor-job.ts";
import { CODER_RATE_CATALOG_VERSION } from "./pricing.ts";
import { renderComparison } from "./render-compare.ts";
import { buildResultRow, type BenchResultRow } from "./results-store.ts";
import { classifyRun, parseSuiteManifest } from "./suite-manifest.ts";

const fixture = (name: string): string =>
  fileURLToPath(new URL(`../fixtures/${name}`, import.meta.url));

/**
 * Every stored row is a full score run of some suite — the store refuses
 * anything else — so the rows these cases compare are built the same way, from
 * a manifest over exactly the tasks the fixture job ran.
 */
const row = (name: string, lane: string, recordedAt: string): BenchResultRow => {
  const report = summarizeRun(
    readHarborJob(fixture(name), {
      suite: "tb2-cross-section",
      lane,
      rateCatalogVersion: CODER_RATE_CATALOG_VERSION,
    }),
  );
  const tasks = report.perTrial.map((trial) => trial.task);
  const manifest = parseSuiteManifest(
    JSON.parse(
      readFileSync(
        fixture(tasks.length === 4 ? "fixture-suite.suite.json" : "fixture-suite-3.suite.json"),
        "utf8",
      ),
    ),
  );
  return buildResultRow(report, null, classifyRun(manifest, tasks), null, { recordedAt });
};

describe("trend on one lane", () => {
  test("reads a regression between two consecutive runs as cost rising", () => {
    const first = row("priced-lane", "proxy", "2026-08-25T10:00:00.000Z");
    const second = row("regressed-lane", "proxy", "2026-08-25T11:00:00.000Z");

    const trends = compareRuns([first, second]).trends;

    expect(trends).toHaveLength(1);
    const step = trends[0]!.steps[0]!;
    expect(step.costDelta.direction).toBe("worse");
    expect(step.costDelta.absolute).toBeGreaterThan(0);
    expect(step.successRateDelta.direction).toBe("worse");
  });

  test("orders by when a run was recorded, not by the order rows were passed", () => {
    const later = row("regressed-lane", "proxy", "2026-08-25T11:00:00.000Z");
    const earlier = row("priced-lane", "proxy", "2026-08-25T10:00:00.000Z");

    const trend = compareRuns([later, earlier]).trends[0]!;

    expect(trend.rows.map((entry) => entry.recordedAt)).toEqual([
      "2026-08-25T10:00:00.000Z",
      "2026-08-25T11:00:00.000Z",
    ]);
    expect(trend.steps[0]!.costDelta.direction).toBe("worse");
  });

  test("produces no trend from a single run", () => {
    expect(compareRuns([row("priced-lane", "proxy", "2026-08-25T10:00:00.000Z")]).trends).toEqual(
      [],
    );
  });
});

describe("lane against lane", () => {
  test("measures every other lane against the baseline", () => {
    const proxy = row("priced-lane", "proxy", "2026-08-25T10:00:00.000Z");
    const local = row("regressed-lane", "local", "2026-08-25T10:05:00.000Z");

    const comparison = compareRuns([proxy, local]).laneComparisons[0]!;

    expect(comparison.baselineLane).toBe("proxy");
    expect(comparison.lanes.find((entry) => entry.lane === "proxy")!.costDelta).toBeNull();
    // Success rate compares across any two lanes; it is measured the same way
    // on both sides whatever either one costs.
    expect(
      comparison.lanes.find((entry) => entry.lane === "local")!.successRateDelta?.direction,
    ).toBe("worse");
  });

  test("refuses a cost delta against the local lane, which bills no metered tokens", () => {
    // This is the lane comparison #34 actually asks for — house models through
    // the proxy against a local model — and its cost delta is permanently
    // unstatable in one direction. Saying so is the point: a blank cell here
    // reads as "the same", and a zero would read as "free".
    const proxy = row("priced-lane", "proxy", "2026-08-25T10:00:00.000Z");
    const local = row("regressed-lane", "local", "2026-08-25T10:05:00.000Z");

    const lane = compareRuns([proxy, local]).laneComparisons[0]!.lanes.find(
      (entry) => entry.lane === "local",
    )!;

    expect(lane.costDelta?.direction).toBe("unpriced");
    expect(lane.costDelta?.absolute).toBeNull();
    expect(lane.costDelta?.reason).toContain("cost_unknown");
  });

  test("honours an explicit baseline lane", () => {
    const proxy = row("priced-lane", "proxy", "2026-08-25T10:00:00.000Z");
    const local = row("regressed-lane", "local", "2026-08-25T10:05:00.000Z");

    const comparison = compareRuns([proxy, local], { baselineLane: "local" }).laneComparisons[0]!;

    expect(comparison.baselineLane).toBe("local");
    expect(comparison.lanes.find((entry) => entry.lane === "local")!.costDelta).toBeNull();
    expect(comparison.lanes.find((entry) => entry.lane === "proxy")!.costDelta?.direction).toBe(
      "unpriced",
    );
  });

  test("compares each lane at its most recent run", () => {
    const stale = row("regressed-lane", "local", "2026-08-25T09:00:00.000Z");
    const fresh = row("priced-lane", "local", "2026-08-25T12:00:00.000Z");
    const proxy = row("priced-lane", "proxy", "2026-08-25T10:00:00.000Z");

    const comparison = compareRuns([stale, fresh, proxy], { baselineLane: "proxy" })
      .laneComparisons[0]!;

    expect(comparison.lanes.find((entry) => entry.lane === "local")!.row.recordedAt).toBe(
      "2026-08-25T12:00:00.000Z",
    );
  });
});

describe("deltas this suite refuses to state", () => {
  test("refuses a cost delta when either side could not be priced", () => {
    const unpriced = row("unpriced-lane", "local", "2026-08-25T10:00:00.000Z");
    const mixed = row("mixed-lane", "proxy", "2026-08-25T10:05:00.000Z");

    const comparison = compareRuns([unpriced, mixed]).laneComparisons[0]!;
    const delta = comparison.lanes.find((entry) => entry.lane === "local")!.costDelta!;

    expect(delta.direction).toBe("unpriced");
    expect(delta.absolute).toBeNull();
    expect(delta.reason).toContain("unmeasured cost");
  });

  test("refuses a success-rate delta when no verifier ran on one side", () => {
    const graded = row("priced-lane", "proxy", "2026-08-25T10:00:00.000Z");
    const crashed = { ...row("crashed-verifier", "local", "2026-08-25T10:05:00.000Z") };
    const forced: BenchResultRow = { ...crashed, suiteKey: graded.suiteKey, successRate: null };

    const comparison = compareRuns([graded, forced]).laneComparisons[0]!;
    const delta = comparison.lanes.find((entry) => entry.lane === "local")!.successRateDelta!;

    expect(delta.direction).toBe("unknown");
    expect(delta.absolute).toBeNull();
  });
});

describe("what a comparison will not fold together", () => {
  test("keeps runs of different task lists in different groups", () => {
    const four = row("priced-lane", "proxy", "2026-08-25T10:00:00.000Z");
    const three = row("unpriced-lane", "local", "2026-08-25T10:05:00.000Z");

    const comparison = compareRuns([four, three]);

    expect(comparison.laneComparisons).toEqual([]);
    expect(comparison.isolatedGroups).toBe(2);
  });

  test("scopes to one suite when asked", () => {
    const kept = row("priced-lane", "proxy", "2026-08-25T10:00:00.000Z");
    const other: BenchResultRow = {
      ...row("regressed-lane", "local", "2026-08-25T10:05:00.000Z"),
      suite: "other",
    };

    const comparison = compareRuns([kept, other], { suite: "tb2-cross-section" });

    expect(comparison.laneComparisons).toEqual([]);
    expect(comparison.isolatedGroups).toBe(1);
  });

  test("flags a lane comparison whose CLI version also varies", () => {
    const proxy = row("priced-lane", "proxy", "2026-08-25T10:00:00.000Z");
    const local: BenchResultRow = {
      ...row("regressed-lane", "local", "2026-08-25T10:05:00.000Z"),
      agentVersions: ["9.9.9"],
    };

    const comparison = compareRuns([proxy, local]).laneComparisons[0]!;

    expect(comparison.confounders.some((note) => note.includes("CLI version also varies"))).toBe(
      true,
    );
  });

  test("flags a trend priced from placeholder rates", () => {
    const first = row("priced-lane", "proxy", "2026-08-25T10:00:00.000Z");
    const second = row("regressed-lane", "proxy", "2026-08-25T11:00:00.000Z");

    const step = compareRuns([first, second]).trends[0]!.steps[0]!;

    expect(step.confounders.some((note) => note.includes("operator placeholder"))).toBe(true);
  });
});

describe("rendering", () => {
  test("prints an unpriced lane as unpriced rather than as a blank cell", () => {
    const unpriced = row("unpriced-lane", "local", "2026-08-25T10:00:00.000Z");
    const mixed = row("mixed-lane", "proxy", "2026-08-25T10:05:00.000Z");

    const text = renderComparison(compareRuns([unpriced, mixed]));

    expect(text).toContain("unpriced");
    expect(text).not.toContain("$0.0000");
  });

  test("says why an empty store has nothing to compare", () => {
    const text = renderComparison(compareRuns([]));

    expect(text).toContain("Nothing to compare");
    expect(text).toContain("The store holds no rows");
  });

  test("prints the trend a regression shows up in", () => {
    const first = row("priced-lane", "proxy", "2026-08-25T10:00:00.000Z");
    const second = row("regressed-lane", "proxy", "2026-08-25T11:00:00.000Z");

    const text = renderComparison(compareRuns([first, second]));

    expect(text).toContain("Trend — tb2-cross-section on the proxy lane");
    expect(text).toContain("worse");
  });

  test("prints each row's own figures, not only the movement between them", () => {
    // #34's acceptance clause asks to see two comparable rows. A trend printed
    // as deltas alone says something moved without saying what it moved from.
    const first = row("priced-lane", "proxy", "2026-08-25T10:00:00.000Z");
    const second = row("regressed-lane", "proxy", "2026-08-25T11:00:00.000Z");

    const text = renderComparison(compareRuns([first, second]));

    expect(text).toContain("2026-08-25T10:00:00.000Z");
    expect(text).toContain("2026-08-25T11:00:00.000Z");
    expect(text).toContain(`job ${String(first.jobId)}`);
    expect(text).toContain(`job ${String(second.jobId)}`);
  });
});
