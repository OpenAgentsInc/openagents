import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { performance } from "node:perf_hooks";
import path from "node:path";

import { Schema } from "effect";
import {
  IdePortableCheckpointManifestSchema,
  checkIdePortableModel,
} from "@openagentsinc/portable-session-contract";

import {
  IDE_PORTABLE_FAULT_SCENARIOS,
  IDE_PORTABLE_PHASES,
  IdePortableEvidenceClassSchema,
  IdePortableEvidenceReceiptSchema,
  IdePortablePhaseSchema,
  IdePortablePlacementCohortSchema,
  validateIdePortableEvidenceReceipt,
} from "../src/ide/portable-evidence-contract.ts";

const appRoot = path.resolve(import.meta.dirname, "..");
const repositoryRoot = path.resolve(appRoot, "../..");
const output = path.join(appRoot, "benchmarks", "ide", "2026-07-20-ide-13-portability.json");
const ownerLocalCohortPath = path.join(
  appRoot,
  "benchmarks",
  "ide",
  "2026-07-20-ide-13-owner-local-real-cohort.json",
);
const ownerLocalPerformancePath = path.join(
  appRoot,
  "benchmarks",
  "ide",
  "2026-07-20-ide-13-owner-local-performance.json",
);
const ownerLocalFaultMatrixPath = path.join(
  appRoot,
  "benchmarks",
  "ide",
  "2026-07-20-ide-13-owner-local-real-fault-matrix.json",
);
const ownerLocalRecoveryFaultsPath = path.join(
  appRoot,
  "benchmarks",
  "ide",
  "2026-07-20-ide-13-owner-local-recovery-faults.json",
);
const ownerLocalExecutorResumePath = path.join(
  appRoot,
  "benchmarks",
  "ide",
  "2026-07-20-ide-13-owner-local-executor-resume.json",
);
const packagedOwnerLocalJourneyPath = path.join(
  appRoot,
  "benchmarks",
  "ide",
  "2026-07-20-ide-13-packaged-owner-local-journey.json",
);
const checkpointAdmissionFaultsPath = path.join(
  appRoot,
  "benchmarks",
  "ide",
  "2026-07-20-ide-13-checkpoint-admission-faults.json",
);
const git = (...args: string[]): string =>
  execFileSync("git", args, { cwd: repositoryRoot, encoding: "utf8" }).trim();
const valueAt = (values: ReadonlyArray<number>, percentile: number): number =>
  values.toSorted((left, right) => left - right)[Math.floor((values.length - 1) * percentile)] ?? 0;
const measureLatency = (
  metricRef: string,
  operation: () => void,
  repetitions: number,
  thresholdP95: number,
  thresholdP99: number,
) => {
  const samples: number[] = [];
  operation();
  for (let index = 0; index < repetitions; index += 1) {
    const start = performance.now();
    operation();
    samples.push(performance.now() - start);
  }
  return {
    metricRef,
    metric: "phase_latency" as const,
    phase: null,
    unit: "milliseconds" as const,
    repetitions,
    p50: valueAt(samples, 0.5),
    p95: valueAt(samples, 0.95),
    p99: valueAt(samples, 0.99),
    thresholdP95,
    thresholdP99,
    passed: valueAt(samples, 0.95) <= thresholdP95 && valueAt(samples, 0.99) <= thresholdP99,
    receiptRef: `local-measurement:${metricRef}`,
  };
};

const digest = `sha256:${"a".repeat(64)}`;
const checkpoint = {
  manifestRef: "manifest.benchmark",
  checkpointRef: "checkpoint.benchmark",
  sessionRef: "session.benchmark",
  sourceAttachmentRef: "attachment.benchmark.1",
  sourceGeneration: 1,
  digest,
  byteSize: 4_096,
  fileCount: 12,
  repositoryPostImageDigest: digest,
  graphDigest: digest,
  project: {
    projectRef: "project.benchmark",
    projectRootRef: "root.benchmark",
    worktreeRef: "worktree.benchmark",
    selectedFileRef: null,
    documentSnapshotRef: null,
    proposalRef: null,
    diagnosticResultRef: null,
    testResultRef: null,
    artifactRef: null,
    evidenceRef: null,
  },
  includedCapabilityRefs: ["capability.files"],
  omittedCapabilityRefs: ["capability.pty", "capability.lsp", "capability.dap"],
  historyRefs: [],
  proposalRefs: [],
  taskRefs: [],
  testRefs: [],
  deliveryEvidenceRefs: [],
  secretMaterial: "excluded",
  processState: "excluded",
  nativeState: "excluded",
  vimState: "destination_setting",
  themeState: "destination_setting",
  policy: {
    maximumBytes: 1_048_576,
    maximumFiles: 10_000,
    encryption: "owner_key",
    encryptionKeyRef: "key.benchmark",
    custody: "owner_device",
    retentionSeconds: 3_600,
    expiresAt: "2030-01-01T00:00:00.000Z",
  },
  integrityReceiptRef: "integrity.benchmark",
} as const;

