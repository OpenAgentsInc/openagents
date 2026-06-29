import { Effect, Schema as S } from "effect";
import {
  PROBE_BENCHMARK_CLOSEOUT_BUNDLE_FILE_NAMES,
  type ProbeBenchmarkCloseoutBundleFileName,
} from "./closeout-writer.js";
import {
  ProbeBenchmarkContractError,
  ProbeBenchmarkEvidenceSplit,
  ProbeBenchmarkFailureFamily,
  ProbeBenchmarkRedactionState,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark.js";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account.js";

export const PROBE_RETAINED_BENCHMARK_FIXTURE_SCHEMA_REF = "probe.retained_benchmark_fixture.v1" as const;
export const PROBE_RETAINED_BENCHMARK_FIXTURE_PACKAGE_SCHEMA_REF =
  "probe.retained_benchmark_fixture_package.v1" as const;

export const ProbeGepaStageRef = S.Literals(["gepa_stage_0", "gepa_stage_1"]);
export type ProbeGepaStageRef = typeof ProbeGepaStageRef.Type;

export const ProbeBenchmarkToolMenuConstraints = S.Struct({
  allowedToolRefs: S.Array(S.String),
  deniedToolRefs: S.Array(S.String),
  maxToolCalls: S.optional(S.Number),
  requiresApprovalForToolRefs: S.Array(S.String),
  requiredToolRefs: S.Array(S.String),
});
export type ProbeBenchmarkToolMenuConstraints = typeof ProbeBenchmarkToolMenuConstraints.Type;

export const ProbeBenchmarkCloseoutRequirement = S.Struct({
  artifactManifestRequired: S.Boolean,
  bundleFileNames: S.Array(S.String),
  failureClassificationRequired: S.Boolean,
  proofBundleRequired: S.Boolean,
  redactionState: ProbeBenchmarkRedactionState,
  resourceUsageRefOrUnavailableReasonRequired: S.Boolean,
  retainedFailureRefRequired: S.Boolean,
});
export type ProbeBenchmarkCloseoutRequirement = typeof ProbeBenchmarkCloseoutRequirement.Type;

export const ProbeBenchmarkScoreExpectation = S.Struct({
  expectationKind: S.Literals(["known_retained_failure", "verifier_ref", "score_threshold"]),
  expectedOutcome: S.Literals(["retained_failure", "pass_after_candidate", "runner_stall_detected"]),
  minimumScore: S.optional(S.Number),
  scorerRef: S.optional(S.String),
  verifierRef: S.optional(S.String),
});
export type ProbeBenchmarkScoreExpectation = typeof ProbeBenchmarkScoreExpectation.Type;

export const ProbeRetainedBenchmarkFixture = S.Struct({
  benchmarkSuiteRef: S.String,
  expectedBlueprintSignatureRefs: S.Array(S.String),
  expectedCloseoutRequirements: ProbeBenchmarkCloseoutRequirement,
  expectedFailureFamilies: S.Array(ProbeBenchmarkFailureFamily),
  expectedScore: ProbeBenchmarkScoreExpectation,
  expectedToolMenuConstraints: ProbeBenchmarkToolMenuConstraints,
  fixtureRef: S.String,
  gepaStageRefs: S.Array(ProbeGepaStageRef),
  primaryFailureFamily: ProbeBenchmarkFailureFamily,
  schemaRef: S.Literal(PROBE_RETAINED_BENCHMARK_FIXTURE_SCHEMA_REF),
  sourceBoundary: S.Literal("public_refs_only"),
  splitMembership: S.Array(ProbeBenchmarkEvidenceSplit),
  taskId: S.String,
});
export type ProbeRetainedBenchmarkFixture = typeof ProbeRetainedBenchmarkFixture.Type;

export const ProbeRetainedBenchmarkFixturePackage = S.Struct({
  benchmarkSuiteRef: S.String,
  fixtures: S.Array(ProbeRetainedBenchmarkFixture),
  packageRef: S.String,
  schemaRef: S.Literal(PROBE_RETAINED_BENCHMARK_FIXTURE_PACKAGE_SCHEMA_REF),
  sourceBoundary: S.Literal("public_refs_only"),
});
export type ProbeRetainedBenchmarkFixturePackage = typeof ProbeRetainedBenchmarkFixturePackage.Type;

const TERMINAL_BENCH_SUITE_REF = "benchmark_suite.terminal_bench_2.harbor.retained.v1";
const RETAINED_SPLITS: ReadonlyArray<typeof ProbeBenchmarkEvidenceSplit.Type> = ["retained"];
const GEPA_STAGE_REFS: ReadonlyArray<ProbeGepaStageRef> = ["gepa_stage_0", "gepa_stage_1"];
const BASE_REQUIRED_TOOL_REFS = [
  "tool.probe.read_file",
  "tool.probe.code_search",
  "tool.probe.shell_command",
  "tool.probe.apply_patch",
  "tool.probe.record_evidence",
];

const commonCloseoutRequirements = (): ProbeBenchmarkCloseoutRequirement => ({
  artifactManifestRequired: true,
  bundleFileNames: [...PROBE_BENCHMARK_CLOSEOUT_BUNDLE_FILE_NAMES],
  failureClassificationRequired: true,
  proofBundleRequired: true,
  redactionState: "public_safe",
  resourceUsageRefOrUnavailableReasonRequired: true,
  retainedFailureRefRequired: true,
});

const commonToolMenuConstraints = (
  additionalToolRefs: ReadonlyArray<string> = [],
): ProbeBenchmarkToolMenuConstraints => ({
  allowedToolRefs: [...BASE_REQUIRED_TOOL_REFS, ...additionalToolRefs],
  deniedToolRefs: [
    "tool.probe.raw_network_exfiltration",
    "tool.probe.unbounded_log_export",
    "tool.probe.hidden_verifier_read",
  ],
  maxToolCalls: 96,
  requiresApprovalForToolRefs: ["tool.probe.propose_action_submission"],
  requiredToolRefs: [...BASE_REQUIRED_TOOL_REFS, ...additionalToolRefs],
});

const retainedFixture = (input: {
  readonly additionalToolRefs?: ReadonlyArray<string>;
  readonly expectedFailureFamilies: ReadonlyArray<typeof ProbeBenchmarkFailureFamily.Type>;
  readonly expectedOutcome?: typeof ProbeBenchmarkScoreExpectation.Type["expectedOutcome"];
  readonly primaryFailureFamily: typeof ProbeBenchmarkFailureFamily.Type;
  readonly signatureRefs: ReadonlyArray<string>;
  readonly taskId: string;
  readonly verifierRef: string;
}): ProbeRetainedBenchmarkFixture => ({
  benchmarkSuiteRef: TERMINAL_BENCH_SUITE_REF,
  expectedBlueprintSignatureRefs: [...input.signatureRefs],
  expectedCloseoutRequirements: commonCloseoutRequirements(),
  expectedFailureFamilies: [...input.expectedFailureFamilies],
  expectedScore: {
    expectationKind: "known_retained_failure",
    expectedOutcome: input.expectedOutcome ?? "retained_failure",
    scorerRef: "scorer.terminal_bench.binary.v1",
    verifierRef: input.verifierRef,
  },
  expectedToolMenuConstraints: commonToolMenuConstraints(input.additionalToolRefs),
  fixtureRef: `fixture.probe.terminal_bench.retained.${input.taskId}.v1`,
  gepaStageRefs: [...GEPA_STAGE_REFS],
  primaryFailureFamily: input.primaryFailureFamily,
  schemaRef: PROBE_RETAINED_BENCHMARK_FIXTURE_SCHEMA_REF,
  sourceBoundary: "public_refs_only",
  splitMembership: [...RETAINED_SPLITS],
  taskId: input.taskId,
});

export const STATIC_RETAINED_TERMINAL_BENCH_FIXTURES: ReadonlyArray<ProbeRetainedBenchmarkFixture> = [
  retainedFixture({
    expectedFailureFamilies: ["service_readiness"],
    primaryFailureFamily: "service_readiness",
    signatureRefs: ["program_signature.probe.benchmark.service_readiness.v1"],
    taskId: "configure-git-webserver",
    verifierRef: "verifier.terminal_bench.configure_git_webserver.v1",
  }),
  retainedFixture({
    expectedFailureFamilies: ["database_recovery", "sqlite_wal_recovery"],
    primaryFailureFamily: "sqlite_wal_recovery",
    signatureRefs: ["program_signature.probe.benchmark.sqlite_wal_recovery.v1"],
    taskId: "db-wal-recovery",
    verifierRef: "verifier.terminal_bench.db_wal_recovery.v1",
  }),
  retainedFixture({
    additionalToolRefs: ["tool.probe.html_parser"],
    expectedFailureFamilies: ["parser_correctness", "xss_sanitizer_policy"],
    primaryFailureFamily: "xss_sanitizer_policy",
    signatureRefs: ["program_signature.probe.benchmark.xss_sanitizer_policy.v1"],
    taskId: "filter-js-from-html",
    verifierRef: "verifier.terminal_bench.filter_js_from_html.v1",
  }),
  retainedFixture({
    additionalToolRefs: ["tool.probe.parser_fixture_runner"],
    expectedFailureFamilies: ["parser_correctness", "gcode_parser_guard"],
    primaryFailureFamily: "gcode_parser_guard",
    signatureRefs: ["program_signature.probe.benchmark.gcode_parser_guard.v1"],
    taskId: "gcode-to-text",
    verifierRef: "verifier.terminal_bench.gcode_to_text.v1",
  }),
  retainedFixture({
    additionalToolRefs: ["tool.probe.python_package_index"],
    expectedFailureFamilies: ["package_indexing", "python_package_index"],
    primaryFailureFamily: "python_package_index",
    signatureRefs: ["program_signature.probe.benchmark.python_package_index.v1"],
    taskId: "pypi-server",
    verifierRef: "verifier.terminal_bench.pypi_server.v1",
  }),
  retainedFixture({
    additionalToolRefs: ["tool.probe.query_plan_inspector"],
    expectedFailureFamilies: ["query_optimization"],
    primaryFailureFamily: "query_optimization",
    signatureRefs: ["program_signature.probe.benchmark.query_optimization.v1"],
    taskId: "query-optimize",
    verifierRef: "verifier.terminal_bench.query_optimize.v1",
  }),
  retainedFixture({
    additionalToolRefs: ["tool.probe.runner_watchdog"],
    expectedFailureFamilies: ["runner_supervision"],
    expectedOutcome: "runner_stall_detected",
    primaryFailureFamily: "runner_supervision",
    signatureRefs: ["program_signature.probe.benchmark.runner_supervision.v1"],
    taskId: "runner-stall-supervision",
    verifierRef: "verifier.probe.runner_stall_supervision.v1",
  }),
];

export const STATIC_RETAINED_TERMINAL_BENCH_FIXTURE_PACKAGE: ProbeRetainedBenchmarkFixturePackage = {
  benchmarkSuiteRef: TERMINAL_BENCH_SUITE_REF,
  fixtures: [...STATIC_RETAINED_TERMINAL_BENCH_FIXTURES],
  packageRef: "fixture_package.probe.terminal_bench.retained.stage_0_1.v1",
  schemaRef: PROBE_RETAINED_BENCHMARK_FIXTURE_PACKAGE_SCHEMA_REF,
  sourceBoundary: "public_refs_only",
};

export function decodeProbeRetainedBenchmarkFixturePackage(
  value: unknown,
): Effect.Effect<ProbeRetainedBenchmarkFixturePackage, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "retainedBenchmarkFixturePackage");
    yield* validateNoHiddenFixtureMaterial(value, "retainedBenchmarkFixturePackage");
    const decoded = yield* S.decodeUnknownEffect(ProbeRetainedBenchmarkFixturePackage)(value).pipe(
      Effect.mapError(
        (error) =>
          new ProbeBenchmarkContractError({
            path: "retainedBenchmarkFixturePackage",
            reason: String(error),
          }),
      ),
    );

    yield* validateRetainedFixturePackage(decoded);
    return decoded;
  });
}

