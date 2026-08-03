import { Schema as S } from "effect";

import { forensicSha256Digest, strictDecode } from "./canonical.ts";
import {
  BoundedRefs,
  CommitSha,
  ForensicRef,
  ForensicTimestamp,
  NonEmptyBoundedRefs,
  NonNegativeInteger,
  PositiveInteger,
  Sha256Digest,
  ShortText,
} from "./primitives.ts";

export const ARTIFACT_WITNESS_CAPTURE_VERSION = "openagents.artifact_witness_capture.v2" as const;
export const ARTIFACT_WITNESS_REPORT_VERSION = "openagents.artifact_witness_report.v2" as const;

export const ArtifactWitnessResult = S.Literals(["satisfied", "violated", "not_proven"]);
export type ArtifactWitnessResult = typeof ArtifactWitnessResult.Type;

export const ArtifactWitnessVariant = S.Literals(["vulnerable", "fixed", "fault_build"]);
export type ArtifactWitnessVariant = typeof ArtifactWitnessVariant.Type;

export const BuildArtifactKind = S.Literals([
  "preprocessed_source",
  "compiler_command",
  "object",
  "archive",
  "symbol_table",
  "link_map",
  "firmware",
  "debug_metadata",
  "build_log",
]);
export type BuildArtifactKind = typeof BuildArtifactKind.Type;

const AvailableBuildArtifactSchema = S.Struct({
  artifactRef: ForensicRef,
  digest: Sha256Digest,
  kind: BuildArtifactKind,
  status: S.Literal("available"),
});

const MissingBuildArtifactSchema = S.Struct({
  artifactRef: ForensicRef,
  kind: BuildArtifactKind,
  status: S.Literal("missing"),
  unavailableReasonRef: ForensicRef,
});

export const BuildArtifactSchema = S.Union([
  AvailableBuildArtifactSchema,
  MissingBuildArtifactSchema,
]);
export type BuildArtifact = typeof BuildArtifactSchema.Type;

export const PreprocessedMacroObservationSchema = S.Struct({
  macroName: ForensicRef,
  preprocessedArtifactRef: ForensicRef,
  value: S.String,
});
export type PreprocessedMacroObservation = typeof PreprocessedMacroObservationSchema.Type;

export const SymbolProviderObservationSchema = S.Struct({
  archiveArtifactRef: ForensicRef,
  linkMapArtifactRef: ForensicRef,
  objectArtifactRef: ForensicRef,
  providerRef: ForensicRef,
  symbolName: ForensicRef,
  symbolTableArtifactRef: ForensicRef,
});
export type SymbolProviderObservation = typeof SymbolProviderObservationSchema.Type;

export const ArtifactCallEdgeSchema = S.Struct({
  evidenceArtifactRefs: S.Array(ForensicRef).check(S.isMinLength(1), S.isMaxLength(16)),
  fromSymbol: ForensicRef,
  toSymbol: ForensicRef,
});
export type ArtifactCallEdge = typeof ArtifactCallEdgeSchema.Type;

export const RetainedWidthObservationSchema = S.Struct({
  evidenceArtifactRefs: S.Array(ForensicRef).check(S.isMinLength(1), S.isMaxLength(16)),
  retainedBits: NonNegativeInteger,
  symbolName: ForensicRef,
  truncationObserved: S.Boolean,
});
export type RetainedWidthObservation = typeof RetainedWidthObservationSchema.Type;

export const SymbolInventoryEntrySchema = S.Struct({
  definedInArtifactRef: ForensicRef,
  providerRef: ForensicRef,
  symbolName: ForensicRef,
});
export type SymbolInventoryEntry = typeof SymbolInventoryEntrySchema.Type;

/**
 * How a capture reached this repository.
 *
 * A `conformance_vector` is a checked-in schema/evaluator exercise. It is never
 * evidence about any real firmware, and the Coldcard suite gate below refuses
 * it. Only an `admitted_worker_run` — a capture bound to one exact OpenAgents
 * Cloud managed-sandbox generation, guest image, and its emitted receipts —
 * can carry an acceptance-level claim about a build.
 */
