import { createHash } from "node:crypto";
import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  decodeProbeBenchmarkAssignment,
  type ProbeBenchmarkAssignment,
  type ProbeBenchmarkCandidateRefs,
  type ProbeBenchmarkFailureClassification,
  type ProbeBenchmarkFailureFamily,
  type ProbeBenchmarkPolicyFinding,
} from "../contracts/benchmark";
import { JsonValue, ProbePublicProjectionUnsafe, validateProbePublicProjection } from "../contracts/provider-account";
import { makeProbeBenchmarkCloseoutBundle, type ProbeBenchmarkCloseoutBundle } from "./closeout-writer";
import { type ProbeRetainedBenchmarkFixture, type ProbeBenchmarkToolMenuConstraints } from "./fixtures";
import { shortHash } from "./stable-hash";

export const PROBE_GEPA_CANDIDATE_MANIFEST_SCHEMA_VERSION = "psionic.probe_gepa_candidate_manifest.v1" as const;
export const PROBE_GEPA_PROBE_IMPORT_SCHEMA_VERSION = "probe.prompt_candidate_import.v1" as const;
export const PROBE_GEPA_BENCHMARK_CLOUD_IMPORT_SCHEMA_VERSION = "benchmark_cloud.probe_candidate_import.v1" as const;

export const ProbeGepaCandidateComponents = S.Struct({
  closeout_policy: S.String,
  failure_family_playbooks: S.Record(S.String, S.String),
  patch_and_test_policy: S.String,
  probe_system_prompt: S.String,
  signature_selection_policy: S.String,
  terminal_bench_global_playbook: S.String,
  tool_menu_policy: S.String,
});
export type ProbeGepaCandidateComponents = typeof ProbeGepaCandidateComponents.Type;

export const ProbeGepaCandidateComponentHashes = S.Struct({
  closeout_policy: S.String,
  failure_family_playbooks: S.Record(S.String, S.String),
  patch_and_test_policy: S.String,
  probe_system_prompt: S.String,
  signature_selection_policy: S.String,
  terminal_bench_global_playbook: S.String,
  tool_menu_policy: S.String,
});
export type ProbeGepaCandidateComponentHashes = typeof ProbeGepaCandidateComponentHashes.Type;

export const ProbeGepaProbeImportRefs = S.Struct({
  blueprint_candidate_ref: S.String,
  loop_policy_candidate_ref: S.String,
  prompt_candidate_ref: S.String,
  schema_version: S.Literal(PROBE_GEPA_PROBE_IMPORT_SCHEMA_VERSION),
  tool_menu_candidate_ref: S.String,
});
export type ProbeGepaProbeImportRefs = typeof ProbeGepaProbeImportRefs.Type;

export const ProbeGepaBenchmarkCloudImportRefs = S.Struct({
  artifact_contract_refs: S.Array(S.String),
  benchmark_run_manifest_refs: S.Array(S.String),
  schema_version: S.Literal(PROBE_GEPA_BENCHMARK_CLOUD_IMPORT_SCHEMA_VERSION),
  split_refs: S.Array(S.String),
});
export type ProbeGepaBenchmarkCloudImportRefs = typeof ProbeGepaBenchmarkCloudImportRefs.Type;

export const ProbeGepaCandidateSafetyBoundary = S.Struct({
  inherited_runtime_authority_refs: S.Array(S.String),
  no_new_runtime_authority: S.Boolean,
  public_claim_upgrade_authority: S.Boolean,
  release_gate_ref: S.String,
});
export type ProbeGepaCandidateSafetyBoundary = typeof ProbeGepaCandidateSafetyBoundary.Type;

