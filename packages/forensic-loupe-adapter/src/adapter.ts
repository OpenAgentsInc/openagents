import { Schema as S } from "effect";

import {
  FORENSIC_FINDING_VERSION,
  FORENSIC_HYPOTHESIS_VERSION,
  FORENSIC_PROMPT_ARTIFACT_VERSION,
  BoundedRefs,
  ForensicBudgetSchema,
  ForensicCoverageManifestSchema,
  ForensicFindingSchema,
  ForensicHypothesisSchema,
  ForensicPath,
  ForensicPromptArtifactSchema,
  ForensicPromptIrSchema,
  ForensicRef,
  ForensicScanProfileSchema,
  ForensicSourceBundleSchema,
  ForensicTimestamp,
  PositiveInteger,
  Sha256Digest,
  forensicPromptArtifactDigest,
  forensicSha256Digest,
  strictDecode,
  type ForensicCoverageManifest,
  type ForensicFinding,
  type ForensicHypothesis,
  type ForensicPromptArtifact,
  type ForensicPromptIr,
  type ForensicScanProfile,
  type ForensicSourceBundle,
} from "@openagentsinc/forensic-contract";

export const LOUPE_FORENSIC_EXECUTION_PLAN_VERSION =
  "openagents.loupe_forensic_execution_plan.v1" as const;
export const LOUPE_FORENSIC_DRIVER_EVENT_VERSION =
  "openagents.loupe_forensic_driver_event.v1" as const;
export const LOUPE_FORENSIC_OUTPUT_ENVELOPE_VERSION =
  "openagents.loupe_forensic_output_envelope.v1" as const;
export const LOUPE_VERIFICATION_DEFAULT_MODE = "discovery_only" as const;

export const SUBMIT_FORENSIC_FINDING_TOOL_REF = "tool.submit_forensic_finding.v1" as const;
export const SUBMIT_FORENSIC_HYPOTHESIS_TOOL_REF = "tool.submit_forensic_hypothesis.v1" as const;

const MANAGED_SANDBOX_NETWORK_POLICY_REF =
  "network-policy-ref://openagents/managed-sandbox/broker-only-v1" as const;

export const LOUPE_FORENSIC_ADAPTER_REVISION_DIGEST = forensicSha256Digest({
  executionPlanSchema: LOUPE_FORENSIC_EXECUTION_PLAN_VERSION,
  driverEventSchema: LOUPE_FORENSIC_DRIVER_EVENT_VERSION,
  outputEnvelopeSchema: LOUPE_FORENSIC_OUTPUT_ENVELOPE_VERSION,
  findingSchema: FORENSIC_FINDING_VERSION,
  hypothesisSchema: FORENSIC_HYPOTHESIS_VERSION,
  immutableCheckout: true,
  reporterMode: "manual_no_reporting",
  verificationMode: LOUPE_VERIFICATION_DEFAULT_MODE,
});

const CompiledPrompt = S.String.check(S.isMinLength(1), S.isMaxLength(32_000));