const decodeCheckpoint = Schema.decodeUnknownSync(IdePortableCheckpointManifestSchema);
const model = checkIdePortableModel({ maximumDepth: 12 });
if (!model.passed) throw new Error("IDE-13 model check failed");
const localMetrics = [
  measureLatency(
    "bounded-transition-model",
    () => {
      checkIdePortableModel({ maximumDepth: 12 });
    },
    100,
    5,
    10,
  ),
  measureLatency(
    "checkpoint-schema-decode",
    () => {
      decodeCheckpoint(checkpoint);
    },
    1_000,
    1,
    2,
  ),
];
if (localMetrics.some((metric) => !metric.passed)) {
  throw new Error(`IDE-13 local threshold failed: ${JSON.stringify(localMetrics)}`);
}

const ownerLocalInput = Schema.decodeUnknownSync(
  Schema.Struct({
    cohort: IdePortablePlacementCohortSchema,
    proofs: Schema.Struct({
      replayReceiptRef: Schema.String,
      staleGenerationReceiptRef: Schema.String,
    }),
  }),
)(JSON.parse(readFileSync(ownerLocalCohortPath, "utf8")));
const ownerLocalPerformance = Schema.decodeUnknownSync(
  Schema.Struct({
    candidateCommitSha: Schema.String,
    baseCommitSha: Schema.String,
    cohortRef: Schema.String,
    evidenceClass: Schema.Literal("real_local"),
    repetitions: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(5)),
    metrics: IdePortablePlacementCohortSchema.fields.metrics,
    receiptRef: Schema.String,
  }),
)(JSON.parse(readFileSync(ownerLocalPerformancePath, "utf8")));
const ownerLocalFaultMatrix = Schema.decodeUnknownSync(
  Schema.Struct({
    candidateCommitSha: Schema.String,
    baseCommitSha: Schema.String,
    cohortRef: Schema.String,
    cases: Schema.Array(
      Schema.Struct({
        faultRef: Schema.String,
        scenario: Schema.Literals(IDE_PORTABLE_FAULT_SCENARIOS),
        phase: Schema.NullOr(IdePortablePhaseSchema),
        evidenceClass: IdePortableEvidenceClassSchema,
        outcome: Schema.Literals(["passed", "not_run"]),
        recoveryPointRef: Schema.NullOr(Schema.String),
        receiptRef: Schema.NullOr(Schema.String),
      }),
    ),
    summary: Schema.Struct({
      requiredCaseCount: Schema.Number,
      passedRealLocalCount: Schema.Number,
      notRunCount: Schema.Number,
      acceptanceReady: Schema.Literal(false),
    }),
  }),
)(JSON.parse(readFileSync(ownerLocalFaultMatrixPath, "utf8")));
const ownerLocalRecoveryFaults = Schema.decodeUnknownSync(
  Schema.Struct({
    candidateCommitSha: Schema.String,
    baseCommitSha: Schema.String,
    cohortRef: Schema.String,
    cases: Schema.Array(
      Schema.Struct({
        scenario: Schema.Literals(IDE_PORTABLE_FAULT_SCENARIOS),
        evidenceClass: IdePortableEvidenceClassSchema,
        outcome: Schema.Literal("passed"),
        recoveryPointRef: Schema.String,
        receiptRef: Schema.String,
      }),
    ),
  }),
)(JSON.parse(readFileSync(ownerLocalRecoveryFaultsPath, "utf8")));
const ownerLocalExecutorResume = Schema.decodeUnknownSync(
  Schema.Struct({
    candidateCommitSha: Schema.String,
    baseCommitSha: Schema.String,
    evidenceClass: Schema.Literal("real_local"),
    execution: Schema.Struct({
      acceptedWorkRefCount: Schema.Literal(1),
      duplicateExecutionCount: Schema.Literal(0),
      staleGenerationRefused: Schema.Literal(true),
      receiptRef: Schema.String,
    }),
    authority: Schema.Struct({
      productionDispatchEnabled: Schema.Literal(false),
      networkCalls: Schema.Literal(0),
      providerCalls: Schema.Literal(0),
      secretMaterialInReceipt: Schema.Literal(false),
    }),
  }),
)(JSON.parse(readFileSync(ownerLocalExecutorResumePath, "utf8")));
const packagedOwnerLocalJourney = Schema.decodeUnknownSync(
  Schema.Struct({
    candidateCommitSha: Schema.String,
    evidenceClass: Schema.Literal("real_local"),
    proofClass: Schema.Literal("packaged_shell_concurrent_owner_local_target"),
    packagedShell: Schema.Struct({
      isolatedAppProof: Schema.Literal(true),
      signedOutLocalOnly: Schema.Literal(true),
      authenticatedSyncClaimed: Schema.Literal(false),
      initiatedMoveClaimed: Schema.Literal(false),
    }),
    teardown: Schema.Struct({
      survivingProcessCount: Schema.Literal(0),
      temporaryRootsRemoved: Schema.Literal(true),
    }),
    passed: Schema.Literal(true),
  }),
)(JSON.parse(readFileSync(packagedOwnerLocalJourneyPath, "utf8")));
const checkpointAdmissionFaults = Schema.decodeUnknownSync(
  Schema.Struct({
    candidateCommitSha: Schema.String,
    baseCommitSha: Schema.String,
    cases: Schema.Array(
      Schema.Struct({
        scenario: Schema.Literals(IDE_PORTABLE_FAULT_SCENARIOS),
        evidenceClass: IdePortableEvidenceClassSchema,
        outcome: Schema.Literals(["passed", "not_run"]),
        rejectionRef: Schema.NullOr(Schema.String),
      }),
    ),
    summary: Schema.Struct({
      passedSimulatorCount: Schema.Literal(6),
      notRunCount: Schema.Literal(2),
      acceptanceContributionCount: Schema.Literal(0),
      acceptanceReady: Schema.Literal(false),
    }),
  }),
)(JSON.parse(readFileSync(checkpointAdmissionFaultsPath, "utf8")));
const candidateCommitSha = ownerLocalInput.cohort.candidateCommitSha;
const baseCommitSha = ownerLocalInput.cohort.baseCommitSha;
if (
  ownerLocalPerformance.candidateCommitSha !== candidateCommitSha ||
  ownerLocalPerformance.baseCommitSha !== baseCommitSha ||
  ownerLocalPerformance.cohortRef !== ownerLocalInput.cohort.cohortRef ||
  ownerLocalFaultMatrix.candidateCommitSha !== candidateCommitSha ||
  ownerLocalFaultMatrix.baseCommitSha !== baseCommitSha ||
  ownerLocalFaultMatrix.cohortRef !== ownerLocalInput.cohort.cohortRef ||
  ownerLocalRecoveryFaults.candidateCommitSha !== candidateCommitSha ||
  ownerLocalRecoveryFaults.baseCommitSha !== baseCommitSha ||
  ownerLocalRecoveryFaults.cohortRef !== ownerLocalInput.cohort.cohortRef ||
  ownerLocalExecutorResume.candidateCommitSha !== candidateCommitSha ||
  ownerLocalExecutorResume.baseCommitSha !== baseCommitSha ||
  packagedOwnerLocalJourney.candidateCommitSha !== candidateCommitSha ||
  checkpointAdmissionFaults.candidateCommitSha !== candidateCommitSha ||
  checkpointAdmissionFaults.baseCommitSha !== baseCommitSha
) {
  throw new Error("IDE-13 owner-local cohort evidence is stale");
}
const strongerFaultsByScenario = new Map(
  [...checkpointAdmissionFaults.cases, ...ownerLocalRecoveryFaults.cases].map((fault) => [
    fault.scenario,
    {
      evidenceClass: fault.evidenceClass,
      outcome: fault.outcome,
      recoveryPointRef: "recoveryPointRef" in fault ? fault.recoveryPointRef : fault.rejectionRef,
      receiptRef: "receiptRef" in fault ? fault.receiptRef : fault.rejectionRef,
    },
  ]),
);
const ownerLocalCohort = {
  ...ownerLocalInput.cohort,
  journeys: {
    ...ownerLocalInput.cohort.journeys,
    faultMatrixReceiptRef:
      "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-owner-local-real-fault-matrix.json",
  },
  metrics: ownerLocalPerformance.metrics,
  result: `${ownerLocalInput.cohort.result} The phase and resource distributions use ${ownerLocalPerformance.repetitions} complete real owner-local runs under ${ownerLocalPerformance.receiptRef}. The required fault inventory records ${ownerLocalFaultMatrix.summary.passedRealLocalCount} passed injected transition-partition rows. One additional checkpoint-store crash ran as real local evidence. Nine worker or checkpoint-admission cases ran only as simulator evidence. One bounded accepted work ref resumed and settled exactly once at the destination. A signed-out packaged shell stayed alive concurrently with a same-host owner-local journey, but it did not initiate or authenticate that move.`,
};
git("merge-base", "--is-ancestor", candidateCommitSha, "HEAD");
const allowedEvidencePaths = new Set([
  "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-owner-local-real-cohort.json",
  "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-owner-local-performance.json",
  "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-owner-local-real-fault-matrix.json",
  "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-owner-local-recovery-faults.json",
  "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-owner-local-executor-resume.json",
  "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-packaged-owner-local-journey.json",
  "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-packaged-owner-local-journey-trace.json",
  "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-packaged-owner-local-journey.png",
  "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-checkpoint-admission-faults.json",
  "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-portability.json",
  "apps/openagents-desktop/scripts/ide-portable-evidence.ts",
]);
const laterPaths = git("diff", "--name-only", candidateCommitSha, "HEAD")
  .split("\n")
  .filter((path) => path.length > 0);