export const ProbeGepaCandidateManifest = S.Struct({
  benchmark_cloud_import: ProbeGepaBenchmarkCloudImportRefs,
  campaign_id: S.String,
  candidate_hash: S.String,
  candidate_id: S.String,
  component_hashes: ProbeGepaCandidateComponentHashes,
  components: ProbeGepaCandidateComponents,
  evaluation_trace_digests: S.Array(S.String),
  manifest_hash: S.String,
  optimizer_acceptance_state: S.Literals(["draft", "optimizer_accepted", "rejected"]),
  optimizer_run_id: S.String,
  parent_candidate_id: S.NullOr(S.String),
  policy_gate_state: S.Literals(["pending", "passed", "failed", "blocked"]),
  probe_import: ProbeGepaProbeImportRefs,
  promotion_state: S.Literals(["draft", "optimizer_accepted", "shadow", "release_candidate", "active", "rejected", "reverted"]),
  runtime_promotion_state: S.Literals(["not_promoted", "shadow", "release_candidate", "active", "reverted"]),
  safety_boundary: ProbeGepaCandidateSafetyBoundary,
  schema_version: S.Literal(PROBE_GEPA_CANDIDATE_MANIFEST_SCHEMA_VERSION),
  split_refs: S.Array(S.String),
  target_failure_families: S.Array(S.String),
  target_suites: S.Array(S.String),
  training_trace_digests: S.Array(S.String),
});
export type ProbeGepaCandidateManifest = typeof ProbeGepaCandidateManifest.Type;

export interface ProbeBenchmarkCandidateExecutionInput {
  readonly assignment: ProbeBenchmarkAssignment | unknown;
  readonly candidateManifest?: ProbeGepaCandidateManifest | unknown;
  readonly completedAt?: string;
  readonly fixture: ProbeRetainedBenchmarkFixture;
  readonly projectedToolRefs?: ReadonlyArray<string>;
  readonly resourceUsageRef?: string;
  readonly runRef?: string;
  readonly selectedSignatureRefs?: ReadonlyArray<string>;
  readonly startedAt?: string;
  readonly toolMenuSnapshot?: JsonValue;
}

export interface ProbeBenchmarkCandidateExecutionResult {
  readonly assignment: ProbeBenchmarkAssignment;
  readonly bundle: ProbeBenchmarkCloseoutBundle;
  readonly candidateComponentRefs: ReadonlyArray<string>;
  readonly candidateHash: string;
  readonly mode: "baseline" | "candidate";
  readonly projectedToolRefs: ReadonlyArray<string>;
  readonly selectedSignatureRefs: ReadonlyArray<string>;
}

export class ProbeBenchmarkCandidateExecutionError extends S.TaggedErrorClass<ProbeBenchmarkCandidateExecutionError>()(
  "ProbeBenchmarkCandidateExecutionError",
  {
    path: S.String,
    reason: S.String,
  },
) {}

export function decodeProbeGepaCandidateManifest(
  value: unknown,
): Effect.Effect<ProbeGepaCandidateManifest, ProbeBenchmarkCandidateExecutionError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbePublicProjection(value, "gepaCandidateManifest");
    const manifest = yield* S.decodeUnknownEffect(ProbeGepaCandidateManifest)(value).pipe(
      Effect.mapError(
        (error) =>
          new ProbeBenchmarkCandidateExecutionError({
            path: "gepaCandidateManifest",
            reason: String(error),
          }),
      ),
    );

    yield* validateProbeGepaCandidateManifest(manifest);
    return manifest;
  });
}

