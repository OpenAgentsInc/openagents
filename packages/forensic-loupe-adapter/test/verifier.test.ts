import { describe, expect, it } from "vite-plus/test";

import { forensicSha256Digest, strictDecode } from "@openagentsinc/forensic-contract";

import {
  LOUPE_ADMITTED_WORKER_RECEIPT_VERSION,
  LOUPE_INITIAL_VERDICT_VERSION,
  LOUPE_VERIFICATION_EVIDENCE_VERSION,
  LOUPE_VERIFICATION_PLAN_VERSION,
  LoupeInitialVerdictSchema,
  LoupeVerificationEvidenceSchema,
  LoupeVerificationPlanSchema,
  deriveControlTestOutcome,
  evaluateLoupeVerificationReleaseGate,
  executeLoupeVerification,
  type CommitInitialVerdict,
  type LoupeInitialVerdict,
  type LoupeVerificationBackend,
  type LoupeVerificationEvidence,
  type LoupeVerificationPlan,
  type ResolveAdmittedWorkerReceipt,
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

/**
 * The admitted-worker lifecycle authority every honest verification injects.
 * It knows exactly the receipt refs this plan's worker actually emitted.
 */
const admittedWorkerReceipt = (
  verificationPlan: LoupeVerificationPlan,
  receiptRef: string,
  overrides: Record<string, unknown> = {},
) => ({
  schema: LOUPE_ADMITTED_WORKER_RECEIPT_VERSION,
  receiptRef,
  sandboxRef: verificationPlan.admittedWorkers[0]?.sandboxRef ?? "sandbox.absent",
  resourceGeneration: verificationPlan.admittedWorkers[0]?.resourceGeneration ?? 1,
  placementRef: verificationPlan.admittedWorkers[0]?.placementRef ?? "placement.absent",
  imageDigest: verificationPlan.workerImageDigest,
  profileDigest: verificationPlan.workerProfileDigest,
  lifecycleState: "admitted",
  exact: true,
  observedAt: "2026-08-01T16:00:00.000Z",
  expiresAt: "2026-08-01T17:00:00.000Z",
  ...overrides,
});

/**
 * A durable first-verdict ledger, reduced to the only property that matters:
 * the first verdict written for a verification is the one every later reader
 * gets back. A real deployment backs this with durable storage; the contract
 * the verifier depends on is compare-and-set, not the storage medium.
 */
const firstVerdictLedger = (
  store: Map<string, LoupeInitialVerdict> = new Map(),
): CommitInitialVerdict =>
  async (verificationPlan, candidate) => {
    const existing = store.get(verificationPlan.verificationRef);
    if (existing !== undefined) return existing;
    store.set(verificationPlan.verificationRef, candidate);
    return candidate;
  };

const KNOWN_WORKER_RECEIPT_REFS = ["worker.receipt.4", "worker.receipt.5", "worker.receipt.6"];

const workerAuthority =
  (
    verificationPlan: LoupeVerificationPlan,
    overrides: Record<string, unknown> = {},
  ): ResolveAdmittedWorkerReceipt =>
  async (_receivedPlan, workerReceiptRef) =>
    KNOWN_WORKER_RECEIPT_REFS.includes(workerReceiptRef)
      ? admittedWorkerReceipt(verificationPlan, workerReceiptRef, overrides)
      : undefined;

const workerEnvironmentDigest = (verificationPlan: LoupeVerificationPlan) =>
  forensicSha256Digest({
    workerImageDigest: verificationPlan.workerImageDigest,
    workerProfileDigest: verificationPlan.workerProfileDigest,
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
      ? {
          workerRef: verificationPlan.admittedWorkers[0]?.workerRef ?? "worker.absent",
          workerReceiptRef: `worker.receipt.${sequence}`,
        }
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
    observedTermination: { status: "observed", exitStatus: 1 },
  }),
  evidence(verificationPlan, 6, "control_test_observed", {
    controlRevision: "fixed",
    expectedTestOutcome: "success",
    observedTermination:
      fixedOutcome === "success"
        ? { status: "observed", exitStatus: 0, resultArtifactDigest: digest("c") }
        : { status: "observed", exitStatus: 1 },
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
  resolveAdmittedWorkerReceipt: workerAuthority(verificationPlan),
  commitInitialVerdict: firstVerdictLedger(),
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
    expect(result.vulnerableControlPassed).toBe(true);
    expect(result.fixedControlPassed).toBe(true);
    expect(result.derivedVulnerableTestOutcome).toBe("failure");
    expect(result.derivedFixedTestOutcome).toBe("success");
    expect(result.completionAuthority).toBe("adapter_atomic_result");
    expect(result.initialVerdictAuthority).toBe("durable_first_verdict_ledger");
    expect(result.productMode).toBe("discovery_only");

    // THE FILE-LEVEL POINT. Everything above is a conformance vector: the
    // digests are repeated characters and the lifecycle authority is written a
    // few lines up by the same session that writes the evidence. It exercises
    // ordering, locking, and the control sequence, and it must never be able to
    // launder itself into an acceptance claim. Real acceptance evidence lives
    // in `verifier-live.test.ts`, which runs against admitted-worker artifacts.
    expect(result.evidenceProvenance).toBe("conformance_vector");
    expect(result.outcome).toBe("inconclusive");
    expect(result.evidenceTier).toBe("executed");

    const gate = evaluateLoupeVerificationReleaseGate({
      gateRef: "gate.verification.coldcard.v1",
      results: [result],
      evaluatedAt: "2026-08-01T16:00:08.000Z",
    });
    expect(gate.productMode).toBe("discovery_only");
    expect(gate.blockerRefs).toContain("blocker.verification.admittedProvenancePassed");

    // The downgrade above happens inside `executeLoupeVerification`. The same
    // rule has to hold for a result that arrives from somewhere else: every
    // other confirmation prerequisite is satisfied here, and the provenance is
    // the only thing left to refuse on.
    expect(() =>
      evaluateLoupeVerificationReleaseGate({
        gateRef: "gate.verification.forged.v1",
        results: [{ ...result, evidenceTier: "independently_verified", outcome: "confirmed" }],
        evaluatedAt: "2026-08-01T16:00:08.000Z",
      }),
    ).toThrow("conformance-vector evidence can never be independently verified");
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
          resolveAdmittedWorkerReceipt: async () => {
            throw new Error("unreachable");
          },
          commitInitialVerdict: async () => {
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
          resolveAdmittedWorkerReceipt: workerAuthority(verificationPlan),
          commitInitialVerdict: firstVerdictLedger(),
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
          observedTermination: { status: "observed", exitStatus: 1 },
        }),
        observedTermination: { status: "not_observed" },
      }),
    ).toThrow("executed control evidence");
  });

  it("derives a control outcome instead of accepting the one a backend asserts", () => {
    const verificationPlan = plan();
    const control = (termination: Record<string, unknown>) =>
      evidence(verificationPlan, 5, "control_test_observed", {
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
    // The defect this closes: a producer could report a clean exit and no
    // result at all, and the old shape let it call that a passing control.
    expect(deriveControlTestOutcome(control({ status: "observed", exitStatus: 0 }))).toBe(
      "not_observed",
    );
  });

  it("refuses a fixed control that exited cleanly but produced no result artifact", async () => {
    const verificationPlan = plan();
    const controls = [
      evidence(verificationPlan, 4, "poc_applied"),
      evidence(verificationPlan, 5, "control_test_observed", {
        controlRevision: "vulnerable",
        expectedTestOutcome: "failure",
        observedTermination: { status: "observed", exitStatus: 1 },
      }),
      evidence(verificationPlan, 6, "control_test_observed", {
        controlRevision: "fixed",
        expectedTestOutcome: "success",
        observedTermination: { status: "observed", exitStatus: 0 },
      }),
    ];
    const result = await executeLoupeVerification(
      verificationPlan,
      backend(verificationPlan, [], "confirmed", controls),
      "2026-08-01T16:00:07.000Z",
    );
    expect(result.derivedFixedTestOutcome).toBe("not_observed");
    expect(result.fixedControlPassed).toBe(false);
    expect(result.outcome).toBe("inconclusive");
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

  describe("admitted-worker receipt resolution", () => {
    const executeWith = async (
      verificationPlan: LoupeVerificationPlan,
      resolveAdmittedWorkerReceipt: LoupeVerificationBackend["resolveAdmittedWorkerReceipt"],
    ) =>
      executeLoupeVerification(
        verificationPlan,
        { ...backend(verificationPlan, []), resolveAdmittedWorkerReceipt },
        "2026-08-01T16:00:07.000Z",
      );

    it("refuses an arbitrary well-formed worker receipt ref the authority never emitted", async () => {
      const verificationPlan = plan();
      const forged = controlEvidence(verificationPlan).map((receipt) =>
        strictDecode(LoupeVerificationEvidenceSchema, {
          ...receipt,
          workerReceiptRef: "worker.receipt.forged.by.the.session",
        }),
      );
      await expect(
        executeLoupeVerification(
          verificationPlan,
          backend(verificationPlan, [], "confirmed", forged),
          "2026-08-01T16:00:07.000Z",
        ),
      ).rejects.toThrow("cannot resolve");
    });

    it("refuses when the authority does not know the cited receipt", async () => {
      const verificationPlan = plan();
      await expect(executeWith(verificationPlan, async () => undefined)).rejects.toThrow(
        "cannot resolve",
      );
    });

    it("refuses a receipt bound to a different resource generation", async () => {
      const verificationPlan = plan();
      await expect(
        executeWith(
          verificationPlan,
          workerAuthority(verificationPlan, { resourceGeneration: 8 }),
        ),
      ).rejects.toThrow("admitted sandbox and resource generation");
    });

    it("refuses a receipt bound to a different sandbox", async () => {
      const verificationPlan = plan();
      await expect(
        executeWith(
          verificationPlan,
          workerAuthority(verificationPlan, { sandboxRef: "sandbox.live_gce.other.v1" }),
        ),
      ).rejects.toThrow("admitted sandbox and resource generation");
    });

    it("refuses a receipt bound to a different admitted worker environment", async () => {
      const verificationPlan = plan();
      await expect(
        executeWith(
          verificationPlan,
          workerAuthority(verificationPlan, { placementRef: "placement.gce.forensic.other.v1" }),
        ),
      ).rejects.toThrow("admitted worker environment");
    });

    it.each(["released", "revoked", "expired"] as const)(
      "refuses a receipt whose lifecycle state is %s",
      async (lifecycleState) => {
        const verificationPlan = plan();
        await expect(
          executeWith(verificationPlan, workerAuthority(verificationPlan, { lifecycleState })),
        ).rejects.toThrow("admitted lifecycle state");
      },
    );

    it("refuses a receipt that had already expired when the evidence was observed", async () => {
      const verificationPlan = plan();
      await expect(
        executeWith(
          verificationPlan,
          workerAuthority(verificationPlan, { expiresAt: "2026-08-01T16:00:04.000Z" }),
        ),
      ).rejects.toThrow("expired worker receipt");
    });

    it("refuses evidence observed before the receipt that authorizes it", async () => {
      const verificationPlan = plan();
      await expect(
        executeWith(
          verificationPlan,
          workerAuthority(verificationPlan, {
            observedAt: "2026-08-01T16:30:00.000Z",
            expiresAt: "2026-08-01T17:00:00.000Z",
          }),
        ),
      ).rejects.toThrow("cannot precede the worker receipt");
    });

    it("refuses an authority that substitutes a different receipt for the requested ref", async () => {
      const verificationPlan = plan();
      await expect(
        executeWith(verificationPlan, async () =>
          admittedWorkerReceipt(verificationPlan, "worker.receipt.substituted"),
        ),
      ).rejects.toThrow("exact requested worker receipt");
    });

    it("refuses a verification with no injected lifecycle authority instead of accepting the ref", async () => {
      const verificationPlan = plan();
      const { resolveAdmittedWorkerReceipt: _omitted, ...withoutAuthority } = backend(
        verificationPlan,
        [],
      );
      await expect(
        executeLoupeVerification(
          verificationPlan,
          withoutAuthority as unknown as LoupeVerificationBackend,
          "2026-08-01T16:00:07.000Z",
        ),
      ).rejects.toThrow("injected admitted-worker lifecycle authority");
    });
  });

  describe("durable first-verdict authority", () => {
    it("refuses a second verdict once one is durably locked for the verification", async () => {
      const verificationPlan = plan();
      // One ledger, two executions: the second reaches a different verdict and
      // must not be able to overwrite the first.
      const ledger = firstVerdictLedger();
      const first = await executeLoupeVerification(
        verificationPlan,
        { ...backend(verificationPlan, []), commitInitialVerdict: ledger },
        "2026-08-01T16:00:07.000Z",
      );
      expect(first.initialVerdictRef).toBe("verdict.coldcard.initial.v1");

      const relocking = backend(verificationPlan, [], "dismissed");
      await expect(
        executeLoupeVerification(
          verificationPlan,
          {
            ...relocking,
            commitInitialVerdict: ledger,
            submitInitialVerdict: async (receivedPlan, receipts) => ({
              ...(await relocking.submitInitialVerdict(receivedPlan, receipts)),
              verdictRef: "verdict.coldcard.second-opinion.v1",
            }),
          },
          "2026-08-01T16:00:07.000Z",
        ),
      ).rejects.toThrow("already durably locked");
    });

    it("is idempotent when the same verdict is committed again", async () => {
      const verificationPlan = plan();
      const ledger = firstVerdictLedger();
      const options = { ...backend(verificationPlan, []), commitInitialVerdict: ledger };
      const first = await executeLoupeVerification(
        verificationPlan,
        options,
        "2026-08-01T16:00:07.000Z",
      );
      const second = await executeLoupeVerification(
        verificationPlan,
        options,
        "2026-08-01T16:00:07.000Z",
      );
      expect(second.initialVerdictDigest).toBe(first.initialVerdictDigest);
    });

    it("refuses an authority that forgets the verdict it was handed", async () => {
      const verificationPlan = plan();
      await expect(
        executeLoupeVerification(
          verificationPlan,
          { ...backend(verificationPlan, []), commitInitialVerdict: async () => undefined },
          "2026-08-01T16:00:07.000Z",
        ),
      ).rejects.toThrow("returned no stored verdict");
    });

    it("refuses an authority that answers with another verification's verdict", async () => {
      const verificationPlan = plan();
      const other = plan({ verificationRef: "verification.coldcard.other.v1" });
      const otherLedger = firstVerdictLedger();
      await executeLoupeVerification(
        other,
        { ...backend(other, []), commitInitialVerdict: otherLedger },
        "2026-08-01T16:00:07.000Z",
      );
      await expect(
        executeLoupeVerification(
          verificationPlan,
          {
            ...backend(verificationPlan, []),
            commitInitialVerdict: (_receivedPlan, candidate) => otherLedger(other, candidate),
          },
          "2026-08-01T16:00:07.000Z",
        ),
      ).rejects.toThrow("another verification's verdict");
    });

    it("refuses a verification with no durable verdict authority at all", async () => {
      const verificationPlan = plan();
      const { commitInitialVerdict: _omitted, ...withoutLedger } = backend(verificationPlan, []);
      await expect(
        executeLoupeVerification(
          verificationPlan,
          withoutLedger as unknown as LoupeVerificationBackend,
          "2026-08-01T16:00:07.000Z",
        ),
      ).rejects.toThrow("durable first-verdict authority");
    });
  });

  describe("admitted-worker provenance", () => {
    it("refuses evidence that cites a worker the plan never declared", async () => {
      const verificationPlan = plan();
      const controls = controlEvidence(verificationPlan).map((receipt) =>
        strictDecode(LoupeVerificationEvidenceSchema, {
          ...receipt,
          workerRef: "worker.coldcard.undeclared.v1",
        }),
      );
      await expect(
        executeLoupeVerification(
          verificationPlan,
          backend(verificationPlan, [], "confirmed", controls),
          "2026-08-01T16:00:07.000Z",
        ),
      ).rejects.toThrow("never declared as admitted");
    });

    it("refuses a worker receipt ref with no worker to attribute it to", () => {
      const verificationPlan = plan();
      const { workerRef: _dropped, ...withoutWorkerRef } = evidence(
        verificationPlan,
        4,
        "poc_applied",
      );
      expect(() => strictDecode(LoupeVerificationEvidenceSchema, withoutWorkerRef)).toThrow(
        "must name the admitted worker",
      );
    });

    it("refuses admitted-worker provenance for mechanical evidence with no worker receipt", async () => {
      // The mechanical tier used to carry no worker receipt at all, so a plan
      // could claim admitted-worker provenance for three bare backend
      // assertions. Under admitted provenance every receipt must resolve.
      const verificationPlan = plan({ evidenceProvenance: "admitted_worker_run" });
      await expect(
        executeLoupeVerification(
          verificationPlan,
          backend(verificationPlan, []),
          "2026-08-01T16:00:07.000Z",
        ),
      ).rejects.toThrow("every evidence receipt to cite an admitted worker receipt");
    });
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
