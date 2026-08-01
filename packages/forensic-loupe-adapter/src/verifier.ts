import { Schema as S } from "effect";

import {
  BoundedDigests,
  BoundedRefs,
  ForensicRef,
  ForensicTimestamp,
  PositiveInteger,
  Sha256Digest,
  forensicSha256Digest,
  strictDecode,
} from "@openagentsinc/forensic-contract";

export const LOUPE_VERIFICATION_PLAN_VERSION = "openagents.loupe_verification_plan.v1" as const;
export const LOUPE_VERIFICATION_EVIDENCE_VERSION =
  "openagents.loupe_verification_evidence.v1" as const;
export const LOUPE_INITIAL_VERDICT_VERSION = "openagents.loupe_initial_verdict.v1" as const;
export const LOUPE_VERIFICATION_RESULT_VERSION = "openagents.loupe_verification_result.v1" as const;
export const LOUPE_VERIFICATION_RELEASE_GATE_VERSION =
  "openagents.loupe_verification_release_gate.v1" as const;

const VerificationOutcome = S.Literals(["confirmed", "dismissed", "inconclusive"]);
const ControlRevision = S.Literals(["vulnerable", "fixed"]);
const TestOutcome = S.Literals(["failure", "success", "not_run"]);

export const LoupeVerificationPlanSchema = S.Struct({
  schema: S.Literal(LOUPE_VERIFICATION_PLAN_VERSION),
  verificationRef: ForensicRef,
  runRef: ForensicRef,
  findingRef: ForensicRef,
  findingDigest: Sha256Digest,
  discoveryActorRef: ForensicRef,
  verifierActorRef: ForensicRef,
  sourceBundleRef: ForensicRef,
  sourceBundleDigest: Sha256Digest,
  dependencyManifestDigest: Sha256Digest,
  vulnerableTargetDigest: Sha256Digest,
  fixedTargetDigest: Sha256Digest,
  workerImageDigest: Sha256Digest,
  workerProfileDigest: Sha256Digest,
  workerPlacementRef: ForensicRef,
  createdAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter((plan) => plan.discoveryActorRef !== plan.verifierActorRef, {
        message: "discovery and verification require distinct execution identities",
      }),
      S.makeFilter((plan) => plan.vulnerableTargetDigest !== plan.fixedTargetDigest, {
        message: "vulnerable and fixed controls require distinct immutable targets",
      }),
    ),
  )
  .annotate({ identifier: "LoupeVerificationPlan" });
export interface LoupeVerificationPlan extends S.Schema.Type<typeof LoupeVerificationPlanSchema> {}

const EvidenceOperation = S.Literals([
  "source_ref_resolved",
  "macro_value_observed",
  "symbol_provider_resolved",
  "poc_applied",
  "control_test_observed",
]);