export function runProbeRetainedBenchmarkCandidate(
  input: ProbeBenchmarkCandidateExecutionInput,
): Effect.Effect<
  ProbeBenchmarkCandidateExecutionResult,
  ProbeBenchmarkCandidateExecutionError | ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    const decodedAssignment = yield* decodeProbeBenchmarkAssignment(input.assignment);
    const candidateManifest = input.candidateManifest === undefined
      ? undefined
      : yield* decodeProbeGepaCandidateManifest(input.candidateManifest);
    const selectedSignatureRefs = [...(input.selectedSignatureRefs ?? decodedAssignment.selectedBlueprintSignatureRefs)];
    const projectedToolRefs = [...(input.projectedToolRefs ?? input.fixture.expectedToolMenuConstraints.requiredToolRefs)];

    yield* validateFixtureMatchesAssignment(decodedAssignment, input.fixture);
    yield* validateSelectedSignatures(selectedSignatureRefs, decodedAssignment, input.fixture);
    yield* validateProjectedToolRefs(projectedToolRefs, input.fixture.expectedToolMenuConstraints);

    const candidateHash = candidateManifest?.candidate_hash ?? decodedAssignment.candidateHash;
    const candidateRefs = candidateManifest === undefined
      ? decodedAssignment.candidateRefs
      : candidateRefsFromManifest(candidateManifest.probe_import);
    const candidateComponentRefs = candidateManifest === undefined ? [] : candidateComponentRefsFromManifest(candidateManifest);
    const assignment: ProbeBenchmarkAssignment = {
      ...decodedAssignment,
      candidateHash,
      candidateRefs,
      selectedBlueprintSignatureRefs: selectedSignatureRefs,
    };
    const runRef = input.runRef ?? defaultRunRef(input.fixture.taskId, candidateHash, candidateManifest === undefined);
    const runStatus = defaultRunStatusForFixture(input.fixture);
    const failureClassification = failureClassificationForFixture(input.fixture, runStatus);
    const verifierRef = input.fixture.expectedScore.verifierRef ?? `verifier.probe.retained.${input.fixture.taskId}.v1`;
    const scorerRef = input.fixture.expectedScore.scorerRef ?? "scorer.terminal_bench.binary.v1";
    const verifierResultRefs = [`verifier_result.${verifierRef}.${shortHash(candidateHash)}.${runStatus}`];
    const policyFindings = policyFindingsForCandidate(runRef, candidateManifest);

    const bundle = yield* makeProbeBenchmarkCloseoutBundle({
      assignment,
      candidateComponentRefs,
      completedAt: input.completedAt,
      decisionStepRefs: decisionStepRefsFor(runRef, candidateManifest),
      failureClassification,
      policyFindings,
      resourceUsageRef: input.resourceUsageRef ?? `resource_usage.${runRef}`,
      runRef,
      runStatus,
      scorerRef,
      startedAt: input.startedAt,
      toolMenuSnapshot: input.toolMenuSnapshot ?? toolMenuSnapshotFor(input.fixture, projectedToolRefs, candidateHash),
      verifierRef,
      verifierResultRefs,
    });

    return {
      assignment,
      bundle,
      candidateComponentRefs,
      candidateHash,
      mode: candidateManifest === undefined ? "baseline" : "candidate",
      projectedToolRefs,
      selectedSignatureRefs,
    };
  });
}

function validateProbeGepaCandidateManifest(
  manifest: ProbeGepaCandidateManifest,
): Effect.Effect<void, ProbeBenchmarkCandidateExecutionError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(manifest.candidate_id, "gepaCandidateManifest.candidate_id");
    yield* requireNonEmpty(manifest.candidate_hash, "gepaCandidateManifest.candidate_hash");
    yield* requireNonEmpty(manifest.manifest_hash, "gepaCandidateManifest.manifest_hash");
    yield* requireNonEmptyArray(manifest.split_refs, "gepaCandidateManifest.split_refs");
    yield* requireNonEmptyArray(manifest.target_suites, "gepaCandidateManifest.target_suites");
    yield* requireNonEmptyArray(manifest.target_failure_families, "gepaCandidateManifest.target_failure_families");
    yield* requireNonEmptyArray(
      manifest.benchmark_cloud_import.artifact_contract_refs,
      "gepaCandidateManifest.benchmark_cloud_import.artifact_contract_refs",
    );

    if (!manifest.candidate_hash.startsWith("sha256:")) {
      return yield* candidateError("gepaCandidateManifest.candidate_hash", "must be a sha256 content hash ref");
    }

    const expectedCandidateId = `probe_gepa_candidate.${shortHash(manifest.candidate_hash)}`;
    if (manifest.candidate_id !== expectedCandidateId) {
      return yield* candidateError("gepaCandidateManifest.candidate_id", "must match candidate hash prefix");
    }

    if (!manifest.safety_boundary.no_new_runtime_authority || manifest.safety_boundary.inherited_runtime_authority_refs.length === 0) {
      return yield* candidateError(
        "gepaCandidateManifest.safety_boundary",
        "must inherit runtime authority from the Probe assignment",
      );
    }

    if (manifest.safety_boundary.public_claim_upgrade_authority) {
      return yield* candidateError(
        "gepaCandidateManifest.safety_boundary.public_claim_upgrade_authority",
        "candidate text cannot carry public-claim upgrade authority",
      );
    }

    yield* validateComponentText(manifest);
    yield* validateComponentHashes(manifest);
  });
}

