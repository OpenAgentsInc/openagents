/**
 * Comparison: what two rows in the results store may honestly be said about
 * each other, and what they may not.
 *
 * #34 asks for two things off the same store. **Trend** is the same suite on
 * the same lane over time — the shape "two consecutive scheduled runs producing
 * comparable rows" needs, and the shape a regression shows up in. **Lane
 * comparison** is the same suite across lanes at one moment: the same tasks
 * through the proxy and through a local model, which is the comparative view
 * the issue says falls out for free.
 *
 * The rules that keep both honest:
 *
 * 1. **Only rows sharing a `suiteKey` are compared.** Different task lists are
 *    different measurements. Rows in other groups are reported as their own
 *    groups, never folded into one table with a footnote.
 * 2. **A cost delta needs both sides priced.** If either row's cost per
 *    accepted outcome is `null`, the delta is `unpriced`, not zero and not
 *    "improved". This is the same refusal the report makes about a single run,
 *    applied to the difference between two.
 * 3. **A comparison that varies two things says so.** Lane comparison assumes
 *    the CLI is held still while the lane changes; a group whose rows carry
 *    different agent versions is flagged `confounded`. Trend assumes the lane
 *    is held still while the CLI changes, so a trend that also changes model is
 *    flagged the same way. Neither is suppressed — a confounded comparison is
 *    often the only one available, and it is readable as long as it is labelled.
 * 4. **Deltas are directional facts, not verdicts.** Cost per accepted outcome
 *    going up is `worse` and success rate going up is `better`, and this module
 *    says which without deciding whether the run passes. That is the gate's
 *    job, and it already has a third verdict for the cases nothing measured.
 */

import { surfacePinOf } from "./results-store.ts";
import type { BenchResultRow } from "./results-store.ts";

export type DeltaDirection = "better" | "worse" | "unchanged" | "unpriced" | "unknown";

export interface Delta {
  readonly from: number | null;
  readonly to: number | null;
  readonly absolute: number | null;
  /** Change as a fraction of `from`. `null` when `from` is 0 or unknown. */
  readonly relative: number | null;
  readonly direction: DeltaDirection;
  readonly reason: string;
}

export interface LaneRow {
  readonly lane: string;
  readonly row: BenchResultRow;
  /** `null` on the baseline lane itself. */
  readonly costDelta: Delta | null;
  readonly successRateDelta: Delta | null;
}

export interface LaneComparison {
  readonly suiteKey: string;
  readonly suite: string;
  readonly tasks: ReadonlyArray<string>;
  readonly baselineLane: string;
  readonly lanes: ReadonlyArray<LaneRow>;
  /** Set when something other than the lane also varies across these rows. */
  readonly confounders: ReadonlyArray<string>;
}

export interface TrendStep {
  readonly from: BenchResultRow;
  readonly to: BenchResultRow;
  readonly costDelta: Delta;
  readonly successRateDelta: Delta;
  readonly confounders: ReadonlyArray<string>;
}

export interface LaneTrend {
  readonly suiteKey: string;
  readonly suite: string;
  readonly lane: string;
  readonly rows: ReadonlyArray<BenchResultRow>;
  readonly steps: ReadonlyArray<TrendStep>;
}

export interface Comparison {
  readonly laneComparisons: ReadonlyArray<LaneComparison>;
  readonly trends: ReadonlyArray<LaneTrend>;
  /** Suite keys the store holds that no other row is comparable to. */
  readonly isolatedGroups: number;
}

/**
 * Compare a store's rows.
 *
 * `lower is better` is passed per metric rather than inferred, because the two
 * metrics here disagree: cost per accepted outcome improves downward and
 * success rate improves upward, and a single "delta" helper that guessed would
 * eventually guess wrong on a third metric.
 */
