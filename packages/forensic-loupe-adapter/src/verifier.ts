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
export const LOUPE_ADMITTED_WORKER_RECEIPT_VERSION =
  "openagents.loupe_admitted_worker_receipt.v1" as const;

const VerificationOutcome = S.Literals(["confirmed", "dismissed", "inconclusive"]);
const ControlRevision = S.Literals(["vulnerable", "fixed"]);
const DerivedTestOutcome = S.Literals(["failure", "success", "not_observed"]);

/**
 * How the evidence behind a verification was produced.
 *
 * `conformance_vector` exercises the schemas and the evaluator with synthetic
 * values. It is a self-test, never an acceptance claim, and can never reach the
 * `independently_verified` tier no matter how well formed it is.
 *
 * `admitted_worker_run` means every evidence receipt was produced by, or over a
 * target produced by, an OpenAgents Cloud managed sandbox whose runtime receipt
 * the injected lifecycle authority resolves. This mirrors the same distinction
 * `openagents.artifact_witness_capture.v2` draws, and exists for the same
 * reason: without it a conformance value can be closed as acceptance evidence.
 */
export const LoupeEvidenceProvenance = S.Literals(["conformance_vector", "admitted_worker_run"]);
export type LoupeEvidenceProvenance = typeof LoupeEvidenceProvenance.Type;

/**
 * One admitted managed sandbox this plan is allowed to draw evidence from.
 *
 * A real vulnerable/fixed control pair runs in more than one isolated sandbox,
 * so a single pinned sandbox ref cannot describe honest control evidence. The
 * plan therefore enumerates the exact admitted workers, and every worker
 * receipt must bind one of them at its exact resource generation.
 */
export const LoupeAdmittedWorkerSchema = S.Struct({
  workerRef: ForensicRef,
  sandboxRef: ForensicRef,
  resourceGeneration: PositiveInteger,
  placementRef: ForensicRef,
}).annotate({ identifier: "LoupeAdmittedWorker" });
export interface LoupeAdmittedWorker extends S.Schema.Type<typeof LoupeAdmittedWorkerSchema> {}

export const LoupeVerificationPlanSchema = S.Struct({
  schema: S.Literal(LOUPE_VERIFICATION_PLAN_VERSION),
  verificationRef: ForensicRef,
  runRef: ForensicRef,
  findingRef: ForensicRef,
  findingDigest: Sha256Digest,
  discoveryActorRef: ForensicRef,
  verifierActorRef: ForensicRef,
  evidenceProvenance: LoupeEvidenceProvenance,
  sourceBundleRef: ForensicRef,
  sourceBundleDigest: Sha256Digest,
  dependencyManifestDigest: Sha256Digest,
  vulnerableTargetDigest: Sha256Digest,
  fixedTargetDigest: Sha256Digest,
  workerImageDigest: Sha256Digest,
  workerProfileDigest: Sha256Digest,
  admittedWorkers: S.Array(LoupeAdmittedWorkerSchema).check(S.isMinLength(1), S.isMaxLength(16)),
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
      S.makeFilter(
        (plan) =>
          new Set(plan.admittedWorkers.map((worker) => worker.workerRef)).size ===
            plan.admittedWorkers.length &&
          new Set(plan.admittedWorkers.map((worker) => worker.sandboxRef)).size ===
            plan.admittedWorkers.length,
        { message: "admitted workers must be distinct identities bound to distinct sandboxes" },
      ),
    ),
  )
  .annotate({ identifier: "LoupeVerificationPlan" });
export interface LoupeVerificationPlan extends S.Schema.Type<typeof LoupeVerificationPlanSchema> {}

/**
 * The lifecycle-authority record a `workerReceiptRef` must resolve to.
 *
 * This mirrors the receipt the managed-sandbox authority actually emits
 * (`ManagedSandboxRuntimeReceipt` in `crates/oa-codex-control`, surfaced as the
 * forensic worker receipts in the Cloud Run forensic managed-sandbox facade).
 * `ForensicRef` only constrains a ref's shape, so the verifier must resolve the
 * ref through an authority instead of accepting any well-formed string.
 */