function validateComponentText(
  manifest: ProbeGepaCandidateManifest,
): Effect.Effect<void, ProbeBenchmarkCandidateExecutionError> {
  return Effect.gen(function* () {
    const components = manifest.components;
    const entries = [
      ["probe_system_prompt", components.probe_system_prompt],
      ["terminal_bench_global_playbook", components.terminal_bench_global_playbook],
      ["signature_selection_policy", components.signature_selection_policy],
      ["tool_menu_policy", components.tool_menu_policy],
      ["patch_and_test_policy", components.patch_and_test_policy],
      ["closeout_policy", components.closeout_policy],
      ...Object.entries(components.failure_family_playbooks).map(([family, text]) => [
        `failure_family_playbooks.${family}`,
        text,
      ]),
    ] as ReadonlyArray<readonly [string, string]>;

    if (Object.keys(components.failure_family_playbooks).length === 0) {
      return yield* candidateError("gepaCandidateManifest.components.failure_family_playbooks", "must be non-empty");
    }

    for (const [field, text] of entries) {
      yield* requireNonEmpty(text, `gepaCandidateManifest.components.${field}`);
      if (containsUnsafeCandidateText(text)) {
        return yield* candidateError(
          `gepaCandidateManifest.components.${field}`,
          "contains authority-bypass, credential, private-repo, or release-gate bypass text",
        );
      }
    }
  });
}

function validateComponentHashes(
  manifest: ProbeGepaCandidateManifest,
): Effect.Effect<void, ProbeBenchmarkCandidateExecutionError> {
  return Effect.gen(function* () {
    const expected = componentHashesFor(manifest.components);
    if (JSON.stringify(expected) !== JSON.stringify(manifest.component_hashes)) {
      return yield* candidateError("gepaCandidateManifest.component_hashes", "must match component text hashes");
    }
  });
}

function validateFixtureMatchesAssignment(
  assignment: ProbeBenchmarkAssignment,
  fixture: ProbeRetainedBenchmarkFixture,
): Effect.Effect<void, ProbeBenchmarkCandidateExecutionError> {
  if (!fixture.splitMembership.includes(assignment.split.evidenceSplit)) {
    return candidateError("benchmarkAssignment.split", "assignment split must be admitted by retained fixture");
  }

  if (assignment.task.taskRef !== undefined && !assignment.task.taskRef.includes(fixture.taskId)) {
    return candidateError("benchmarkAssignment.task.taskRef", "task ref must match retained fixture task id");
  }

  return Effect.void;
}

function validateSelectedSignatures(
  selectedSignatureRefs: ReadonlyArray<string>,
  assignment: ProbeBenchmarkAssignment,
  fixture: ProbeRetainedBenchmarkFixture,
): Effect.Effect<void, ProbeBenchmarkCandidateExecutionError> {
  return Effect.gen(function* () {
    for (const ref of selectedSignatureRefs) {
      if (!assignment.selectedBlueprintSignatureRefs.includes(ref)) {
        return yield* candidateError("selectedSignatureRefs", "candidate cannot select signatures outside the assignment");
      }
    }

    for (const ref of fixture.expectedBlueprintSignatureRefs) {
      if (!selectedSignatureRefs.includes(ref)) {
        return yield* candidateError("selectedSignatureRefs", "candidate must preserve fixture-required Blueprint signatures");
      }
    }
  });
}