export const LoupeForensicExecutionPlanSchema = S.Struct({
  schema: S.Literal(LOUPE_FORENSIC_EXECUTION_PLAN_VERSION),
  planRef: ForensicRef,
  runRef: ForensicRef,
  adapterRevisionDigest: Sha256Digest,
  promptArtifactRef: ForensicRef,
  promptDigest: Sha256Digest,
  modelDigest: Sha256Digest,
  modelParametersDigest: Sha256Digest,
  targetRef: ForensicRef,
  targetDigest: Sha256Digest,
  sourceBundleRef: ForensicRef,
  sourceBundleDigest: Sha256Digest,
  coverageRef: ForensicRef,
  coverageStatus: S.Literals(["complete", "incomplete", "denied"]),
  workerImageDigest: Sha256Digest,
  workerProfileDigest: Sha256Digest,
  networkPolicyRef: S.Literal(MANAGED_SANDBOX_NETWORK_POLICY_REF),
  budget: ForensicBudgetSchema,
  availableToolRefs: BoundedRefs,
  missingToolRefs: BoundedRefs,
  missingDependencyPaths: S.Array(ForensicPath).check(S.isMaxLength(256)),
  toolSurfaceDigest: Sha256Digest,
  findingSchemaRef: S.Literal(FORENSIC_FINDING_VERSION),
  hypothesisSchemaRef: S.Literal(FORENSIC_HYPOTHESIS_VERSION),
  checkoutMode: S.Literal("read_only"),
  reporterMode: S.Literal("manual_no_reporting"),
  verificationMode: S.Literal(LOUPE_VERIFICATION_DEFAULT_MODE),
  outputDisclosureState: S.Literal("private"),
  compiledPrompt: CompiledPrompt,
  createdAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter(
        (plan) =>
          plan.availableToolRefs.includes(SUBMIT_FORENSIC_FINDING_TOOL_REF) &&
          plan.availableToolRefs.includes(SUBMIT_FORENSIC_HYPOTHESIS_TOOL_REF),
        { message: "execution plans require both typed submission tools" },
      ),
      S.makeFilter(
        (plan) =>
          !plan.availableToolRefs.some((toolRef) => plan.missingToolRefs.includes(toolRef)) &&
          new Set(plan.availableToolRefs).size === plan.availableToolRefs.length &&
          new Set(plan.missingToolRefs).size === plan.missingToolRefs.length,
        { message: "available and missing tool surfaces must be unique and disjoint" },
      ),
      S.makeFilter(
        (plan) => plan.toolSurfaceDigest === forensicSha256Digest(plan.availableToolRefs),
        { message: "tool surface digest must bind the advertised available tools" },
      ),
      S.makeFilter((plan) => plan.coverageStatus !== "denied", {
        message: "denied source coverage cannot execute a Loupe forensic plan",
      }),
    ),
  )
  .annotate({ identifier: "LoupeForensicExecutionPlan" });
export interface LoupeForensicExecutionPlan extends S.Schema.Type<
  typeof LoupeForensicExecutionPlanSchema
> {}

export const LoupeForensicOutputBindingSchema = S.Struct({
  adapterRevisionDigest: Sha256Digest,
  promptDigest: Sha256Digest,
  modelDigest: Sha256Digest,
  modelParametersDigest: Sha256Digest,
  targetRef: ForensicRef,
  targetDigest: Sha256Digest,
  sourceBundleRef: ForensicRef,
  sourceBundleDigest: Sha256Digest,
  coverageRef: ForensicRef,
  coverageStatus: S.Literals(["complete", "incomplete"]),
  workerImageDigest: Sha256Digest,
  workerProfileDigest: Sha256Digest,
  toolSurfaceDigest: Sha256Digest,
  budgetDigest: Sha256Digest,
  networkPolicyRef: S.Literal(MANAGED_SANDBOX_NETWORK_POLICY_REF),
  verificationMode: S.Literal(LOUPE_VERIFICATION_DEFAULT_MODE),
});
export interface LoupeForensicOutputBinding extends S.Schema.Type<
  typeof LoupeForensicOutputBindingSchema
> {}

const LoupeFindingEnvelopeSchema = S.Struct({
  schema: S.Literal(LOUPE_FORENSIC_OUTPUT_ENVELOPE_VERSION),
  envelopeRef: ForensicRef,
  runRef: ForensicRef,
  lane: S.Literal("finding"),
  binding: LoupeForensicOutputBindingSchema,
  payloadDigest: Sha256Digest,
  finding: ForensicFindingSchema,
});

const LoupeHypothesisEnvelopeSchema = S.Struct({
  schema: S.Literal(LOUPE_FORENSIC_OUTPUT_ENVELOPE_VERSION),
  envelopeRef: ForensicRef,
  runRef: ForensicRef,
  lane: S.Literal("hypothesis"),
  binding: LoupeForensicOutputBindingSchema,
  payloadDigest: Sha256Digest,
  hypothesis: ForensicHypothesisSchema,
});

