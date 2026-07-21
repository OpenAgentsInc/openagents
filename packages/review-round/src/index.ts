import { Schema as S } from "effect";

// The canonical anti-laundering rules for an adversarial review round.
//
// A review round runs several finder "lenses" over a frozen commit. The
// failure mode this module exists to remove is a review whose failure is
// indistinguishable from success: a dead lens whose empty output reads as
// "no problems", or a lens that reports zero findings without proving it
// looked at anything. Both launder an unrun review into a clean result.
//
// The rules, learned from the `chenglou/freerange` review workflow and
// recorded in the Freerange teardown (docs/teardowns/2026-07-21-freerange-teardown.md):
//
//   1. Positive control. A lens that reports zero findings must also report a
//      positive control (`probesRun >= 1`). An empty findings list only means
//      "clean" when the lens proved a sweep happened.
//   2. Died lens surfaces. A lens whose runner died (its report is missing)
//      becomes an explicit AGENT-DIED failure, never a dropped row and never a
//      clean pass.
//   3. Reproduced contradiction. A finding must carry an observed
//      contradiction (a failing command, a crash, or counterexample output).
//      A finding with no observation is unsubstantiated, not a confirmed
//      finding and not a clean pass.
//   4. Fail-closed round status. A round reaches `clean` only when every lens
//      proved a sweep and no failure row exists. Any AGENT-DIED, unproven, or
//      unsubstantiated row makes the round `failed` (rerun), distinct from a
//      round that produced real findings.
//
// The `.claude/workflows/review-round.js` workflow inlines these same rules
// (workflow scripts must be self-contained plain JavaScript). This package is
// the tested authority for the semantics; the workflow must not diverge from
// `aggregateRound` here.

export const REVIEW_ROUND_SCHEMA_VERSION = "openagents.review-round.v1" as const;

/**
 * One finding from a lens. A finding is a claimed defect plus the probe that
 * was run and the observation that reproduces the contradiction. `observed`
 * must be non-empty for the finding to count as confirmed (rule 3).
 */
export const ReviewFinding = S.Struct({
  lens: S.String,
  title: S.String,
  severity: S.Literals(["low", "medium", "high", "critical"]),
  claim: S.String,
  probe: S.String,
  observed: S.String,
});
export type ReviewFinding = typeof ReviewFinding.Type;

/**
 * What a lens returns when its runner completed. `probesRun` is the positive
 * control: the number of probes the lens actually executed. It exists so an
 * empty `findings` list can prove a sweep happened (rule 1).
 */
export const LensReport = S.Struct({
  lens: S.String,
  probesRun: S.Number,
  findings: S.Array(ReviewFinding),
});
export type LensReport = typeof LensReport.Type;

/**
 * The result of running one lens. `report` is `null` exactly when the lens
 * runner died (the Workflow `agent()` call returns `null` on a terminal
 * failure). Keeping the lens key alongside a null report is what lets rule 2
 * surface the death instead of silently dropping the row.
 */
export type LensOutcome = {
  lens: string;
  report: LensReport | null;
};

export const FAILURE_KINDS = [
  // The lens runner died. Its absence must never read as "clean".
  "agent-died",
  // The lens reported zero findings but ran zero probes: no positive control,
  // so the empty result cannot prove a sweep happened.
  "lens-unproven",
  // A finding without a reproduced observation. Not counted as a real finding,
  // and not allowed to pass as clean either.
  "unsubstantiated-finding",
  // The lens report did not match the expected shape.
  "malformed-report",
  // The round proved no sweep at all — zero lenses swept and nothing found.
  // An empty or fully-inconclusive round must not read as clean.
  "no-sweep",
] as const;
export type FailureKind = (typeof FAILURE_KINDS)[number];

export type RoundFailure = {
  lens: string;
  kind: FailureKind;
  detail: string;
};

export type RoundStatus = "clean" | "findings" | "failed";

export type RoundResult = {
  schemaVersion: typeof REVIEW_ROUND_SCHEMA_VERSION;
  status: RoundStatus;
  confirmedFindings: ReadonlyArray<ReviewFinding>;
  failures: ReadonlyArray<RoundFailure>;
  // Lenses that produced a proven sweep: a runner completed, the report shape
  // decoded, and `probesRun >= 1`.
  lensesSwept: number;
  // Lenses whose runner produced any report at all (proven or not).
  lensesReported: number;
  // Total lenses attempted, including died runners.
  lensesAttempted: number;
  probesRun: number;
};

