import { describe, expect, it } from "vite-plus/test";

import { forensicSha256Digest, strictDecode } from "@openagentsinc/forensic-contract";

import {
  LOUPE_INITIAL_VERDICT_VERSION,
  LOUPE_VERIFICATION_EVIDENCE_VERSION,
  LOUPE_VERIFICATION_PLAN_VERSION,
  LoupeInitialVerdictSchema,
  LoupeVerificationEvidenceSchema,
  LoupeVerificationPlanSchema,
  deriveControlTestOutcome,
  evaluateLoupeVerificationReleaseGate,
  type LoupeVerificationEvidence,
  type LoupeVerificationPlan,
} from "../src/verifier.ts";
import {
  LOUPE_VERIFICATION_SESSION_BRAND,
  evaluateLoupeVerificationSession,
  type LoupeVerificationSession,
} from "../src/verification-contract.ts";

/**
 * Schema-level and evaluator-level gates.
 *
 * The driver in `verifier.test.ts` never produces the shapes below, because it
 * derives them. They are kept enforceable anyway: a verification record that
 * arrives from somewhere else — a future orchestrator, a replayed archive, an
 * Omega envelope — must not be able to assert what only measurement can decide.
 */

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
    evidenceProvenance: "conformance_vector",
    sourceBundleRef: "bundle.coldcard.complete-vulnerable.v1",
    sourceBundleDigest: digest("2"),
    dependencyManifestDigest: digest("3"),
    vulnerableTargetDigest: digest("4"),
    fixedTargetDigest: digest("5"),
    workerImageDigest: digest("6"),
    workerProfileDigest: digest("7"),
    admittedWorkers: [
      {
        workerRef: "worker.coldcard.mechanical.v1",
        sandboxRef: "sandbox.live_gce.forensic.coldcard.v1",
        resourceGeneration: 7,
        placementRef: "placement.gce.forensic.coldcard.v1",
      },
    ],
    createdAt: "2026-08-01T16:00:00.000Z",
    ...overrides,
  });

const evidence = (
  verificationPlan: LoupeVerificationPlan,
  sequence: number,
  operation: LoupeVerificationEvidence["operation"],
  overrides: Record<string, unknown> = {},
): LoupeVerificationEvidence =>
  strictDecode(LoupeVerificationEvidenceSchema, {
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
    inputDigests: [verificationPlan.sourceBundleDigest, verificationPlan.dependencyManifestDigest],
    outcome: "succeeded",
    resultDigest: digest("9"),
    environmentDigest: forensicSha256Digest({
      workerImageDigest: verificationPlan.workerImageDigest,
      workerProfileDigest: verificationPlan.workerProfileDigest,
    }),
    ...(operation === "poc_applied" || operation === "control_test_observed"
      ? {
          workerRef: verificationPlan.admittedWorkers[0]?.workerRef ?? "worker.absent",
          workerReceiptRef: `worker.receipt.${sequence}`,
        }
      : {}),
    observedAt: `2026-08-01T16:00:0${sequence}.000Z`,
    ...overrides,
  });

