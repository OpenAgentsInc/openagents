import { describe, expect, it } from "vite-plus/test";

import { forensicSha256Digest, strictDecode } from "@openagentsinc/forensic-contract";

import {
  LOUPE_INITIAL_VERDICT_VERSION,
  LOUPE_VERIFICATION_EVIDENCE_VERSION,
  LOUPE_VERIFICATION_PLAN_VERSION,
  LoupeInitialVerdictSchema,
  LoupeVerificationEvidenceSchema,
  LoupeVerificationPlanSchema,
  evaluateLoupeVerificationReleaseGate,
  executeLoupeVerification,
  type LoupeVerificationBackend,
  type LoupeVerificationEvidence,
  type LoupeVerificationPlan,
} from "../src/verifier.ts";

const digest = (character: string) => `sha256:${character.repeat(64)}`;

const plan = (overrides: Record<string, unknown> = {}): LoupeVerificationPlan =>
  strictDecode(LoupeVerificationPlanSchema, {
    schema: LOUPE_VERIFICATION_PLAN_VERSION,
    verificationRef: "verification.coldcard.rng.v1",
    runRef: "run.coldcard.complete-vulnerable.v1",
    findingRef: "finding.coldcard.rng-fallback.v1",
    findingDigest: digest("1"),
    discoveryActorRef: "actor.discovery.coldcard.v1",
    verifierActorRef: "actor.verifier.coldcard.v1",
    sourceBundleRef: "bundle.coldcard.complete-vulnerable.v1",
    sourceBundleDigest: digest("2"),
    dependencyManifestDigest: digest("3"),
    vulnerableTargetDigest: digest("4"),
    fixedTargetDigest: digest("5"),
    workerImageDigest: digest("6"),
    workerProfileDigest: digest("7"),
    workerPlacementRef: "placement.gce.forensic.coldcard.v1",
    createdAt: "2026-08-01T16:00:00.000Z",
    ...overrides,
  });

const workerEnvironmentDigest = (verificationPlan: LoupeVerificationPlan) =>
  forensicSha256Digest({
    workerImageDigest: verificationPlan.workerImageDigest,
    workerProfileDigest: verificationPlan.workerProfileDigest,
    workerPlacementRef: verificationPlan.workerPlacementRef,
  });

const evidence = (
  verificationPlan: LoupeVerificationPlan,
  sequence: number,
  operation: LoupeVerificationEvidence["operation"],
  overrides: Record<string, unknown> = {},
): LoupeVerificationEvidence => {
  const controlRevision = overrides.controlRevision as "vulnerable" | "fixed" | undefined;
  const defaultInputs =
    operation === "poc_applied"
      ? [verificationPlan.vulnerableTargetDigest, verificationPlan.findingDigest]
      : operation === "control_test_observed"
        ? [
            controlRevision === "fixed"
              ? verificationPlan.fixedTargetDigest
              : verificationPlan.vulnerableTargetDigest,
          ]
        : [verificationPlan.sourceBundleDigest, verificationPlan.dependencyManifestDigest];
  return strictDecode(LoupeVerificationEvidenceSchema, {
    schema: LOUPE_VERIFICATION_EVIDENCE_VERSION,
    receiptRef: `receipt.verification.${sequence}`,
    verificationRef: verificationPlan.verificationRef,
    sequence,
    operation,
    evidenceTier:
      operation === "control_test_observed"
        ? "executed"
        : operation === "poc_applied"
          ? "artifact_observed"
          : "source_observed",
    subjectRef: `subject.verification.${sequence}`,
    commandDigest: digest("8"),
    inputDigests: defaultInputs,
    outcome: "succeeded",
    resultDigest: digest("9"),
    environmentDigest: workerEnvironmentDigest(verificationPlan),
    ...(operation === "poc_applied" || operation === "control_test_observed"
      ? { workerReceiptRef: `worker.receipt.${sequence}` }
      : {}),
    observedAt: `2026-08-01T16:00:0${sequence}.000Z`,
    ...overrides,
  });
};

const mechanicalEvidence = (verificationPlan: LoupeVerificationPlan) => [
  evidence(verificationPlan, 1, "source_ref_resolved"),
  evidence(verificationPlan, 2, "macro_value_observed"),
  evidence(verificationPlan, 3, "symbol_provider_resolved"),
];