export const ArtifactWitnessProvenanceSchema = S.Union([
  S.Struct({
    conformanceNoteRef: ForensicRef,
    kind: S.Literal("conformance_vector"),
  }),
  S.Struct({
    guestImageDigest: Sha256Digest,
    isolationClass: S.Literal("gce_vm"),
    kind: S.Literal("admitted_worker_run"),
    providerKind: S.Literal("live_gce"),
    receiptRefs: NonEmptyBoundedRefs,
    resourceGeneration: PositiveInteger,
    sandboxRef: ForensicRef,
  }),
]);
export type ArtifactWitnessProvenance = typeof ArtifactWitnessProvenanceSchema.Type;

/**
 * The observed termination of the build process.
 *
 * The capture reports the exit status it saw; it does not report whether the
 * build "succeeded". That conclusion is derived by {@link deriveBuildOutcome},
 * which additionally requires a linked firmware image before it will call an
 * exit-zero build successful.
 */
export const BuildTerminationSchema = S.Union([
  S.Struct({
    buildLogArtifactRef: ForensicRef,
    exitStatus: NonNegativeInteger,
    status: S.Literal("observed"),
  }),
  S.Struct({
    status: S.Literal("unobserved"),
    unavailableReasonRef: ForensicRef,
  }),
]);
export type BuildTermination = typeof BuildTerminationSchema.Type;

export const ArtifactWitnessCaptureSchema = S.Struct({
  schema: S.Literal(ARTIFACT_WITNESS_CAPTURE_VERSION),
  artifacts: S.Array(BuildArtifactSchema).check(S.isMinLength(1), S.isMaxLength(10_000)),
  buildConfigurationDigest: Sha256Digest,
  buildTermination: BuildTerminationSchema,
  callEdges: S.Array(ArtifactCallEdgeSchema).check(S.isMaxLength(10_000)),
  callGraphSourceArtifactRefs: S.Array(ForensicRef).check(S.isMaxLength(10_000)),
  captureRef: ForensicRef,
  capturedAt: ForensicTimestamp,
  faultMutationRef: S.optionalKey(ForensicRef),
  macroObservations: S.Array(PreprocessedMacroObservationSchema).check(S.isMaxLength(10_000)),
  provenance: ArtifactWitnessProvenanceSchema,
  sourceBundleDigest: Sha256Digest,
  symbolInventory: S.Array(SymbolInventoryEntrySchema).check(S.isMaxLength(100_000)),
  symbolInventorySourceArtifactRefs: S.Array(ForensicRef).check(S.isMaxLength(10_000)),
  symbolProviders: S.Array(SymbolProviderObservationSchema).check(S.isMaxLength(10_000)),
  targetCommit: CommitSha,
  targetSnapshotRef: ForensicRef,
  toolchainDigest: Sha256Digest,
  unresolvedIndirectCallSites: NonNegativeInteger,
  variant: ArtifactWitnessVariant,
  widthObservations: S.Array(RetainedWidthObservationSchema).check(S.isMaxLength(10_000)),
  workerPlacementRef: ForensicRef,
  workerProfileDigest: Sha256Digest,
})
  .pipe(
    S.check(
      S.makeFilter(
        (capture) => capture.variant === "fault_build" || capture.faultMutationRef === undefined,
        { message: "only a fault build may carry a fault mutation" },
      ),
      S.makeFilter(
        (capture) => capture.variant !== "fault_build" || capture.faultMutationRef !== undefined,
        { message: "fault builds require an explicit mutation reference" },
      ),
      S.makeFilter(
        (capture) =>
          new Set(capture.artifacts.map((artifact) => artifact.artifactRef)).size ===
          capture.artifacts.length,
        { message: "artifact witness capture refs must be unique" },
      ),
      S.makeFilter(
        (capture) => {
          const available = new Map(
            capture.artifacts
              .filter((artifact) => artifact.status === "available")
              .map((artifact) => [artifact.artifactRef, artifact.kind]),
          );
          return (
            capture.macroObservations.every(
              (observation) =>
                available.get(observation.preprocessedArtifactRef) === "preprocessed_source",
            ) &&
            capture.symbolProviders.every(
              (observation) =>
                available.get(observation.objectArtifactRef) === "object" &&
                available.get(observation.archiveArtifactRef) === "archive" &&
                available.get(observation.symbolTableArtifactRef) === "symbol_table" &&
                available.get(observation.linkMapArtifactRef) === "link_map",
            ) &&
            [...capture.callEdges, ...capture.widthObservations].every((observation) =>
              observation.evidenceArtifactRefs.every((reference) => available.has(reference)),
            ) &&
            capture.symbolInventory.every((entry) => {
              const kind = available.get(entry.definedInArtifactRef);
              return kind === "object" || kind === "archive";
            }) &&
            capture.symbolInventorySourceArtifactRefs.every((reference) =>
              available.has(reference),
            ) &&
            capture.callGraphSourceArtifactRefs.every((reference) => available.has(reference)) &&
            (capture.buildTermination.status !== "observed" ||
              available.get(capture.buildTermination.buildLogArtifactRef) === "build_log")
          );
        },
        { message: "artifact observations must bind available artifacts of the required kind" },
      ),
    ),
  )
  .annotate({ identifier: "ArtifactWitnessCapture" });