export const LoupeForensicOutputEnvelopeSchema = S.Union([
  LoupeFindingEnvelopeSchema,
  LoupeHypothesisEnvelopeSchema,
])
  .pipe(
    S.check(
      S.makeFilter(
        (envelope) => {
          const payload = envelope.lane === "finding" ? envelope.finding : envelope.hypothesis;
          return (
            envelope.runRef === payload.runRef &&
            envelope.payloadDigest === forensicSha256Digest(payload)
          );
        },
        { message: "typed output envelope must bind its run and exact payload bytes" },
      ),
      S.makeFilter(
        (envelope) => envelope.lane !== "finding" || envelope.finding.disclosureState === "private",
        { message: "adapter findings must enter the manual reporter as private" },
      ),
      S.makeFilter(
        (envelope) => envelope.lane !== "hypothesis" || envelope.hypothesis.state === "unverified",
        { message: "adapter hypotheses must enter the unverified lane" },
      ),
    ),
  )
  .annotate({ identifier: "LoupeForensicOutputEnvelope" });
export type LoupeForensicOutputEnvelope = typeof LoupeForensicOutputEnvelopeSchema.Type;

export const LoupeForensicDriverObservationSchema = S.Struct({
  kind: S.Literals([
    "focal_file_started",
    "tranche_started",
    "tool_observed",
    "dependency_crossed",
    "error_observed",
  ]),
  subjectRef: ForensicRef,
  detailRefs: BoundedRefs,
  observedAt: ForensicTimestamp,
});
export interface LoupeForensicDriverObservation extends S.Schema.Type<
  typeof LoupeForensicDriverObservationSchema
> {}

export const LoupeForensicDriverEventSchema = S.Struct({
  schema: S.Literal(LOUPE_FORENSIC_DRIVER_EVENT_VERSION),
  eventRef: ForensicRef,
  runRef: ForensicRef,
  sequence: PositiveInteger,
  kind: S.Literals([
    "focal_file_started",
    "tranche_started",
    "tool_observed",
    "dependency_crossed",
    "finding_submitted",
    "hypothesis_submitted",
    "error_observed",
    "settled",
  ]),
  bindingDigest: Sha256Digest,
  subjectRef: ForensicRef,
  detailRefs: BoundedRefs,
  observedAt: ForensicTimestamp,
}).annotate({ identifier: "LoupeForensicDriverEvent" });
export interface LoupeForensicDriverEvent extends S.Schema.Type<
  typeof LoupeForensicDriverEventSchema
> {}

const deepFreeze = <Value>(value: Value): Value => {
  if (value !== null && typeof value === "object" && !Object.isFrozen(value)) {
    for (const child of Object.values(value)) deepFreeze(child);
    Object.freeze(value);
  }
  return value;
};

export interface CreateForensicPromptArtifactInput {
  readonly promptArtifactRef: string;
  readonly parentPromptArtifactRef?: string;
  readonly promptIr: ForensicPromptIr;
  readonly exampleRefs: ReadonlyArray<string>;
  readonly parameterRefs: ReadonlyArray<string>;
  readonly datasetRevisionRef: string;
  readonly compatibilityRefs: ReadonlyArray<string>;
  readonly createdAt: string;
}

export const createForensicPromptArtifact = (
  input: CreateForensicPromptArtifactInput,
): ForensicPromptArtifact => {
  const promptIr = strictDecode(ForensicPromptIrSchema, input.promptIr);
  const digestInput = {
    ...(input.parentPromptArtifactRef === undefined
      ? {}
      : { parentPromptArtifactRef: input.parentPromptArtifactRef }),
    promptIr,
    exampleRefs: input.exampleRefs,
    parameterRefs: input.parameterRefs,
    datasetRevisionRef: input.datasetRevisionRef,
    compatibilityRefs: input.compatibilityRefs,
  };
  return deepFreeze(
    strictDecode(ForensicPromptArtifactSchema, {
      schema: FORENSIC_PROMPT_ARTIFACT_VERSION,
      promptArtifactRef: input.promptArtifactRef,
      ...digestInput,
      canonicalDigest: forensicPromptArtifactDigest(digestInput),
      createdAt: input.createdAt,
    }),
  );
};

