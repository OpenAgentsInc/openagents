import { execFileSync } from "node:child_process"
import { mkdirSync, writeFileSync } from "node:fs"
import { performance } from "node:perf_hooks"
import path from "node:path"

import { Schema } from "effect"
import {
  IdePortableCheckpointManifestSchema,
  checkIdePortableModel,
} from "@openagentsinc/portable-session-contract"

import {
  IDE_PORTABLE_PHASES,
  IdePortableEvidenceReceiptSchema,
  validateIdePortableEvidenceReceipt,
} from "../src/ide/portable-evidence-contract.ts"

const appRoot = path.resolve(import.meta.dirname, "..")
const repositoryRoot = path.resolve(appRoot, "../..")
const output = path.join(appRoot, "benchmarks", "ide", "2026-07-20-ide-13-portability.json")
const git = (...args: string[]): string =>
  execFileSync("git", args, { cwd: repositoryRoot, encoding: "utf8" }).trim()
const valueAt = (values: ReadonlyArray<number>, percentile: number): number =>
  values.toSorted((left, right) => left - right)[Math.floor((values.length - 1) * percentile)] ?? 0
const measureLatency = (
  metricRef: string,
  operation: () => void,
  repetitions: number,
  thresholdP95: number,
  thresholdP99: number,
) => {
  const samples: number[] = []
  operation()
  for (let index = 0; index < repetitions; index += 1) {
    const start = performance.now()
    operation()
    samples.push(performance.now() - start)
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
  }
}

const digest = `sha256:${"a".repeat(64)}`
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
    projectRef: "project.benchmark", projectRootRef: "root.benchmark", worktreeRef: "worktree.benchmark",
    selectedFileRef: null, documentSnapshotRef: null, proposalRef: null, diagnosticResultRef: null,
    testResultRef: null, artifactRef: null, evidenceRef: null,
  },
  includedCapabilityRefs: ["capability.files"],
  omittedCapabilityRefs: ["capability.pty", "capability.lsp", "capability.dap"],
  historyRefs: [], proposalRefs: [], taskRefs: [], testRefs: [], deliveryEvidenceRefs: [],
  secretMaterial: "excluded", processState: "excluded", nativeState: "excluded",
  vimState: "destination_setting", themeState: "destination_setting",
  policy: {
    maximumBytes: 1_048_576, maximumFiles: 10_000, encryption: "owner_key",
    encryptionKeyRef: "key.benchmark", custody: "owner_device", retentionSeconds: 3_600,
    expiresAt: "2030-01-01T00:00:00.000Z",
  },
  integrityReceiptRef: "integrity.benchmark",
} as const

const decodeCheckpoint = Schema.decodeUnknownSync(IdePortableCheckpointManifestSchema)
const model = checkIdePortableModel({ maximumDepth: 12 })
if (!model.passed) throw new Error("IDE-13 model check failed")
const localMetrics = [
  measureLatency("bounded-transition-model", () => { checkIdePortableModel({ maximumDepth: 12 }) }, 100, 5, 10),
  measureLatency("checkpoint-schema-decode", () => { decodeCheckpoint(checkpoint) }, 1_000, 1, 2),
]
if (localMetrics.some(metric => !metric.passed)) {
  throw new Error(`IDE-13 local threshold failed: ${JSON.stringify(localMetrics)}`)
}