export function loadStaticRetainedTerminalBenchFixturePackage(): Effect.Effect<
  ProbeRetainedBenchmarkFixturePackage,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return decodeProbeRetainedBenchmarkFixturePackage(STATIC_RETAINED_TERMINAL_BENCH_FIXTURE_PACKAGE);
}

export function retainedTerminalBenchFixtureByTaskId(
  packageRecord: ProbeRetainedBenchmarkFixturePackage,
  taskId: string,
): ProbeRetainedBenchmarkFixture | undefined {
  return packageRecord.fixtures.find((fixture) => fixture.taskId === taskId);
}

function validateRetainedFixturePackage(
  packageRecord: ProbeRetainedBenchmarkFixturePackage,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    if (packageRecord.fixtures.length === 0) {
      return yield* fixtureError("retainedBenchmarkFixturePackage.fixtures", "must include retained fixtures");
    }

    const taskIds = new Set<string>();

    for (const [index, fixture] of packageRecord.fixtures.entries()) {
      const path = `retainedBenchmarkFixturePackage.fixtures[${index}]`;

      if (taskIds.has(fixture.taskId)) {
        return yield* fixtureError(`${path}.taskId`, "must be unique inside the package");
      }

      taskIds.add(fixture.taskId);

      if (!fixture.splitMembership.includes("retained")) {
        return yield* fixtureError(`${path}.splitMembership`, "must include retained split membership");
      }

      if (!fixture.gepaStageRefs.includes("gepa_stage_0") || !fixture.gepaStageRefs.includes("gepa_stage_1")) {
        return yield* fixtureError(`${path}.gepaStageRefs`, "must support GEPA Stage 0 and Stage 1 retained runs");
      }

      if (!fixture.expectedFailureFamilies.includes(fixture.primaryFailureFamily)) {
        return yield* fixtureError(`${path}.primaryFailureFamily`, "must be included in expectedFailureFamilies");
      }

      if (fixture.expectedBlueprintSignatureRefs.length === 0) {
        return yield* fixtureError(`${path}.expectedBlueprintSignatureRefs`, "must include a Blueprint signature ref");
      }

      if (fixture.expectedToolMenuConstraints.requiredToolRefs.length === 0) {
        return yield* fixtureError(`${path}.expectedToolMenuConstraints.requiredToolRefs`, "must include tool refs");
      }

      if (!fixture.expectedCloseoutRequirements.bundleFileNames.includes("probe-closeout.json")) {
        return yield* fixtureError(`${path}.expectedCloseoutRequirements.bundleFileNames`, "must include closeout file");
      }
    }
  });
}