export const LoupeVerificationEvidenceSchema = S.Struct({
  schema: S.Literal(LOUPE_VERIFICATION_EVIDENCE_VERSION),
  receiptRef: ForensicRef,
  verificationRef: ForensicRef,
  sequence: PositiveInteger,
  operation: EvidenceOperation,
  evidenceTier: S.Literals(["source_observed", "artifact_observed", "executed"]),
  subjectRef: ForensicRef,
  commandDigest: Sha256Digest,
  inputDigests: BoundedDigests,
  outcome: S.Literals(["succeeded", "failed", "inconclusive"]),
  resultDigest: S.optionalKey(Sha256Digest),
  environmentDigest: Sha256Digest,
  workerReceiptRef: S.optionalKey(ForensicRef),
  controlRevision: S.optionalKey(ControlRevision),
  expectedTestOutcome: S.optionalKey(S.Literals(["failure", "success"])),
  observedTestOutcome: S.optionalKey(TestOutcome),
  observedAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter(
        (receipt) => receipt.outcome !== "succeeded" || receipt.resultDigest !== undefined,
        { message: "successful verification evidence requires a result digest" },
      ),
      S.makeFilter(
        (receipt) =>
          receipt.operation !== "poc_applied" ||
          (receipt.evidenceTier === "artifact_observed" &&
            receipt.workerReceiptRef !== undefined &&
            receipt.outcome === "succeeded"),
        {
          message:
            "PoC application is artifact evidence and requires a successful admitted-worker receipt",
        },
      ),
      S.makeFilter(
        (receipt) =>
          receipt.operation !== "control_test_observed" ||
          (receipt.evidenceTier === "executed" &&
            receipt.workerReceiptRef !== undefined &&
            receipt.controlRevision !== undefined &&
            receipt.expectedTestOutcome !== undefined &&
            receipt.observedTestOutcome !== undefined &&
            receipt.observedTestOutcome !== "not_run"),
        {
          message:
            "executed control evidence requires a worker receipt, revision, expectation, and observed test outcome",
        },
      ),
      S.makeFilter(
        (receipt) =>
          receipt.evidenceTier !== "executed" || receipt.operation === "control_test_observed",
        { message: "an unexecuted PoC or source observation cannot claim executed evidence" },
      ),
      S.makeFilter(
        (receipt) =>
          !["source_ref_resolved", "macro_value_observed", "symbol_provider_resolved"].includes(
            receipt.operation,
          ) || receipt.evidenceTier !== "executed",
        { message: "mechanical source checks cannot claim execution evidence" },
      ),
    ),
  )
  .annotate({ identifier: "LoupeVerificationEvidence" });
export interface LoupeVerificationEvidence extends S.Schema.Type<
  typeof LoupeVerificationEvidenceSchema
> {}

export const LoupeInitialVerdictSchema = S.Struct({
  schema: S.Literal(LOUPE_INITIAL_VERDICT_VERSION),
  verdictRef: ForensicRef,
  verificationRef: ForensicRef,
  findingDigest: Sha256Digest,
  verifierActorRef: ForensicRef,
  outcome: VerificationOutcome,
  evidenceReceiptRefs: BoundedRefs.check(S.isMinLength(3)),
  rationaleDigest: Sha256Digest,
  lockedAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter(
        (verdict) =>
          new Set(verdict.evidenceReceiptRefs).size === verdict.evidenceReceiptRefs.length,
        { message: "initial verdict evidence refs must be unique" },
      ),
    ),
  )
  .annotate({ identifier: "LoupeInitialVerdict" });
export interface LoupeInitialVerdict extends S.Schema.Type<typeof LoupeInitialVerdictSchema> {}

export const LoupeVerificationResultSchema = S.Struct({
  schema: S.Literal(LOUPE_VERIFICATION_RESULT_VERSION),
  resultRef: ForensicRef,
  verificationRef: ForensicRef,
  findingRef: ForensicRef,
  discoveryActorRef: ForensicRef,
  verifierActorRef: ForensicRef,
  outcome: VerificationOutcome,
  evidenceTier: S.Literals(["source_observed", "executed", "independently_verified"]),
  initialVerdictRef: ForensicRef,
  initialVerdictDigest: Sha256Digest,
  mechanicalEvidenceReceiptRefs: BoundedRefs.check(S.isMinLength(3)),
  evidenceReceiptRefs: BoundedRefs,
  evidenceAggregateDigest: Sha256Digest,
  pocReceiptRef: S.optionalKey(ForensicRef),
  vulnerableControlReceiptRef: S.optionalKey(ForensicRef),
  fixedControlReceiptRef: S.optionalKey(ForensicRef),
  vulnerableControlPassed: S.Boolean,
  fixedControlPassed: S.Boolean,
  circularVerificationRejected: S.Literal(true),
  completionAuthority: S.Literal("adapter_atomic_result"),
  productMode: S.Literal("discovery_only"),
  completedAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter((result) => result.discoveryActorRef !== result.verifierActorRef, {
        message: "verification results require independent actor identities",
      }),
      S.makeFilter(
        (result) =>
          result.outcome !== "confirmed" ||
          (result.evidenceTier === "independently_verified" &&
            result.vulnerableControlPassed &&
            result.fixedControlPassed &&
            result.pocReceiptRef !== undefined &&
            result.vulnerableControlReceiptRef !== undefined &&
            result.fixedControlReceiptRef !== undefined),
        {
          message:
            "confirmed results require independent evidence and the vulnerable/fixed control pair",
        },
      ),
      S.makeFilter(
        (result) =>
          result.evidenceTier !== "independently_verified" || result.outcome === "confirmed",
        { message: "only a confirmed result can be independently verified" },
      ),
      S.makeFilter(
        (result) =>
          new Set(result.mechanicalEvidenceReceiptRefs).size ===
            result.mechanicalEvidenceReceiptRefs.length &&
          result.mechanicalEvidenceReceiptRefs.every((receiptRef) =>
            result.evidenceReceiptRefs.includes(receiptRef),
          ) &&
          new Set(result.evidenceReceiptRefs).size === result.evidenceReceiptRefs.length,
        { message: "verification results require unique, retained mechanical evidence refs" },
      ),
    ),
  )
  .annotate({ identifier: "LoupeVerificationResult" });
