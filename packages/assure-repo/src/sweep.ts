import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

import { Schema as S } from "effect";

import {
  buildFalseGreenReport,
  FALSE_GREEN_REPORT_PATH,
  serializeFalseGreenReport,
} from "./audit.ts";
import { runDriftOracles } from "./drift.ts";
import { buildInventory, loadPolicy } from "./inventory.ts";
import {
  compareStrings,
  serializeSurfaceInventory,
  SURFACE_INVENTORY_PATH,
  validateSurfaceInventory,
} from "./schema.ts";

/**
 * AR-3 (issue #9059): the standing verification sweep.
 *
 * Re-derives the AR-0 inventory and re-runs the ASSURE-REPO oracles against
 * the current tree, then lands a receipt. Readiness surfaces consume the
 * receipt under the no-receipt-no-light rule (see readiness.ts). This runs
 * DEGRADED and honestly labeled before IDE-10 host-observed tests and the SBX
 * sandbox land: it re-checks the deterministic ASSURE-REPO artifacts and drift
 * oracles (which need no external process), and records `degraded_terminal_observed`
 * as its evidence class. It is read-only over source — it cannot alter Full
 * Auto guardrails because it mutates nothing but its own receipt output.
 */

export const SWEEP_RECEIPT_FORMAT_VERSION = "1" as const;

export const SweepOracleOutcome = S.Struct({
  oracle: S.String,
  outcome: S.Literals(["pass", "fail", "unavailable"]),
  detail: S.String,
});
export type SweepOracleOutcome = typeof SweepOracleOutcome.Type;

export const SweepReceipt = S.Struct({
  schemaVersion: S.Literal(SWEEP_RECEIPT_FORMAT_VERSION),
  repository: S.Literal("OpenAgentsInc/openagents"),
  commit: S.String,
  generatedAt: S.String,
  /** Honest evidence class: what kind of observation backs this sweep. */
  evidenceClass: S.Literals(["degraded_terminal_observed", "host_observed"]),
  inventorySourceDigest: S.String,
  oracleOutcomes: S.Array(SweepOracleOutcome),
  obligationSummary: S.Record(S.String, S.Number),
  driftSummary: S.Struct({ broken: S.Number, open: S.Number, unverifiable: S.Number }),
  /** green only when every oracle passed; unknown when an oracle is unavailable. */
  overall: S.Literals(["green", "red", "unknown"]),
});
export type SweepReceipt = typeof SweepReceipt.Type;

export const decodeSweepReceipt = S.decodeUnknownSync(SweepReceipt);
export const serializeSweepReceipt = (receipt: SweepReceipt): string =>
  `${JSON.stringify(receipt, null, 2)}\n`;

const headCommit = (root: string): string => {
  try {
    return execFileSync("git", ["-C", root, "rev-parse", "HEAD"], { encoding: "utf8" }).trim();
  } catch {
    return "unknown";
  }
};

const committed = (root: string, path: string): string | null => {
  const full = join(root, path);
  return existsSync(full) ? readFileSync(full, "utf8") : null;
};

/**
 * Run the sweep. `now` is injected so callers/tests control the timestamp.
 */
export const runSweep = (root: string, now: string): SweepReceipt => {
  const outcomes: SweepOracleOutcome[] = [];

  // Oracle 1: inventory freshness (deterministic byte-compare).
  const inventory = buildInventory(root);
  const inventoryValidation = validateSurfaceInventory(inventory);
  const inventoryFresh =
    committed(root, SURFACE_INVENTORY_PATH) === serializeSurfaceInventory(inventory);
  outcomes.push({
    oracle: "assure-repo.inventory_fresh",
    outcome: inventoryValidation.ok && inventoryFresh ? "pass" : "fail",
    detail: !inventoryValidation.ok
      ? "inventory fails validation"
      : inventoryFresh
        ? "committed inventory matches fresh regeneration"
        : "committed inventory is stale",
  });

  // Oracle 2: false-green candidate report freshness.
  const audit = buildFalseGreenReport(root);
  const auditFresh = committed(root, FALSE_GREEN_REPORT_PATH) === serializeFalseGreenReport(audit);
  outcomes.push({
    oracle: "assure-repo.audit_fresh",
    outcome: auditFresh ? "pass" : "fail",
    detail: auditFresh
      ? "candidate report matches fresh regeneration"
      : "candidate report is stale",
  });

  // Oracle 3: documentation drift (no open broken claims).
  const policy = loadPolicy(root);
  const drift = runDriftOracles(
    root,
    [...policy.governedDocuments, "docs/assure-repo/README.md", "packages/assure-repo/README.md"],
    policy.driftDispositions,
  );
  outcomes.push({
    oracle: "assure-repo.doc_drift",
    outcome: drift.summary.brokenUndispositioned === 0 ? "pass" : "fail",
    detail: `${drift.summary.brokenUndispositioned} open broken documented claim(s)`,
  });

  const obligationSummary: Record<string, number> = {};
  for (const key of Object.keys(inventory.summary.byObligationState).sort(compareStrings)) {
    obligationSummary[key] = inventory.summary.byObligationState[key]!;
  }

  const anyUnavailable = outcomes.some((o) => o.outcome === "unavailable");
  const anyFail = outcomes.some((o) => o.outcome === "fail");
  const overall = anyUnavailable ? "unknown" : anyFail ? "red" : "green";

  return {
    schemaVersion: SWEEP_RECEIPT_FORMAT_VERSION,
    repository: "OpenAgentsInc/openagents",
    commit: headCommit(root),
    generatedAt: now,
    evidenceClass: "degraded_terminal_observed",
    inventorySourceDigest: inventory.sourceDigest,
    oracleOutcomes: outcomes,
    obligationSummary,
    driftSummary: {
      broken: drift.summary.broken,
      open: drift.summary.brokenUndispositioned,
      unverifiable: drift.summary.unverifiable,
    },
    overall,
  };
};

export type SweepDriftFinding = {
  readonly kind: "oracle_regressed" | "obligation_regressed" | "overall_regressed";
  readonly detail: string;
};

/**
 * Diff two sweep receipts into typed drift findings. A surface losing coverage
 * or an oracle regressing is visible, not silent.
 */
export const diffSweeps = (
  previous: SweepReceipt,
  current: SweepReceipt,
): ReadonlyArray<SweepDriftFinding> => {
  const findings: SweepDriftFinding[] = [];
  const prevByOracle = new Map(previous.oracleOutcomes.map((o) => [o.oracle, o.outcome]));
  for (const outcome of current.oracleOutcomes) {
    const before = prevByOracle.get(outcome.oracle);
    if (before === "pass" && outcome.outcome !== "pass") {
      findings.push({
        kind: "oracle_regressed",
        detail: `${outcome.oracle}: pass -> ${outcome.outcome}`,
      });
    }
  }
  const prevDesigned = previous.obligationSummary["designed"] ?? 0;
  const currDesigned = current.obligationSummary["designed"] ?? 0;
  if (currDesigned < prevDesigned) {
    findings.push({
      kind: "obligation_regressed",
      detail: `designed surfaces ${prevDesigned} -> ${currDesigned}`,
    });
  }
  if (previous.overall === "green" && current.overall !== "green") {
    findings.push({
      kind: "overall_regressed",
      detail: `overall ${previous.overall} -> ${current.overall}`,
    });
  }
  return findings;
};