export type ArtifactWitnessCapture = typeof ArtifactWitnessCaptureSchema.Type;

export const DerivedBuildOutcome = S.Literals(["succeeded", "failed", "not_observed"]);
export type DerivedBuildOutcome = typeof DerivedBuildOutcome.Type;

const availableKinds = (capture: ArtifactWitnessCapture): ReadonlySet<BuildArtifactKind> =>
  new Set(
    capture.artifacts
      .filter((artifact) => artifact.status === "available")
      .map((artifact) => artifact.kind),
  );

const availableRefsOfKind = (
  capture: ArtifactWitnessCapture,
  kinds: ReadonlyArray<BuildArtifactKind>,
): ReadonlyArray<string> =>
  capture.artifacts
    .filter((artifact) => artifact.status === "available" && kinds.includes(artifact.kind))
    .map((artifact) => artifact.artifactRef);

/**
 * Derive the build outcome from what the capture actually contains.
 *
 * An exit status of zero is not by itself a successful build: a build that
 * produced no firmware image did not build the firmware. A capture cannot
 * assert `succeeded` — it can only report the exit status it observed and the
 * artifacts it collected, and this function decides.
 */
const deriveBuildOutcome = (capture: ArtifactWitnessCapture): DerivedBuildOutcome => {
  if (capture.buildTermination.status !== "observed") return "not_observed";
  if (capture.buildTermination.exitStatus !== 0) return "failed";
  return availableKinds(capture).has("firmware") ? "succeeded" : "not_observed";
};

/**
 * A symbol inventory is complete only when it enumerates symbols and its
 * declared sources cover every linked artifact the capture collected. An empty
 * inventory, or one read from a subset of the objects/archives/tables/maps that
 * are present, cannot prove that a forbidden symbol is absent.
 */
const deriveSymbolInventoryComplete = (capture: ArtifactWitnessCapture): boolean => {
  const required = availableRefsOfKind(capture, ["object", "archive", "symbol_table", "link_map"]);
  if (capture.symbolInventory.length === 0 || required.length === 0) return false;
  const sources = new Set(capture.symbolInventorySourceArtifactRefs);
  return required.every((reference) => sources.has(reference));
};

/**
 * A call graph is complete only when every collected object contributed to it
 * and no indirect call site was left unresolved. One unresolved indirect call
 * is enough to make non-reachability unprovable.
 */
const deriveCallGraphComplete = (capture: ArtifactWitnessCapture): boolean => {
  if (capture.unresolvedIndirectCallSites !== 0) return false;
  const required = availableRefsOfKind(capture, ["object", "link_map"]);
  if (required.length === 0) return false;
  const sources = new Set(capture.callGraphSourceArtifactRefs);
  return required.every((reference) => sources.has(reference));
};