/**
 * A structural well-formedness check for an incoming lens report. This mirrors
 * the check the `.claude/workflows/review-round.js` workflow inlines, so the
 * package and the workflow reject the same malformed shapes. It intentionally
 * does not decode with `S.decodeUnknownSync` (which throws) — a malformed
 * report must become a failure row, never an exception in the fold.
 */
function isWellFormedReport(report: unknown): report is LensReport {
  if (typeof report !== "object" || report === null) return false;
  const candidate = report as { probesRun?: unknown; findings?: unknown };
  if (typeof candidate.probesRun !== "number") return false;
  if (!Number.isFinite(candidate.probesRun)) return false;
  if (!Array.isArray(candidate.findings)) return false;
  return candidate.findings.every((finding: unknown) => {
    if (typeof finding !== "object" || finding === null) return false;
    const f = finding as Record<string, unknown>;
    return (
      typeof f["title"] === "string" &&
      typeof f["severity"] === "string" &&
      typeof f["claim"] === "string" &&
      typeof f["probe"] === "string" &&
      typeof f["observed"] === "string"
    );
  });
}

/**
 * Fold one round's lens outcomes into a fail-closed result. This is the whole
 * anti-laundering discipline in one pure function.
 *
 * The status can only be `clean` when every attempted lens proved a sweep and
 * no failure row exists. A died lens, a lens with no positive control, a
 * malformed report, or an unsubstantiated finding each makes the round
 * `failed` — a state that demands a rerun and can never be mistaken for a
 * green.
 */
export function aggregateRound(outcomes: ReadonlyArray<LensOutcome>): RoundResult {
  const confirmedFindings: Array<ReviewFinding> = [];
  const failures: Array<RoundFailure> = [];
  let lensesSwept = 0;
  let lensesReported = 0;
  let probesRun = 0;

  for (const outcome of outcomes) {
    // Rule 2: a died runner is an explicit failure, never a dropped row.
    if (outcome.report === null) {
      failures.push({
        lens: outcome.lens,
        kind: "agent-died",
        detail: "lens runner returned no report (died or terminal error)",
      });
      continue;
    }

    // Rule 4 (belt-and-suspenders): a report whose shape is malformed is a
    // failure, not an optimistic pass.
    if (!isWellFormedReport(outcome.report)) {
      failures.push({
        lens: outcome.lens,
        kind: "malformed-report",
        detail: "lens report did not match the review-round contract",
      });
      continue;
    }
    const report = outcome.report;
    lensesReported += 1;
    probesRun += Math.max(0, Math.floor(report.probesRun));

    // Rule 3: split findings into confirmed (reproduced) and unsubstantiated.
    let lensHasSubstantiatedFinding = false;
    for (const finding of report.findings) {
      if (finding.observed.trim().length === 0) {
        failures.push({
          lens: outcome.lens,
          kind: "unsubstantiated-finding",
          detail: `finding "${finding.title}" has no reproduced observation`,
        });
        continue;
      }
      lensHasSubstantiatedFinding = true;
      confirmedFindings.push(finding);
    }

    // Rule 1: a lens with no confirmed findings must show a positive control.
    // Without it, the empty result cannot prove a sweep happened.
    if (!lensHasSubstantiatedFinding) {
      if (report.probesRun >= 1) {
        lensesSwept += 1;
      } else {
        failures.push({
          lens: outcome.lens,
          kind: "lens-unproven",
          detail: "lens reported no confirmed findings and ran zero probes (no positive control)",
        });
      }
    } else {
      // A lens that produced a real finding demonstrably swept.
      lensesSwept += 1;
    }
  }

  // Rule 4: a round reaches `clean` only when at least one lens proved a sweep
  // and no failure row exists. A round that swept nothing (empty, or every lens
  // inconclusive) is `failed`, never vacuously clean.
  if (failures.length === 0 && confirmedFindings.length === 0 && lensesSwept === 0) {
    failures.push({
      lens: "(round)",
      kind: "no-sweep",
      detail: "no lens proved a sweep; the round cannot be read as clean",
    });
  }

  const status: RoundStatus =
    failures.length > 0 ? "failed" : confirmedFindings.length > 0 ? "findings" : "clean";

  return {
    schemaVersion: REVIEW_ROUND_SCHEMA_VERSION,
    status,
    confirmedFindings,
    failures,
    lensesSwept,
    lensesReported,
    lensesAttempted: outcomes.length,
    probesRun,
  };
}

/**
 * A round is trustworthy as a clean result only when the status is `clean`.
 * `findings` and `failed` both mean "not clean", for different reasons. This
 * helper exists so callers cannot accidentally treat a `failed` round (which
 * must be rerun) as a passing one.
 */
export function roundIsClean(result: RoundResult): boolean {
  return result.status === "clean";
}
