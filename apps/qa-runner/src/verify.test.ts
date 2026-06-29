// Verify-stage tests (#6192): commitments -> investigator verdict, with the
// ANTI-FABRICATION contract pinned directly.
//
// The load-bearing properties:
//   - a FALSE claim (a commitment contradicted by an observed failed step) is
//     REFUTED, never a fake CONFIRMED — and the contradicting evidence is inline.
//   - an UNOBSERVED commitment (no step bears on it) is INCONCLUSIVE, never
//     CONFIRMED. A staged/expected-but-unobserved outcome cannot be confirmed.
//   - uncertainty is never rounded up: any INCONCLUSIVE keeps the roll-up off
//     CONFIRMED; any REFUTED makes the roll-up REFUTED.

import { describe, expect, test } from "bun:test";

import type { QaRunStep } from "./result";
import {
  type Commitment,
  renderVerdictEvidence,
  renderVerdictLine,
  verifyCommitments,
} from "./verify";

const step = (
  index: number,
  kind: string,
  label: string,
  status: "ok" | "failed",
): QaRunStep => ({ index, kind, label, status });

const PASSING_STEPS: ReadonlyArray<QaRunStep> = [
  step(0, "navigate", "open /login", "ok"),
  step(1, "wait-for", "sign-in form renders", "ok"),
  step(2, "assert", "stays at /login (no redirect to home)", "ok"),
  step(3, "assert", 'body contains "Log in to OpenAgents"', "ok"),
];

const WRONG_CLAIM_STEPS: ReadonlyArray<QaRunStep> = [
  step(0, "navigate", "open /login", "ok"),
  step(1, "wait-for", "sign-in form renders", "ok"),
  // the deliberately-wrong assertion the runner records as FAILED
  step(2, "assert", "redirects away from /login (intentionally wrong)", "failed"),
];

describe("verifyCommitments — CONFIRMED on observed evidence", () => {
  test("all commitments backed by observed ok steps -> CONFIRMED", () => {
    const commitments: ReadonlyArray<Commitment> = [
      {
        id: "no-redirect",
        claim: "stays at /login",
        evidence: "step-pass",
        match: "stays at /login",
        kind: "assert",
      },
      {
        id: "renders",
        claim: "renders sign-in copy",
        evidence: "step-pass",
        match: 'body contains "Log in to OpenAgents"',
      },
    ];
    const report = verifyCommitments({
      commitments,
      steps: PASSING_STEPS,
      runStatus: "pass",
    });
    expect(report.verdict).toBe("CONFIRMED");
    expect(report.observed).toBe(true);
    expect(report.findings.every(f => f.verdict === "CONFIRMED")).toBe(true);
  });

  test("run-pass commitment confirms on status=pass", () => {
    const report = verifyCommitments({
      commitments: [{ id: "ran", claim: "the run passed", evidence: "run-pass" }],
      steps: PASSING_STEPS,
      runStatus: "pass",
    });
    expect(report.verdict).toBe("CONFIRMED");
  });
});

