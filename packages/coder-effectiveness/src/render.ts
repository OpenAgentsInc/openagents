/**
 * Render the effectiveness report as text.
 *
 * The rendering rule matches the measuring rule: an unknown prints as
 * `unknown` with the reason beside it, never as `$0.0000` and never as a
 * blank. A reader who skims one line of this output should not be able to
 * come away believing a lane was measured when it was not.
 */

import type { EffectivenessReport } from "./effectiveness.ts";
import { CODER_RATE_CATALOG_SOURCE_REF } from "./pricing.ts";
import type { ThresholdGate } from "./thresholds.ts";

const usd = (value: number): string => `$${value.toFixed(4)}`;

const rate = (value: number | null): string =>
  value === null ? "unknown" : `${(value * 100).toFixed(1)}%`;

const count = (value: number | null): string =>
  value === null ? "unknown" : value.toLocaleString("en-US");

export const renderReport = (report: EffectivenessReport, gate: ThresholdGate | null): string => {
  const lines: Array<string> = [];

  lines.push(`Coder effectiveness — ${report.suite} on the ${report.lane} lane`);
  lines.push(`  run digest      ${report.runDigest}`);
  if (report.jobId !== null) lines.push(`  harbor job      ${report.jobId}`);
  lines.push(`  model           ${report.models.join(", ") || "unknown"}`);
  lines.push(`  cli version     ${report.agentVersions.join(", ") || "unknown"}`);
  lines.push(`  rate catalog    ${report.rateCatalogVersion}`);
  lines.push(`  rate source     ${CODER_RATE_CATALOG_SOURCE_REF}`);
  lines.push("");

  lines.push("Outcomes");
  lines.push(
    `  accepted ${String(report.accepted)} · rejected ${String(report.rejected)} · ungraded ${String(report.ungraded)} of ${String(report.trialsTotal)}`,
  );
  lines.push(
    `  success rate    ${rate(report.successRate)} over ${String(report.graded)} graded trials`,
  );
  if (report.ungraded > 0) {
    lines.push(
      `  ungraded        ${report.ungradedRatio.toFixed(3)} of trials had no verifier result, so they are neither pass nor fail`,
    );
  }
  lines.push("");

  lines.push("Cost per accepted outcome");
  const cost = report.costPerAcceptedOutcome;
  if (cost.usd === null) {
    lines.push(`  ${"unknown".padEnd(15)} (${cost.disposition})`);
    lines.push(`  ${" ".repeat(15)} ${cost.reason}`);
  } else {
    const provisional = cost.rateBasis === "operator_placeholder" ? "  [placeholder rates]" : "";
    lines.push(`  ${usd(cost.usd).padEnd(15)} per accepted outcome${provisional}`);
    lines.push(
      `  ${" ".repeat(15)} ${report.cost.totalUsd === null ? "unknown" : usd(report.cost.totalUsd)} total over ${String(report.accepted)} accepted`,
    );
    if (cost.rateBasis === "operator_placeholder") {
      lines.push(`  ${" ".repeat(15)} ${cost.reason}`);
    }
  }
  lines.push(
    `  cost coverage   ${report.cost.pricedTrials} of ${String(report.cost.pricedTrials + report.cost.unpricedTrials)} trials priced (${report.cost.coverage})`,
  );
  for (const reason of report.cost.unpricedReasons) {
    lines.push(`    unpriced      ${reason}`);
  }
  lines.push("");

  lines.push("Volume");
  lines.push(`  prompt tokens   ${count(report.promptTokens)}`);
  lines.push(`  output tokens   ${count(report.completionTokens)}`);
  lines.push(`  cached reads    ${count(report.cachedInputTokens)}`);
  lines.push(`  tool calls      ${count(report.toolCalls)}`);
  lines.push(
    `  wall clock      ${report.wallClockSeconds === null ? "unknown" : `${report.wallClockSeconds.toFixed(1)}s`}`,
  );
  lines.push("");

  lines.push("Trials");
  for (const trial of report.perTrial) {
    const money = trial.usd === null ? `unknown (${trial.disposition})` : usd(trial.usd);
    lines.push(`  ${trial.outcome.padEnd(9)} ${trial.task.padEnd(34)} ${money}`);
  }

  if (gate !== null) {
    lines.push("");
    lines.push(`Gate ${gate.thresholdsId}: ${gate.status.toUpperCase()}`);
    for (const criterion of gate.criteria) {
      const mark =
        criterion.verdict === "passed" ? "pass" : criterion.verdict === "failed" ? "FAIL" : "n/a ";
      lines.push(`  [${mark}] ${criterion.name}`);
      lines.push(`         ${criterion.detail}`);
    }
    if (gate.status === "unverifiable") {
      lines.push("");
      lines.push("  This gate is unverifiable, which is not a pass. A criterion could not be");
      lines.push("  measured on this run — most often because the lane carries no published");
      lines.push("  rate — so the run proves nothing about that floor.");
    }
  }

  return `${lines.join("\n")}\n`;
};
