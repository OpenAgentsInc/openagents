import { cpus } from "node:os"
import { assertPublicProjectionSafe } from "./state.js"
import {
  PROBE_BENCHMARK_ASSIGNMENT_SCHEMA_REF,
  PROBE_BENCHMARK_CLOSEOUT_SCHEMA_REF,
  PROBE_BENCHMARK_RUN_SCHEMA_REF,
} from "../packages/runtime/src/contracts/benchmark.js"
import { STATIC_RETAINED_TERMINAL_BENCH_FIXTURES } from "../packages/runtime/src/benchmark/fixtures.js"

export const PYLON_GEPA_BENCHMARK_RUNNER_CAPABILITY_REF = "pylon.capability.gepa.benchmark_runner.v0.3"
export const PYLON_GEPA_RETAINED_TERMINAL_BENCH_CAPABILITY_REF =
  "pylon.capability.gepa.retained_terminal_bench.v0.3"
export const PYLON_ARTIFACT_UPLOAD_CAPABILITY_REF = "pylon.capability.artifact_upload.v0.3"
export const PYLON_PROOF_RECEIPT_CAPABILITY_REF = "pylon.capability.proof_receipt.v0.3"
export const PYLON_ASSIGNMENT_CLOSEOUT_CAPABILITY_REF = "pylon.capability.assignment_closeout.v0.3"
export const PYLON_LOCAL_SANDBOX_ISOLATION_REF = "pylon.isolation.local_sandbox.v0.3"
export const PYLON_PROBE_RUNTIME_BACKEND_REF = "pylon.backend.probe_runtime.v0.3"

export type PylonGepaCapabilityEnvelope = {
  schema: "openagents.pylon.gepa_capability_envelope.v0.3"
  stage: "gepa-first"
  runtimeContractRefs: string[]
  capabilityRefs: string[]
  benchmarkSuiteRefs: string[]
  retainedFixtureRefs: string[]
  backendRefs: string[]
  supportedIsolationProfileRefs: string[]
  supportsArtifactUpload: boolean
  supportsProofReceipts: boolean
  supportsCloseout: boolean
  supportsTraining: false
  qwenTrack: "postponed"
  capacity: {
    cpuThreads: number
    ramGb: number
    diskGb: number
    gpuKind: "none" | "apple" | "nvidia" | "amd" | "unknown"
    vramGb: number
  }
  budgets: {
    maxWallClockMs: number
    maxCostUsd: number
  }
  payoutReadiness: {
    ready: boolean
    fresh: boolean
    observedAt: string | null
  }
}

export type PylonGepaAssignmentRequirements = {
  schema: "openagents.pylon.gepa_assignment_requirements.v0.3"
  workKind: "gepa_benchmark_metric_call" | "benchmark_evaluation" | "training"
  benchmarkSuiteRef: string
  retainedFixtureRef?: string
  requiredCapabilityRefs: string[]
  backendRef: string
  isolationProfileRef: string
  artifactUploadRequired: boolean
  proofReceiptRequired: boolean
  closeoutRequired: boolean
  payoutRequired: boolean
  trainingClaim: boolean
  maxWallClockMs: number
  maxCostUsd: number
}

export type PylonGepaAdmission = {
  admissible: boolean
  selectedCapabilityRefs: string[]
  runtimeContractRefs: string[]
  blockerRefs: string[]
  payoutReadyForSettlement: boolean
  trainingPostponed: true
}

export function createDefaultGepaCapabilityEnvelope(
  input: Partial<
    Pick<
      PylonGepaCapabilityEnvelope,
      | "capabilityRefs"
      | "backendRefs"
      | "supportedIsolationProfileRefs"
      | "supportsArtifactUpload"
      | "supportsProofReceipts"
      | "supportsCloseout"
      | "capacity"
      | "budgets"
      | "payoutReadiness"
    >
  > = {},
) {
  const envelope: PylonGepaCapabilityEnvelope = {
    schema: "openagents.pylon.gepa_capability_envelope.v0.3",
    stage: "gepa-first",
    runtimeContractRefs: [
      PROBE_BENCHMARK_ASSIGNMENT_SCHEMA_REF,
      PROBE_BENCHMARK_RUN_SCHEMA_REF,
      PROBE_BENCHMARK_CLOSEOUT_SCHEMA_REF,
    ],
    capabilityRefs: input.capabilityRefs ?? [
      PYLON_GEPA_BENCHMARK_RUNNER_CAPABILITY_REF,
      PYLON_GEPA_RETAINED_TERMINAL_BENCH_CAPABILITY_REF,
      PYLON_ARTIFACT_UPLOAD_CAPABILITY_REF,
      PYLON_PROOF_RECEIPT_CAPABILITY_REF,
      PYLON_ASSIGNMENT_CLOSEOUT_CAPABILITY_REF,
    ],
    benchmarkSuiteRefs: [...new Set(STATIC_RETAINED_TERMINAL_BENCH_FIXTURES.map((fixture) => fixture.benchmarkSuiteRef))],
    retainedFixtureRefs: STATIC_RETAINED_TERMINAL_BENCH_FIXTURES.map((fixture) => fixture.fixtureRef),
    backendRefs: input.backendRefs ?? [PYLON_PROBE_RUNTIME_BACKEND_REF],
    supportedIsolationProfileRefs: input.supportedIsolationProfileRefs ?? [PYLON_LOCAL_SANDBOX_ISOLATION_REF],
    supportsArtifactUpload: input.supportsArtifactUpload ?? true,
    supportsProofReceipts: input.supportsProofReceipts ?? true,
    supportsCloseout: input.supportsCloseout ?? true,
    supportsTraining: false,
    qwenTrack: "postponed",
    capacity: input.capacity ?? {
      cpuThreads: Math.max(1, cpus().length),
      ramGb: 0,
      diskGb: 0,
      gpuKind: "unknown",
      vramGb: 0,
    },
    budgets: input.budgets ?? {
      maxWallClockMs: 20 * 60 * 1000,
      maxCostUsd: 0,
    },
    payoutReadiness: input.payoutReadiness ?? {
      ready: false,
      fresh: false,
      observedAt: null,
    },
  }
  assertPublicProjectionSafe(envelope)
  return envelope
}