const AssertionBase = {
  assertionRef: ForensicRef,
  description: ShortText,
};

export const ArtifactWitnessAssertionSchema = S.Union([
  S.Struct({
    ...AssertionBase,
    expectedValue: S.String,
    kind: S.Literal("preprocessed_macro_value"),
    macroName: ForensicRef,
  }),
  S.Struct({
    ...AssertionBase,
    expectedProviderRef: ForensicRef,
    kind: S.Literal("symbol_provider"),
    symbolName: ForensicRef,
  }),
  S.Struct({
    ...AssertionBase,
    forbiddenProviderRef: ForensicRef,
    forbiddenSymbolName: ForensicRef,
    kind: S.Literal("forbidden_symbol_absent"),
  }),
  S.Struct({
    ...AssertionBase,
    kind: S.Literal("source_to_secret_sink"),
    sinkSymbol: ForensicRef,
    sourceSymbol: ForensicRef,
  }),
  S.Struct({
    ...AssertionBase,
    kind: S.Literal("minimum_retained_width"),
    minimumBits: PositiveInteger,
    symbolName: ForensicRef,
  }),
  S.Struct({
    ...AssertionBase,
    kind: S.Literal("truncation_absent"),
    symbolName: ForensicRef,
  }),
  S.Struct({
    ...AssertionBase,
    expectedMutationRef: ForensicRef,
    kind: S.Literal("fault_build_fails"),
  }),
]);
export type ArtifactWitnessAssertion = typeof ArtifactWitnessAssertionSchema.Type;

export const ArtifactWitnessAssertionResultSchema = S.Struct({
  assertionDigest: Sha256Digest,
  assertionRef: ForensicRef,
  evidenceArtifactRefs: BoundedRefs,
  missingArtifactKinds: S.Array(BuildArtifactKind).check(S.isMaxLength(16)),
  reasonRef: ForensicRef,
  receiptDigest: Sha256Digest,
  status: ArtifactWitnessResult,
}).pipe(
  S.check(
    S.makeFilter(
      (result) =>
        result.receiptDigest ===
        forensicSha256Digest({
          assertionDigest: result.assertionDigest,
          assertionRef: result.assertionRef,
          evidenceArtifactRefs: result.evidenceArtifactRefs,
          missingArtifactKinds: result.missingArtifactKinds,
          reasonRef: result.reasonRef,
          status: result.status,
        }),
      { message: "artifact witness receipt digest must bind the exact result" },
    ),
  ),
);
export type ArtifactWitnessAssertionResult = typeof ArtifactWitnessAssertionResultSchema.Type;

export const ArtifactWitnessReportSchema = S.Struct({
  schema: S.Literal(ARTIFACT_WITNESS_REPORT_VERSION),
  captureDigest: Sha256Digest,
  captureRef: ForensicRef,
  derivedBuildOutcome: DerivedBuildOutcome,
  derivedCallGraphComplete: S.Boolean,
  derivedSymbolInventoryComplete: S.Boolean,
  evaluatedAt: ForensicTimestamp,
  overallResult: ArtifactWitnessResult,
  provenanceKind: S.Literals(["conformance_vector", "admitted_worker_run"]),
  results: S.Array(ArtifactWitnessAssertionResultSchema).check(
    S.isMinLength(1),
    S.isMaxLength(256),
  ),
  statisticalOutputTestsAdmissibleForProvenance: S.Literal(false),
  statisticalOutputTestRefs: BoundedRefs,
  witnessVersionRef: ForensicRef,
});
export type ArtifactWitnessReport = typeof ArtifactWitnessReportSchema.Type;

const REQUIRED_KINDS: Readonly<
  Record<ArtifactWitnessAssertion["kind"], ReadonlyArray<BuildArtifactKind>>