if (laterPaths.some((path) => !allowedEvidencePaths.has(path))) {
  throw new Error("IDE-13 aggregate evidence candidate omits an implementation change");
}
const simulatedPhases = (receiptRef: string) =>
  IDE_PORTABLE_PHASES.map((phase) => ({
    phase,
    evidenceClass: "simulator" as const,
    receiptRef,
    operationRef: `simulated-operation:${phase}`,
    attachmentGeneration: 1,
    result: "passed" as const,
  }));
const absentPhases = () =>
  IDE_PORTABLE_PHASES.map((phase) => ({
    phase,
    evidenceClass: "not_run" as const,
    receiptRef: null,
    operationRef: null,
    attachmentGeneration: null,
    result: "not_run" as const,
  }));

const receipt = Schema.decodeUnknownSync(IdePortableEvidenceReceiptSchema)({
  schemaVersion: "openagents.desktop.ide-portable-evidence.v3",
  issue: "IDE-13",
  candidateCommitSha,
  baseCommitSha,
  generatedAt: new Date().toISOString(),
  producerRef: "openagents:ide-13-implementation",
  acceptanceRefs: {
    candidateRef: null,
    mainCommitSha: null,
    mainRef: null,
    artifactReceiptRef: null,
    rollbackReceiptRef: null,
    verificationCommandRef: null,
    verificationResultRef: null,
  },
  environment: { platform: process.platform, architecture: process.arch, node: process.version },
  model: {
    maximumDepth: model.maximumDepth,
    exploredStates: model.exploredStates,
    exploredTransitions: model.exploredTransitions,
    staleWriteAttempts: model.staleWriteAttempts,
    counterexamples: 0,
    passed: true,
  },
  implementedChecks: [
    {
      checkRef: "bounded-attachment-model",
      evidenceClass: "model",
      result: "passed",
      receiptRef: "packages/portable-session-contract/src/ide13-model.test.ts",
    },
    {
      checkRef: "coordinator-transition-faults",
      evidenceClass: "regression",
      result: "passed",
      receiptRef: "apps/openagents-desktop/src/ide/portable-coordinator-service.test.ts",
    },
    {
      checkRef: "durable-command-replay",
      evidenceClass: "regression",
      result: "passed",
      receiptRef: "packages/khala-sync-server/src/portable-session-command-consumer.test.ts",
    },
    {
      checkRef: "durable-phase-exchange",
      evidenceClass: "regression",
      result: "passed",
      receiptRef: "packages/khala-sync-server/src/portable-phase-operation-store.test.ts",
    },
    {
      checkRef: "phase-target-binding",
      evidenceClass: "regression",
      result: "passed",
      receiptRef: "packages/khala-sync-server/src/portable-phase-target-adapter.test.ts",
    },
    {
      checkRef: "phase-claim-recovery",
      evidenceClass: "regression",
      result: "passed",
      receiptRef: "apps/pylon/src/portable-phase-operation-claim-journal.test.ts",
    },
    {
      checkRef: "encrypted-checkpoint-custody",
      evidenceClass: "regression",
      result: "passed",
      receiptRef: "apps/pylon/tests/portable-session-checkpoint-artifact.test.ts",
    },
    {
      checkRef: "production-command-adapter-composition",
      evidenceClass: "regression",
      result: "passed",
      receiptRef:
        "apps/openagents.com/workers/api/src/portable-session-command-runtime-adapters.test.ts",
    },
    {
      checkRef: "owner-local-fresh-pty-and-watcher",
      evidenceClass: "regression",
      result: "passed",
      receiptRef: "apps/pylon/src/portable-destination-production-helper-adapters.test.ts",
    },
    {
      checkRef: "owner-local-confirmed-pty-teardown",
      evidenceClass: "regression",
      result: "passed",
      receiptRef: "packages/khala-tools/src/process-session-termination.test.ts",
    },
    {
      checkRef: "managed-watcher-lifecycle",
      evidenceClass: "regression",
      result: "passed",
      receiptRef: "apps/pylon/tests/portable-session-control.test.ts",
    },
    {
      checkRef: "managed-helper-observation-time",
      evidenceClass: "regression",
      result: "passed",
      receiptRef: "apps/pylon/tests/portable-session-control.test.ts",
    },
    {
      checkRef: "executable-profile-authority-ref",
      evidenceClass: "regression",
      result: "passed",
      receiptRef: "apps/pylon/src/portable-owner-local-capability-operation-executor.test.ts",
    },
    {
      checkRef: "destination-editor-setting-reprojection",
      evidenceClass: "regression",
      result: "passed",
      receiptRef: "apps/openagents-desktop/src/ide/portable-destination-settings.test.ts",
    },
    {
      checkRef: "managed-live-proof-driver-fail-closed",
      evidenceClass: "regression",
      result: "passed",
      receiptRef: "apps/pylon/deploy/agent-computer/live-retained-proof.test.ts",
    },
    {
      checkRef: "managed-signed-typescript-lsp",
      evidenceClass: "regression",
      result: "passed",
      receiptRef: "apps/pylon/tests/portable-session-control.test.ts",
    },
    {
      checkRef: "owner-local-signed-typescript-lsp",
      evidenceClass: "regression",
      result: "passed",
      receiptRef: "apps/pylon/src/portable-destination-production-helper-adapters.test.ts",
    },
    {
      checkRef: "owner-local-real-move-cohort",
      evidenceClass: "regression",
      result: "passed",
      receiptRef:
        "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-owner-local-real-cohort.json",
    },
    {
      checkRef: "owner-local-real-performance-cohort",
      evidenceClass: "regression",
      result: "passed",
      receiptRef:
        "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-owner-local-performance.json",
    },
    {
      checkRef: "owner-local-real-fault-matrix",
      evidenceClass: "regression",
      result: "passed",
      receiptRef:
        "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-owner-local-real-fault-matrix.json",
    },
    {
      checkRef: "owner-local-recovery-fault-addendum",
      evidenceClass: "regression",
      result: "passed",
      receiptRef:
        "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-owner-local-recovery-faults.json",
    },
    {
      checkRef: "checkpoint-admission-fault-addendum",
      evidenceClass: "regression",
      result: "passed",
      receiptRef:
        "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-checkpoint-admission-faults.json",
    },
    {
      checkRef: "owner-local-bounded-work-resume",
      evidenceClass: "regression",
      result: "passed",
      receiptRef:
        "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-owner-local-executor-resume.json",
    },
    {
      checkRef: "packaged-owner-local-concurrent-journey",
      evidenceClass: "packaged_fail_closed",
      result: "passed",
      receiptRef:
        "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-packaged-owner-local-journey.json",
    },
    {
      checkRef: "workspace-search-revocation",
      evidenceClass: "regression",
      result: "passed",
      receiptRef: "apps/openagents-desktop/src/workspace-search-host.test.ts",
    },
    {
      checkRef: "packaged-fail-closed-boundary",
      evidenceClass: "packaged_fail_closed",
      result: "passed",
      receiptRef:
        "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-portability-packaged.json",
    },
  ],
  placementCohorts: [
    ownerLocalCohort,
    {
      cohortRef: "cohort:owner-managed:simulator",
      targetClass: "owner_managed",
      evidenceClass: "simulator",
      journeyScope: "foundation_only",
      journeys: {
        mainJourneyReceiptRef: null,
        failbackJourneyReceiptRef: null,
        faultMatrixReceiptRef: null,
      },
      operatingSystem: "unknown",
      architecture: "unknown",
      adapter: {
        kind: "deterministic_simulator",
        ref: "adapter.owner-managed.simulator",
        name: "Owner-managed contract simulator",
        version: "2",
      },
      targetRef: null,
      artifact: { ref: null, sha256: null, bytes: null },
      candidateCommitSha,
      baseCommitSha,
      capabilityState: "unverified",
      custody: "owner_managed",
      networkDestinations: [],
      dataDestinations: [],
      retentionSeconds: 0,
      costFact: "Cost was not measured.",
      phaseReceipts: simulatedPhases(
        "apps/openagents-desktop/src/ide/portable-coordinator-service.test.ts",
      ),
      metrics: [],
      result: "Only the deterministic coordinator and target contracts ran.",
    },
    {
      cohortRef: "cohort:openagents-managed:simulator",
      targetClass: "openagents_managed",
      evidenceClass: "simulator",
      journeyScope: "foundation_only",
      journeys: {
        mainJourneyReceiptRef: null,
        failbackJourneyReceiptRef: null,
        faultMatrixReceiptRef: null,
      },
      operatingSystem: "linux",
      architecture: "unknown",
      adapter: {
        kind: "deterministic_simulator",
        ref: "adapter.openagents-managed.simulator",
        name: "OpenAgents managed target simulator",
        version: "2",
      },
      targetRef: null,
      artifact: { ref: null, sha256: null, bytes: null },
      candidateCommitSha,
      baseCommitSha,
      capabilityState: "unverified",
      custody: "openagents_managed",
      networkDestinations: [],
      dataDestinations: [],
      retentionSeconds: 0,
      costFact: "Cost was not measured.",
      phaseReceipts: simulatedPhases(
        "packages/khala-sync-server/src/portable-phase-target-adapter.test.ts",
      ),
      metrics: [],
      result: "Only the managed target adapter regressions ran. No live managed move ran.",
    },
    {
      cohortRef: "cohort:managed-provider:unclaimed",
      targetClass: "managed_provider",
      evidenceClass: "not_run",
      journeyScope: "not_run",
      journeys: {
        mainJourneyReceiptRef: null,
        failbackJourneyReceiptRef: null,
        faultMatrixReceiptRef: null,
      },
      operatingSystem: "unknown",
      architecture: "unknown",
      adapter: { kind: "not_run", ref: null, name: null, version: null },
      targetRef: null,
      artifact: { ref: null, sha256: null, bytes: null },
      candidateCommitSha,
      baseCommitSha,
      capabilityState: "unsupported",
      custody: "unverified",
      networkDestinations: [],
      dataDestinations: [],
      retentionSeconds: 0,
      costFact: "No provider is admitted.",
      phaseReceipts: absentPhases(),
      metrics: [],
      result: "No managed-provider cohort ran.",
    },
  ],
  omissions: [
    {
      omissionRef: "omission:process-state",
      targetClass: "owner_local",
      fact: "The checkpoint excludes process and terminal state.",
      disposition: "accepted_limit",
      evidenceRef: "packages/portable-session-contract/src/ide13-contract.ts",
    },
    {
      omissionRef: "omission:real-cohorts",
      targetClass: "owner_managed",
      fact: "The real owner-managed move cohort did not run.",
      disposition: "acceptance_gap",
      evidenceRef: "github.com/OpenAgentsInc/openagents/issues/9041",
    },
    {
      omissionRef: "omission:managed-live",
      targetClass: "openagents_managed",
      fact: "The real OpenAgents-managed move cohort did not run.",
      disposition: "acceptance_gap",
      evidenceRef: "github.com/OpenAgentsInc/openagents/issues/9041",
    },
    {
      omissionRef: "omission:provider",
      targetClass: "managed_provider",
      fact: "No managed provider is admitted.",
      disposition: "acceptance_gap",
      evidenceRef: "github.com/OpenAgentsInc/openagents/issues/9041",
    },
  ],
  recoveryFacts: [
    {
      recoveryRef: "recovery:owner-local-replay",
      cohortRef: ownerLocalInput.cohort.cohortRef,
      targetClass: "owner_local",
      scenario: "The destination activation result was replayed after the exact move.",
      evidenceClass: "real_local",
      outcome: "passed",
      recoveryPointRef: "owner-local-generation-2-attachment",
      receiptRef: ownerLocalInput.proofs.replayReceiptRef,
    },
    {
      recoveryRef: "recovery:pylon-restart",
      cohortRef: "cohort:owner-managed:simulator",
      targetClass: "owner_managed",
      scenario: "The Pylon process restarted after it claimed a phase.",
      evidenceClass: "simulator",
      outcome: "passed",
      recoveryPointRef: "portable-phase-claim-journal",
      receiptRef: "apps/pylon/src/portable-phase-operation-claim-journal.test.ts",
    },
    {
      recoveryRef: "recovery:owner-local-bounded-work-resume",
      cohortRef: ownerLocalInput.cohort.cohortRef,
      targetClass: "owner_local",
      scenario:
        "One refs-only accepted work item resumed at generation 2, settled once, replayed idempotently, and was fenced after generation 3 became active.",
      evidenceClass: "real_local",
      outcome: "passed",
      recoveryPointRef: "owner-local-generation-2-bounded-handler-settlement",
      receiptRef: ownerLocalExecutorResume.execution.receiptRef,
    },
    {
      recoveryRef: "recovery:provider-eviction",
      cohortRef: "cohort:managed-provider:unclaimed",
      targetClass: "managed_provider",
      scenario: "The provider evicted the target after source revocation.",
      evidenceClass: "not_run",
      outcome: "not_run",
      recoveryPointRef: null,
      receiptRef: null,
    },
  ],
  faultFacts: ownerLocalFaultMatrix.cases.map((fault) => {
    const stronger =
      fault.phase === null ? strongerFaultsByScenario.get(fault.scenario) : undefined;
    return {
      faultRef: fault.faultRef,
      cohortRef: ownerLocalInput.cohort.cohortRef,
      targetClass: "owner_local" as const,
      scenario: fault.scenario,
      phase: fault.phase,
      evidenceClass: stronger?.evidenceClass ?? fault.evidenceClass,
      outcome: stronger?.outcome ?? fault.outcome,
      recoveryPointRef: stronger?.recoveryPointRef ?? fault.recoveryPointRef,
      receiptRef: stronger?.receiptRef ?? fault.receiptRef,
    };
  }),
  security: {
    forbiddenMaterialProjected: false,
    optimisticAuthorityProjected: false,
    staleGenerationAccepted: false,
    rawCredentialProjected: false,
  },
  review: {
    independentReviewerRef: null,
    independentDisposition: "not_run",
    independentDispositionRef: null,
    ownerRef: null,
    ownerDisposition: "not_run",
    ownerDispositionRef: null,
  },
  implementationChecksPassed: true,
  acceptancePassed: false,
  remainingGaps: [
    "The real owner-managed and OpenAgents-managed move cohorts did not run. No managed provider is admitted.",
    "DAP and native helper profiles have no admitted signed installed artifacts.",
    "The owner-managed enrollment and checkpoint-key custody design requires the recorded owner decision.",
    "The managed root filesystem needs a Linux x64 rebuild before the live Firecracker proof can run.",
    "Nine real local fault rows passed. Nine other required rows passed only with simulator fixtures, and nine required fault, restart, teardown, or older-recovery-point rows did not run.",
    "A signed-out packaged shell ran concurrently with one same-host owner-local move. The packaged shell did not initiate or authenticate the move, and packaged authenticated journeys did not run on each required target.",
    "The complete phase and resource metric matrices are absent for the owner-managed, OpenAgents-managed, and admitted provider cohorts.",
    "The independent reviewer and owner dispositions are absent.",
  ],
});

validateIdePortableEvidenceReceipt(receipt, { candidateCommitSha, baseCommitSha });
mkdirSync(path.dirname(output), { recursive: true });
writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`, { mode: 0o600 });
process.stdout.write(`[openagents-desktop] IDE-13 portability evidence: ${output}\n`);
