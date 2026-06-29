#!/usr/bin/env bun

/**
 * Khala M8 measured-metric table emitter.
 *
 * Consumes a head-to-head evidence manifest (the EXACT shape produced by
 * `run-head-to-head.mjs` and validated by `reduce-head-to-head.mjs`) and emits
 * the epic's per-run measured metric table: tokens / $ / wall-clock,
 * cost-per-accepted-outcome, verified-rate, accepted-outcomes-per-kWh, and the
 * in-world-vs-gateway split.
 *
 * Why this exists (issue #6016 task 3): `reduce-head-to-head.mjs` already
 * computes the metrics, but it reports an UNMEASURED token count or cost as the
 * literal number `0` (the runner's honest "no figure reported" default carries a
 * `*_not_measured` blocker alongside it). Publishing a `0` as if it were a
 * measured tokens/$/cost-per-accepted-outcome figure would be dishonest. This
 * emitter reads the run's blocker refs and downgrades those unmeasured-zero
 * cells to the literal string `not_measured`, so the published table never
 * fabricates a metric the recorded run did not actually carry.
 *
 * It is read-only over the manifest: it changes NOTHING about the runner's
 * network/contract behavior. It is a pure projection of the reducer output into
 * a Markdown/JSON table.
 *
 * Usage:
 *   bun scripts/khala-demo/emit-metric-table.mjs <manifest.json> [--json]
 */

import { loadManifest, reduceKhalaHeadToHeadManifest } from "./reduce-head-to-head.mjs";

const NOT_MEASURED = "not_measured";

const TOKENS_NOT_MEASURED_BLOCKERS = new Set([
  "blocker.khala_demo.tokens_not_measured",
]);
const COST_NOT_MEASURED_BLOCKERS = new Set([
  "blocker.khala_demo.cost_usd_not_measured",
  "blocker.khala_demo.cost_not_measured",
]);

function hasAny(blockerRefs, set) {
  return Array.isArray(blockerRefs) && blockerRefs.some((ref) => set.has(ref));
}

/**
 * Honest tokens cell: a literal 0 alongside a tokens-not-measured blocker is an
 * absence of measurement, not a real zero, so it is reported as not_measured.
 */
export function tokensCell(run) {
  if (run.tokens === 0 && hasAny(run.blockerRefs, TOKENS_NOT_MEASURED_BLOCKERS)) {
    return NOT_MEASURED;
  }
  return run.tokens;
}

/**
 * Honest dollars cell: same rule as tokens, keyed on the cost-not-measured
 * blocker.
 */
export function dollarsCell(run) {
  if (run.dollars === 0 && hasAny(run.blockerRefs, COST_NOT_MEASURED_BLOCKERS)) {
    return NOT_MEASURED;
  }
  return run.dollars;
}

/**
 * Honest cost-per-accepted-outcome cell. The reducer reports `not_applicable`
 * for an unaccepted run and the bare cost for an accepted run; but if that cost
 * is an unmeasured zero, the per-accepted-outcome cost is unknown, not $0.
 */
export function costPerAcceptedOutcomeCell(run) {
  const value = run.costPerAcceptedOutcomeUsd;
  if (value === "not_applicable") {
    return value;
  }
  if (value === 0 && hasAny(run.blockerRefs, COST_NOT_MEASURED_BLOCKERS)) {
    return NOT_MEASURED;
  }
  return value;
}

export function wallClockCell(run) {
  const ms = run.wallClockMs;
  if (typeof ms !== "number" || !Number.isFinite(ms) || ms <= 0) {
    return NOT_MEASURED;
  }
  const totalSeconds = Math.round(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return minutes === 0 ? `${seconds}s` : `${minutes}m ${seconds}s`;
}

export function inWorldVsGatewayCell(run) {
  const split = run.inWorldVsGatewaySplit;
  if (!split || split.status !== "measured_from_manifest_units") {
    return NOT_MEASURED;
  }
  const inWorldPct = Math.round(split.inWorldShare * 1000) / 10;
  const gatewayPct = Math.round(split.gatewayShare * 1000) / 10;
  return `${inWorldPct}% in-world / ${gatewayPct}% gateway`;
}

/**
 * Build the row model for one scoreboard run. Pure; no I/O.
 */
export function metricRowForRun(run) {
  return {
    lane: run.lane,
    label: run.label,
    model: run.model,
    coordinatorMode: run.coordinatorMode,
    tokens: tokensCell(run),
    dollars: dollarsCell(run),
    wallClock: wallClockCell(run),
    verificationClass: run.verificationClass,
    accepted: run.accepted,
    costPerAcceptedOutcomeUsd: costPerAcceptedOutcomeCell(run),
    acceptedOutcomesPerKwh: run.acceptedOutcomesPerKwh,
    inWorldVsGatewaySplit: inWorldVsGatewayCell(run),
    settled: run.settlement.settled,
  };
}

/**
 * Build the full table model (rows + verified-rate summary) from a reduced
 * metrics object. Pure.
 */
export function buildMetricTable(metrics) {
  return {
    manifestRef: metrics.manifestRef,
    evidenceMode: metrics.evidenceMode,
    verifiedRate: metrics.summary.verifiedRate,
    rows: metrics.scoreboard.map(metricRowForRun),
  };
}

function fmtUsd(value) {
  return typeof value === "number" ? `$${value.toFixed(2)}` : String(value);
}

function fmtTokens(value) {
  return typeof value === "number"
    ? new Intl.NumberFormat("en-US").format(value)
    : String(value);
}

function fmtBool(value) {
  return value === true ? "yes" : value === false ? "no" : String(value);
}

function row(cells) {
  return `| ${cells.map((cell) => String(cell).replaceAll("|", "\\|")).join(" | ")} |`;
}

/**
 * Render the table model to a Markdown table string. Pure.
 */
export function renderMetricTableMarkdown(table) {
  const header = [
    "lane",
    "model",
    "coordinator",
    "tokens",
    "$",
    "wall-clock",
    "verified",
    "cost/accepted-outcome",
    "AO/kWh",
    "in-world vs gateway",
    "settled",
  ];
  const divider = header.map(() => "---");
  const lines = [
    `Manifest: \`${table.manifestRef}\` (evidenceMode: ${table.evidenceMode})`,
    `Verified-rate: ${table.verifiedRate}`,
    "",
    row(header),
    row(divider),
  ];
  for (const r of table.rows) {
    lines.push(
      row([
        r.lane,
        `\`${r.model}\``,
        r.coordinatorMode,
        fmtTokens(r.tokens),
        fmtUsd(r.dollars),
        r.wallClock,
        `${r.verificationClass} (${fmtBool(r.accepted)})`,
        fmtUsd(r.costPerAcceptedOutcomeUsd),
        String(r.acceptedOutcomesPerKwh),
        r.inWorldVsGatewaySplit,
        fmtBool(r.settled),
      ]),
    );
  }
  return lines.join("\n");
}

export function emitMetricTableFromManifest(rawManifest, { json = false } = {}) {
  const metrics = reduceKhalaHeadToHeadManifest(rawManifest);
  const table = buildMetricTable(metrics);
  return json ? JSON.stringify(table, null, 2) : renderMetricTableMarkdown(table);
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  const json = args.includes("--json");
  const manifestPath = args.find((arg) => !arg.startsWith("--"));
  if (manifestPath === undefined) {
    console.error("usage: bun scripts/khala-demo/emit-metric-table.mjs <manifest.json> [--json]");
    process.exit(2);
  }
  const manifest = loadManifest(manifestPath);
  console.log(emitMetricTableFromManifest(manifest, { json }));
}