const controlEvidence = (
  verificationPlan: LoupeVerificationPlan,
  fixedOutcome: "failure" | "success" = "success",
) => [
  evidence(verificationPlan, 4, "poc_applied"),
  evidence(verificationPlan, 5, "control_test_observed", {
    controlRevision: "vulnerable",
    expectedTestOutcome: "failure",
    observedTestOutcome: "failure",
  }),
  evidence(verificationPlan, 6, "control_test_observed", {
    controlRevision: "fixed",
    expectedTestOutcome: "success",
    observedTestOutcome: fixedOutcome,
  }),
];

const backend = (
  verificationPlan: LoupeVerificationPlan,
  timeline: Array<string>,
  verdictOutcome: "confirmed" | "dismissed" | "inconclusive" = "confirmed",
  controls = controlEvidence(verificationPlan),
): LoupeVerificationBackend => ({
  collectMechanicalEvidence: async (receivedPlan) => {
    expect(receivedPlan).toStrictEqual(verificationPlan);
    expect(Object.isFrozen(receivedPlan)).toBe(true);
    timeline.push("mechanical_evidence");
    return mechanicalEvidence(verificationPlan);
  },
  submitInitialVerdict: async (_receivedPlan, receipts) => {
    expect(Object.isFrozen(receipts)).toBe(true);
    timeline.push("initial_verdict");
    return {
      verdictRef: "verdict.coldcard.initial.v1",
      outcome: verdictOutcome,
      rationaleDigest: digest("a"),
      lockedAt: "2026-08-01T16:00:03.500Z",
    };
  },
  applyPocAndRunControls: async (_receivedPlan, lockedVerdict) => {
    expect(Object.isFrozen(lockedVerdict)).toBe(true);
    expect(Object.isFrozen(lockedVerdict.evidenceReceiptRefs)).toBe(true);
    expect(lockedVerdict.schema).toBe(LOUPE_INITIAL_VERDICT_VERSION);
    timeline.push("poc_and_controls");
    return controls;
  },
});