const candidateCommitSha = git("rev-parse", "HEAD")
const baseCommitSha = git("merge-base", "HEAD", "origin/main")
const simulatedPhases = (receiptRef: string) => IDE_PORTABLE_PHASES.map(phase => ({
  phase,
  evidenceClass: "simulator" as const,
  receiptRef,
  operationRef: `simulated-operation:${phase}`,
  attachmentGeneration: 1,
  result: "passed" as const,
}))
const absentPhases = () => IDE_PORTABLE_PHASES.map(phase => ({
  phase,
  evidenceClass: "not_run" as const,
  receiptRef: null,
  operationRef: null,
  attachmentGeneration: null,
  result: "not_run" as const,
}))

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
    { checkRef: "bounded-attachment-model", evidenceClass: "model", result: "passed", receiptRef: "packages/portable-session-contract/src/ide13-model.test.ts" },
    { checkRef: "coordinator-transition-faults", evidenceClass: "regression", result: "passed", receiptRef: "apps/openagents-desktop/src/ide/portable-coordinator-service.test.ts" },
    { checkRef: "durable-command-replay", evidenceClass: "regression", result: "passed", receiptRef: "packages/khala-sync-server/src/portable-session-command-consumer.test.ts" },
    { checkRef: "durable-phase-exchange", evidenceClass: "regression", result: "passed", receiptRef: "packages/khala-sync-server/src/portable-phase-operation-store.test.ts" },
    { checkRef: "phase-target-binding", evidenceClass: "regression", result: "passed", receiptRef: "packages/khala-sync-server/src/portable-phase-target-adapter.test.ts" },
    { checkRef: "phase-claim-recovery", evidenceClass: "regression", result: "passed", receiptRef: "apps/pylon/src/portable-phase-operation-claim-journal.test.ts" },
    { checkRef: "encrypted-checkpoint-custody", evidenceClass: "regression", result: "passed", receiptRef: "apps/pylon/tests/portable-session-checkpoint-artifact.test.ts" },
    { checkRef: "workspace-search-revocation", evidenceClass: "regression", result: "passed", receiptRef: "apps/openagents-desktop/src/workspace-search-host.test.ts" },
    { checkRef: "packaged-fail-closed-boundary", evidenceClass: "packaged_fail_closed", result: "passed", receiptRef: "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-portability-packaged.json" },
  ],
  placementCohorts: [
    {
      cohortRef: "cohort:owner-local:model", targetClass: "owner_local",
      evidenceClass: "simulator", journeyScope: "foundation_only",
      journeys: { mainJourneyReceiptRef: null, failbackJourneyReceiptRef: null, faultMatrixReceiptRef: null },
      operatingSystem: process.platform === "darwin" ? "darwin" : process.platform === "win32" ? "windows" : "linux",
      architecture: process.arch === "arm64" ? "arm64" : process.arch === "x64" ? "x64" : "unknown",
      adapter: { kind: "deterministic_simulator", ref: "adapter.desktop.local-model", name: "IDE-13 bounded local model", version: "2" },
      targetRef: "target:local-node-process", artifact: { ref: null, sha256: null, bytes: null },
      candidateCommitSha, baseCommitSha, capabilityState: "unverified", custody: "owner_device",
      networkDestinations: [], dataDestinations: ["owner_device"], retentionSeconds: 3_600,
      costFact: "No incremental provider cost was measured.",
      phaseReceipts: simulatedPhases("apps/openagents-desktop/src/ide/portable-coordinator-service.test.ts"),
      metrics: localMetrics,
      result: "The local process ran the model and schema checks. It did not run a real move.",
    },
    {
      cohortRef: "cohort:owner-managed:simulator", targetClass: "owner_managed",
      evidenceClass: "simulator", journeyScope: "foundation_only",
      journeys: { mainJourneyReceiptRef: null, failbackJourneyReceiptRef: null, faultMatrixReceiptRef: null },
      operatingSystem: "unknown", architecture: "unknown",
      adapter: { kind: "deterministic_simulator", ref: "adapter.owner-managed.simulator", name: "Owner-managed contract simulator", version: "2" },
      targetRef: null, artifact: { ref: null, sha256: null, bytes: null }, candidateCommitSha, baseCommitSha,
      capabilityState: "unverified", custody: "owner_managed", networkDestinations: [], dataDestinations: [],
      retentionSeconds: 0, costFact: "Cost was not measured.",
      phaseReceipts: simulatedPhases("apps/openagents-desktop/src/ide/portable-coordinator-service.test.ts"), metrics: [],
      result: "Only the deterministic coordinator and target contracts ran.",
    },
    {
      cohortRef: "cohort:openagents-managed:simulator", targetClass: "openagents_managed",
      evidenceClass: "simulator", journeyScope: "foundation_only",
      journeys: { mainJourneyReceiptRef: null, failbackJourneyReceiptRef: null, faultMatrixReceiptRef: null },
      operatingSystem: "linux", architecture: "unknown",
      adapter: { kind: "deterministic_simulator", ref: "adapter.openagents-managed.simulator", name: "OpenAgents managed target simulator", version: "2" },
      targetRef: null, artifact: { ref: null, sha256: null, bytes: null }, candidateCommitSha, baseCommitSha,
      capabilityState: "unverified", custody: "openagents_managed", networkDestinations: [], dataDestinations: [],
      retentionSeconds: 0, costFact: "Cost was not measured.",
      phaseReceipts: simulatedPhases("packages/khala-sync-server/src/portable-phase-target-adapter.test.ts"), metrics: [],
      result: "Only the managed target adapter regressions ran. No live managed move ran.",
    },
    {
      cohortRef: "cohort:managed-provider:unclaimed", targetClass: "managed_provider",
      evidenceClass: "not_run", journeyScope: "not_run",
      journeys: { mainJourneyReceiptRef: null, failbackJourneyReceiptRef: null, faultMatrixReceiptRef: null },
      operatingSystem: "unknown", architecture: "unknown",
      adapter: { kind: "not_run", ref: null, name: null, version: null }, targetRef: null,
      artifact: { ref: null, sha256: null, bytes: null }, candidateCommitSha, baseCommitSha,
      capabilityState: "unsupported", custody: "unverified", networkDestinations: [], dataDestinations: [],
      retentionSeconds: 0, costFact: "No provider is admitted.", phaseReceipts: absentPhases(), metrics: [],
      result: "No managed-provider cohort ran.",
    },
  ],
  omissions: [
    { omissionRef: "omission:process-state", targetClass: "owner_local", fact: "The checkpoint excludes process and terminal state.", disposition: "accepted_limit", evidenceRef: "packages/portable-session-contract/src/ide13-contract.ts" },
    { omissionRef: "omission:real-cohorts", targetClass: "owner_managed", fact: "The real owner-managed move cohort did not run.", disposition: "acceptance_gap", evidenceRef: "github.com/OpenAgentsInc/openagents/issues/9041" },
    { omissionRef: "omission:managed-live", targetClass: "openagents_managed", fact: "The real OpenAgents-managed move cohort did not run.", disposition: "acceptance_gap", evidenceRef: "github.com/OpenAgentsInc/openagents/issues/9041" },
    { omissionRef: "omission:provider", targetClass: "managed_provider", fact: "No managed provider is admitted.", disposition: "acceptance_gap", evidenceRef: "github.com/OpenAgentsInc/openagents/issues/9041" },
  ],
  recoveryFacts: [
    { recoveryRef: "recovery:lost-ack", cohortRef: "cohort:owner-local:model", targetClass: "owner_local", scenario: "The command result ACK was lost.", evidenceClass: "simulator", outcome: "passed", recoveryPointRef: "durable-command-row", receiptRef: "packages/khala-sync-server/src/portable-session-command-consumer.test.ts" },
    { recoveryRef: "recovery:pylon-restart", cohortRef: "cohort:owner-managed:simulator", targetClass: "owner_managed", scenario: "The Pylon process restarted after it claimed a phase.", evidenceClass: "simulator", outcome: "passed", recoveryPointRef: "portable-phase-claim-journal", receiptRef: "apps/pylon/src/portable-phase-operation-claim-journal.test.ts" },
    { recoveryRef: "recovery:provider-eviction", cohortRef: "cohort:managed-provider:unclaimed", targetClass: "managed_provider", scenario: "The provider evicted the target after source revocation.", evidenceClass: "not_run", outcome: "not_run", recoveryPointRef: null, receiptRef: null },
  ],
  faultFacts: [],
  security: {
    forbiddenMaterialProjected: false,
    optimisticAuthorityProjected: false,
    staleGenerationAccepted: false,
    rawCredentialProjected: false,
  },
  review: {
    independentReviewerRef: null, independentDisposition: "not_run", independentDispositionRef: null,
    ownerRef: null, ownerDisposition: "not_run", ownerDispositionRef: null,
  },
  implementationChecksPassed: true,
  acceptancePassed: false,
  remainingGaps: [
    "The real local, owner-managed, OpenAgents-managed, and admitted-provider move cohorts did not run.",
    "The real transition fault, restart, teardown, and older-recovery-point matrix did not run.",
    "The packaged authenticated move journeys did not run on each required target.",
    "The complete phase, size, CPU, memory, network, queue, lease, resource, and teardown metric matrices are absent.",
    "The independent reviewer and owner dispositions are absent.",
  ],
})

validateIdePortableEvidenceReceipt(receipt, { candidateCommitSha, baseCommitSha })
mkdirSync(path.dirname(output), { recursive: true })
writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`, { mode: 0o600 })
process.stdout.write(`[openagents-desktop] IDE-13 portability evidence: ${output}\n`)