function validateNoHiddenFixtureMaterial(value: unknown, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  if (value === null || value === undefined) {
    return Effect.void;
  }

  if (Array.isArray(value)) {
    return Effect.all(value.map((entry, index) => validateNoHiddenFixtureMaterial(entry, `${path}[${index}]`))).pipe(
      Effect.asVoid,
    );
  }

  if (typeof value !== "object") {
    return Effect.void;
  }

  return Effect.gen(function* () {
    for (const [key, entry] of Object.entries(value)) {
      const childPath = `${path}.${key}`;

      if (hiddenFixtureKey(key)) {
        return yield* fixtureError(childPath, "retained fixtures must not include hidden task data or Harbor traces");
      }

      yield* validateNoHiddenFixtureMaterial(entry, childPath);
    }
  });
}

function hiddenFixtureKey(key: string): boolean {
  const normalized = key.replace(/[_-]/g, "").toLowerCase();

  return [
    "tasktext",
    "taskprompt",
    "tasksolution",
    "solution",
    "expectedanswer",
    "verifieranswer",
    "harbortrace",
    "privateharbortrace",
    "privatetrace",
  ].includes(normalized);
}

function fixtureError(path: string, reason: string): Effect.Effect<never, ProbeBenchmarkContractError> {
  return Effect.fail(new ProbeBenchmarkContractError({ path, reason }));
}

export function retainedFixtureBundleFileNames(
  fixture: ProbeRetainedBenchmarkFixture,
): ReadonlyArray<ProbeBenchmarkCloseoutBundleFileName> {
  return fixture.expectedCloseoutRequirements.bundleFileNames.filter((fileName) =>
    PROBE_BENCHMARK_CLOSEOUT_BUNDLE_FILE_NAMES.includes(fileName as ProbeBenchmarkCloseoutBundleFileName),
  ) as ReadonlyArray<ProbeBenchmarkCloseoutBundleFileName>;
}
