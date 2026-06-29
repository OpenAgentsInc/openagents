// The verify stage (#6192): commitments -> investigator verdict.
//
// Rhys's thesis is "verify an agent's work without running anything locally,
// purely by looking at the e2e test and its output," and the public heckle was
// literally "verify the output before you post." This adopts droid-control's
// strongest pattern (`/verify`): a run declares COMMITMENTS up front (what must
// be PROVEN, and the evidence type), and AFTER the run a verify stage checks the
// produced artifacts/steps against those commitments and emits an INVESTIGATOR
// VERDICT:
//
//   CONFIRMED     — every commitment is backed by OBSERVED, non-fabricated
//                   evidence in this run's steps.
//   REFUTED       — at least one commitment is CONTRADICTED by observed evidence
//                   (a step that should have passed failed, or an assertion that
//                   should have held was violated). A FALSE claim is a VALID
//                   finding — it is reported as REFUTED, never as a fake pass.
//   INCONCLUSIVE  — a commitment is neither confirmed nor contradicted (the run
//                   produced no evidence bearing on it). Uncertainty is NEVER
//                   inflated to CONFIRMED.
//
// ANTI-FABRICATION (the load-bearing discipline; mirrors our "verified, not
// trusted / no fake green" posture):
//   - CONFIRMED requires OBSERVED evidence. A commitment whose evidence was not
//     produced by THIS run is INCONCLUSIVE, never CONFIRMED — a staged or
//     expected-but-unobserved outcome can never be reported as confirmed.
//   - A contradicted commitment is REFUTED with the contradicting evidence
//     inline. A false claim is a finding, not a failure of the verifier.
//   - Uncertain -> INCONCLUSIVE. We never round uncertainty up to CONFIRMED.
//
// PURE + PUBLIC-SAFE: this module is pure (no IO) so it is unit-testable on its
// own, and the emitted `VerifyReport` carries only labels/evidence summaries
// (no prompts/tokens/secrets — the runner's tripwire also re-checks it on write).
//
// Verdict vocabulary intentionally mirrors the Tassadar verification-class
// vocabulary (CONFIRMED/REFUTED/INCONCLUSIVE) so a statistical/uncertain result
// is never inflated to CONFIRMED.

import type { QaRunStep } from "./result";

// ---------------------------------------------------------------------------
// Commitments — declared up front (what must be proven + the evidence type).
// ---------------------------------------------------------------------------

// The kind of evidence a commitment is satisfied by. Today the runner produces
// step-level evidence (each scripted/inferred step is recorded ok|failed); the
// `evidence` discriminant names WHICH observation backs the commitment so the
// verifier never confuses "I asserted X" with "I observed an unrelated step".
export type Commitment =
  // The commitment is proven by a specific step PASSING. `match` selects the
  // step by (a substring of) its label; `kind` optionally narrows by step kind
  // (e.g. only an "assert" step). Confirmed when a matching step is `ok`;
  // refuted when a matching step is `failed`; inconclusive when no step matches.
  | {
      readonly id: string;
      readonly claim: string;
      readonly evidence: "step-pass";
      /** Substring matched against a step's label (case-insensitive). */
      readonly match: string;
      /** Optionally require the matching step to be of this kind. */
      readonly kind?: string;
    }
  // The commitment is proven by the OVERALL run passing (no honest failure).
  // Confirmed when the run's status is "pass"; refuted when it is "fail".
  | {
      readonly id: string;
      readonly claim: string;
      readonly evidence: "run-pass";
    };

export type Verdict = "CONFIRMED" | "REFUTED" | "INCONCLUSIVE";

// One commitment's investigated outcome, with the OBSERVED evidence that backs
// it. `evidenceSummary` is a public-safe, one-line account of what was observed
// (the matching step's label + status, or the run status) so a reviewer can see
// WHY the verdict landed without re-running anything.
export interface CommitmentFinding {
  readonly id: string;
  readonly claim: string;
  readonly verdict: Verdict;
  /** Public-safe one-line account of the observed evidence (or its absence). */
  readonly evidenceSummary: string;
}

// The whole verify stage's report: the rolled-up verdict + per-commitment
// findings. The roll-up is the STRICTEST honest verdict across commitments:
//   - any REFUTED      -> REFUTED (a single contradiction refutes the run)
//   - else any INCONCLUSIVE -> INCONCLUSIVE (uncertainty is never rounded up)
//   - else (all CONFIRMED, and at least one commitment) -> CONFIRMED
//   - no commitments   -> INCONCLUSIVE (nothing was committed to, so nothing is
//                         confirmed; never a vacuous CONFIRMED).
export interface VerifyReport {
  readonly verdict: Verdict;
  readonly findings: ReadonlyArray<CommitmentFinding>;
  /** True only when EVERY finding is backed by observed evidence (anti-fabrication). */
  readonly observed: boolean;
}

// ---------------------------------------------------------------------------
// The verify stage (pure): commitments x produced steps + run status -> report.
// ---------------------------------------------------------------------------