export const LoupeAdmittedWorkerReceiptSchema = S.Struct({
  schema: S.Literal(LOUPE_ADMITTED_WORKER_RECEIPT_VERSION),
  receiptRef: ForensicRef,
  sandboxRef: ForensicRef,
  resourceGeneration: PositiveInteger,
  placementRef: ForensicRef,
  imageDigest: Sha256Digest,
  profileDigest: Sha256Digest,
  lifecycleState: S.Literals(["admitted", "released", "revoked", "expired"]),
  exact: S.Boolean,
  observedAt: ForensicTimestamp,
  expiresAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter((receipt) => Date.parse(receipt.expiresAt) > Date.parse(receipt.observedAt), {
        message: "an admitted-worker receipt must expire after it was observed",
      }),
    ),
  )
  .annotate({ identifier: "LoupeAdmittedWorkerReceipt" });
export interface LoupeAdmittedWorkerReceipt extends S.Schema.Type<
  typeof LoupeAdmittedWorkerReceiptSchema
> {}

/**
 * Injected boundary that resolves a worker receipt ref against the admitted
 * worker lifecycle authority. Return `undefined` for a ref the authority does
 * not know. There is deliberately no default implementation: a verification
 * without this boundary refuses instead of accepting an unresolved ref.
 */
export type ResolveAdmittedWorkerReceipt = (
  plan: LoupeVerificationPlan,
  workerReceiptRef: string,
) => Promise<unknown>;

/**
 * Injected boundary that durably records the first initial verdict for a
 * verification and returns the verdict that is durably stored afterwards.
 * There is deliberately no default implementation: without it the exactly-once
 * property would hold only inside one process, which is what the acceptance
 * audit named as missing.
 */
export type CommitInitialVerdict = (
  plan: LoupeVerificationPlan,
  candidate: LoupeInitialVerdict,
) => Promise<unknown>;

const EvidenceOperation = S.Literals([
  "source_ref_resolved",
  "macro_value_observed",
  "symbol_provider_resolved",
  "poc_applied",
  "control_test_observed",
]);

/**
 * What the verifier is allowed to know about a control test run.
 *
 * This is deliberately an OBSERVATION and not a conclusion. The previous shape
 * carried `observedTestOutcome`, so a backend asserted the verdict the verifier
 * was supposed to reach. A producer now reports how the control test process
 * terminated and whether it left a result artifact behind, and
 * {@link deriveControlTestOutcome} decides what that means. An exit-zero run
 * that produced no result artifact is `not_observed`, never `success`.
 */
export const LoupeControlTerminationSchema = S.Struct({
  status: S.Literals(["observed", "not_observed"]),
  exitStatus: S.optionalKey(S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0))),
  resultArtifactDigest: S.optionalKey(Sha256Digest),
})
  .pipe(
    S.check(
      S.makeFilter(
        (termination) => termination.status !== "observed" || termination.exitStatus !== undefined,
        { message: "an observed control termination must carry the exit status it observed" },
      ),
    ),
  )
  .annotate({ identifier: "LoupeControlTermination" });
export interface LoupeControlTermination extends S.Schema.Type<
  typeof LoupeControlTerminationSchema