> = {
  fault_build_fails: ["build_log"],
  forbidden_symbol_absent: ["symbol_table", "link_map"],
  minimum_retained_width: ["preprocessed_source", "object", "debug_metadata"],
  preprocessed_macro_value: ["preprocessed_source", "compiler_command"],
  source_to_secret_sink: ["object", "link_map", "debug_metadata"],
  symbol_provider: ["object", "archive", "symbol_table", "link_map"],
  truncation_absent: ["preprocessed_source", "object", "debug_metadata"],
};

const result = (
  assertion: ArtifactWitnessAssertion,
  status: ArtifactWitnessResult,
  reasonRef: string,
  evidenceArtifactRefs: ReadonlyArray<string>,
  missingArtifactKinds: ReadonlyArray<BuildArtifactKind> = [],
): ArtifactWitnessAssertionResult => {
  const value = {
    assertionDigest: forensicSha256Digest(assertion),
    assertionRef: assertion.assertionRef,
    evidenceArtifactRefs,
    missingArtifactKinds,
    reasonRef,
    status,
  };
  return strictDecode(ArtifactWitnessAssertionResultSchema, {
    ...value,
    receiptDigest: forensicSha256Digest(value),
  });
};

const evaluateAssertion = (
  capture: ArtifactWitnessCapture,
  assertion: ArtifactWitnessAssertion,
): ArtifactWitnessAssertionResult => {
  const requiredKinds = REQUIRED_KINDS[assertion.kind];
  const present = availableKinds(capture);
  const missingKinds = requiredKinds.filter((kind) => !present.has(kind));
  const evidenceRefs = availableRefsOfKind(capture, requiredKinds);
  if (missingKinds.length > 0) {
    return result(
      assertion,
      "not_proven",
      "reason.artifact_witness.required_artifact_missing",
      evidenceRefs,
      missingKinds,
    );
  }

  switch (assertion.kind) {
    case "preprocessed_macro_value": {
      const observation = capture.macroObservations.find(
        (candidate) => candidate.macroName === assertion.macroName,
      );
      if (observation === undefined) {
        return result(
          assertion,
          "not_proven",
          "reason.artifact_witness.macro_not_observed",
          evidenceRefs,
        );
      }
      return result(
        assertion,
        observation.value === assertion.expectedValue ? "satisfied" : "violated",
        observation.value === assertion.expectedValue
          ? "reason.artifact_witness.macro_value_observed"
          : "reason.artifact_witness.macro_value_mismatch",
        [...evidenceRefs, observation.preprocessedArtifactRef],
      );
    }
    case "symbol_provider": {
      const observation = capture.symbolProviders.find(
        (candidate) => candidate.symbolName === assertion.symbolName,
      );
      if (observation === undefined) {
        return result(
          assertion,
          "not_proven",
          "reason.artifact_witness.symbol_provider_not_observed",
          evidenceRefs,
        );
      }
      return result(
        assertion,
        observation.providerRef === assertion.expectedProviderRef ? "satisfied" : "violated",
        observation.providerRef === assertion.expectedProviderRef
          ? "reason.artifact_witness.symbol_provider_observed"
          : "reason.artifact_witness.symbol_provider_mismatch",
        [
          ...evidenceRefs,
          observation.objectArtifactRef,
          observation.archiveArtifactRef,
          observation.symbolTableArtifactRef,
          observation.linkMapArtifactRef,
        ],
      );
    }
    case "forbidden_symbol_absent": {
      if (!deriveSymbolInventoryComplete(capture)) {
        return result(
          assertion,
          "not_proven",
          "reason.artifact_witness.symbol_inventory_incomplete",
          evidenceRefs,
        );
      }
      const forbidden = capture.symbolInventory.find(
        (entry) =>
          entry.symbolName === assertion.forbiddenSymbolName ||
          entry.providerRef === assertion.forbiddenProviderRef,
      );
      return result(
        assertion,
        forbidden === undefined ? "satisfied" : "violated",
        forbidden === undefined
          ? "reason.artifact_witness.forbidden_symbol_absent"
          : "reason.artifact_witness.forbidden_symbol_present",
        evidenceRefs,
      );
    }
    case "source_to_secret_sink": {
      const edge = capture.callEdges.find(
        (candidate) =>
          candidate.fromSymbol === assertion.sourceSymbol &&
          candidate.toSymbol === assertion.sinkSymbol,
      );
      if (edge === undefined && !deriveCallGraphComplete(capture)) {
        return result(
          assertion,
          "not_proven",
          "reason.artifact_witness.call_graph_incomplete",
          evidenceRefs,
        );
      }
      return result(
        assertion,
        edge === undefined ? "violated" : "satisfied",
        edge === undefined
          ? "reason.artifact_witness.secret_sink_unreachable"
          : "reason.artifact_witness.secret_sink_reachable",
        edge === undefined ? evidenceRefs : [...evidenceRefs, ...edge.evidenceArtifactRefs],
      );
    }
    case "minimum_retained_width": {
      const width = capture.widthObservations.find(
        (candidate) => candidate.symbolName === assertion.symbolName,
      );
      if (width === undefined) {
        return result(
          assertion,
          "not_proven",
          "reason.artifact_witness.retained_width_not_observed",
          evidenceRefs,
        );
      }
      return result(
        assertion,
        width.retainedBits >= assertion.minimumBits ? "satisfied" : "violated",
        width.retainedBits >= assertion.minimumBits
          ? "reason.artifact_witness.minimum_width_retained"
          : "reason.artifact_witness.retained_width_too_small",
        [...evidenceRefs, ...width.evidenceArtifactRefs],
      );
    }
    case "truncation_absent": {
      const width = capture.widthObservations.find(
        (candidate) => candidate.symbolName === assertion.symbolName,
      );
      if (width === undefined) {
        return result(
          assertion,
          "not_proven",
          "reason.artifact_witness.truncation_not_observed",
          evidenceRefs,
        );
      }
      return result(
        assertion,
        width.truncationObserved ? "violated" : "satisfied",
        width.truncationObserved
          ? "reason.artifact_witness.truncation_observed"
          : "reason.artifact_witness.truncation_absent",
        [...evidenceRefs, ...width.evidenceArtifactRefs],
      );
    }
    case "fault_build_fails": {
      const mutationMatches = capture.faultMutationRef === assertion.expectedMutationRef;
      const outcome = deriveBuildOutcome(capture);
      if (outcome === "not_observed") {
        return result(
          assertion,
          "not_proven",
          "reason.artifact_witness.build_termination_not_observed",
          evidenceRefs,
        );
      }
      const failedClosed =
        capture.variant === "fault_build" && mutationMatches && outcome === "failed";
      return result(
        assertion,
        failedClosed ? "satisfied" : "violated",
        failedClosed
          ? "reason.artifact_witness.fault_build_failed_closed"
          : "reason.artifact_witness.fault_build_did_not_fail_closed",
        evidenceRefs,
      );
    }
  }
};