describe("verifyCommitments — anti-fabrication", () => {
  test("a FALSE claim contradicted by an observed failed step is REFUTED (not a fake pass)", () => {
    const commitments: ReadonlyArray<Commitment> = [
      {
        id: "claims-redirect",
        claim: "/login redirects away (FALSE claim)",
        evidence: "step-pass",
        match: "redirects away from /login",
        kind: "assert",
      },
    ];
    const report = verifyCommitments({
      commitments,
      steps: WRONG_CLAIM_STEPS,
      runStatus: "fail",
    });
    expect(report.verdict).toBe("REFUTED");
    // the contradicting evidence is inline
    const finding = report.findings[0]!;
    expect(finding.verdict).toBe("REFUTED");
    expect(finding.evidenceSummary).toContain("failed");
    expect(finding.evidenceSummary).toContain("contradicting evidence");
  });

  test("an UNOBSERVED commitment is INCONCLUSIVE, never CONFIRMED", () => {
    const commitments: ReadonlyArray<Commitment> = [
      {
        id: "checkout",
        claim: "checkout succeeded",
        evidence: "step-pass",
        // nothing in the run bears on this claim
        match: "checkout completed",
      },
    ];
    const report = verifyCommitments({
      commitments,
      steps: PASSING_STEPS,
      runStatus: "pass",
    });
    expect(report.verdict).toBe("INCONCLUSIVE");
    expect(report.observed).toBe(false);
    expect(report.findings[0]!.verdict).toBe("INCONCLUSIVE");
    expect(report.findings[0]!.evidenceSummary).toContain("not observed");
  });

  test("a staged/expected-but-unobserved outcome cannot be reported as CONFIRMED even when the run passed", () => {
    // The run passed overall, but the specific claimed outcome was never
    // exercised. CONFIRMED must NOT be inferred from an unrelated green run.
    const report = verifyCommitments({
      commitments: [
        {
          id: "unrun",
          claim: "the export button works",
          evidence: "step-pass",
          match: "clicked export",
        },
      ],
      steps: PASSING_STEPS,
      runStatus: "pass",
    });
    expect(report.verdict).not.toBe("CONFIRMED");
    expect(report.verdict).toBe("INCONCLUSIVE");
  });

  test("uncertainty is never rounded up: a CONFIRMED + INCONCLUSIVE mix rolls up to INCONCLUSIVE", () => {
    const report = verifyCommitments({
      commitments: [
        {
          id: "observed",
          claim: "stays at /login",
          evidence: "step-pass",
          match: "stays at /login",
        },
        {
          id: "unobserved",
          claim: "logout works",
          evidence: "step-pass",
          match: "clicked logout",
        },
      ],
      steps: PASSING_STEPS,
      runStatus: "pass",
    });
    expect(report.verdict).toBe("INCONCLUSIVE");
  });

  test("any REFUTED dominates the roll-up", () => {
    const report = verifyCommitments({
      commitments: [
        { id: "ok", claim: "stays at /login", evidence: "step-pass", match: "stays at /login" },
        {
          id: "bad",
          claim: "redirects away",
          evidence: "step-pass",
          match: "redirects away from /login",
        },
      ],
      // mix a passing step set with the wrong-claim failed step
      steps: [...PASSING_STEPS, WRONG_CLAIM_STEPS[2]!],
      runStatus: "fail",
    });
    expect(report.verdict).toBe("REFUTED");
  });

  test("no commitments declared -> INCONCLUSIVE (never a vacuous CONFIRMED)", () => {
    const report = verifyCommitments({
      commitments: [],
      steps: PASSING_STEPS,
      runStatus: "pass",
    });
    expect(report.verdict).toBe("INCONCLUSIVE");
  });

  test("run-pass commitment is REFUTED on status=fail", () => {
    const report = verifyCommitments({
      commitments: [{ id: "ran", claim: "the run passed", evidence: "run-pass" }],
      steps: WRONG_CLAIM_STEPS,
      runStatus: "fail",
    });
    expect(report.verdict).toBe("REFUTED");
  });
});

describe("verdict renderers (public-safe, pure)", () => {
  test("renderVerdictLine summarizes counts honestly", () => {
    const report = verifyCommitments({
      commitments: [
        { id: "a", claim: "stays at /login", evidence: "step-pass", match: "stays at /login" },
        {
          id: "b",
          claim: "redirects away",
          evidence: "step-pass",
          match: "redirects away from /login",
        },
      ],
      steps: [...PASSING_STEPS, WRONG_CLAIM_STEPS[2]!],
      runStatus: "fail",
    });
    const line = renderVerdictLine(report);
    expect(line).toContain("Verify verdict: REFUTED");
    expect(line).toContain("1/2 confirmed");
    expect(line).toContain("1 refuted");
  });

  test("renderVerdictEvidence lists each finding with its claim + evidence", () => {
    const report = verifyCommitments({
      commitments: [
        {
          id: "claims-redirect",
          claim: "/login redirects away (FALSE claim)",
          evidence: "step-pass",
          match: "redirects away from /login",
        },
      ],
      steps: WRONG_CLAIM_STEPS,
      runStatus: "fail",
    });
    const lines = renderVerdictEvidence(report);
    expect(lines).toHaveLength(1);
    expect(lines[0]).toContain("claims-redirect");
    expect(lines[0]).toContain("FALSE claim");
    expect(lines[0]).toContain("failed");
  });
});