> {}

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
  workerRef: S.optionalKey(ForensicRef),
  workerReceiptRef: S.optionalKey(ForensicRef),
  controlRevision: S.optionalKey(ControlRevision),
  expectedTestOutcome: S.optionalKey(S.Literals(["failure", "success"])),
  observedTermination: S.optionalKey(LoupeControlTerminationSchema),
  observedAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter(
        (receipt) => receipt.outcome !== "succeeded" || receipt.resultDigest !== undefined,
        { message: "successful verification evidence requires a result digest" },
      ),
      S.makeFilter(
        (receipt) => (receipt.workerReceiptRef === undefined) === (receipt.workerRef === undefined),
        {
          message:
            "a worker receipt ref must name the admitted worker it belongs to, and vice versa",
        },
      ),
      S.makeFilter(
        (receipt) =>
          receipt.operation !== "poc_applied" ||
          (receipt.evidenceTier === "artifact_observed" &&
            receipt.workerReceiptRef !== undefined &&
            receipt.outcome === "succeeded"),
        {
          message:
            "PoC application is artifact evidence and requires a successful worker receipt ref, which the verifier resolves against the admitted-worker authority",
        },
      ),
      S.makeFilter(
        (receipt) =>
          receipt.operation !== "control_test_observed" ||
          (receipt.evidenceTier === "executed" &&
            receipt.workerReceiptRef !== undefined &&
            receipt.controlRevision !== undefined &&
            receipt.expectedTestOutcome !== undefined &&
            receipt.observedTermination !== undefined &&
            receipt.observedTermination.status === "observed"),
        {
          message:
            "executed control evidence requires a worker receipt ref, revision, expectation, and an observed control termination, with the ref resolved by the verifier against the admitted-worker authority",
        },
      ),
      S.makeFilter(
        (receipt) =>
          receipt.operation === "control_test_observed" ||
          receipt.observedTermination === undefined,
        { message: "only an executed control test may report a control termination" },
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
  evidenceProvenance: LoupeEvidenceProvenance,
  evidenceTier: S.Literals(["source_observed", "executed", "independently_verified"]),
  initialVerdictRef: ForensicRef,
  initialVerdictDigest: Sha256Digest,
  initialVerdictAuthority: S.Literal("durable_first_verdict_ledger"),
  mechanicalEvidenceReceiptRefs: BoundedRefs.check(S.isMinLength(3)),
  evidenceReceiptRefs: BoundedRefs,
  evidenceAggregateDigest: Sha256Digest,
  pocReceiptRef: S.optionalKey(ForensicRef),
  vulnerableControlReceiptRef: S.optionalKey(ForensicRef),
  fixedControlReceiptRef: S.optionalKey(ForensicRef),
  derivedVulnerableTestOutcome: DerivedTestOutcome,
  derivedFixedTestOutcome: DerivedTestOutcome,
  vulnerableControlPassed: S.Boolean,
  fixedControlPassed: S.Boolean,
  circularVerificationRejected: S.Literal(true),
  admittedWorkerReceiptsResolved: S.Literal(true),
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
          result.evidenceTier !== "independently_verified" ||
          result.evidenceProvenance === "admitted_worker_run",
        {
          message:
            "conformance-vector evidence can never be independently verified, however well formed it is",
        },
      ),
      S.makeFilter(
        (result) =>
          (!result.vulnerableControlPassed ||
            result.derivedVulnerableTestOutcome === "failure") &&
          (!result.fixedControlPassed || result.derivedFixedTestOutcome === "success"),
        {
          message:
            "a passed control requires the outcome the verifier derived from its observed termination",
        },
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
  admittedProvenancePassed: S.Boolean,
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
            gate.admittedProvenancePassed &&
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
  /**
   * Required. Resolves an evidence `workerReceiptRef` against the admitted
   * worker lifecycle authority. A missing implementation refuses the whole
   * verification, it does not fall back to accepting the raw ref.
   */
  readonly resolveAdmittedWorkerReceipt: ResolveAdmittedWorkerReceipt;
  /**
   * Required. Durably commits the candidate initial verdict for
   * `plan.verificationRef` and returns whatever verdict is durably stored for
   * that verification afterwards.
   *
   * The contract is compare-and-set with first-writer-wins: an authority that
   * already holds a verdict for this verification must return the stored one
   * rather than overwriting it. The verifier then refuses when the returned
   * verdict differs from the candidate, which is what makes the first verdict
   * exactly-once across processes instead of only within one call.
   */
  readonly commitInitialVerdict: CommitInitialVerdict;
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

/**
 * Fails closed for every evidence receipt that carries a `workerReceiptRef`.
 *
 * A ref is admitted only when the injected authority resolves it to the exact
 * requested receipt, that receipt is still in the `admitted` lifecycle state,
 * it is exact, it binds the exact admitted worker the evidence named at that
 * worker's exact resource generation and placement, it binds the plan's worker
 * image and profile, and it was observed before, and expires after, the
 * evidence it authorizes. Unresolvable, unknown, forged, expired,
 * wrong-generation, undeclared-sandbox, and cross-worker refs all refuse.
 *
 * Under `admitted_worker_run` provenance a receipt is not merely checked when
 * present: EVERY evidence receipt must carry one. Mechanical evidence with no
 * worker receipt is a bare backend assertion, and a plan is not allowed to
 * claim admitted-worker provenance for it.
 */
const requireAdmittedWorkerReceipts = async (
  plan: LoupeVerificationPlan,
  evidence: ReadonlyArray<LoupeVerificationEvidence>,
  resolve: ResolveAdmittedWorkerReceipt,
): Promise<void> => {
  for (const receipt of evidence) {
    const workerReceiptRef = receipt.workerReceiptRef;
    if (workerReceiptRef === undefined) {
      if (plan.evidenceProvenance === "admitted_worker_run") {
        throw new Error(
          "admitted-worker provenance requires every evidence receipt to cite an admitted worker receipt",
        );
      }
      continue;
    }
    const declaredWorker = plan.admittedWorkers.find(
      (worker) => worker.workerRef === receipt.workerRef,
    );
    if (declaredWorker === undefined) {
      throw new Error("verification evidence cites a worker the plan never declared as admitted");
    }
    const resolved = await resolve(plan, workerReceiptRef);
    if (resolved === undefined || resolved === null) {
      throw new Error(
        "verification evidence cites a worker receipt the admitted-worker authority cannot resolve",
      );
    }
    const admitted = strictDecode(LoupeAdmittedWorkerReceiptSchema, resolved);
    if (admitted.receiptRef !== workerReceiptRef) {
      throw new Error("admitted-worker resolution must return the exact requested worker receipt");
    }
    if (admitted.lifecycleState !== "admitted" || !admitted.exact) {
      throw new Error(
        "verification evidence requires an exact worker receipt in the admitted lifecycle state",
      );
    }
    if (
      admitted.sandboxRef !== declaredWorker.sandboxRef ||
      admitted.resourceGeneration !== declaredWorker.resourceGeneration
    ) {
      throw new Error(
        "a worker receipt must bind the exact admitted sandbox and resource generation of its plan",
      );
    }
    if (
      admitted.placementRef !== declaredWorker.placementRef ||
      admitted.imageDigest !== plan.workerImageDigest ||
      admitted.profileDigest !== plan.workerProfileDigest
    ) {
      throw new Error("a worker receipt must bind the admitted worker environment of its plan");
    }
    const observedAt = Date.parse(receipt.observedAt);
    if (Date.parse(admitted.observedAt) > observedAt) {
      throw new Error("verification evidence cannot precede the worker receipt that authorizes it");
    }
    if (Date.parse(admitted.expiresAt) <= observedAt) {
      throw new Error("verification evidence cannot cite an expired worker receipt");
    }
  }
};

/**
 * Decides what a control test run means from what was observed of it.
 *
 * The verifier owns this, not the producer. A run whose termination was never
 * observed is `not_observed` and can satisfy no expectation. A non-zero exit is
 * a `failure`. A zero exit is a `success` only when the run also left the
 * result artifact behind that a real passing test produces; a zero exit with
 * nothing to show for it is `not_observed`, because "the process ended and I
 * kept nothing" is not evidence that the test passed.
 */
export const deriveControlTestOutcome = (
  receipt: LoupeVerificationEvidence,
): "failure" | "success" | "not_observed" => {
  const termination = receipt.observedTermination;
  if (
    termination === undefined ||
    termination.status !== "observed" ||
    termination.exitStatus === undefined
  ) {
    return "not_observed";
  }
  if (termination.exitStatus !== 0) return "failure";
  return termination.resultArtifactDigest === undefined ? "not_observed" : "success";
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
  const resolveAdmittedWorkerReceipt = backend.resolveAdmittedWorkerReceipt;
  if (typeof resolveAdmittedWorkerReceipt !== "function") {
    throw new Error(
      "verification requires an injected admitted-worker lifecycle authority to resolve worker receipts",
    );
  }
  const mechanicalEvidence = decodeEvidence(plan, await backend.collectMechanicalEvidence(plan));
  await requireAdmittedWorkerReceipts(plan, mechanicalEvidence, resolveAdmittedWorkerReceipt);
  if (
    mechanicalEvidence.some((receipt) => !REQUIRED_MECHANICAL_OPERATION_SET.has(receipt.operation))
  ) {
    throw new Error("patch or control work cannot precede the immutable initial verdict");
  }
  if (!mechanicalEvidenceIsComplete(mechanicalEvidence)) {
    throw new Error("initial verdict requires source-ref, macro, and symbol-provider evidence");
  }

  const commitInitialVerdict = backend.commitInitialVerdict;
  if (typeof commitInitialVerdict !== "function") {
    throw new Error(
      "verification requires an injected durable first-verdict authority to lock the initial verdict",
    );
  }

  const draft = await backend.submitInitialVerdict(plan, deepFreeze(mechanicalEvidence));
  const candidateVerdict = strictDecode(LoupeInitialVerdictSchema, {
    schema: LOUPE_INITIAL_VERDICT_VERSION,
    verdictRef: draft.verdictRef,
    verificationRef: plan.verificationRef,
    findingDigest: plan.findingDigest,
    verifierActorRef: plan.verifierActorRef,
    outcome: draft.outcome,
    evidenceReceiptRefs: mechanicalEvidence.map((receipt) => receipt.receiptRef),
    rationaleDigest: draft.rationaleDigest,
    lockedAt: draft.lockedAt,
  });
  // Exactly-once is a property of the durable ledger, not of this call stack.
  // The ledger returns whatever verdict it already holds for this verification,
  // and a candidate that disagrees with it refuses instead of relocking.
  const committed = await commitInitialVerdict(plan, candidateVerdict);
  if (committed === undefined || committed === null) {
    throw new Error("the durable first-verdict authority returned no stored verdict");
  }
  const storedVerdict = strictDecode(LoupeInitialVerdictSchema, committed);
  if (storedVerdict.verificationRef !== plan.verificationRef) {
    throw new Error("the durable first-verdict authority returned another verification's verdict");
  }
  if (forensicSha256Digest(storedVerdict) !== forensicSha256Digest(candidateVerdict)) {
    throw new Error(
      "an initial verdict is already durably locked for this verification and cannot be relocked",
    );
  }
  const initialVerdict = deepFreeze(storedVerdict);
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
    await requireAdmittedWorkerReceipts(plan, controlEvidence, resolveAdmittedWorkerReceipt);
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
  const controlReceipt = (revision: "vulnerable" | "fixed") =>
    controlEvidence.find(
      (receipt) =>
        receipt.operation === "control_test_observed" && receipt.controlRevision === revision,
    );
  const vulnerableReceipt = controlReceipt("vulnerable");
  const fixedReceipt = controlReceipt("fixed");
  const derivedVulnerableTestOutcome =
    vulnerableReceipt === undefined ? "not_observed" : deriveControlTestOutcome(vulnerableReceipt);
  const derivedFixedTestOutcome =
    fixedReceipt === undefined ? "not_observed" : deriveControlTestOutcome(fixedReceipt);
  const vulnerableControlPassed =
    pocApplied &&
    vulnerableReceipt !== undefined &&
    vulnerableReceipt.expectedTestOutcome === "failure" &&
    vulnerableReceipt.outcome === "succeeded" &&
    derivedVulnerableTestOutcome === "failure";
  const fixedControlPassed =
    pocApplied &&
    fixedReceipt !== undefined &&
    fixedReceipt.expectedTestOutcome === "success" &&
    fixedReceipt.outcome === "succeeded" &&
    derivedFixedTestOutcome === "success";
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
  // A conformance vector can be perfectly well formed and still prove nothing,
  // so it never carries a confirmation out of this function.
  const admittedProvenance = plan.evidenceProvenance === "admitted_worker_run";
  const outcome =
    initialVerdict.outcome === "confirmed" && (!controlsPassed || !admittedProvenance)
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
      evidenceProvenance: plan.evidenceProvenance,
      evidenceTier:
        outcome === "confirmed"
          ? "independently_verified"
          : controlEvidence.some((receipt) => receipt.evidenceTier === "executed")
            ? "executed"
            : "source_observed",
      initialVerdictRef: initialVerdict.verdictRef,
      initialVerdictDigest,
      initialVerdictAuthority: "durable_first_verdict_ledger",
      mechanicalEvidenceReceiptRefs: mechanicalEvidence.map((receipt) => receipt.receiptRef),
      evidenceReceiptRefs: allEvidence.map((receipt) => receipt.receiptRef),
      evidenceAggregateDigest: forensicSha256Digest(allEvidence),
      ...(pocReceiptRef === undefined ? {} : { pocReceiptRef }),
      ...(vulnerableControlReceiptRef === undefined ? {} : { vulnerableControlReceiptRef }),
      ...(fixedControlReceiptRef === undefined ? {} : { fixedControlReceiptRef }),
      derivedVulnerableTestOutcome,
      derivedFixedTestOutcome,
      vulnerableControlPassed,
      fixedControlPassed,
      circularVerificationRejected: true,
      admittedWorkerReceiptsResolved: true,
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
    admittedProvenancePassed: results.every(
      (result) =>
        result.evidenceProvenance === "admitted_worker_run" &&
        result.admittedWorkerReceiptsResolved &&
        result.initialVerdictAuthority === "durable_first_verdict_ledger",
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
  deriveControlTestOutcome,
  planSchema: LoupeVerificationPlanSchema,
  controlTerminationSchema: LoupeControlTerminationSchema,
  admittedWorkerSchema: LoupeAdmittedWorkerSchema,
  evidenceSchema: LoupeVerificationEvidenceSchema,
  initialVerdictSchema: LoupeInitialVerdictSchema,
  resultSchema: LoupeVerificationResultSchema,
  releaseGateSchema: LoupeVerificationReleaseGateSchema,
  admittedWorkerReceiptSchema: LoupeAdmittedWorkerReceiptSchema,
});