export const compareRuns = (
  rows: ReadonlyArray<BenchResultRow>,
  options: { readonly suite?: string; readonly baselineLane?: string } = {},
): Comparison => {
  const scoped =
    options.suite === undefined ? rows : rows.filter((row) => row.suite === options.suite);
  const groups = groupBy(scoped, (row) => row.suiteKey);

  const laneComparisons: Array<LaneComparison> = [];
  const trends: Array<LaneTrend> = [];
  let isolatedGroups = 0;

  for (const group of groups) {
    const latestByLane = latestPerLane(group);
    if (latestByLane.length >= 2) {
      laneComparisons.push(laneComparisonOf(latestByLane, options.baselineLane));
    }

    let trendsFromGroup = 0;
    for (const laneRows of groupBy(group, (row) => row.lane)) {
      if (laneRows.length < 2) continue;
      trends.push(trendOf(laneRows));
      trendsFromGroup += 1;
    }

    // Isolated means this group produced nothing — no lane comparison AND no
    // trend. A group with several runs on a single lane is not isolated: it is
    // the trend printed above, and counting it here made the report tell a
    // reader that the thing they had just read had been left out.
    if (latestByLane.length < 2 && trendsFromGroup === 0) {
      isolatedGroups += 1;
    }
  }

  return { laneComparisons, trends, isolatedGroups };
};

/**
 * One row per lane: the most recently recorded.
 *
 * A lane with several rows in the store is a lane that has been run more than
 * once, and the lane comparison wants where it stands now. Its history is not
 * discarded — that is what the trend reads.
 */
const latestPerLane = (rows: ReadonlyArray<BenchResultRow>): ReadonlyArray<BenchResultRow> =>
  groupBy(rows, (row) => row.lane)
    .map((laneRows) =>
      laneRows.reduce((latest, row) => (row.recordedAt >= latest.recordedAt ? row : latest)),
    )
    .toSorted((left, right) => left.lane.localeCompare(right.lane));

const laneComparisonOf = (
  rows: ReadonlyArray<BenchResultRow>,
  requestedBaseline: string | undefined,
): LaneComparison => {
  const baseline =
    rows.find((row) => row.lane === requestedBaseline) ??
    rows.find((row) => row.lane === "proxy") ??
    rows[0]!;

  return {
    suiteKey: baseline.suiteKey,
    suite: baseline.suite,
    tasks: baseline.tasks,
    baselineLane: baseline.lane,
    lanes: rows.map((row) => ({
      lane: row.lane,
      row,
      costDelta: row === baseline ? null : costDelta(baseline, row),
      successRateDelta: row === baseline ? null : successRateDelta(baseline, row),
    })),
    confounders: confoundersOf(rows, "lane"),
  };
};

const trendOf = (laneRows: ReadonlyArray<BenchResultRow>): LaneTrend => {
  const ordered = laneRows.toSorted((left, right) =>
    left.recordedAt.localeCompare(right.recordedAt),
  );
  const steps: Array<TrendStep> = [];
  for (let index = 1; index < ordered.length; index += 1) {
    const from = ordered[index - 1]!;
    const to = ordered[index]!;
    steps.push({
      from,
      to,
      costDelta: costDelta(from, to),
      successRateDelta: successRateDelta(from, to),
      confounders: confoundersOf([from, to], "recordedAt"),
    });
  }
  return {
    suiteKey: ordered[0]!.suiteKey,
    suite: ordered[0]!.suite,
    lane: ordered[0]!.lane,
    rows: ordered,
    steps,
  };
};

/**
 * Cost per accepted outcome, where up is worse.
 *
 * An unpriced side makes the delta `unpriced` and carries the disposition that
 * explains why, so a reader sees "the local lane bills no metered tokens"
 * rather than an empty cell.
 */