export interface LoupeVerificationResult extends S.Schema.Type<
  typeof LoupeVerificationResultSchema
> {}

export const LoupeVerificationReleaseGateSchema = S.Struct({
  schema: S.Literal(LOUPE_VERIFICATION_RELEASE_GATE_VERSION),
  gateRef: ForensicRef,
  resultRefs: BoundedRefs.check(S.isMinLength(1)),
  immutableInitialVerdictPassed: S.Boolean,
  independentActorPassed: S.Boolean,
  deterministicEvidencePassed: S.Boolean,
  vulnerableControlPassed: S.Boolean,
  fixedControlPassed: S.Boolean,
  atomicCompletionPassed: S.Boolean,
  productMode: S.Literals(["discovery_only", "independent_verification"]),
  blockerRefs: BoundedRefs,
  evaluatedAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter(
        (gate) => {
          const passed =
            gate.immutableInitialVerdictPassed &&
            gate.independentActorPassed &&
            gate.deterministicEvidencePassed &&
            gate.vulnerableControlPassed &&
            gate.fixedControlPassed &&
            gate.atomicCompletionPassed;
          return passed
            ? gate.productMode === "independent_verification" && gate.blockerRefs.length === 0
            : gate.productMode === "discovery_only" && gate.blockerRefs.length > 0;
        },
        { message: "verification product mode must be derived from every release gate" },
      ),
    ),
  )
  .annotate({ identifier: "LoupeVerificationReleaseGate" });
export interface LoupeVerificationReleaseGate extends S.Schema.Type<
  typeof LoupeVerificationReleaseGateSchema
> {}

export interface LoupeInitialVerdictDraft {
  readonly verdictRef: string;
  readonly outcome: "confirmed" | "dismissed" | "inconclusive";
  readonly rationaleDigest: string;
  readonly lockedAt: string;
}

export interface LoupeVerificationBackend {
  readonly collectMechanicalEvidence: (
    plan: LoupeVerificationPlan,
  ) => Promise<ReadonlyArray<unknown>>;
  readonly submitInitialVerdict: (
    plan: LoupeVerificationPlan,
    evidence: ReadonlyArray<LoupeVerificationEvidence>,
  ) => Promise<LoupeInitialVerdictDraft>;
  readonly applyPocAndRunControls: (
    plan: LoupeVerificationPlan,
    lockedVerdict: LoupeInitialVerdict,
  ) => Promise<ReadonlyArray<unknown>>;
}

const REQUIRED_MECHANICAL_OPERATIONS = [
  "source_ref_resolved",
  "macro_value_observed",
  "symbol_provider_resolved",
] as const;
const REQUIRED_MECHANICAL_OPERATION_SET = new Set<string>(REQUIRED_MECHANICAL_OPERATIONS);

const deepFreeze = <Value>(value: Value): Value => {
  if (value !== null && typeof value === "object" && !Object.isFrozen(value)) {
    for (const child of Object.values(value)) deepFreeze(child);
    Object.freeze(value);
  }
  return value;
};