export interface EvaluateArtifactWitnessInput {
  readonly assertions: ReadonlyArray<ArtifactWitnessAssertion>;
  readonly capture: ArtifactWitnessCapture;
  readonly evaluatedAt: string;
  readonly statisticalOutputTestRefs?: ReadonlyArray<string>;
  readonly witnessVersionRef: string;
}

export const evaluateArtifactWitness = (
  input: EvaluateArtifactWitnessInput,
): ArtifactWitnessReport => {
  const capture = strictDecode(ArtifactWitnessCaptureSchema, input.capture);
  if (input.assertions.length === 0) {
    throw new Error("artifact witness evaluation requires at least one assertion");
  }
  const assertions = input.assertions.map((assertion) =>
    strictDecode(ArtifactWitnessAssertionSchema, assertion),
  );
  const results = assertions.map((assertion) => evaluateAssertion(capture, assertion));
  const overallResult = results.some((entry) => entry.status === "violated")
    ? "violated"
    : results.some((entry) => entry.status === "not_proven")
      ? "not_proven"
      : "satisfied";
  return strictDecode(ArtifactWitnessReportSchema, {
    schema: ARTIFACT_WITNESS_REPORT_VERSION,
    captureDigest: forensicSha256Digest(capture),
    captureRef: capture.captureRef,
    derivedBuildOutcome: deriveBuildOutcome(capture),
    derivedCallGraphComplete: deriveCallGraphComplete(capture),
    derivedSymbolInventoryComplete: deriveSymbolInventoryComplete(capture),
    evaluatedAt: input.evaluatedAt,
    overallResult,
    provenanceKind: capture.provenance.kind,
    results,
    statisticalOutputTestsAdmissibleForProvenance: false,
    statisticalOutputTestRefs: input.statisticalOutputTestRefs ?? [],
    witnessVersionRef: input.witnessVersionRef,
  });
};

