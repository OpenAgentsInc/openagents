// QA failure learning — Blueprint/GEPA half (#6195). Pure, no IO.
//
// Proves: a captured failure pattern emits a claim-level GEPA candidate-feedback
// signal that is evidence-only + governed (Release-Gate, no self-promotion); the
// Release Gate REJECTS it unapproved (no `promoted: true` path); the governance
// guard fails closed; the signal is public-safe (refs/summaries only).

import { describe, expect, test } from "bun:test";

import { captureFailurePattern } from "./failure-learning";
import {
  assertGepaCandidateFeedbackGoverned,
  emitGepaCandidateFeedback,
  evaluateGepaReleaseGateWithoutApproval,
  GEPA_CANDIDATE_FEEDBACK_SCHEMA_VERSION,
  GepaCandidateFeedbackGovernanceError,
  QA_FAILURE_GEPA_MODEL,
  QA_FAILURE_GEPA_RELEASE_GATE_REF,
  type GepaCandidateFeedback,
} from "./failure-learning-gepa";
import { assertPublicSafeResult, type QaRunResult } from "./result";

const refutedResult = (): QaRunResult => ({
  schemaVersion: "openagents.qa_runner.result.v1",
  status: "fail",
  target: { name: "openagents.com", baseUrl: "https://example.test" },
  brain: "scripted",
  backend: "local",
  startedAt: "2026-06-24T00:00:00.000Z",
  endedAt: "2026-06-24T00:00:01.000Z",
  durationMs: 1000,
  steps: [
    {
      index: 0,
      kind: "assert",
      label: "redirects away from /login (intentionally wrong)",
      status: "failed",
    },
  ],
  artifacts: { screenshots: [] },
  failure: "assertion failed",
  verify: {
    verdict: "REFUTED",
    observed: true,
    findings: [
      {
        id: "claims-redirect",
        claim: "/login redirects away (FALSE claim under test)",
        verdict: "REFUTED",
        evidenceSummary: 'observed step "redirects away from /login" = failed',
      },
    ],
  },
});

const feedbackFromRefuted = (): GepaCandidateFeedback =>
  emitGepaCandidateFeedback({ pattern: captureFailurePattern(refutedResult())! });

describe("emitGepaCandidateFeedback (evidence-only, governed)", () => {
  test("emits a claim-level candidate-feedback signal in the brain-audit family", () => {
    const fb = feedbackFromRefuted();
    expect(fb.schemaVersion).toBe(GEPA_CANDIDATE_FEEDBACK_SCHEMA_VERSION);
    expect(fb.schemaVersion).toContain("psionic.probe_gepa_candidate");
    expect(fb.model).toBe(QA_FAILURE_GEPA_MODEL);
    expect(fb.trigger).toBe("verify_refuted");
    expect(fb.items).toHaveLength(1);
    expect(fb.items[0]!.claimId).toBe("claims-redirect");
    expect(fb.items[0]!.polarity).toBe("negative");
    expect(fb.feedbackRef).toContain("gepa_candidate_feedback:qa_runner:");
    expect(fb.sourcePatternRef).toContain("failure_pattern:qa_runner:");
  });

  test("the signal is governed: evidence-only, gated, never self-promoted, not live", () => {
    const fb = feedbackFromRefuted();
    expect(fb.governance.authorityBoundary).toBe("evidence_only");
    expect(fb.governance.requiresReleaseGate).toBe(true);
    expect(fb.governance.selfPromotionAllowed).toBe(false);
    expect(fb.governance.live).toBe(false);
    expect(fb.governance.releaseGateRef).toBe(QA_FAILURE_GEPA_RELEASE_GATE_REF);
    expect(() => assertGepaCandidateFeedbackGoverned(fb)).not.toThrow();
  });

  test("the signal is public-safe (refs/summaries only)", () => {
    expect(() => assertPublicSafeResult(feedbackFromRefuted())).not.toThrow();
  });
});

describe("Release Gate REJECTS an unapproved candidate (no self-promotion)", () => {
  test("evaluateGepaReleaseGateWithoutApproval never promotes", () => {
    const decision = evaluateGepaReleaseGateWithoutApproval(feedbackFromRefuted());
    expect(decision.promoted).toBe(false);
    expect(decision.reason).toContain("release_gate_rejected");
    expect(decision.reason).toContain("requires an explicit operator approval");
  });
});

describe("governance guard fails closed", () => {
  test("a self-promoting feedback throws", () => {
    const fb = feedbackFromRefuted();
    const tampered = {
      ...fb,
      governance: { ...fb.governance, selfPromotionAllowed: true as unknown as false },
    };
    expect(() => assertGepaCandidateFeedbackGoverned(tampered)).toThrow(
      GepaCandidateFeedbackGovernanceError,
    );
  });

  test("a write-authority feedback throws", () => {
    const fb = feedbackFromRefuted();
    const tampered = {
      ...fb,
      governance: {
        ...fb.governance,
        authorityBoundary: "write" as unknown as "evidence_only",
      },
    };
    expect(() => assertGepaCandidateFeedbackGoverned(tampered)).toThrow(
      GepaCandidateFeedbackGovernanceError,
    );
  });

  test("a positive-polarity item throws (failure learning never self-promotes a refinement)", () => {
    const fb = feedbackFromRefuted();
    const tampered = {
      ...fb,
      items: [{ ...fb.items[0]!, polarity: "positive" as unknown as "negative" }],
    };
    expect(() => assertGepaCandidateFeedbackGoverned(tampered)).toThrow(
      GepaCandidateFeedbackGovernanceError,
    );
  });
});