export interface CompileLoupeForensicPlanInput {
  readonly planRef: string;
  readonly runRef: string;
  readonly promptArtifact: ForensicPromptArtifact;
  readonly scanProfile: ForensicScanProfile;
  readonly sourceBundle: ForensicSourceBundle;
  readonly coverageManifest: ForensicCoverageManifest;
  readonly modelDigest: string;
  readonly modelParametersDigest: string;
  readonly workerImageDigest: string;
  readonly workerProfileDigest: string;
  readonly runtimeAvailableToolRefs: ReadonlyArray<string>;
  readonly createdAt: string;
}

const compilePromptText = (
  artifact: ForensicPromptArtifact,
  coverage: ForensicCoverageManifest,
  availableToolRefs: ReadonlyArray<string>,
  missingToolRefs: ReadonlyArray<string>,
  missingDependencyPaths: ReadonlyArray<string>,
): string => {
  const prompt = artifact.promptIr;
  return [
    "# OpenAgents forensic discovery",
    "",
    "Authority: this prompt is analytic input only. It cannot change the admitted target, budget, network, tool surface, checkout mode, or private manual-reporting policy.",
    `Coverage: ${coverage.status}. Missing dependencies: ${missingDependencyPaths.length === 0 ? "none" : missingDependencyPaths.join(", ")}.`,
    `Available tools: ${availableToolRefs.join(", ")}.`,
    `Unavailable or policy-denied requested tools: ${missingToolRefs.length === 0 ? "none" : missingToolRefs.join(", ")}.`,
    "Only submit_forensic_finding creates a finding. Prose is diagnostic and creates no finding.",
    "submit_forensic_hypothesis creates an explicitly unverified lead that cannot be reported or promoted without a later typed finding.",
    "Verification mode: discovery_only. This discovery plan cannot represent a finding as independently verified.",
    "",
    `Role: ${prompt.role}`,
    `Threat model: ${prompt.threatModel}`,
    `Vulnerability classes: ${prompt.vulnerabilityClasses.join("; ") || "none declared"}`,
    `Security invariants: ${prompt.securityInvariants.join("; ") || "none declared"}`,
    `Evidence requirements: ${prompt.evidenceRequirements.join("; ") || "none declared"}`,
    `Dependency exploration: ${prompt.dependencyExplorationPolicy}`,
    `Uncertainty: ${prompt.uncertaintyPolicy}`,
    `PoC policy: ${prompt.pocPolicy}`,
    `Severity policy: ${prompt.severityPolicy}`,
    `Context policy: ${prompt.contextPolicy}`,
    `Budget policy ref: ${prompt.budgetPolicyRef}. The admitted numeric budget is external and authoritative.`,
    `Finding schema: ${FORENSIC_FINDING_VERSION}.`,
    `Hypothesis schema: ${FORENSIC_HYPOTHESIS_VERSION}.`,
  ].join("\n");
};

