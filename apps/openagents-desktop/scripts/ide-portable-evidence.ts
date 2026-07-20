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
  IdePortableEvidenceMetricSchema,
  IdePortableEvidenceReceiptSchema,
} from "../src/ide/portable-evidence-contract.ts"

const appRoot = path.resolve(import.meta.dirname, "..")
const repositoryRoot = path.resolve(appRoot, "../..")
const output = path.join(appRoot, "benchmarks", "ide", "2026-07-20-ide-13-portability.json")
const git = (...args: string[]): string => execFileSync("git", args, { cwd: repositoryRoot, encoding: "utf8" }).trim()
const valuesAt = (values: number[], amount: number): number =>
  [...values].sort((left, right) => left - right)[Math.floor((values.length - 1) * amount)] ?? 0
const measure = (operation: string, repetitions: number, run: () => void, thresholdP95Ms: number, thresholdP99Ms: number) => {
  const samples: number[] = []
  run()
  for (let index = 0; index < repetitions; index += 1) {
    const start = performance.now()
    run()
    samples.push(performance.now() - start)
  }
  const p95Ms = valuesAt(samples, 0.95)
  const p99Ms = valuesAt(samples, 0.99)
  return IdePortableEvidenceMetricSchema.make({
    operation,
    repetitions,
    p50Ms: valuesAt(samples, 0.5),
    p95Ms,
    p99Ms,
    thresholdP95Ms,
    thresholdP99Ms,
    passed: p95Ms <= thresholdP95Ms && p99Ms <= thresholdP99Ms,
  })
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
const metrics = [
  measure("bounded-transition-model", 100, () => { checkIdePortableModel({ maximumDepth: 12 }) }, 5, 10),
  measure("checkpoint-schema-decode", 1_000, () => { decodeCheckpoint(checkpoint) }, 1, 2),
]
if (metrics.some(value => !value.passed)) throw new Error(`IDE-13 performance threshold failed: ${JSON.stringify(metrics)}`)

const receipt = Schema.decodeUnknownSync(IdePortableEvidenceReceiptSchema)({
  schemaVersion: "openagents.desktop.ide-portable-evidence.v1",
  issue: "IDE-13",
  candidateCommitSha: git("rev-parse", "HEAD"),
  generatedAt: new Date().toISOString(),
  environment: { platform: process.platform, architecture: process.arch, node: process.version },
  model: {
    maximumDepth: model.maximumDepth,
    exploredStates: model.exploredStates,
    exploredTransitions: model.exploredTransitions,
    staleWriteAttempts: model.staleWriteAttempts,
    counterexamples: model.counterexamples.length,
    passed: true,
  },
  metrics,
  placementCohorts: [
    {
      targetClass: "owner_local", evidenceClass: "real_local", operatingSystem: process.platform === "darwin" ? "darwin" : process.platform === "win32" ? "windows" : "linux",
      architecture: process.arch === "arm64" ? "arm64" : process.arch === "x64" ? "x64" : "unknown",
      adapterRef: "adapter.desktop.local", capabilityState: "ready", custody: "owner_device",
      networkDestinations: [], dataDestinations: ["owner_device"], retentionSeconds: 3_600, costFact: "no incremental provider cost",
      result: "Real local Node process ran the bounded model and decoded checkpoint boundary. It did not prove a cross-host move.",
    },
    {
      targetClass: "owner_managed", evidenceClass: "deterministic_simulator", operatingSystem: "unknown", architecture: "unknown",
      adapterRef: "adapter.owner-managed.simulator", capabilityState: "unverified", custody: "owner_managed",
      networkDestinations: [], dataDestinations: [], retentionSeconds: 0, costFact: "not measured",
      result: "Contract and coordinator simulator only. No real owner-managed host cohort ran.",
    },
    {
      targetClass: "openagents_managed", evidenceClass: "deterministic_simulator", operatingSystem: "linux", architecture: "unknown",
      adapterRef: "adapter.openagents-managed.simulator", capabilityState: "unverified", custody: "openagents_managed",
      networkDestinations: [], dataDestinations: [], retentionSeconds: 0, costFact: "not measured",
      result: "Existing managed-placement projection plus coordinator simulator only. SBX-09 live acceptance is absent.",
    },
    {
      targetClass: "managed_provider", evidenceClass: "not_run", operatingSystem: "unknown", architecture: "unknown",
      adapterRef: "adapter.managed-provider.unadmitted", capabilityState: "unsupported", custody: "unverified",
      networkDestinations: [], dataDestinations: [], retentionSeconds: 0, costFact: "not applicable",
      result: "No audited provider is admitted or claimed.",
    },
  ],
  faultCoverage: [
    { fault: "stale generation command", evidence: "model", result: "passed", evidenceRef: "packages/portable-session-contract/src/ide13-model.test.ts" },
    { fault: "destination attach before source revocation", evidence: "model", result: "passed", evidenceRef: "packages/portable-session-contract/src/ide13-model.test.ts" },
    { fault: "duplicate, lost-ack, and replay", evidence: "model", result: "passed", evidenceRef: "packages/portable-session-contract/src/ide13-model.test.ts" },
    { fault: "cancel after destination stage and before source revoke", evidence: "regression", result: "passed", evidenceRef: "apps/openagents-desktop/src/ide/portable-coordinator-service.test.ts" },
    { fault: "real network partition at each transition", evidence: "not_run", result: "gap", evidenceRef: "github.com/OpenAgentsInc/openagents/issues/9041" },
    { fault: "real provider eviction and older recovery-point failback", evidence: "not_run", result: "gap", evidenceRef: "github.com/OpenAgentsInc/openagents/issues/9041" },
  ],
  security: {
    forbiddenMaterialProjected: false,
    optimisticAuthorityProjected: false,
    staleGenerationAccepted: false,
    rawCredentialProjected: false,
  },
  implementationChecksPassed: true,
  acceptancePassed: false,
  remainingGaps: [
    "Real owner-managed, OpenAgents-managed, and admitted-provider move cohorts did not run.",
    "The complete transition fault-injection, restart, teardown, and older-recovery-point matrix did not run on real placements.",
    "Packaged macOS, Windows, and Linux move journeys and independent owner or AssuranceSpec review are absent.",
    "End-to-end p50, p95, and p99 quiesce, upload, redeem, attach, helper readiness, failback, resource, and size curves are absent.",
  ],
})
mkdirSync(path.dirname(output), { recursive: true })
writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`, { mode: 0o600 })
process.stdout.write(`[openagents-desktop] IDE-13 portability evidence: ${output}\n`)