function validateProjectedToolRefs(
  projectedToolRefs: ReadonlyArray<string>,
  constraints: ProbeBenchmarkToolMenuConstraints,
): Effect.Effect<void, ProbeBenchmarkCandidateExecutionError> {
  return Effect.gen(function* () {
    for (const ref of constraints.requiredToolRefs) {
      if (!projectedToolRefs.includes(ref)) {
        return yield* candidateError("projectedToolRefs", "projected tool menu must include required tool refs");
      }
    }

    for (const ref of projectedToolRefs) {
      if (!constraints.allowedToolRefs.includes(ref)) {
        return yield* candidateError("projectedToolRefs", "projected tool menu can only include allowed tool refs");
      }
      if (constraints.deniedToolRefs.includes(ref)) {
        return yield* candidateError("projectedToolRefs", "projected tool menu cannot include denied tool refs");
      }
    }
  });
}

function candidateRefsFromManifest(probeImport: ProbeGepaProbeImportRefs): ProbeBenchmarkCandidateRefs {
  return {
    blueprintCandidateRef: probeImport.blueprint_candidate_ref,
    loopPolicyCandidateRef: probeImport.loop_policy_candidate_ref,
    promptCandidateRef: probeImport.prompt_candidate_ref,
    toolMenuCandidateRef: probeImport.tool_menu_candidate_ref,
  };
}

function candidateComponentRefsFromManifest(manifest: ProbeGepaCandidateManifest): ReadonlyArray<string> {
  const refs = [
    componentRef(manifest, "probe_system_prompt", manifest.component_hashes.probe_system_prompt),
    componentRef(manifest, "terminal_bench_global_playbook", manifest.component_hashes.terminal_bench_global_playbook),
    componentRef(manifest, "signature_selection_policy", manifest.component_hashes.signature_selection_policy),
    componentRef(manifest, "tool_menu_policy", manifest.component_hashes.tool_menu_policy),
    componentRef(manifest, "patch_and_test_policy", manifest.component_hashes.patch_and_test_policy),
    componentRef(manifest, "closeout_policy", manifest.component_hashes.closeout_policy),
  ];

  return [
    ...refs,
    ...Object.entries(manifest.component_hashes.failure_family_playbooks).map(([family, hash]) =>
      componentRef(manifest, `failure_family_playbooks.${family}`, hash),
    ),
  ];
}

function componentRef(manifest: ProbeGepaCandidateManifest, component: string, hash: string): string {
  return `${manifest.candidate_id}.component.${component}.${shortHash(hash)}`;
}

function defaultRunStatusForFixture(fixture: ProbeRetainedBenchmarkFixture): "failed" | "timed_out" {
  return fixture.expectedScore.expectedOutcome === "runner_stall_detected" ? "timed_out" : "failed";
}

function failureClassificationForFixture(
  fixture: ProbeRetainedBenchmarkFixture,
  runStatus: "failed" | "timed_out",
): ProbeBenchmarkFailureClassification {
  const family: ProbeBenchmarkFailureFamily = runStatus === "timed_out" ? "timeout" : fixture.primaryFailureFamily;
  return {
    classificationRef: `failure_classification.probe.retained.${fixture.taskId}.${family}`,
    family,
    summaryRef: `summary.probe.retained.${fixture.taskId}.${family}`,
  };
}

function policyFindingsForCandidate(
  runRef: string,
  manifest: ProbeGepaCandidateManifest | undefined,
): ReadonlyArray<ProbeBenchmarkPolicyFinding> {
  if (manifest === undefined) {
    return [];
  }

  return [
    {
      findingRef: `policy_finding.probe.benchmark.${runRef}.candidate_import_boundary`,
      severity: "info",
    },
  ];
}