export type ColdcardArtifactWitnessSuiteResult =
  | Readonly<{ _tag: "Verified"; reportRefs: ReadonlyArray<string> }>
  | Readonly<{ _tag: "Refused"; blockerRef: string }>;

export const evaluateColdcardArtifactWitnessSuite = (
  captures: ReadonlyArray<ArtifactWitnessCapture>,
  reports: ReadonlyArray<ArtifactWitnessReport>,
): ColdcardArtifactWitnessSuiteResult => {
  const requiredVariants: ReadonlyArray<ArtifactWitnessVariant> = [
    "vulnerable",
    "fixed",
    "fault_build",
  ];
  if (
    captures.length !== 3 ||
    requiredVariants.some(
      (variant) => captures.filter((capture) => capture.variant === variant).length !== 1,
    )
  ) {
    return { _tag: "Refused", blockerRef: "blocker.artifact_witness.variant_matrix_incomplete" };
  }
  const admitted = captures.flatMap((capture) =>
    capture.provenance.kind === "admitted_worker_run" ? [capture.provenance] : [],
  );
  if (admitted.length !== captures.length) {
    return { _tag: "Refused", blockerRef: "blocker.artifact_witness.provenance_not_admitted" };
  }
  if (
    captures.some((capture) =>
      capture.variant === "fault_build"
        ? deriveBuildOutcome(capture) !== "failed"
        : deriveBuildOutcome(capture) !== "succeeded",
    )
  ) {
    return { _tag: "Refused", blockerRef: "blocker.artifact_witness.build_outcome_invalid" };
  }
  if (
    new Set(captures.map((capture) => capture.workerProfileDigest)).size !== 1 ||
    new Set(captures.map((capture) => capture.toolchainDigest)).size !== 1 ||
    new Set(admitted.map((provenance) => provenance.guestImageDigest)).size !== 1
  ) {
    return { _tag: "Refused", blockerRef: "blocker.artifact_witness.worker_profile_drift" };
  }
  const successfulBuildKinds: ReadonlyArray<BuildArtifactKind> = [
    "preprocessed_source",
    "compiler_command",
    "object",
    "archive",
    "symbol_table",
    "link_map",
    "firmware",
    "debug_metadata",
    "build_log",
  ];
  if (
    captures
      .filter((capture) => capture.variant !== "fault_build")
      .some((capture) => {
        const availableKinds = new Set(
          capture.artifacts
            .filter((artifact) => artifact.status === "available")
            .map((artifact) => artifact.kind),
        );
        return successfulBuildKinds.some((kind) => !availableKinds.has(kind));
      })
  ) {
    return { _tag: "Refused", blockerRef: "blocker.artifact_witness.capture_incomplete" };
  }
  const captureDigests = new Set(captures.map((capture) => forensicSha256Digest(capture)));
  if (
    reports.length !== captures.length ||
    new Set(reports.map((report) => report.captureDigest)).size !== captures.length ||
    reports.some(
      (report) => !captureDigests.has(report.captureDigest) || report.overallResult !== "satisfied",
    )
  ) {
    return { _tag: "Refused", blockerRef: "blocker.artifact_witness.report_matrix_failed" };
  }
  return { _tag: "Verified", reportRefs: reports.map((report) => report.captureRef) };
};