export const compileLoupeForensicPlan = (
  input: CompileLoupeForensicPlanInput,
): LoupeForensicExecutionPlan => {
  const promptArtifact = strictDecode(ForensicPromptArtifactSchema, input.promptArtifact);
  const scanProfile = strictDecode(ForensicScanProfileSchema, input.scanProfile);
  const sourceBundle = strictDecode(ForensicSourceBundleSchema, input.sourceBundle);
  const coverageManifest = strictDecode(ForensicCoverageManifestSchema, input.coverageManifest);
  if (scanProfile.promptArtifactRef !== promptArtifact.promptArtifactRef) {
    throw new Error("scan profile must bind the exact prompt artifact ref");
  }
  if (coverageManifest.bundleRef !== sourceBundle.bundleRef) {
    throw new Error("coverage manifest must bind the exact immutable source bundle");
  }
  if (scanProfile.networkPolicyRef !== MANAGED_SANDBOX_NETWORK_POLICY_REF) {
    throw new Error("Loupe adapter requires the managed broker-only network policy");
  }
  if (
    promptArtifact.promptIr.findingSchemaRef !== FORENSIC_FINDING_VERSION ||
    promptArtifact.promptIr.hypothesisSchemaRef !== FORENSIC_HYPOTHESIS_VERSION
  ) {
    throw new Error("Loupe adapter requires the canonical typed finding and hypothesis schemas");
  }

  const admittedTools = new Set(scanProfile.toolRefs);
  const runtimeTools = new Set(input.runtimeAvailableToolRefs);
  const requestedTools = [...promptArtifact.promptIr.toolPolicyRefs];
  const effectiveRequestedTools = requestedTools.filter(
    (toolRef) => admittedTools.has(toolRef) && runtimeTools.has(toolRef),
  );
  const missingToolRefs = requestedTools.filter(
    (toolRef) => !admittedTools.has(toolRef) || !runtimeTools.has(toolRef),
  );
  const availableToolRefs = [
    SUBMIT_FORENSIC_FINDING_TOOL_REF,
    SUBMIT_FORENSIC_HYPOTHESIS_TOOL_REF,
    ...effectiveRequestedTools,
  ];
  const missingDependencyPaths = coverageManifest.entries
    .filter(
      (entry) =>
        entry.classification === "dependency" && entry.required && entry.presence !== "present",
    )
    .map((entry) => entry.path);
  const compiledPrompt = compilePromptText(
    promptArtifact,
    coverageManifest,
    availableToolRefs,
    missingToolRefs,
    missingDependencyPaths,
  );

  return deepFreeze(
    strictDecode(LoupeForensicExecutionPlanSchema, {
      schema: LOUPE_FORENSIC_EXECUTION_PLAN_VERSION,
      planRef: input.planRef,
      runRef: input.runRef,
      adapterRevisionDigest: LOUPE_FORENSIC_ADAPTER_REVISION_DIGEST,
      promptArtifactRef: promptArtifact.promptArtifactRef,
      promptDigest: promptArtifact.canonicalDigest,
      modelDigest: input.modelDigest,
      modelParametersDigest: input.modelParametersDigest,
      targetRef: sourceBundle.targetRef,
      targetDigest: sourceBundle.treeDigest,
      sourceBundleRef: sourceBundle.bundleRef,
      sourceBundleDigest: sourceBundle.sourceDigest,
      coverageRef: coverageManifest.coverageRef,
      coverageStatus: coverageManifest.status,
      workerImageDigest: input.workerImageDigest,
      workerProfileDigest: input.workerProfileDigest,
      networkPolicyRef: MANAGED_SANDBOX_NETWORK_POLICY_REF,
      budget: scanProfile.budget,
      availableToolRefs,
      missingToolRefs,
      missingDependencyPaths,
      toolSurfaceDigest: forensicSha256Digest(availableToolRefs),
      findingSchemaRef: FORENSIC_FINDING_VERSION,
      hypothesisSchemaRef: FORENSIC_HYPOTHESIS_VERSION,
      checkoutMode: "read_only",
      reporterMode: "manual_no_reporting",
      verificationMode: LOUPE_VERIFICATION_DEFAULT_MODE,
      outputDisclosureState: "private",
      compiledPrompt,
      createdAt: input.createdAt,
    }),
  );
};

export type LoupeTypedSubmission =
  | Readonly<{ lane: "finding"; payload: unknown }>
  | Readonly<{ lane: "hypothesis"; payload: unknown }>;

const LoupeTypedSubmissionSchema = S.Union([
  S.Struct({ lane: S.Literal("finding"), payload: S.Unknown }),
  S.Struct({ lane: S.Literal("hypothesis"), payload: S.Unknown }),
]);