const findStep = (
  steps: ReadonlyArray<QaRunStep>,
  match: string,
  kind?: string,
): QaRunStep | undefined => {
  const needle = match.toLowerCase();
  return steps.find(
    (s) =>
      s.label.toLowerCase().includes(needle) &&
      (kind === undefined || s.kind === kind),
  );
};

const investigate = (
  commitment: Commitment,
  steps: ReadonlyArray<QaRunStep>,
  runStatus: "pass" | "fail",
): CommitmentFinding => {
  if (commitment.evidence === "run-pass") {
    // Observed evidence: the overall run outcome. Pass confirms; fail refutes.
    return runStatus === "pass"
      ? {
          id: commitment.id,
          claim: commitment.claim,
          verdict: "CONFIRMED",
          evidenceSummary: "run completed with status=pass (no honest failure)",
        }
      : {
          id: commitment.id,
          claim: commitment.claim,
          verdict: "REFUTED",
          evidenceSummary: "run completed with status=fail (contradicting evidence)",
        };
  }

  // evidence === "step-pass"
  const step = findStep(steps, commitment.match, commitment.kind);
  if (step === undefined) {
    // ANTI-FABRICATION: no step bears on this commitment, so it was NOT observed
    // in this run. We never report an unobserved outcome as CONFIRMED.
    return {
      id: commitment.id,
      claim: commitment.claim,
      verdict: "INCONCLUSIVE",
      evidenceSummary: `no step matched "${commitment.match}"${commitment.kind ? ` (kind=${commitment.kind})` : ""} — outcome not observed`,
    };
  }
  return step.status === "ok"
    ? {
        id: commitment.id,
        claim: commitment.claim,
        verdict: "CONFIRMED",
        evidenceSummary: `observed step "${step.label}" = ok`,
      }
    : {
        id: commitment.id,
        claim: commitment.claim,
        verdict: "REFUTED",
        evidenceSummary: `observed step "${step.label}" = failed (contradicting evidence)`,
      };
};

const rollUp = (findings: ReadonlyArray<CommitmentFinding>): Verdict => {
  if (findings.length === 0) return "INCONCLUSIVE";
  if (findings.some((f) => f.verdict === "REFUTED")) return "REFUTED";
  if (findings.some((f) => f.verdict === "INCONCLUSIVE")) return "INCONCLUSIVE";
  return "CONFIRMED";
};

/**
 * Run the verify stage: check each commitment against the produced steps + run
 * status, emit per-commitment findings, and roll up to a single investigator
 * verdict. Pure (no IO). Anti-fabrication is enforced structurally:
 *   - CONFIRMED requires an OBSERVED ok step (or run-pass).
 *   - a contradicting observation is REFUTED, not swallowed.
 *   - an unobserved commitment is INCONCLUSIVE, never CONFIRMED.
 */
export const verifyCommitments = (input: {
  readonly commitments: ReadonlyArray<Commitment>;
  readonly steps: ReadonlyArray<QaRunStep>;
  readonly runStatus: "pass" | "fail";
}): VerifyReport => {
  const findings = input.commitments.map((c) =>
    investigate(c, input.steps, input.runStatus),
  );
  return {
    verdict: rollUp(findings),
    findings,
    // Observed = every finding rests on observed evidence (none INCONCLUSIVE for
    // lack of evidence). A CONFIRMED verdict is always observed; this flag also
    // exposes whether a REFUTED/INCONCLUSIVE mix had any unobserved commitment.
    observed: findings.every((f) => f.verdict !== "INCONCLUSIVE"),
  };
};

// ---------------------------------------------------------------------------
// Rendering helpers (public-safe, pure) — shared by pr-comment + CLIs.
// ---------------------------------------------------------------------------

const VERDICT_EMOJI: Readonly<Record<Verdict, string>> = {
  CONFIRMED: "✅",
  REFUTED: "❌",
  INCONCLUSIVE: "⚠️",
};

/** A one-line verdict summary for a PR comment / console (no fabrication). */
export const renderVerdictLine = (report: VerifyReport): string => {
  const n = report.findings.length;
  const refuted = report.findings.filter((f) => f.verdict === "REFUTED").length;
  const confirmed = report.findings.filter(
    (f) => f.verdict === "CONFIRMED",
  ).length;
  const counts =
    n === 0
      ? "no commitments declared"
      : `${confirmed}/${n} confirmed${refuted > 0 ? `, ${refuted} refuted` : ""}`;
  return `${VERDICT_EMOJI[report.verdict]} **Verify verdict: ${report.verdict}** — ${counts}`;
};

/** The contradicting/observed evidence lines for a PR comment (details block). */
export const renderVerdictEvidence = (
  report: VerifyReport,
): ReadonlyArray<string> =>
  report.findings.map(
    (f) => `- ${VERDICT_EMOJI[f.verdict]} \`${f.id}\` — ${f.claim}: ${f.evidenceSummary}`,
  );