const decodeEvidence = (
  plan: LoupeVerificationPlan,
  untrustedEvidence: ReadonlyArray<unknown>,
): Array<LoupeVerificationEvidence> => {
  const evidence = untrustedEvidence.map((receipt) =>
    strictDecode(LoupeVerificationEvidenceSchema, receipt),
  );
  if (evidence.some((receipt) => receipt.verificationRef !== plan.verificationRef)) {
    throw new Error("verification evidence must bind the exact verification ref");
  }
  if (new Set(evidence.map((receipt) => receipt.receiptRef)).size !== evidence.length) {
    throw new Error("verification evidence refs must be unique");
  }
  const expectedSequence = evidence.map((_, index) => index + 1);
  if (evidence.some((receipt, index) => receipt.sequence !== expectedSequence[index])) {
    throw new Error("verification evidence must be a contiguous ordered sequence");
  }
  const environmentDigest = forensicSha256Digest({
    workerImageDigest: plan.workerImageDigest,
    workerProfileDigest: plan.workerProfileDigest,
    workerPlacementRef: plan.workerPlacementRef,
  });
  for (const receipt of evidence) {
    if (receipt.environmentDigest !== environmentDigest) {
      throw new Error("verification evidence must bind the admitted worker environment");
    }
    if (
      ["source_ref_resolved", "macro_value_observed", "symbol_provider_resolved"].includes(
        receipt.operation,
      ) &&
      (!receipt.inputDigests.includes(plan.sourceBundleDigest) ||
        !receipt.inputDigests.includes(plan.dependencyManifestDigest))
    ) {
      throw new Error("mechanical evidence must bind source and dependency inputs");
    }
    if (
      receipt.operation === "poc_applied" &&
      !receipt.inputDigests.includes(plan.vulnerableTargetDigest)
    ) {
      throw new Error("PoC application must bind the vulnerable target");
    }
    if (receipt.operation === "control_test_observed") {
      const targetDigest =
        receipt.controlRevision === "vulnerable"
          ? plan.vulnerableTargetDigest
          : plan.fixedTargetDigest;
      if (!receipt.inputDigests.includes(targetDigest)) {
        throw new Error("control evidence must bind its immutable target revision");
      }
    }
  }
  return evidence;
};

const mechanicalEvidenceIsComplete = (evidence: ReadonlyArray<LoupeVerificationEvidence>) =>
  evidence.length === REQUIRED_MECHANICAL_OPERATIONS.length &&
  evidence.every(
    (receipt, index) =>
      receipt.operation === REQUIRED_MECHANICAL_OPERATIONS[index] &&
      receipt.outcome === "succeeded",
  );