export const LoupeBackendResponseSchema = S.Struct({
  diagnosticProse: S.String.check(S.isMaxLength(32_000)),
  submissions: S.Array(LoupeTypedSubmissionSchema).check(S.isMaxLength(512)),
  observations: S.Array(S.Unknown).check(S.isMaxLength(10_000)),
  settledAt: ForensicTimestamp,
}).annotate({ identifier: "LoupeBackendResponse" });
export interface LoupeBackendResponse extends S.Schema.Type<typeof LoupeBackendResponseSchema> {}

export interface LoupeForensicBackend {
  readonly observeCheckoutDigest: () => Promise<string>;
  readonly runDiscovery: (plan: LoupeForensicExecutionPlan) => Promise<LoupeBackendResponse>;
}

export interface LoupeForensicExecutionResult {
  readonly planDigest: string;
  readonly coverageStatus: "complete" | "incomplete";
  readonly diagnosticProse: string;
  readonly outputs: ReadonlyArray<LoupeForensicOutputEnvelope>;
  readonly events: ReadonlyArray<LoupeForensicDriverEvent>;
  readonly checkoutDigestBefore: string;
  readonly checkoutDigestAfter: string;
  readonly reporterMode: "manual_no_reporting";
  readonly verificationMode: "discovery_only";
}

const outputBinding = (plan: LoupeForensicExecutionPlan): LoupeForensicOutputBinding =>
  strictDecode(LoupeForensicOutputBindingSchema, {
    adapterRevisionDigest: plan.adapterRevisionDigest,
    promptDigest: plan.promptDigest,
    modelDigest: plan.modelDigest,
    modelParametersDigest: plan.modelParametersDigest,
    targetRef: plan.targetRef,
    targetDigest: plan.targetDigest,
    sourceBundleRef: plan.sourceBundleRef,
    sourceBundleDigest: plan.sourceBundleDigest,
    coverageRef: plan.coverageRef,
    coverageStatus: plan.coverageStatus,
    workerImageDigest: plan.workerImageDigest,
    workerProfileDigest: plan.workerProfileDigest,
    toolSurfaceDigest: plan.toolSurfaceDigest,
    budgetDigest: forensicSha256Digest(plan.budget),
    networkPolicyRef: plan.networkPolicyRef,
    verificationMode: plan.verificationMode,
  });

const typedEnvelope = (
  plan: LoupeForensicExecutionPlan,
  submission: LoupeTypedSubmission,
  index: number,
): LoupeForensicOutputEnvelope => {
  const binding = outputBinding(plan);
  if (submission.lane === "finding") {
    const finding = strictDecode(ForensicFindingSchema, submission.payload);
    return strictDecode(LoupeForensicOutputEnvelopeSchema, {
      schema: LOUPE_FORENSIC_OUTPUT_ENVELOPE_VERSION,
      envelopeRef: `envelope.${plan.runRef}.finding.${index + 1}`,
      runRef: plan.runRef,
      lane: "finding",
      binding,
      payloadDigest: forensicSha256Digest(finding),
      finding,
    });
  }
  const hypothesis = strictDecode(ForensicHypothesisSchema, submission.payload);
  return strictDecode(LoupeForensicOutputEnvelopeSchema, {
    schema: LOUPE_FORENSIC_OUTPUT_ENVELOPE_VERSION,
    envelopeRef: `envelope.${plan.runRef}.hypothesis.${index + 1}`,
    runRef: plan.runRef,
    lane: "hypothesis",
    binding,
    payloadDigest: forensicSha256Digest(hypothesis),
    hypothesis,
  });
};

