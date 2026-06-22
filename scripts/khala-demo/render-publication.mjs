#!/usr/bin/env bun

import { loadManifest, reduceKhalaHeadToHeadManifest } from "./reduce-head-to-head.mjs";

function formatNumber(value) {
  if (typeof value === "number") {
    return new Intl.NumberFormat("en-US", { maximumFractionDigits: 6 }).format(value);
  }
  return String(value);
}
function formatUsd(value) {
  if (typeof value !== "number") {
    return String(value);
  }
  return `$${value.toFixed(2)}`;
}

function formatDuration(ms) {
  if (typeof ms !== "number" || !Number.isFinite(ms)) {
    return "not_measured";
  }
  const totalSeconds = Math.round(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  if (minutes === 0) {
    return `${seconds}s`;
  }
  return `${minutes}m ${seconds}s`;
}

function formatSplit(split) {
  if (split.status === "not_measured") {
    return "not_measured";
  }
  return `${formatNumber(split.inWorldShare * 100)}% in-world / ${formatNumber(
    split.gatewayShare * 100,
  )}% gateway`;
}

function formatList(values) {
  if (!Array.isArray(values) || values.length === 0) {
    return "- none";
  }
  return values.map((value) => `- \`${value}\``).join("\n");
}

function tableRow(cells) {
  return `| ${cells.join(" | ")} |`;
}

export function renderKhalaHeadToHeadPublication(metrics) {
  const status =
    metrics.evidenceMode === "live"
      ? "LIVE EVIDENCE"
      : "FIXTURE SCAFFOLD - not product proof";
  const scoreboardRows = metrics.scoreboard.map((run) =>
    tableRow([
      run.lane,
      run.model,
      run.evidenceMode,
      formatNumber(run.tokens),
      formatUsd(run.dollars),
      formatDuration(run.wallClockMs),
      run.accepted ? "yes" : "no",
      run.verificationClass,
      typeof run.costPerAcceptedOutcomeUsd === "number"
        ? formatUsd(run.costPerAcceptedOutcomeUsd)
        : run.costPerAcceptedOutcomeUsd,
      formatNumber(run.acceptedOutcomesPerKwh),
      formatSplit(run.inWorldVsGatewaySplit),
    ]),
  );

  const externalRows = metrics.externalReportedClaims.map((claim) =>
    tableRow([
      claim.label,
      claim.citationStatus,
      formatNumber(claim.tokens ?? "not_reported"),
      formatUsd(claim.costUsd ?? "not_reported"),
      formatDuration(claim.wallClockMs),
      claim.verdictSummary,
    ]),
  );

  return [
    "# Khala Head-to-Head Demo Publication Draft",
    "",
    `Status: **${status}**`,
    "",
    `Manifest: \`${metrics.manifestRef}\``,
    `Issue: \`${metrics.scope.issueRef}\``,
    `Prompt: ${metrics.scope.prompt}`,
    "",
    "## Closure Audit",
    "",
    `canClose: \`${String(metrics.closureAudit.canClose)}\``,
    "",
    "Required evidence:",
    "",
    formatList(metrics.closureAudit.requiredEvidence),
    "",
    "Current blockers:",
    "",
    formatList(metrics.closureAudit.blockerRefs),
    "",
    "## Scoreboard",
    "",
    tableRow([
      "lane",
      "model",
      "evidence",
      "tokens",
      "cost",
      "wall-clock",
      "accepted",
      "verification",
      "cost/AO",
      "AO/kWh",
      "in-world/gateway",
    ]),
    tableRow([
      "---",
      "---",
      "---",
      "---:",
      "---:",
      "---:",
      "---",
      "---",
      "---:",
      "---:",
      "---",
    ]),
    ...scoreboardRows,
    "",
    "## External Reported Claims",
    "",
    "These rows are not OpenAgents measurements.",
    "",
    tableRow(["claim", "citation", "tokens", "cost", "wall-clock", "summary"]),
    tableRow(["---", "---", "---:", "---:", "---:", "---"]),
    ...externalRows,
    "",
    "## Publication Boundary",
    "",
    "- Fixture rows are integration evidence only.",
    "- Product, world-first, and AO/kWh claims require dereferenceable live evidence.",
    "- Settlement claims require public worker and validator settlement refs.",
    "- The final published comparison must keep failures and missing telemetry visible.",
    "",
  ].join("\n");
}

if (import.meta.main) {
  const manifestPath = process.argv[2];
  if (manifestPath === undefined) {
    console.error("usage: bun scripts/khala-demo/render-publication.mjs <manifest.json>");
    process.exit(2);
  }
  const manifest = loadManifest(manifestPath);
  const metrics = reduceKhalaHeadToHeadManifest(manifest);
  console.log(renderKhalaHeadToHeadPublication(metrics));
}