describe("Loupe verification contract", () => {
  it("refuses a session the adapter did not open against a control plane", async () => {
    // The last open OFR-007 blocker in one test. Before this, a caller passed a
    // `LoupeVerificationBackend`: five functions supplying both the evidence and
    // the authority that validated it. This object is exactly that backend, in
    // the shape the evaluator now expects, and it is refused because the brand
    // it lacks is only stamped by `openLoupeVerificationSession`.
    const forged = {
      evidenceProvenance: "admitted_worker_run",
      originRef: "control-plane.live.forged.v1",
      plan: plan({ evidenceProvenance: "admitted_worker_run" }),
      collectMechanicalEvidence: async () => [],
      submitInitialVerdict: async () => ({
        verdictRef: "verdict.forged.v1",
        outcome: "confirmed" as const,
        rationaleDigest: digest("a"),
        lockedAt: "2026-08-01T16:00:04.000Z",
      }),
      applyPocAndRunControls: async () => [],
      resolveAdmittedWorkerReceipt: async () => undefined,
      commitInitialVerdict: async (candidate: unknown) => candidate,
    } as unknown as LoupeVerificationSession;

    await expect(
      evaluateLoupeVerificationSession(forged, "2026-08-01T16:00:07.000Z"),
    ).rejects.toThrow("not a supplied backend");

    // And the brand alone is not enough: the plan cannot claim a provenance the
    // session did not observe.
    await expect(
      evaluateLoupeVerificationSession(
        {
          ...forged,
          [LOUPE_VERIFICATION_SESSION_BRAND]: true,
          evidenceProvenance: "conformance_vector",
        } as unknown as LoupeVerificationSession,
        "2026-08-01T16:00:07.000Z",
      ),
    ).rejects.toThrow("cannot claim a provenance its session did not observe");
  });

  it("rejects circular discovery self-confirmation", () => {
    expect(() =>
      strictDecode(LoupeVerificationPlanSchema, {
        ...plan(),
        verifierActorRef: "actor.discovery.coldcard.v1",
      }),
    ).toThrow("distinct execution identities");
  });

  it("does not admit an unexecuted PoC as executed evidence", () => {
    const verificationPlan = plan();
    expect(() =>
      strictDecode(LoupeVerificationEvidenceSchema, {
        ...evidence(verificationPlan, 4, "poc_applied", {
          inputDigests: [verificationPlan.vulnerableTargetDigest],
        }),
        evidenceTier: "executed",
      }),
    ).toThrow("PoC application is artifact evidence");
    expect(() =>
      strictDecode(LoupeVerificationEvidenceSchema, {
        ...evidence(verificationPlan, 5, "control_test_observed", {
          inputDigests: [verificationPlan.vulnerableTargetDigest],
          controlRevision: "vulnerable",
          expectedTestOutcome: "failure",
          observedTermination: { status: "observed", exitStatus: 1 },
        }),
        observedTermination: { status: "not_observed" },
      }),
    ).toThrow("executed control evidence");
  });

  it("derives a control outcome instead of accepting the one a producer asserts", () => {
    const verificationPlan = plan();
    const control = (termination: Record<string, unknown>) =>
      evidence(verificationPlan, 5, "control_test_observed", {
        inputDigests: [verificationPlan.vulnerableTargetDigest],
        controlRevision: "vulnerable",
        expectedTestOutcome: "failure",
        observedTermination: termination,
      });

    expect(deriveControlTestOutcome(control({ status: "observed", exitStatus: 2 }))).toBe("failure");
    expect(
      deriveControlTestOutcome(
        control({ status: "observed", exitStatus: 0, resultArtifactDigest: digest("c") }),
      ),
    ).toBe("success");
    // A producer could report a clean exit and no result at all, and the old
    // shape let it call that a passing control.
    expect(deriveControlTestOutcome(control({ status: "observed", exitStatus: 0 }))).toBe(
      "not_observed",
    );
  });

  it("refuses a worker receipt ref with no worker to attribute it to", () => {
    const verificationPlan = plan();
    const { workerRef: _dropped, ...withoutWorkerRef } = evidence(
      verificationPlan,
      4,
      "poc_applied",
      { inputDigests: [verificationPlan.vulnerableTargetDigest] },
    );
    expect(() => strictDecode(LoupeVerificationEvidenceSchema, withoutWorkerRef)).toThrow(
      "must name the admitted worker",
    );
  });

  it("requires unique initial-verdict evidence refs", () => {
    const verificationPlan = plan();
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

  it("refuses a release gate over a conformance result that claims independent verification", () => {
    expect(() =>
      evaluateLoupeVerificationReleaseGate({
        gateRef: "gate.verification.forged.v1",
        results: [
          {
            schema: "openagents.loupe_verification_result.v1",
            resultRef: "result.forged.v1",
            verificationRef: "verification.forged.v1",
            findingRef: "finding.forged.v1",
            discoveryActorRef: "actor.discovery.forged.v1",
            verifierActorRef: "actor.verifier.forged.v1",
            outcome: "confirmed",
            evidenceProvenance: "conformance_vector",
            evidenceOriginRef: "control-plane.conformance.forged",
            evidenceTier: "independently_verified",
            initialVerdictRef: "verdict.forged.v1",
            initialVerdictDigest: digest("1"),
            initialVerdictAuthority: "durable_first_verdict_ledger",
            mechanicalEvidenceReceiptRefs: ["receipt.1", "receipt.2", "receipt.3"],
            evidenceReceiptRefs: [
              "receipt.1",
              "receipt.2",
              "receipt.3",
              "receipt.4",
              "receipt.5",
              "receipt.6",
            ],
            evidenceAggregateDigest: digest("2"),
            pocReceiptRef: "receipt.4",
            vulnerableControlReceiptRef: "receipt.5",
            fixedControlReceiptRef: "receipt.6",
            derivedVulnerableTestOutcome: "failure",
            derivedFixedTestOutcome: "success",
            vulnerableControlPassed: true,
            fixedControlPassed: true,
            circularVerificationRejected: true,
            admittedWorkerReceiptsResolved: true,
            completionAuthority: "adapter_atomic_result",
            productMode: "discovery_only",
            completedAt: "2026-08-01T16:00:08.000Z",
          },
        ],
        evaluatedAt: "2026-08-01T16:00:08.000Z",
      }),
    ).toThrow("conformance-vector evidence can never be independently verified");
  });
});

/**
 * A branded session built by hand, so the evaluator's own gates can be
 * exercised on records the driver cannot produce. A future orchestrator, a
 * replayed archive, or an Omega envelope could produce them, and the evaluator
 * must still refuse what only measurement can decide.
 */
const brandedSession = (options: {
  readonly plan: LoupeVerificationPlan;
  readonly mechanical: ReadonlyArray<unknown>;
  readonly controls: ReadonlyArray<unknown>;
  readonly lockedAt: string;
}) => {
  const worker = options.plan.admittedWorkers[0];
  return {
    [LOUPE_VERIFICATION_SESSION_BRAND]: true,
    evidenceProvenance: options.plan.evidenceProvenance,
    originRef: "control-plane.conformance.contract-test",
    plan: options.plan,
    collectMechanicalEvidence: async () => options.mechanical,
    submitInitialVerdict: async () => ({
      verdictRef: "verdict.contract.v1",
      outcome: "confirmed" as const,
      rationaleDigest: digest("a"),
      lockedAt: options.lockedAt,
    }),
    applyPocAndRunControls: async () => options.controls,
    resolveAdmittedWorkerReceipt: async (workerReceiptRef: string) => ({
      schema: "openagents.loupe_admitted_worker_receipt.v1",
      receiptRef: workerReceiptRef,
      sandboxRef: worker?.sandboxRef ?? "sandbox.absent",
      resourceGeneration: worker?.resourceGeneration ?? 1,
      placementRef: worker?.placementRef ?? "placement.absent",
      imageDigest: options.plan.workerImageDigest,
      profileDigest: options.plan.workerProfileDigest,
      lifecycleState: "admitted",
      exact: true,
      observedAt: "2026-08-01T15:59:00.000Z",
      expiresAt: "2026-08-01T17:00:00.000Z",
    }),
    commitInitialVerdict: async (candidate: unknown) => candidate,
  } as unknown as LoupeVerificationSession;
};

const mechanicalEvidence = (verificationPlan: LoupeVerificationPlan) => [
  evidence(verificationPlan, 1, "source_ref_resolved"),
  evidence(verificationPlan, 2, "macro_value_observed"),
  evidence(verificationPlan, 3, "symbol_provider_resolved"),
];

const controlEvidence = (verificationPlan: LoupeVerificationPlan, observedAt: string) => [
  evidence(verificationPlan, 4, "poc_applied", {
    inputDigests: [verificationPlan.vulnerableTargetDigest],
    observedAt,
  }),
  evidence(verificationPlan, 5, "control_test_observed", {
    inputDigests: [verificationPlan.vulnerableTargetDigest],
    controlRevision: "vulnerable",
    expectedTestOutcome: "failure",
    observedTermination: { status: "observed", exitStatus: 1 },
    observedAt,
  }),
  evidence(verificationPlan, 6, "control_test_observed", {
    inputDigests: [verificationPlan.fixedTargetDigest],
    controlRevision: "fixed",
    expectedTestOutcome: "success",
    observedTermination: { status: "observed", exitStatus: 0, resultArtifactDigest: digest("c") },
    observedAt,
  }),
];

describe("evidence order the evaluator refuses whatever produced it", () => {
  it("refuses control evidence observed at or before the verdict lock", async () => {
    const verificationPlan = plan();
    await expect(
      evaluateLoupeVerificationSession(
        brandedSession({
          plan: verificationPlan,
          mechanical: mechanicalEvidence(verificationPlan),
          // Observed a second BEFORE the lock that is supposed to precede them.
          controls: controlEvidence(verificationPlan, "2026-08-01T16:00:03.000Z"),
          lockedAt: "2026-08-01T16:00:04.000Z",
        }),
        "2026-08-01T16:00:09.000Z",
      ),
    ).rejects.toThrow("observed after the verdict lock");
  });

  it("refuses mechanical evidence observed after the verdict that rests on it", async () => {
    const verificationPlan = plan();
    await expect(
      evaluateLoupeVerificationSession(
        brandedSession({
          plan: verificationPlan,
          mechanical: mechanicalEvidence(verificationPlan),
          controls: controlEvidence(verificationPlan, "2026-08-01T16:00:05.000Z"),
          lockedAt: "2026-08-01T16:00:02.000Z",
        }),
        "2026-08-01T16:00:09.000Z",
      ),
    ).rejects.toThrow("cannot be locked before its mechanical evidence");
  });

  it("admits the same records once the controls follow the lock, and still caps them", async () => {
    const verificationPlan = plan();
    const result = await evaluateLoupeVerificationSession(
      brandedSession({
        plan: verificationPlan,
        mechanical: mechanicalEvidence(verificationPlan),
        controls: controlEvidence(verificationPlan, "2026-08-01T16:00:05.000Z"),
        lockedAt: "2026-08-01T16:00:04.000Z",
      }),
      "2026-08-01T16:00:09.000Z",
    );
    expect(result.vulnerableControlPassed).toBe(true);
    expect(result.fixedControlPassed).toBe(true);
    // Conformance provenance still cannot carry a confirmation out, however
    // well ordered the records are.
    expect(result.outcome).toBe("inconclusive");
  });
});
