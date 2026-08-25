/**
 * Render a comparison as text.
 *
 * Same rendering rule as `render.ts`: an unmeasured figure prints as `unknown`
 * with its reason beside it, never as `$0.0000` and never as a blank cell. A
 * lane comparison is exactly where a blank would be read as "the same", so the
 * unpriced lane says `unpriced` and says why on the next line.
 */

import type { Comparison, Delta, LaneComparison, LaneTrend } from "./compare.ts";

const usd = (value: number | null): string => (value === null ? "unknown" : `$${value.toFixed(4)}`);

const pct = (value: number | null): string =>
  value === null ? "unknown" : `${(value * 100).toFixed(1)}%`;

const signed = (value: number, digits: number): string =>
  `${value >= 0 ? "+" : ""}${value.toFixed(digits)}`;

const deltaLine = (label: string, delta: Delta, digits: number, money: boolean): string => {
  if (delta.absolute === null) {
    return `    ${label.padEnd(22)} ${delta.direction}`;
  }
  const shown = money
    ? `${delta.absolute >= 0 ? "+" : "-"}$${Math.abs(delta.absolute).toFixed(digits)}`
    : signed(delta.absolute, digits);
  const relative = delta.relative === null ? "" : ` (${signed(delta.relative * 100, 1)}%)`;
  return `    ${label.padEnd(22)} ${shown}${relative}  ${delta.direction}`;
};

const renderLaneComparison = (comparison: LaneComparison): ReadonlyArray<string> => {
  const lines: Array<string> = [];
  lines.push(`Lanes — ${comparison.suite}, ${String(comparison.tasks.length)} tasks`);
  lines.push(`  suite key       ${comparison.suiteKey}`);
  lines.push(`  baseline lane   ${comparison.baselineLane}`);
  for (const note of comparison.confounders) {
    lines.push(`  confounded      ${note}`);
  }
  lines.push("");
  for (const lane of comparison.lanes) {
    const marker = lane.lane === comparison.baselineLane ? " (baseline)" : "";
    lines.push(`  ${lane.lane}${marker}`);
    lines.push(
      `    ${"cost per accepted".padEnd(22)} ${usd(lane.row.costPerAcceptedOutcomeUsd)}${lane.row.costPerAcceptedOutcomeUsd === null ? ` (${lane.row.costDisposition})` : ""}`,
    );
    lines.push(`    ${"success rate".padEnd(22)} ${pct(lane.row.successRate)}`);
    lines.push(
      `    ${"outcomes".padEnd(22)} ${String(lane.row.accepted)} accepted, ${String(lane.row.rejected)} rejected, ${String(lane.row.ungraded)} ungraded`,
    );
    if (lane.costDelta !== null) {
      lines.push(deltaLine("Δ cost per accepted", lane.costDelta, 4, true));
      if (lane.costDelta.absolute === null) lines.push(`      ${lane.costDelta.reason}`);
    }
    if (lane.successRateDelta !== null) {
      lines.push(deltaLine("Δ success rate", lane.successRateDelta, 3, false));
    }
    lines.push("");
  }
  return lines;
};

const renderTrend = (trend: LaneTrend): ReadonlyArray<string> => {
  const lines: Array<string> = [];
  lines.push(`Trend — ${trend.suite} on the ${trend.lane} lane, ${String(trend.rows.length)} runs`);
  for (const step of trend.steps) {
    lines.push(`  ${step.from.recordedAt} → ${step.to.recordedAt}`);
    lines.push(deltaLine("Δ cost per accepted", step.costDelta, 4, true));
    if (step.costDelta.absolute === null) lines.push(`      ${step.costDelta.reason}`);
    lines.push(deltaLine("Δ success rate", step.successRateDelta, 3, false));
    for (const note of step.confounders) {
      lines.push(`    confounded             ${note}`);
    }
  }
  lines.push("");
  return lines;
};

export const renderComparison = (comparison: Comparison): string => {
  const lines: Array<string> = [];

  if (comparison.laneComparisons.length === 0 && comparison.trends.length === 0) {
    lines.push("Nothing to compare.");
    lines.push("");
    lines.push(
      comparison.isolatedGroups === 0
        ? "  The store holds no rows. Append one with `effectiveness:report --append <store>`."
        : `  The store holds ${String(comparison.isolatedGroups)} run shape(s), none with a second comparable row.`,
    );
    lines.push("  Two rows are comparable when they share a suite key: the same suite, the same");
    lines.push("  task list, and the same rate catalog. Lane, model, and CLI version may differ —");
    lines.push("  those are the axes a comparison varies.");
    return `${lines.join("\n")}\n`;
  }

  for (const laneComparison of comparison.laneComparisons) {
    lines.push(...renderLaneComparison(laneComparison));
  }
  for (const trend of comparison.trends) {
    lines.push(...renderTrend(trend));
  }
  if (comparison.isolatedGroups > 0) {
    lines.push(
      `${String(comparison.isolatedGroups)} run shape(s) in this store had no comparable second row and were left out.`,
    );
  }
  return `${lines.join("\n")}\n`;
};