export const executeLoupeVerification = async (
  untrustedPlan: unknown,
  backend: LoupeVerificationBackend,
  completedAt: string,
): Promise<LoupeVerificationResult> => {
  const plan = deepFreeze(strictDecode(LoupeVerificationPlanSchema, untrustedPlan));
  const mechanicalEvidence = decodeEvidence(plan, await backend.collectMechanicalEvidence(plan));
  if (
    mechanicalEvidence.some((receipt) => !REQUIRED_MECHANICAL_OPERATION_SET.has(receipt.operation))
  ) {
    throw new Error("patch or control work cannot precede the immutable initial verdict");
  }
  if (!mechanicalEvidenceIsComplete(mechanicalEvidence)) {
    throw new Error("initial verdict requires source-ref, macro, and symbol-provider evidence");
  }

  const draft = await backend.submitInitialVerdict(plan, deepFreeze(mechanicalEvidence));
  const initialVerdict = deepFreeze(
    strictDecode(LoupeInitialVerdictSchema, {
      schema: LOUPE_INITIAL_VERDICT_VERSION,
      verdictRef: draft.verdictRef,
      verificationRef: plan.verificationRef,
      findingDigest: plan.findingDigest,
      verifierActorRef: plan.verifierActorRef,
      outcome: draft.outcome,
      evidenceReceiptRefs: mechanicalEvidence.map((receipt) => receipt.receiptRef),
      rationaleDigest: draft.rationaleDigest,
      lockedAt: draft.lockedAt,
    }),
  );
  const lockedAt = Date.parse(initialVerdict.lockedAt);
  if (mechanicalEvidence.some((receipt) => Date.parse(receipt.observedAt) > lockedAt)) {
    throw new Error("initial verdict cannot be locked before its mechanical evidence");
  }
  const initialVerdictDigest = forensicSha256Digest(initialVerdict);

  let controlEvidence: Array<LoupeVerificationEvidence> = [];
  if (initialVerdict.outcome === "confirmed") {
    const returned = await backend.applyPocAndRunControls(plan, initialVerdict);
    controlEvidence = decodeEvidence(plan, [...mechanicalEvidence, ...returned]).slice(
      mechanicalEvidence.length,
    );
    const pocReceipts = controlEvidence.filter((receipt) => receipt.operation === "poc_applied");
    const vulnerableReceipts = controlEvidence.filter(
      (receipt) =>
        receipt.operation === "control_test_observed" && receipt.controlRevision === "vulnerable",
    );
    const fixedReceipts = controlEvidence.filter(
      (receipt) =>
        receipt.operation === "control_test_observed" && receipt.controlRevision === "fixed",
    );
    if (pocReceipts.length > 1 || vulnerableReceipts.length > 1 || fixedReceipts.length > 1) {
      throw new Error("verification permits only one PoC and one receipt per control revision");
    }
    if (
      controlEvidence.length === 3 &&
      (controlEvidence[0]?.operation !== "poc_applied" ||
        controlEvidence[1]?.controlRevision !== "vulnerable" ||
        controlEvidence[2]?.controlRevision !== "fixed")
    ) {
      throw new Error(
        "complete control evidence must order PoC, vulnerable, then fixed observations",
      );
    }
    if (controlEvidence.some((receipt) => Date.parse(receipt.observedAt) <= lockedAt)) {
      throw new Error("PoC and control receipts must be observed after the verdict lock");
    }
  }
  const allEvidence = [...mechanicalEvidence, ...controlEvidence];
  const pocApplied = controlEvidence.some(
    (receipt) => receipt.operation === "poc_applied" && receipt.outcome === "succeeded",
  );
  const vulnerableControlPassed =
    pocApplied &&
    controlEvidence.some(
      (receipt) =>
        receipt.operation === "control_test_observed" &&
        receipt.controlRevision === "vulnerable" &&
        receipt.expectedTestOutcome === "failure" &&
        receipt.observedTestOutcome === "failure" &&
        receipt.outcome === "succeeded",
    );
  const fixedControlPassed =
    pocApplied &&
    controlEvidence.some(
      (receipt) =>
        receipt.operation === "control_test_observed" &&
        receipt.controlRevision === "fixed" &&
        receipt.expectedTestOutcome === "success" &&
        receipt.observedTestOutcome === "success" &&
        receipt.outcome === "succeeded",
    );
  const pocReceiptRef = controlEvidence.find(
    (receipt) => receipt.operation === "poc_applied",
  )?.receiptRef;
  const vulnerableControlReceiptRef = controlEvidence.find(
    (receipt) =>
      receipt.operation === "control_test_observed" && receipt.controlRevision === "vulnerable",
  )?.receiptRef;
  const fixedControlReceiptRef = controlEvidence.find(
    (receipt) =>
      receipt.operation === "control_test_observed" && receipt.controlRevision === "fixed",
  )?.receiptRef;
  const controlsPassed = vulnerableControlPassed && fixedControlPassed;
  const outcome =
    initialVerdict.outcome === "confirmed" && !controlsPassed
      ? "inconclusive"
      : initialVerdict.outcome;
  if (allEvidence.some((receipt) => Date.parse(receipt.observedAt) > Date.parse(completedAt))) {
    throw new Error("verification cannot complete before its evidence was observed");
  }

  return deepFreeze(
    strictDecode(LoupeVerificationResultSchema, {
      schema: LOUPE_VERIFICATION_RESULT_VERSION,
      resultRef: `result.${plan.verificationRef}`,
      verificationRef: plan.verificationRef,
      findingRef: plan.findingRef,
      discoveryActorRef: plan.discoveryActorRef,
      verifierActorRef: plan.verifierActorRef,
      outcome,
      evidenceTier:
        outcome === "confirmed"
          ? "independently_verified"
          : controlEvidence.some((receipt) => receipt.evidenceTier === "executed")
            ? "executed"
            : "source_observed",
      initialVerdictRef: initialVerdict.verdictRef,
      initialVerdictDigest,
      mechanicalEvidenceReceiptRefs: mechanicalEvidence.map((receipt) => receipt.receiptRef),
      evidenceReceiptRefs: allEvidence.map((receipt) => receipt.receiptRef),
      evidenceAggregateDigest: forensicSha256Digest(allEvidence),
      ...(pocReceiptRef === undefined ? {} : { pocReceiptRef }),
      ...(vulnerableControlReceiptRef === undefined ? {} : { vulnerableControlReceiptRef }),
      ...(fixedControlReceiptRef === undefined ? {} : { fixedControlReceiptRef }),
      vulnerableControlPassed,
      fixedControlPassed,
      circularVerificationRejected: true,
      completionAuthority: "adapter_atomic_result",
      productMode: "discovery_only",
      completedAt,
    }),
  );
};