export const executeLoupeForensicPlan = async (
  untrustedPlan: unknown,
  backend: LoupeForensicBackend,
): Promise<LoupeForensicExecutionResult> => {
  const plan = deepFreeze(strictDecode(LoupeForensicExecutionPlanSchema, untrustedPlan));
  if (plan.coverageStatus === "denied") {
    throw new Error("denied source coverage cannot execute a Loupe forensic plan");
  }
  const checkoutDigestBefore = strictDecode(Sha256Digest, await backend.observeCheckoutDigest());
  if (checkoutDigestBefore !== plan.sourceBundleDigest) {
    throw new Error("checkout digest before execution does not match the immutable source bundle");
  }

  let response: LoupeBackendResponse | undefined;
  let backendFailure: unknown;
  try {
    response = await backend.runDiscovery(plan);
  } catch (error) {
    backendFailure = error;
  }
  const checkoutDigestAfter = strictDecode(Sha256Digest, await backend.observeCheckoutDigest());
  if (checkoutDigestAfter !== checkoutDigestBefore) {
    throw new Error("Loupe backend mutated the immutable checkout");
  }
  if (backendFailure !== undefined) throw backendFailure;
  if (response === undefined) throw new Error("Loupe backend returned no structural response");
  response = strictDecode(LoupeBackendResponseSchema, response);

  const observations = response.observations.map((observation) =>
    strictDecode(LoupeForensicDriverObservationSchema, observation),
  );
  const outputs = response.submissions.map((submission, index) =>
    typedEnvelope(plan, submission, index),
  );
  const bindingDigest = forensicSha256Digest(outputBinding(plan));
  const events: Array<LoupeForensicDriverEvent> = observations.map((observation, index) =>
    strictDecode(LoupeForensicDriverEventSchema, {
      schema: LOUPE_FORENSIC_DRIVER_EVENT_VERSION,
      eventRef: `event.${plan.runRef}.${index + 1}`,
      runRef: plan.runRef,
      sequence: index + 1,
      kind: observation.kind,
      bindingDigest,
      subjectRef: observation.subjectRef,
      detailRefs: observation.detailRefs,
      observedAt: observation.observedAt,
    }),
  );
  for (const output of outputs) {
    events.push(
      strictDecode(LoupeForensicDriverEventSchema, {
        schema: LOUPE_FORENSIC_DRIVER_EVENT_VERSION,
        eventRef: `event.${plan.runRef}.${events.length + 1}`,
        runRef: plan.runRef,
        sequence: events.length + 1,
        kind: output.lane === "finding" ? "finding_submitted" : "hypothesis_submitted",
        bindingDigest,
        subjectRef: output.envelopeRef,
        detailRefs: [output.payloadDigest],
        observedAt:
          output.lane === "finding" ? output.finding.submittedAt : output.hypothesis.submittedAt,
      }),
    );
  }
  events.push(
    strictDecode(LoupeForensicDriverEventSchema, {
      schema: LOUPE_FORENSIC_DRIVER_EVENT_VERSION,
      eventRef: `event.${plan.runRef}.${events.length + 1}`,
      runRef: plan.runRef,
      sequence: events.length + 1,
      kind: "settled",
      bindingDigest,
      subjectRef: plan.planRef,
      detailRefs: outputs.map((output) => output.envelopeRef),
      observedAt: strictDecode(ForensicTimestamp, response.settledAt),
    }),
  );

  return deepFreeze({
    planDigest: forensicSha256Digest(plan),
    coverageStatus: plan.coverageStatus,
    diagnosticProse: response.diagnosticProse,
    outputs,
    events,
    checkoutDigestBefore,
    checkoutDigestAfter,
    reporterMode: plan.reporterMode,
    verificationMode: plan.verificationMode,
  });
};

export const loupeForensicAdapter = Object.freeze({
  createPromptArtifact: createForensicPromptArtifact,
  compilePlan: compileLoupeForensicPlan,
  executePlan: executeLoupeForensicPlan,
  executionPlanSchema: LoupeForensicExecutionPlanSchema,
  backendResponseSchema: LoupeBackendResponseSchema,
  outputEnvelopeSchema: LoupeForensicOutputEnvelopeSchema,
  driverEventSchema: LoupeForensicDriverEventSchema,
  adapterRevisionDigest: LOUPE_FORENSIC_ADAPTER_REVISION_DIGEST,
});