export function createRetainedGepaAssignmentRequirements(
  input: Partial<PylonGepaAssignmentRequirements> = {},
) {
  const fixture = STATIC_RETAINED_TERMINAL_BENCH_FIXTURES[0]
  const requirements: PylonGepaAssignmentRequirements = {
    schema: "openagents.pylon.gepa_assignment_requirements.v0.3",
    workKind: "gepa_benchmark_metric_call",
    benchmarkSuiteRef: fixture.benchmarkSuiteRef,
    retainedFixtureRef: fixture.fixtureRef,
    requiredCapabilityRefs: [
      PYLON_GEPA_BENCHMARK_RUNNER_CAPABILITY_REF,
      PYLON_GEPA_RETAINED_TERMINAL_BENCH_CAPABILITY_REF,
      PYLON_ARTIFACT_UPLOAD_CAPABILITY_REF,
      PYLON_PROOF_RECEIPT_CAPABILITY_REF,
      PYLON_ASSIGNMENT_CLOSEOUT_CAPABILITY_REF,
    ],
    backendRef: PYLON_PROBE_RUNTIME_BACKEND_REF,
    isolationProfileRef: PYLON_LOCAL_SANDBOX_ISOLATION_REF,
    artifactUploadRequired: true,
    proofReceiptRequired: true,
    closeoutRequired: true,
    payoutRequired: false,
    trainingClaim: false,
    maxWallClockMs: 10 * 60 * 1000,
    maxCostUsd: 0,
    ...input,
  }
  assertPublicProjectionSafe(requirements)
  return requirements
}

export function admitGepaAssignmentToEnvelope(
  envelope: PylonGepaCapabilityEnvelope,
  requirements: PylonGepaAssignmentRequirements,
): PylonGepaAdmission {
  const blockerRefs = new Set<string>()
  const capabilitySet = new Set(envelope.capabilityRefs)
  const selectedCapabilityRefs = requirements.requiredCapabilityRefs.filter((ref) => capabilitySet.has(ref))

  if (!requirements.requiredCapabilityRefs.every((ref) => capabilitySet.has(ref))) {
    blockerRefs.add("blocker.gepa.wrong_capability")
  }
  if (!envelope.backendRefs.includes(requirements.backendRef)) {
    blockerRefs.add("blocker.gepa.unsupported_backend")
  }
  if (!envelope.supportedIsolationProfileRefs.includes(requirements.isolationProfileRef)) {
    blockerRefs.add("blocker.gepa.missing_isolation_profile")
  }
  if (requirements.artifactUploadRequired && !envelope.supportsArtifactUpload) {
    blockerRefs.add("blocker.gepa.artifact_upload_unavailable")
  }
  if (requirements.proofReceiptRequired && !envelope.supportsProofReceipts) {
    blockerRefs.add("blocker.gepa.proof_receipts_unavailable")
  }
  if (requirements.closeoutRequired && !envelope.supportsCloseout) {
    blockerRefs.add("blocker.gepa.closeout_unavailable")
  }
  if (requirements.payoutRequired && (!envelope.payoutReadiness.ready || !envelope.payoutReadiness.fresh)) {
    blockerRefs.add("blocker.gepa.payout_readiness_stale")
  }
  if (requirements.trainingClaim || requirements.workKind === "training") {
    blockerRefs.add("blocker.gepa.training_claim_postponed")
  }
  if (requirements.maxWallClockMs > envelope.budgets.maxWallClockMs) {
    blockerRefs.add("blocker.gepa.wall_clock_budget_exceeded")
  }
  if (requirements.maxCostUsd > envelope.budgets.maxCostUsd) {
    blockerRefs.add("blocker.gepa.cost_budget_exceeded")
  }
  if (!envelope.benchmarkSuiteRefs.includes(requirements.benchmarkSuiteRef)) {
    blockerRefs.add("blocker.gepa.unsupported_benchmark_suite")
  }
  if (requirements.retainedFixtureRef && !envelope.retainedFixtureRefs.includes(requirements.retainedFixtureRef)) {
    blockerRefs.add("blocker.gepa.unsupported_retained_fixture")
  }

  const admission: PylonGepaAdmission = {
    admissible: blockerRefs.size === 0,
    selectedCapabilityRefs,
    runtimeContractRefs: envelope.runtimeContractRefs,
    blockerRefs: [...blockerRefs],
    payoutReadyForSettlement: requirements.payoutRequired && envelope.payoutReadiness.ready && envelope.payoutReadiness.fresh,
    trainingPostponed: true,
  }
  assertPublicProjectionSafe(admission)
  return admission
}