describe("Loupe forensic verifier", () => {
  it("locks exactly one independent verdict before the Coldcard vulnerable/fixed controls", async () => {
    const verificationPlan = plan();
    const timeline: Array<string> = [];
    let verdictCalls = 0;
    const verifierBackend = backend(verificationPlan, timeline);
    const result = await executeLoupeVerification(
      verificationPlan,
      {
        ...verifierBackend,
        submitInitialVerdict: async (...arguments_) => {
          verdictCalls += 1;
          return verifierBackend.submitInitialVerdict(...arguments_);
        },
      },
      "2026-08-01T16:00:07.000Z",
    );

    expect(timeline).toEqual(["mechanical_evidence", "initial_verdict", "poc_and_controls"]);
    expect(verdictCalls).toBe(1);
    expect(result.outcome).toBe("confirmed");
    expect(result.evidenceTier).toBe("independently_verified");
    expect(result.vulnerableControlPassed).toBe(true);
    expect(result.fixedControlPassed).toBe(true);
    expect(result.completionAuthority).toBe("adapter_atomic_result");
    expect(result.productMode).toBe("discovery_only");

    const gate = evaluateLoupeVerificationReleaseGate({
      gateRef: "gate.verification.coldcard.v1",
      results: [result],
      evaluatedAt: "2026-08-01T16:00:08.000Z",
    });
    expect(gate.productMode).toBe("independent_verification");
    expect(gate.blockerRefs).toEqual([]);
  });

  it("rejects circular discovery self-confirmation before invoking a backend", async () => {
    const untrustedPlan = {
      ...plan(),
      verifierActorRef: "actor.discovery.coldcard.v1",
    };
    let called = false;
    await expect(
      executeLoupeVerification(
        untrustedPlan,
        {
          collectMechanicalEvidence: async () => {
            called = true;
            return [];
          },
          submitInitialVerdict: async () => {
            throw new Error("unreachable");
          },
          applyPocAndRunControls: async () => {
            throw new Error("unreachable");
          },
        },
        "2026-08-01T16:00:07.000Z",
      ),
    ).rejects.toThrow("distinct execution identities");
    expect(called).toBe(false);
  });

  it("rejects patch work before the immutable initial verdict", async () => {
    const verificationPlan = plan();
    let verdictCalls = 0;
    await expect(
      executeLoupeVerification(
        verificationPlan,
        {
          collectMechanicalEvidence: async () => [
            ...mechanicalEvidence(verificationPlan),
            evidence(verificationPlan, 4, "poc_applied"),
          ],
          submitInitialVerdict: async () => {
            verdictCalls += 1;
            throw new Error("unreachable");
          },
          applyPocAndRunControls: async () => [],
        },
        "2026-08-01T16:00:07.000Z",
      ),
    ).rejects.toThrow("cannot precede");
    expect(verdictCalls).toBe(0);
  });

  it("does not admit an unexecuted PoC as executed evidence", () => {
    const verificationPlan = plan();
    expect(() =>
      strictDecode(LoupeVerificationEvidenceSchema, {
        ...evidence(verificationPlan, 4, "poc_applied"),
        evidenceTier: "executed",
      }),
    ).toThrow("PoC application is artifact evidence");
    expect(() =>
      strictDecode(LoupeVerificationEvidenceSchema, {
        ...evidence(verificationPlan, 5, "control_test_observed", {
          controlRevision: "vulnerable",
          expectedTestOutcome: "failure",
          observedTestOutcome: "failure",
        }),
        observedTestOutcome: "not_run",
      }),
    ).toThrow("executed control evidence");
  });

  it("keeps a failed fixed control inconclusive and discovery-only", async () => {
    const verificationPlan = plan();
    const timeline: Array<string> = [];
    const result = await executeLoupeVerification(
      verificationPlan,
      backend(
        verificationPlan,
        timeline,
        "confirmed",
        controlEvidence(verificationPlan, "failure"),
      ),
      "2026-08-01T16:00:07.000Z",
    );
    expect(result.outcome).toBe("inconclusive");
    expect(result.evidenceTier).toBe("executed");
    expect(result.fixedControlPassed).toBe(false);

    const gate = evaluateLoupeVerificationReleaseGate({
      gateRef: "gate.verification.coldcard.failed.v1",
      results: [result],
      evaluatedAt: "2026-08-01T16:00:08.000Z",
    });
    expect(gate.productMode).toBe("discovery_only");
    expect(gate.blockerRefs).toContain("blocker.verification.fixedControlPassed");
  });

  it.each(["dismissed", "inconclusive"] as const)(
    "retains a distinct %s verdict without generating or applying a patch",
    async (verdictOutcome) => {
      const verificationPlan = plan();
      const timeline: Array<string> = [];
      const result = await executeLoupeVerification(
        verificationPlan,
        backend(verificationPlan, timeline, verdictOutcome),
        "2026-08-01T16:00:07.000Z",
      );
      expect(result.outcome).toBe(verdictOutcome);
      expect(result.evidenceTier).toBe("source_observed");
      expect(timeline).toEqual(["mechanical_evidence", "initial_verdict"]);
    },
  );

  it("requires source, macro, and symbol-provider receipts before verdict submission", async () => {
    const verificationPlan = plan();
    await expect(
      executeLoupeVerification(
        verificationPlan,
        {
          ...backend(verificationPlan, []),
          collectMechanicalEvidence: async () => mechanicalEvidence(verificationPlan).slice(0, 2),
        },
        "2026-08-01T16:00:07.000Z",
      ),
    ).rejects.toThrow("requires source-ref, macro, and symbol-provider evidence");
  });

  it("binds all receipts to immutable source, dependency, target, and worker inputs", () => {
    const verificationPlan = plan();
    const receipt = evidence(verificationPlan, 1, "source_ref_resolved");
    const decoded = strictDecode(LoupeVerificationEvidenceSchema, receipt);
    expect(decoded.inputDigests).toContain(verificationPlan.sourceBundleDigest);
    expect(decoded.inputDigests).toContain(verificationPlan.dependencyManifestDigest);

    expect(() =>
      strictDecode(LoupeInitialVerdictSchema, {
        schema: LOUPE_INITIAL_VERDICT_VERSION,
        verdictRef: "verdict.duplicate-evidence.v1",
        verificationRef: verificationPlan.verificationRef,
        findingDigest: verificationPlan.findingDigest,
        verifierActorRef: verificationPlan.verifierActorRef,
        outcome: "confirmed",
        evidenceReceiptRefs: ["receipt.one", "receipt.one", "receipt.three"],
        rationaleDigest: digest("b"),
        lockedAt: "2026-08-01T16:00:04.000Z",
      }),
    ).toThrow("must be unique");
  });
});