export interface EvaluateLoupeVerificationReleaseGateInput {
  readonly gateRef: string;
  readonly results: ReadonlyArray<unknown>;
  readonly evaluatedAt: string;
}

export const evaluateLoupeVerificationReleaseGate = (
  input: EvaluateLoupeVerificationReleaseGateInput,
): LoupeVerificationReleaseGate => {
  if (input.results.length === 0) throw new Error("release gate requires verification results");
  const results = input.results.map((result) =>
    strictDecode(LoupeVerificationResultSchema, result),
  );
  const confirmed = results.every((result) => result.outcome === "confirmed");
  const values = {
    immutableInitialVerdictPassed: results.every(
      (result) => result.initialVerdictRef.length > 0 && result.initialVerdictDigest.length > 0,
    ),
    independentActorPassed: results.every(
      (result) =>
        result.circularVerificationRejected && result.discoveryActorRef !== result.verifierActorRef,
    ),
    deterministicEvidencePassed: results.every(
      (result) =>
        result.mechanicalEvidenceReceiptRefs.length >= 3 &&
        result.evidenceReceiptRefs.length >= 6 &&
        result.evidenceAggregateDigest.length > 0,
    ),
    vulnerableControlPassed: confirmed && results.every((result) => result.vulnerableControlPassed),
    fixedControlPassed: confirmed && results.every((result) => result.fixedControlPassed),
    atomicCompletionPassed: results.every(
      (result) => result.completionAuthority === "adapter_atomic_result",
    ),
  };
  const blockers = Object.entries(values)
    .filter(([, passed]) => !passed)
    .map(([name]) => `blocker.verification.${name}`);
  return deepFreeze(
    strictDecode(LoupeVerificationReleaseGateSchema, {
      schema: LOUPE_VERIFICATION_RELEASE_GATE_VERSION,
      gateRef: input.gateRef,
      resultRefs: results.map((result) => result.resultRef),
      ...values,
      productMode: blockers.length === 0 ? "independent_verification" : "discovery_only",
      blockerRefs: blockers,
      evaluatedAt: input.evaluatedAt,
    }),
  );
};

export const loupeForensicVerifier = Object.freeze({
  execute: executeLoupeVerification,
  evaluateReleaseGate: evaluateLoupeVerificationReleaseGate,
  planSchema: LoupeVerificationPlanSchema,
  evidenceSchema: LoupeVerificationEvidenceSchema,
  initialVerdictSchema: LoupeInitialVerdictSchema,
  resultSchema: LoupeVerificationResultSchema,
  releaseGateSchema: LoupeVerificationReleaseGateSchema,
});