const costDelta = (from: BenchResultRow, to: BenchResultRow): Delta => {
  if (from.costPerAcceptedOutcomeUsd === null || to.costPerAcceptedOutcomeUsd === null) {
    const unpriced = from.costPerAcceptedOutcomeUsd === null ? from : to;
    return {
      from: from.costPerAcceptedOutcomeUsd,
      to: to.costPerAcceptedOutcomeUsd,
      absolute: null,
      relative: null,
      direction: "unpriced",
      reason: `no cost delta: the ${unpriced.lane} lane reports ${unpriced.costDisposition}, and a delta against an unmeasured cost would be an invention`,
    };
  }
  return numericDelta(from.costPerAcceptedOutcomeUsd, to.costPerAcceptedOutcomeUsd, "lower", {
    better: "cost per accepted outcome fell",
    worse: "cost per accepted outcome rose",
  });
};

/** Success rate, where up is better. `null` on either side is unknown. */
const successRateDelta = (from: BenchResultRow, to: BenchResultRow): Delta => {
  if (from.successRate === null || to.successRate === null) {
    return {
      from: from.successRate,
      to: to.successRate,
      absolute: null,
      relative: null,
      direction: "unknown",
      reason:
        "no success-rate delta: a run with no graded trials has no success rate rather than a zero one",
    };
  }
  return numericDelta(from.successRate, to.successRate, "higher", {
    better: "success rate rose",
    worse: "success rate fell",
  });
};

const numericDelta = (
  from: number,
  to: number,
  betterWhen: "lower" | "higher",
  reasons: { readonly better: string; readonly worse: string },
): Delta => {
  const absolute = to - from;
  const relative = from === 0 ? null : absolute / from;
  const improved = betterWhen === "lower" ? absolute < 0 : absolute > 0;
  return {
    from,
    to,
    absolute,
    relative,
    direction: absolute === 0 ? "unchanged" : improved ? "better" : "worse",
    reason: absolute === 0 ? "unchanged" : improved ? reasons.better : reasons.worse,
  };
};

/**
 * What else varies across the rows being compared, besides the axis being
 * compared on.
 *
 * Named rather than counted, so the reader can decide whether the confound
 * matters. Comparing two lanes on different CLI versions is still worth doing;
 * reading it as a clean lane comparison is not.
 */
const confoundersOf = (
  rows: ReadonlyArray<BenchResultRow>,
  axis: "lane" | "recordedAt",
): ReadonlyArray<string> => {
  const notes: Array<string> = [];
  const versions = distinct(rows.flatMap((row) => row.agentVersions));
  const models = distinct(rows.flatMap((row) => row.models));
  const bases = distinct(rows.map((row) => row.rateBasis));

  if (versions.length > 1) {
    notes.push(`CLI version also varies (${versions.join(", ")})`);
  }
  if (axis === "recordedAt" && models.length > 1) {
    notes.push(`model also varies (${models.join(", ")})`);
  }
  // The staged text is a variable like any other (OpenAgentsInc/openagents#122).
  // Named on both axes: a lane comparison assumes the prompt is held still
  // while the lane changes, and a trend step that changed the prompt is a
  // cycle whose delta belongs to that change and to nothing else in the step.
  const pins = distinct(rows.map(surfacePinOf));
  if (pins.length > 1) {
    notes.push(`staged text also varies (${pins.join(" | ")})`);
  }
  if (rows.some((row) => surfacePinOf(row) === null) && pins.length > 0) {
    notes.push("at least one row names no staged text, so what it measured cannot be identified");
  }
  if (bases.includes("operator_placeholder")) {
    notes.push(
      "at least one row is priced from operator placeholder rates, so its cost is provisional",
    );
  }
  return notes;
};

const distinct = (values: ReadonlyArray<string | null>): ReadonlyArray<string> =>
  [...new Set(values.filter((value): value is string => value !== null))].toSorted();

const groupBy = <T>(
  values: ReadonlyArray<T>,
  key: (value: T) => string,
): ReadonlyArray<ReadonlyArray<T>> => {
  const buckets = new Map<string, Array<T>>();
  for (const value of values) {
    const bucket = buckets.get(key(value));
    if (bucket === undefined) buckets.set(key(value), [value]);
    else bucket.push(value);
  }
  return [...buckets.values()];
};