function decisionStepRefsFor(
  runRef: string,
  manifest: ProbeGepaCandidateManifest | undefined,
): ReadonlyArray<string> {
  return manifest === undefined
    ? [`decision_step.probe.benchmark.${runRef}.baseline_assignment`]
    : [
        `decision_step.probe.benchmark.${runRef}.candidate_manifest_decoded`,
        `decision_step.probe.benchmark.${runRef}.candidate_policy_boundary_checked`,
      ];
}

function toolMenuSnapshotFor(
  fixture: ProbeRetainedBenchmarkFixture,
  projectedToolRefs: ReadonlyArray<string>,
  candidateHash: string,
): JsonValue {
  return {
    candidateHash,
    deniedToolRefs: [...fixture.expectedToolMenuConstraints.deniedToolRefs],
    projectedToolRefs: [...projectedToolRefs],
    requiredToolRefs: [...fixture.expectedToolMenuConstraints.requiredToolRefs],
    schemaRef: "probe.projected_tool_menu_snapshot.v1",
  };
}

function componentHashesFor(components: ProbeGepaCandidateComponents): ProbeGepaCandidateComponentHashes {
  const failureFamilyPlaybooks: Record<string, string> = {};
  for (const [family, playbook] of Object.entries(components.failure_family_playbooks).sort(([left], [right]) =>
    left.localeCompare(right),
  )) {
    failureFamilyPlaybooks[family] = stableSha256(`failure_family_playbook:${family}`, playbook);
  }

  return {
    closeout_policy: stableSha256("closeout_policy", components.closeout_policy),
    failure_family_playbooks: failureFamilyPlaybooks,
    patch_and_test_policy: stableSha256("patch_and_test_policy", components.patch_and_test_policy),
    probe_system_prompt: stableSha256("probe_system_prompt", components.probe_system_prompt),
    signature_selection_policy: stableSha256("signature_selection_policy", components.signature_selection_policy),
    terminal_bench_global_playbook: stableSha256(
      "terminal_bench_global_playbook",
      components.terminal_bench_global_playbook,
    ),
    tool_menu_policy: stableSha256("tool_menu_policy", components.tool_menu_policy),
  };
}

function stableSha256(label: string, value: string): string {
  const hash = createHash("sha256");
  hash.update(label);
  hash.update("\0");
  hash.update(value);
  return `sha256:${hash.digest("hex")}`;
}

function containsUnsafeCandidateText(value: string): boolean {
  const normalized = value.toLowerCase();
  const unsafeLiteral = [
    "access_token",
    "refresh_token",
    "bearer ",
    "mdk_mnemonic",
    "wallet_mnemonic",
    "private-repo://",
    "bypass_release_gate",
    "ignore_release_gate",
    "disable_release_gate",
    "public_claim_upgrade_authority",
    "request_new_runtime_authority",
    "new_runtime_authority",
    "grant_runtime_authority",
  ].some((needle) => normalized.includes(needle));

  return unsafeLiteral || normalized.startsWith("sk-") || normalized.includes(" sk-");
}

function defaultRunRef(taskId: string, candidateHash: string, baseline: boolean): string {
  return `probe_run.retained.${taskId}.${baseline ? "baseline" : shortHash(candidateHash)}`;
}

function requireNonEmpty(value: string, path: string): Effect.Effect<void, ProbeBenchmarkCandidateExecutionError> {
  return value.trim().length > 0 ? Effect.void : candidateError(path, "must be non-empty");
}

function requireNonEmptyArray(
  values: ReadonlyArray<string>,
  path: string,
): Effect.Effect<void, ProbeBenchmarkCandidateExecutionError> {
  return values.length > 0 && values.every((value) => value.trim().length > 0)
    ? Effect.void
    : candidateError(path, "must include non-empty refs");
}

function candidateError(path: string, reason: string): Effect.Effect<never, ProbeBenchmarkCandidateExecutionError> {
  return Effect.fail(new ProbeBenchmarkCandidateExecutionError({ path, reason }));
}
