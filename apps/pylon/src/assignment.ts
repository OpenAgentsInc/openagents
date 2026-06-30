import { existsSync } from "node:fs"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { createHash, randomUUID } from "node:crypto"
import {
  collectInterpreterOutputs,
  executeTassadarNumericModel,
  TASSADAR_EXECUTOR_CAPABILITY_REF,
  TASSADAR_EXECUTOR_TRACE_HOMEWORK_JOB_KIND,
  TASSADAR_EXECUTOR_TRACE_JOB_KIND,
} from "@openagentsinc/tassadar-executor"
import type { BootstrapSummary } from "./bootstrap.js"
import {
  loadClaudeAgentConfig,
  probeClaudeAgentReadiness,
  type ClaudeAgentProbeOptions,
} from "./claude-agent.js"
import {
  type ClaudeAgentCheckoutRunner,
  type ClaudeAgentRunner,
} from "./claude-agent-executor.js"
import {
  loadCodexAgentConfig,
  probeCodexAgentReadiness,
  type CodexAgentProbeOptions,
} from "./codex-agent.js"
import type { PylonCodexAuthValidityProbe } from "./account-connect.js"
import { probeAndRecordCodexAccountAuthHealth } from "./codex-account-auth-health.js"
import type { CodexAgentRuntimePhase, CodexAgentRunner } from "./codex-agent-executor.js"
import {
  agentRunnerForLease,
  agentRunnerResolutionForLease,
  agentRunnerServiceForLease,
  executeRegisteredAgentRunner,
  type AgentRunnerCloseoutRecord,
} from "./agent-runner-registry.js"
import { createSignedHeaders, sendHeartbeat } from "./presence.js"
import { PresenceRequestError } from "./presence-error.js"
import {
  finishActiveCodingRun,
  refreshActiveCodingRun,
  registerActiveCodingRun,
  type PylonCodingServiceRef,
} from "./active-assignment-runs.js"
import {
  assertPublicProjectionSafe,
  ensurePylonLocalState,
  ensureStateDirectories,
  loadOrCreatePresenceState,
  type PylonLocalState,
} from "./state.js"
import {
  admitGepaAssignmentToEnvelope,
  createDefaultGepaCapabilityEnvelope,
  type PylonGepaAssignmentRequirements,
  type PylonGepaCapabilityEnvelope,
} from "./gepa-capability.js"
import {
  PSIONIC_QWEN_MODEL_REFS,
  selectPsionicQwenModel,
  type PsionicQwenModelAdmission,
  type PsionicQwenTaskMode,
} from "../packages/runtime/src/index.js"
import {
  pylonAccountEnvironment,
  resolvePylonAccountSelection,
  type PylonAccountProvider,
  type ResolvedPylonAccountSelection,
} from "./account-registry.js"
import {
  resolvePylonAccountUsageRefreshTargets,
  type PylonAccountUsageRefreshTarget,
} from "./account-usage.js"
import { isAccountAvailable, loadQuotaRecord } from "./account-quota-ledger.js"
import {
  codexAccountHealthBlocksReadiness,
  loadCodexAccountHealthRecord,
} from "./codex-account-health-ledger.js"
import {
  admitPylonDelegation,
  pylonDelegationChainFrom,
  type PylonDelegationChain,
} from "./capability-delegation.js"

export type AssignmentPaymentMode = "no-spend" | "paid"
export type AssignmentStatus = "offered" | "accepted" | "running" | "closed" | "rejected" | "cancelled" | "timed-out" | "stale"

export type PylonAssignmentLease = {
  schema: "openagents.pylon.assignment_lease.v0.3"
  assignmentRef: string
  leaseRef: string
  goal: string
  paymentMode: AssignmentPaymentMode
  capabilityRefs: string[]
  codingAssignment?: AutopilotCodingAssignmentPayload
  delegation?: PylonDelegationChain
  delegationInvalid?: boolean
  backendRef?: string
  gepaRequirements?: PylonGepaAssignmentRequirements
  psionicQwenRequirements?: PylonPsionicQwenAssignmentRequirements
  expiresAt: string
  createdAt?: string
}

export type PylonPsionicQwenAssignmentRequirements = {
  workClass: "local_inference"
  mode: PsionicQwenTaskMode
  requiredModelRef?: string
  receiptRefs?: string[]
}

export type AssignmentPollResponse = {
  schema?: "openagents.pylon.assignment_poll_response.v0.3"
  leases?: PylonAssignmentLease[]
  assignment?: PylonAssignmentLease
}

export type AssignmentAcceptance = {
  ok: boolean
  accepted: boolean
  assignmentRef: string
  leaseRef: string
  statusRef: string
  denialRef?: string
  blockerRefs: string[]
}

export type AssignmentProgress = {
  schema: "openagents.pylon.assignment_progress.v0.3"
  assignmentRef: string
  leaseRef: string
  sequence: number
  status: "accepted" | "running" | "artifact-ready" | "proof-ready" | "closeout-submitted"
  message: string
  artifactRefs: string[]
  proofRefs: string[]
  observedAt: string
  elapsedMs?: number
  phase?: "runtime_active" | CodexAgentRuntimePhase
  tokensSoFar?: number
  lastProgressEvent?: AssignmentRunLifecycleEvent["event"] | string
}

export type AssignmentCloseout = {
  schema: "openagents.pylon.assignment_closeout.v0.3"
  assignmentRef: string
  leaseRef: string
  status: "accepted" | "rejected" | "cancelled" | "timed-out" | "stale"
  paymentMode: AssignmentPaymentMode
  settlementState: "not_applicable" | "pending" | "recorded" | "blocked"
  payoutClaimAllowed: boolean
  artifactRefs: string[]
  blockerRefs: string[]
  buildRefs: string[]
  closeoutRefs: string[]
  previewRefs: string[]
  proofRefs: string[]
  receiptRefs: string[]
  resultRefs: string[]
  summaryRefs: string[]
  testRefs: string[]
  redacted: true
  completedAt: string
}

export type AssignmentClientOptions = {
  agentToken?: string
  accountHome?: string
  accountRef?: string
  assignmentRef?: string
  baseUrl: string
  fetch?: typeof fetch
  now?: () => Date
  staleAfterMs?: number
  gepaEnvelope?: PylonGepaCapabilityEnvelope
  psionicQwenAdmission?: PsionicQwenModelAdmission
  claudeAgentCheckoutRunner?: ClaudeAgentCheckoutRunner
  claudeAgentRunner?: ClaudeAgentRunner
  claudeAgentProbe?: ClaudeAgentProbeOptions
  codexAgentRunner?: CodexAgentRunner
  codexAgentProbe?: CodexAgentProbeOptions
  codexAuthValidityProbe?: PylonCodexAuthValidityProbe
  localAssignmentHeartbeatStaleAfterMs?: number
  localProcessIsAlive?: (processId: number) => boolean
  onLifecycleEvent?: (event: AssignmentRunLifecycleEvent) => void | Promise<void>
  requestTimeoutMs?: number
  runtimeHeartbeatIntervalMs?: number
  runtimeProgressIntervalMs?: number
}

export type AssignmentRunLifecycleEvent = {
  schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1"
  event:
    | "assignment_run.poll_complete"
    | "assignment_run.accepted"
    | "assignment_run.runtime_started"
    | "assignment_run.runtime_progress"
    | "assignment_run.runtime_failed"
    | "assignment_run.progress_submitted"
    | "assignment_run.artifacts_submitted"
    | "assignment_run.closeout_submitted"
    | "assignment_run.completed"
    | "assignment_run.no_assignment"
  observedAt: string
  assignmentRef?: string
  leaseRef?: string
  leaseCount?: number
  candidateCount?: number
  status?: AssignmentStatus | AssignmentProgress["status"]
  statusRef?: string
  progressRef?: string
  artifactRef?: string
  closeoutRef?: string
  accountRefHash?: string
  elapsedMs?: number
  phase?: "runtime_active" | CodexAgentRuntimePhase
  tokensSoFar?: number
  lastProgressEvent?: AssignmentRunLifecycleEvent["event"] | string
  blockerRefs?: string[]
}

export type AssignmentRecoveryDiagnostic = {
  schema: "openagents.pylon.assignment_recovery_diagnostic.v0.1"
  blockerRefs: string[]
  diagnosticRef: string
  heartbeatStatus?: number
  recoveryCommand: string
}

type AssignmentStoreLeaseRecord = {
  assignmentRef: string
  status: AssignmentStatus
  acceptedAt?: string
  closedAt?: string
  leaseExpiresAt?: string
  ownerHeartbeatAt?: string
  ownerHeartbeatSequence?: number
  ownerProcessId?: number
  ownerStartedAt?: string
  paymentMode?: AssignmentPaymentMode
  serverCloseoutRef?: string
  serverCloseoutSubmittedAt?: string
}

type AssignmentStore = {
  schema: "openagents.pylon.assignment_state.v0.3"
  leases: Record<string, AssignmentStoreLeaseRecord>
}

type AssignmentCodexAccountSelection = {
  account: ResolvedPylonAccountSelection | null
  accountRefHash?: string
}

export type TrainingWorkerReceipt = {
  schema: "openagents.psionic.training_worker_receipt.v0.3"
  receiptRef: string
  assignmentRef: string
  workerRef: string
  runRef: string
  artifactRefs: string[]
  checkpointRefs: string[]
  metricRefs: string[]
  proofRefs: string[]
  signature: {
    signatureRef: string
    signerRef: string
    verificationRef: string
  }
}

export type TrainingWorkerReceiptsBundle = {
  schema: "openagents.pylon.training_worker_receipts_bundle.v0.3"
  generatedAt: string
  sourceRefs: string[]
  workerReceipts: TrainingWorkerReceipt[]
  budgetLabel?: string
  budgetRef?: string
  evalRef?: string
  lossCurve?: Array<{ step: number; validationLoss: number }>
  maxValidationLoss?: number
  mergeRef?: string
}

type JsonRecord = Record<string, unknown>
type AutopilotCodingAssignmentPayload = Readonly<Record<string, unknown>>
type RuntimeGatePayload = Readonly<{
  agentKind: "codex_cli_or_fixture"
  fixtureRef: "fixture.public.pylon.codex_runtime.sum_repair.v1"
  schema: "openagents.pylon.runtime_gate.v0.3"
}>
type PublicPylonAssignmentProjection = Readonly<{
  assignmentRef?: unknown
  codingAssignment?: unknown
  jobKind?: unknown
  leaseExpiresInSeconds?: unknown
  state?: unknown
  taskRefs?: unknown
}>

const DEFAULT_ASSIGNMENT_REQUEST_TIMEOUT_MS = 30_000

function stableRef(prefix: string, value: string) {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`
}

function safeStringArray(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : []
}

const publicSafeTrainingRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]*$/
const trainingWorkerReceiptsFilename = "training-worker-receipts.json"

const uniqueRefs = (refs: ReadonlyArray<string | null | undefined>): string[] => {
  const seen = new Set<string>()
  const result: string[] = []
  for (const ref of refs) {
    const trimmed = ref?.trim() ?? ""
    if (trimmed === "" || seen.has(trimmed)) continue
    seen.add(trimmed)
    result.push(trimmed)
  }
  return result
}

const publicTrainingRef = (value: unknown): string | null => {
  if (typeof value !== "string") return null
  const trimmed = value.trim()
  if (trimmed === "" || !publicSafeTrainingRefPattern.test(trimmed)) return null
  try {
    assertPublicProjectionSafe(trimmed)
  } catch {
    return null
  }
  return trimmed
}

const publicTrainingRefs = (value: unknown): string[] =>
  uniqueRefs(safeStringArray(value).map(publicTrainingRef))

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null

export function trainingWorkerReceiptsPathForHome(home: string): string {
  return join(home, trainingWorkerReceiptsFilename)
}

const trainingWorkerReceiptFromUnknown = (
  value: unknown,
): TrainingWorkerReceipt | null => {
  if (!isRecord(value)) return null
  const receiptRef = publicTrainingRef(value.receiptRef)
  const assignmentRef = publicTrainingRef(value.assignmentRef)
  const workerRef = publicTrainingRef(value.workerRef)
  const runRef = publicTrainingRef(value.runRef)
  const signature = isRecord(value.signature) ? value.signature : {}
  const signatureRef = publicTrainingRef(signature.signatureRef)
  const signerRef = publicTrainingRef(signature.signerRef)
  const verificationRef = publicTrainingRef(signature.verificationRef)
  if (
    receiptRef === null ||
    assignmentRef === null ||
    workerRef === null ||
    runRef === null ||
    signatureRef === null ||
    signerRef === null ||
    verificationRef === null
  ) {
    return null
  }

  return {
    schema: "openagents.psionic.training_worker_receipt.v0.3",
    receiptRef,
    assignmentRef,
    workerRef,
    runRef,
    artifactRefs: publicTrainingRefs(value.artifactRefs),
    checkpointRefs: publicTrainingRefs(value.checkpointRefs),
    metricRefs: publicTrainingRefs(value.metricRefs),
    proofRefs: publicTrainingRefs(value.proofRefs),
    signature: {
      signatureRef,
      signerRef,
      verificationRef,
    },
  }
}

const optionalBundleRef = (
  bundle: Record<string, unknown>,
  key: keyof TrainingWorkerReceiptsBundle,
): string | undefined => publicTrainingRef(bundle[key]) ?? undefined

const optionalLossCurve = (
  value: unknown,
): Array<{ step: number; validationLoss: number }> | undefined => {
  const points = Array.isArray(value)
    ? value.flatMap(point => {
        if (!isRecord(point)) return []
        const { step, validationLoss } = point
        return typeof step === "number" &&
          Number.isFinite(step) &&
          typeof validationLoss === "number" &&
          Number.isFinite(validationLoss)
          ? [{ step, validationLoss }]
          : []
      })
    : []
  return points.length === 0 ? undefined : points
}

const readTrainingWorkerReceiptsBundle = async (
  path: string,
): Promise<TrainingWorkerReceiptsBundle | null> => {
  if (!existsSync(path)) return null
  try {
    const parsed = JSON.parse(await readFile(path, "utf8")) as unknown
    if (!isRecord(parsed)) return null
    const workerReceipts = Array.isArray(parsed.workerReceipts)
      ? parsed.workerReceipts
          .map(trainingWorkerReceiptFromUnknown)
          .filter((receipt): receipt is TrainingWorkerReceipt => receipt !== null)
      : []
    const generatedAt =
      typeof parsed.generatedAt === "string" ? parsed.generatedAt : new Date().toISOString()
    const budgetLabel =
      typeof parsed.budgetLabel === "string" && parsed.budgetLabel.trim() !== ""
        ? parsed.budgetLabel.trim()
        : undefined
    const maxValidationLoss =
      typeof parsed.maxValidationLoss === "number" &&
      Number.isFinite(parsed.maxValidationLoss)
        ? parsed.maxValidationLoss
        : undefined
    const budgetRef = optionalBundleRef(parsed, "budgetRef")
    const evalRef = optionalBundleRef(parsed, "evalRef")
    const mergeRef = optionalBundleRef(parsed, "mergeRef")
    const lossCurve = optionalLossCurve(parsed.lossCurve)
    return {
      schema: "openagents.pylon.training_worker_receipts_bundle.v0.3",
      generatedAt,
      sourceRefs: publicTrainingRefs(parsed.sourceRefs),
      workerReceipts,
      ...(budgetLabel === undefined ? {} : { budgetLabel }),
      ...(budgetRef === undefined ? {} : { budgetRef }),
      ...(evalRef === undefined ? {} : { evalRef }),
      ...(mergeRef === undefined ? {} : { mergeRef }),
      ...(lossCurve === undefined ? {} : { lossCurve }),
      ...(maxValidationLoss === undefined ? {} : { maxValidationLoss }),
    }
  } catch {
    return null
  }
}

const trainingRunRefForCloseout = (closeout: AssignmentCloseout): string =>
  publicTrainingRef(
    [
      ...closeout.receiptRefs,
      ...closeout.resultRefs,
      ...closeout.summaryRefs,
      ...closeout.buildRefs,
    ].find(ref => ref.startsWith("run.") || ref.includes(".training.")),
  ) ?? stableRef("run.pylon.assignment", closeout.assignmentRef)

const trainingWorkerReceiptFromCloseout = (
  state: PylonLocalState,
  closeout: AssignmentCloseout,
  closeoutRef: string,
): TrainingWorkerReceipt => {
  const receiptRef = stableRef(
    "receipt.pylon.training_worker",
    `${state.identity.pylonRef}:${closeoutRef}`,
  )
  return {
    schema: "openagents.psionic.training_worker_receipt.v0.3",
    receiptRef,
    assignmentRef:
      publicTrainingRef(closeout.assignmentRef) ??
      stableRef("assignment.pylon", closeout.leaseRef),
    workerRef:
      publicTrainingRef(state.identity.pylonRef) ??
      stableRef("pylon.identity", state.identity.nodeId),
    runRef: trainingRunRefForCloseout(closeout),
    artifactRefs: publicTrainingRefs(closeout.artifactRefs),
    checkpointRefs: publicTrainingRefs([
      closeoutRef,
      ...closeout.closeoutRefs,
      ...closeout.buildRefs,
    ]),
    metricRefs: publicTrainingRefs([...closeout.resultRefs, ...closeout.summaryRefs]),
    proofRefs: publicTrainingRefs([...closeout.proofRefs, ...closeout.testRefs]),
    signature: {
      signatureRef: stableRef("signature.pylon.training_worker", receiptRef),
      signerRef: state.identity.pylonRef,
      verificationRef: stableRef(
        "verification.pylon.training_worker",
        `${receiptRef}:${closeout.completedAt}`,
      ),
    },
  }
}

async function writeTrainingWorkerReceiptsBundle(
  state: PylonLocalState,
  closeout: AssignmentCloseout,
  closeoutRef: string,
): Promise<void> {
  const path = trainingWorkerReceiptsPathForHome(state.paths.home)
  const existing = await readTrainingWorkerReceiptsBundle(path)
  const nextReceipt = trainingWorkerReceiptFromCloseout(state, closeout, closeoutRef)
  const receiptsByAssignment = new Map<string, TrainingWorkerReceipt>()
  for (const receipt of existing?.workerReceipts ?? []) {
    receiptsByAssignment.set(`${receipt.assignmentRef}:${receipt.workerRef}`, receipt)
  }
  receiptsByAssignment.set(
    `${nextReceipt.assignmentRef}:${nextReceipt.workerRef}`,
    nextReceipt,
  )

  const bundle: TrainingWorkerReceiptsBundle = {
    schema: "openagents.pylon.training_worker_receipts_bundle.v0.3",
    generatedAt: closeout.completedAt,
    sourceRefs: uniqueRefs([
      ...(existing?.sourceRefs ?? []),
      "source.pylon.assignment_closeout",
      closeoutRef,
      ...closeout.closeoutRefs,
      ...closeout.summaryRefs,
    ]),
    workerReceipts: [...receiptsByAssignment.values()],
    ...(existing?.budgetLabel === undefined ? {} : { budgetLabel: existing.budgetLabel }),
    ...(existing?.budgetRef === undefined ? {} : { budgetRef: existing.budgetRef }),
    ...(existing?.evalRef === undefined ? {} : { evalRef: existing.evalRef }),
    ...(existing?.lossCurve === undefined ? {} : { lossCurve: existing.lossCurve }),
    ...(existing?.maxValidationLoss === undefined ? {} : { maxValidationLoss: existing.maxValidationLoss }),
    ...(existing?.mergeRef === undefined ? {} : { mergeRef: existing.mergeRef }),
  }
  assertPublicProjectionSafe(bundle)
  await writeFile(path, `${JSON.stringify(bundle, null, 2)}\n`)
}

function runtimeGatePayloadFrom(codingAssignment: unknown): RuntimeGatePayload | null {
  const runtimeGate = (codingAssignment as { runtimeGate?: unknown } | null)?.runtimeGate
  if (runtimeGate === null || typeof runtimeGate !== "object") return null
  const payload = runtimeGate as RuntimeGatePayload
  return (
    payload.schema === "openagents.pylon.runtime_gate.v0.3" &&
    payload.agentKind === "codex_cli_or_fixture" &&
    payload.fixtureRef === "fixture.public.pylon.codex_runtime.sum_repair.v1"
  )
    ? payload
    : null
}

async function runCommand(input: {
  args: string[]
  cwd: string
}): Promise<{ exitCode: number; stderrBytes: number; stdoutBytes: number }> {
  const proc = Bun.spawn(input.args, {
    cwd: input.cwd,
    stderr: "pipe",
    stdout: "pipe",
  })
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).arrayBuffer(),
    new Response(proc.stderr).arrayBuffer(),
    proc.exited,
  ])

  return {
    exitCode,
    stderrBytes: stderr.byteLength,
    stdoutBytes: stdout.byteLength,
  }
}

type TassadarAssignmentPayload = {
  expectedTraceDigest?: string
  fixtureId?: string
  model: Parameters<typeof executeTassadarNumericModel>[0]
  steps: ReadonlyArray<ReadonlyArray<number>>
}

export function tassadarPayloadFrom(codingAssignment: unknown): TassadarAssignmentPayload | null {
  const record = codingAssignment as
    | { kind?: unknown; tassadar?: TassadarAssignmentPayload }
    | null
    | undefined
  const kind = record?.kind
  if (record === null || record === undefined) return null
  if (kind !== TASSADAR_EXECUTOR_TRACE_JOB_KIND && kind !== TASSADAR_EXECUTOR_TRACE_HOMEWORK_JOB_KIND) {
    return null
  }
  const tassadar = record.tassadar
  if (tassadar === undefined || typeof tassadar !== "object") return null
  if (tassadar.model === undefined || !Array.isArray(tassadar.steps)) return null
  // The dispatch payload renames the model's seed_writes field to
  // initialChannelWrites so it survives the public-projection scanner;
  // restore the executor's wire format here.
  const transit = tassadar.model as unknown as {
    initialChannelWrites?: ReadonlyArray<readonly [number, number, number]>
    seed_writes?: ReadonlyArray<readonly [number, number, number]>
  }
  if (transit.seed_writes === undefined && transit.initialChannelWrites !== undefined) {
    const { initialChannelWrites, ...rest } = transit
    return {
      ...tassadar,
      model: {
        ...(rest as object),
        seed_writes: initialChannelWrites,
      } as TassadarAssignmentPayload["model"],
    }
  }
  return tassadar
}

/**
 * Executes a Tassadar executor-trace assignment: runs the embedded
 * digest-pinned numeric-model workload through the shared executor and
 * carries the computed trace digest into closeout refs. Exact-replay
 * verification happens on a separate validator device; this gate only
 * reports what this device computed.
 */
export async function executeTassadarAssignment(
  lease: PylonAssignmentLease,
  now: Date,
) {
  const payload = tassadarPayloadFrom(lease.codingAssignment)
  if (payload === null) return null
  const runRef = stableRef(
    "run.pylon.tassadar_executor_trace",
    `${lease.leaseRef}:${payload.fixtureId ?? "workload"}:${now.toISOString()}`,
  )
  try {
    const trace = await executeTassadarNumericModel(payload.model, payload.steps)
    const { outputs, halted } = collectInterpreterOutputs(trace.stepOutputs)
    const digestMatchesExpectation =
      payload.expectedTraceDigest === undefined ||
      payload.expectedTraceDigest === trace.traceDigest
    const artifactRef = `artifact.tassadar_poc.trace_digest.${trace.traceDigest}`
    const proofRef = stableRef(
      "proof.pylon.tassadar_executor_trace",
      `${lease.assignmentRef}:${trace.traceDigest}:${trace.stepCount}`,
    )
    return {
      artifactRefs: [artifactRef],
      blockerRefs: digestMatchesExpectation
        ? []
        : ["blocker.assignment.tassadar_trace_digest_mismatch"],
      buildRefs: [runRef],
      message: digestMatchesExpectation
        ? `Tassadar executor-trace workload executed: ${trace.stepCount} steps, halted=${halted}, ${outputs.length} output(s), trace digest ${trace.traceDigest.slice(0, 16)}… matches the dispatched expectation.`
        : "Tassadar executor-trace workload executed but the trace digest does not match the dispatched expectation.",
      previewRefs: [],
      proofRefs: [proofRef],
      resultRefs: [
        `result.tassadar_poc.trace_digest.${trace.traceDigest}`,
        `result.tassadar_poc.step_count.${trace.stepCount}`,
        `result.tassadar_poc.halted.${halted}`,
      ],
      runRefs: [runRef],
      status: digestMatchesExpectation ? ("accepted" as const) : ("rejected" as const),
      summaryRefs: [
        digestMatchesExpectation
          ? "summary.tassadar_poc.trace_digest_match"
          : "summary.tassadar_poc.trace_digest_mismatch",
      ],
      testRefs: [proofRef],
    }
  } catch (error) {
    const failureRef = stableRef(
      "proof.pylon.tassadar_executor_trace.refused",
      `${lease.leaseRef}:${String(error)}`,
    )
    return {
      artifactRefs: [],
      blockerRefs: ["blocker.assignment.tassadar_execution_refused"],
      buildRefs: [runRef],
      message: "Tassadar executor-trace workload refused with a typed execution error.",
      previewRefs: [],
      proofRefs: [failureRef],
      resultRefs: ["result.tassadar_poc.execution_refused"],
      runRefs: [runRef],
      status: "rejected" as const,
      summaryRefs: ["summary.tassadar_poc.execution_refused"],
      testRefs: [failureRef],
    }
  }
}

async function executeRuntimeGate(
  state: PylonLocalState,
  lease: PylonAssignmentLease,
  now: Date,
) {
  const runtimeGate = runtimeGatePayloadFrom(lease.codingAssignment)

  if (runtimeGate === null) {
    return null
  }

  const workspaceRef = stableRef("workspace.pylon.runtime_gate", lease.leaseRef)
  const workspace = join(state.paths.cache, "runtime-gates", workspaceRef)

  await mkdir(workspace, { recursive: true })
  await writeFile(
    join(workspace, "package.json"),
    `${JSON.stringify({
      private: true,
      scripts: {
        test: "bun test sum.test.ts",
      },
      type: "module",
    }, null, 2)}\n`,
  )
  await writeFile(
    join(workspace, "sum.ts"),
    "export const sum = (left: number, right: number) => left - right\n",
  )
  await writeFile(
    join(workspace, "sum.test.ts"),
    [
      'import { describe, expect, test } from "bun:test"',
      'import { sum } from "./sum"',
      "",
      'describe("sum fixture", () => {',
      '  test("adds two numbers", () => {',
      "    expect(sum(2, 3)).toBe(5)",
      "  })",
      "})",
      "",
    ].join("\n"),
  )
  await writeFile(
    join(workspace, "sum.ts"),
    "export const sum = (left: number, right: number) => left + right\n",
  )

  const command = await runCommand({
    args: ["bun", "test", "sum.test.ts"],
    cwd: workspace,
  })
  const commandRef = stableRef(
    "command.pylon.runtime_gate.bun_test",
    `${lease.leaseRef}:${command.exitCode}:${command.stdoutBytes}:${command.stderrBytes}`,
  )
  const runRef = stableRef(
    "run.pylon.runtime_gate",
    `${lease.leaseRef}:${runtimeGate.fixtureRef}:${now.toISOString()}`,
  )
  const artifactRef = stableRef(
    "artifact.pylon.runtime_gate.fixture_patch",
    `${lease.assignmentRef}:${runtimeGate.fixtureRef}:sum_plus`,
  )
  const proofRef = stableRef(
    "proof.pylon.runtime_gate.test_passed",
    `${artifactRef}:${commandRef}`,
  )

  if (command.exitCode !== 0) {
    return {
      artifactRefs: [artifactRef],
      blockerRefs: ["blocker.assignment.runtime_gate_test_failed"],
      buildRefs: [commandRef],
      message: "Bounded runtime gate fixture repair failed its public-safe test command.",
      previewRefs: [workspaceRef],
      proofRefs: [proofRef],
      resultRefs: ["result.public.pylon_runtime_gate.failed"],
      runRefs: [runRef],
      status: "rejected" as const,
      summaryRefs: ["summary.public.pylon_runtime_gate.fixture_repair_failed"],
      testRefs: [commandRef],
    }
  }

  return {
    artifactRefs: [artifactRef],
    blockerRefs: [],
    buildRefs: [commandRef],
    message: "Bounded runtime gate fixture repair executed and verified by the local Pylon runtime.",
    previewRefs: [workspaceRef],
    proofRefs: [proofRef],
    resultRefs: ["result.public.pylon_runtime_gate.fixture_repair_passed"],
    runRefs: [runRef],
    status: "accepted" as const,
    summaryRefs: ["summary.public.pylon_runtime_gate.fixture_repair_passed"],
    testRefs: [commandRef],
  }
}

const emptyAssignmentStore = (): AssignmentStore => ({
  schema: "openagents.pylon.assignment_state.v0.3",
  leases: {},
})

async function loadAssignmentStore(state: PylonLocalState): Promise<AssignmentStore> {
  await ensureStateDirectories(state.paths)
  if (!existsSync(state.paths.assignmentState)) {
    return emptyAssignmentStore()
  }
  // Fail-soft: a truncated/corrupt local assignment-state file (e.g. a process
  // killed mid-write during a previous run) must not brick every future
  // run-no-spend with an uncaught JSON parse error. Treat an unreadable store as
  // empty; the next write re-materializes a valid store, and server-side lease
  // expiry still reconciles any abandoned leases.
  try {
    const parsed = JSON.parse(
      await readFile(state.paths.assignmentState, "utf8"),
    ) as unknown
    if (
      parsed !== null &&
      typeof parsed === "object" &&
      typeof (parsed as AssignmentStore).leases === "object" &&
      (parsed as AssignmentStore).leases !== null
    ) {
      return parsed as AssignmentStore
    }
    return emptyAssignmentStore()
  } catch {
    return emptyAssignmentStore()
  }
}

async function writeAssignmentStore(state: PylonLocalState, store: AssignmentStore) {
  assertPublicProjectionSafe(store)
  await writeFile(state.paths.assignmentState, `${JSON.stringify(store, null, 2)}\n`)
}

const locallyTerminalAssignmentStatuses = new Set<AssignmentStatus>([
  "closed",
  "rejected",
  "cancelled",
  "timed-out",
  "stale",
])
const LOCAL_ASSIGNMENT_HEARTBEAT_INTERVAL_MS = 5_000
const DEFAULT_LOCAL_ASSIGNMENT_HEARTBEAT_STALE_AFTER_MS = 90_000
const LEGACY_LOCAL_ACTIVE_LEASE_TTL_MS = 5 * 60 * 1000

function defaultLocalProcessIsAlive(processId: number): boolean {
  if (!Number.isInteger(processId) || processId <= 0) return false
  try {
    process.kill(processId, 0)
    return true
  } catch {
    return false
  }
}

function localLeaseRecordIsExpired(
  record: AssignmentStoreLeaseRecord,
  now: Date,
): boolean {
  if (locallyTerminalAssignmentStatuses.has(record.status)) return false
  if (record.leaseExpiresAt !== undefined) {
    const expiresAt = Date.parse(record.leaseExpiresAt)
    return Number.isFinite(expiresAt) && expiresAt <= now.getTime()
  }
  return false
}

function localLeaseRecordIsInterrupted(
  record: AssignmentStoreLeaseRecord,
  now: Date,
  options: Pick<AssignmentClientOptions, "localAssignmentHeartbeatStaleAfterMs" | "localProcessIsAlive"> = {},
): boolean {
  if (locallyTerminalAssignmentStatuses.has(record.status)) return false
  if (localLeaseRecordIsExpired(record, now)) return true
  const localProcessIsAlive = options.localProcessIsAlive ?? defaultLocalProcessIsAlive
  const heartbeatStaleAfterMs =
    typeof options.localAssignmentHeartbeatStaleAfterMs === "number" &&
    Number.isFinite(options.localAssignmentHeartbeatStaleAfterMs) &&
    options.localAssignmentHeartbeatStaleAfterMs > 0
      ? Math.max(1, Math.floor(options.localAssignmentHeartbeatStaleAfterMs))
      : DEFAULT_LOCAL_ASSIGNMENT_HEARTBEAT_STALE_AFTER_MS
  if (record.ownerProcessId !== undefined) {
    if (!localProcessIsAlive(record.ownerProcessId)) return true
    const heartbeatAt = Date.parse(
      record.ownerHeartbeatAt ?? record.acceptedAt ?? record.ownerStartedAt ?? "",
    )
    return Number.isFinite(heartbeatAt) &&
      now.getTime() - heartbeatAt > heartbeatStaleAfterMs
  }
  if (record.acceptedAt !== undefined) {
    const acceptedAt = Date.parse(record.acceptedAt)
    return Number.isFinite(acceptedAt) &&
      now.getTime() - acceptedAt > LEGACY_LOCAL_ACTIVE_LEASE_TTL_MS
  }
  return false
}

function staleExpiredLocalLeases(
  store: AssignmentStore,
  now: Date,
  options: Pick<AssignmentClientOptions, "localAssignmentHeartbeatStaleAfterMs" | "localProcessIsAlive"> = {},
): { changed: boolean; store: AssignmentStore } {
  let changed = false
  const leases: AssignmentStore["leases"] = {}
  for (const [leaseRef, record] of Object.entries(store.leases)) {
    if (!localLeaseRecordIsInterrupted(record, now, options)) {
      leases[leaseRef] = record
      continue
    }
    changed = true
    leases[leaseRef] = {
      ...record,
      status: "stale",
      closedAt: now.toISOString(),
    }
  }
  return changed ? { changed, store: { ...store, leases } } : { changed, store }
}

function expiredActiveLocalLeaseEntries(
  store: AssignmentStore,
  now: Date,
  options: Pick<AssignmentClientOptions, "localAssignmentHeartbeatStaleAfterMs" | "localProcessIsAlive"> = {},
): Array<readonly [string, AssignmentStoreLeaseRecord]> {
  return Object.entries(store.leases).filter(([, record]) =>
    !locallyTerminalAssignmentStatuses.has(record.status) &&
    localLeaseRecordIsInterrupted(record, now, options),
  )
}

function interruptedNoSpendLeaseEntriesNeedingCloseout(
  store: AssignmentStore,
  now: Date,
  options: Pick<AssignmentClientOptions, "localAssignmentHeartbeatStaleAfterMs" | "localProcessIsAlive"> = {},
): Array<readonly [string, AssignmentStoreLeaseRecord]> {
  const entries = new Map<string, AssignmentStoreLeaseRecord>()
  for (const [leaseRef, record] of expiredActiveLocalLeaseEntries(store, now, options)) {
    if (record.paymentMode !== "paid") {
      entries.set(leaseRef, record)
    }
  }
  for (const [leaseRef, record] of Object.entries(store.leases)) {
    if (
      record.status === "stale" &&
      record.paymentMode !== "paid" &&
      record.serverCloseoutSubmittedAt === undefined
    ) {
      entries.set(leaseRef, record)
    }
  }
  return [...entries.entries()]
}

async function loadPrunedAssignmentStore(
  state: PylonLocalState,
  now: Date,
  options: Pick<AssignmentClientOptions, "localAssignmentHeartbeatStaleAfterMs" | "localProcessIsAlive"> = {},
): Promise<AssignmentStore> {
  const loaded = await loadAssignmentStore(state)
  const pruned = staleExpiredLocalLeases(loaded, now, options)
  if (pruned.changed) {
    await writeAssignmentStore(state, pruned.store)
  }
  return pruned.store
}

function localLeaseIsTerminal(store: AssignmentStore, leaseRef: string): boolean {
  const local = store.leases[leaseRef]
  return local !== undefined && locallyTerminalAssignmentStatuses.has(local.status)
}

function codingRunServiceForLease(lease: PylonAssignmentLease): PylonCodingServiceRef | null {
  return agentRunnerServiceForLease(lease)
}

function codingRunAccountRefHashForLease(lease: PylonAssignmentLease): string | null {
  const codingAssignment = lease.codingAssignment as { claudeAgent?: unknown; codex?: unknown } | undefined
  const service = codingRunServiceForLease(lease)
  const payload =
    service === "claude"
      ? codingAssignment?.claudeAgent
      : service === "codex"
        ? codingAssignment?.codex
        : null
  if (payload === null || typeof payload !== "object") {
    return null
  }
  const accountRefHash = (payload as { accountRefHash?: unknown }).accountRefHash
  return typeof accountRefHash === "string" && accountRefHash.trim() !== ""
    ? accountRefHash.trim()
    : null
}

function isLegacyLease(value: unknown): value is PylonAssignmentLease {
  const lease = value as PylonAssignmentLease
  return (
    lease?.schema === "openagents.pylon.assignment_lease.v0.3" &&
    typeof lease.assignmentRef === "string" &&
    typeof lease.leaseRef === "string" &&
    typeof lease.goal === "string" &&
    (lease.paymentMode === "no-spend" || lease.paymentMode === "paid") &&
    Array.isArray(lease.capabilityRefs) &&
    typeof lease.expiresAt === "string"
  )
}

function codingAssignmentPaymentMode(codingAssignment: unknown): AssignmentPaymentMode {
  const budget = (codingAssignment as { budget?: { paymentMode?: unknown } } | null)?.budget
  return budget?.paymentMode === "buyer_funded" ? "paid" : "no-spend"
}

function codingAssignmentGoal(codingAssignment: unknown, fallback: string): string {
  const objective = (codingAssignment as { objective?: { objectiveRef?: unknown } } | null)?.objective
  return typeof objective?.objectiveRef === "string" ? objective.objectiveRef : fallback
}

function codingAssignmentCapabilityRefs(codingAssignment: unknown): string[] {
  return safeStringArray(
    (codingAssignment as { requiredCapabilityRefs?: unknown } | null)?.requiredCapabilityRefs,
  )
}

function normalizeProjectedAssignment(
  assignment: PublicPylonAssignmentProjection,
  now: Date,
): PylonAssignmentLease | null {
  if (typeof assignment.assignmentRef !== "string") {
    return null
  }
  const codingAssignment =
    assignment.codingAssignment !== null &&
    typeof assignment.codingAssignment === "object"
      ? assignment.codingAssignment as AutopilotCodingAssignmentPayload
      : undefined
  const rawDelegation =
    (assignment as { delegation?: unknown }).delegation ??
    (codingAssignment as { delegation?: unknown } | undefined)?.delegation
  const delegation = pylonDelegationChainFrom(rawDelegation)
  const expiresInSeconds =
    typeof assignment.leaseExpiresInSeconds === "number" &&
    Number.isFinite(assignment.leaseExpiresInSeconds)
      ? Math.max(0, assignment.leaseExpiresInSeconds)
      : 15 * 60
  const taskRef = safeStringArray(assignment.taskRefs)[0] ?? assignment.assignmentRef

  return {
    schema: "openagents.pylon.assignment_lease.v0.3",
    assignmentRef: assignment.assignmentRef,
    leaseRef: assignment.assignmentRef,
    goal: codingAssignmentGoal(codingAssignment, taskRef),
    paymentMode: codingAssignmentPaymentMode(codingAssignment),
    capabilityRefs: codingAssignmentCapabilityRefs(codingAssignment),
    ...(codingAssignment === undefined ? {} : { codingAssignment }),
    ...(delegation === null ? {} : { delegation }),
    ...(rawDelegation !== undefined && delegation === null ? { delegationInvalid: true } : {}),
    expiresAt: new Date(now.getTime() + expiresInSeconds * 1000).toISOString(),
  }
}

function normalizeLeaseDelegation(lease: PylonAssignmentLease): PylonAssignmentLease {
  const rawDelegation = (lease as { delegation?: unknown }).delegation
  if (rawDelegation === undefined) return lease
  const delegation = pylonDelegationChainFrom(rawDelegation)
  if (delegation === null) {
    const { delegation: _dropped, ...rest } = lease
    return { ...rest, delegationInvalid: true }
  }
  return { ...lease, delegation }
}

function normalizePollResponse(value: unknown, now: Date): PylonAssignmentLease[] {
  const response = value as AssignmentPollResponse
  const leases = response.leases ?? (response.assignment ? [response.assignment] : [])
  const assignments = (value as { assignments?: unknown }).assignments

  if (Array.isArray(assignments)) {
    return assignments.flatMap((assignment) => {
      if (isLegacyLease(assignment)) {
        return [normalizeLeaseDelegation(assignment)]
      }

      const normalized = normalizeProjectedAssignment(
        assignment as PublicPylonAssignmentProjection,
        now,
      )

      return normalized === null ? [] : [normalized]
    })
  }

  return leases.filter(isLegacyLease).map(normalizeLeaseDelegation)
}

function hasRequiredCapabilities(state: PylonLocalState, lease: PylonAssignmentLease) {
  const local = new Set(state.runtime.capabilityRefs)
  return lease.capabilityRefs.every((ref) => local.has(ref))
}

function isExpired(lease: PylonAssignmentLease, now: Date) {
  return new Date(lease.expiresAt).getTime() <= now.getTime()
}

export async function computeAssignmentAdmission(
  state: PylonLocalState,
  lease: PylonAssignmentLease,
  options: Pick<AssignmentClientOptions, "now" | "staleAfterMs" | "psionicQwenAdmission" | "gepaEnvelope"> = {},
) {
  const now = options.now?.() ?? new Date()
  const presence = await loadOrCreatePresenceState(state.paths, state.identity)
  const blockerRefs = new Set<string>()

  if (state.runtime.lifecycle === "paused") blockerRefs.add("blocker.assignment.lifecycle_paused")
  if (state.runtime.lifecycle === "offline") blockerRefs.add("blocker.assignment.lifecycle_offline")
  if (state.runtime.lifecycle === "degraded") blockerRefs.add("blocker.assignment.lifecycle_degraded")
  if (!presence.lastHeartbeatAt) blockerRefs.add("blocker.assignment.presence_never_heartbeat")
  if (presence.stale) blockerRefs.add("blocker.assignment.presence_stale")
  if (presence.lastHeartbeatAt) {
    const age = now.getTime() - new Date(presence.lastHeartbeatAt).getTime()
    if (age > (options.staleAfterMs ?? 120_000)) blockerRefs.add("blocker.assignment.presence_stale")
  }
  if (!hasRequiredCapabilities(state, lease)) blockerRefs.add("blocker.assignment.wrong_capability")
  const runnerResolution = agentRunnerResolutionForLease(lease)
  if (runnerResolution.status === "ambiguous") blockerRefs.add(runnerResolution.blockerRef)
  if (lease.backendRef && !state.runtime.capabilityRefs.includes(lease.backendRef)) {
    blockerRefs.add("blocker.assignment.unsupported_backend")
  }
  if (lease.delegationInvalid) {
    blockerRefs.add("blocker.delegation.invalid_chain")
  }
  if (lease.delegation) {
    const delegationAdmission = admitPylonDelegation({
      chain: lease.delegation,
      localCapabilityRefs: state.runtime.capabilityRefs,
      localPylonRef: state.identity.pylonRef,
      now,
      objectiveText: lease.goal,
      requestedCapabilityRefs: lease.capabilityRefs,
    })
    for (const blockerRef of delegationAdmission.blockerRefs) blockerRefs.add(blockerRef)
  }
  if (lease.gepaRequirements) {
    const envelope = options.gepaEnvelope ?? createDefaultGepaCapabilityEnvelope()
    const gepaAdmission = admitGepaAssignmentToEnvelope(envelope, lease.gepaRequirements)
    for (const blockerRef of gepaAdmission.blockerRefs) blockerRefs.add(blockerRef)
  }
  if (lease.psionicQwenRequirements) {
    const psionicAdmission = options.psionicQwenAdmission ?? psionicAdmissionFromCapabilityRefs(state.runtime.capabilityRefs)
    const selection = selectPsionicQwenModel(psionicAdmission, lease.psionicQwenRequirements.mode)
    if (!selection.admitted) {
      for (const blockerRef of selection.blockerRefs) blockerRefs.add(blockerRef)
    }
    if (
      lease.psionicQwenRequirements.requiredModelRef &&
      selection.selectedModelRef !== lease.psionicQwenRequirements.requiredModelRef
    ) {
      blockerRefs.add(
        lease.psionicQwenRequirements.requiredModelRef === PSIONIC_QWEN_MODEL_REFS.qwen35_2b
          ? "blocker.psionic_qwen35.model_2b_missing"
          : "blocker.psionic_qwen35.required_model_missing",
      )
    }
  }
  if (isExpired(lease, now)) blockerRefs.add("blocker.assignment.lease_expired")
  return { admissible: blockerRefs.size === 0, blockerRefs: [...blockerRefs] }
}

async function postJson(
  options: AssignmentClientOptions,
  path: string,
  body: JsonRecord,
  state: PylonLocalState,
  idempotencyRef?: string,
): Promise<JsonRecord> {
  assertPublicProjectionSafe(body)
  const fetchImpl = options.fetch ?? fetch
  const url = new URL(path, options.baseUrl).toString()
  const text = JSON.stringify(body)
  const idempotencyKey = `pylon.assignment.${state.identity.pylonRef}.${idempotencyRef ?? stableRef("request", `${path}:${text}`)}`
  const headers = options.agentToken
    ? {
        authorization: `Bearer ${options.agentToken}`,
        "content-type": "application/json",
        "Idempotency-Key": idempotencyKey,
      }
    : {
        ...(await createSignedHeaders({
          method: "POST",
          url,
          body: text,
          pylonRef: state.identity.pylonRef,
          paths: state.paths,
          now: options.now?.(),
        })),
        "Idempotency-Key": idempotencyKey,
      }
  const response = await fetchImpl(url, {
    method: "POST",
    headers,
    body: text,
    signal: assignmentRequestSignal(options),
  })
  const responseText = await response.text()
  const json = responseText.trim() ? (JSON.parse(responseText) as JsonRecord) : {}
  assertPublicProjectionSafe(json)
  if (!response.ok) {
    throw new Error(`OpenAgents assignment request failed (${response.status}): ${responseText}`)
  }
  return json
}

async function getJson(options: AssignmentClientOptions, path: string, state: PylonLocalState) {
  const fetchImpl = options.fetch ?? fetch
  const url = new URL(path, options.baseUrl).toString()
  const headers = options.agentToken
    ? {
        authorization: `Bearer ${options.agentToken}`,
      }
    : await createSignedHeaders({
        method: "GET",
        url,
        body: "",
        pylonRef: state.identity.pylonRef,
        paths: state.paths,
        now: options.now?.(),
      })
  const response = await fetchImpl(url, {
    method: "GET",
    headers,
    signal: assignmentRequestSignal(options),
  })
  const responseText = await response.text()
  const json = responseText.trim() ? (JSON.parse(responseText) as JsonRecord) : {}
  // Per-assignment safety isolation: one projection-unsafe assignment must
  // not poison the entire poll. Unsafe entries are dropped with a typed
  // marker; everything else still passes the full assertion.
  const assignments = (json as { assignments?: unknown }).assignments
  if (Array.isArray(assignments)) {
    const safeAssignments: unknown[] = []
    const droppedRefs: string[] = []
    for (const assignment of assignments) {
      try {
        assertPublicProjectionSafe(assignment, "projection.assignment")
        safeAssignments.push(assignment)
      } catch {
        const ref = (assignment as { assignmentRef?: unknown })?.assignmentRef
        droppedRefs.push(typeof ref === "string" ? ref : "assignment.unknown")
      }
    }
    const filtered: JsonRecord = {
      ...json,
      assignments: safeAssignments,
      ...(droppedRefs.length > 0
        ? { droppedUnsafeAssignmentRefs: droppedRefs }
        : {}),
    }
    assertPublicProjectionSafe(filtered)
    if (!response.ok) {
      throw new Error(`OpenAgents assignment request failed (${response.status}): ${responseText}`)
    }
    return filtered
  }
  assertPublicProjectionSafe(json)
  if (!response.ok) {
    throw new Error(`OpenAgents assignment request failed (${response.status}): ${responseText}`)
  }
  return json
}

function assignmentRequestSignal(options: AssignmentClientOptions): AbortSignal {
  const requested = options.requestTimeoutMs
  const timeoutMs =
    typeof requested === "number" &&
    Number.isFinite(requested) &&
    requested > 0
      ? Math.min(Math.floor(requested), 300_000)
      : DEFAULT_ASSIGNMENT_REQUEST_TIMEOUT_MS
  return AbortSignal.timeout(timeoutMs)
}

export async function pollAssignments(summary: BootstrapSummary, options: AssignmentClientOptions) {
  const state = await ensurePylonLocalState(summary)
  const response = await getJson(
    options,
    `/api/pylons/${encodeURIComponent(state.identity.pylonRef)}/assignments`,
    state,
  )
  return normalizePollResponse(response, options.now?.() ?? new Date())
}

export async function acceptAssignment(
  summary: BootstrapSummary,
  lease: PylonAssignmentLease,
  options: AssignmentClientOptions,
): Promise<AssignmentAcceptance> {
  const state = await ensurePylonLocalState(summary)
  const acceptedAt = options.now?.() ?? new Date()
  const store = await loadPrunedAssignmentStore(state, acceptedAt, options)
  const existing = store.leases[lease.leaseRef]
  if (
    existing?.status === "accepted" ||
    existing?.status === "running" ||
    localLeaseIsTerminal(store, lease.leaseRef)
  ) {
    return {
      ok: false,
      accepted: false,
      assignmentRef: lease.assignmentRef,
      leaseRef: lease.leaseRef,
      statusRef: stableRef("assignment.denial.duplicate", lease.leaseRef),
      denialRef: "denial.assignment.duplicate_lease",
      blockerRefs: ["blocker.assignment.duplicate_lease"],
    }
  }

  const admission = await computeAssignmentAdmission(state, lease, options)
  if (!admission.admissible) {
    return {
      ok: false,
      accepted: false,
      assignmentRef: lease.assignmentRef,
      leaseRef: lease.leaseRef,
      statusRef: stableRef("assignment.denial", `${lease.leaseRef}:${admission.blockerRefs.join(",")}`),
      denialRef: "denial.assignment.admission_blocked",
      blockerRefs: admission.blockerRefs,
    }
  }

  const body = {
    acceptanceRefs: [
      stableRef("assignment.acceptance", `${lease.assignmentRef}:${lease.goal}`),
    ],
    accepted: true,
    status: "accepted",
  }
  const claimAttemptRef = stableRef(
    "claim.pylon.assignment_acceptance",
    `${lease.leaseRef}:${state.identity.nodeId}:${randomUUID()}`,
  )
  let response: JsonRecord
  try {
    response = await postJson(
      options,
      `/api/pylons/${encodeURIComponent(state.identity.pylonRef)}/assignments/${encodeURIComponent(lease.leaseRef)}/accept`,
      body,
      state,
      claimAttemptRef,
    )
  } catch (error) {
    return {
      ok: false,
      accepted: false,
      assignmentRef: lease.assignmentRef,
      leaseRef: lease.leaseRef,
      statusRef: stableRef("assignment.denial.server", `${lease.leaseRef}:${String(error)}`),
      denialRef: "denial.assignment.server_rejected",
      blockerRefs: ["blocker.assignment.server_rejected"],
    }
  }
  const statusRef = String(response.statusRef ?? stableRef("assignment.accepted", lease.leaseRef))
  store.leases[lease.leaseRef] = {
    assignmentRef: lease.assignmentRef,
    status: "accepted",
    acceptedAt: acceptedAt.toISOString(),
    leaseExpiresAt: lease.expiresAt,
    ownerHeartbeatAt: acceptedAt.toISOString(),
    ownerHeartbeatSequence: 0,
    ownerProcessId: process.pid,
    ownerStartedAt: acceptedAt.toISOString(),
    paymentMode: lease.paymentMode,
  }
  await writeAssignmentStore(state, store)
  return { ok: true, accepted: true, assignmentRef: lease.assignmentRef, leaseRef: lease.leaseRef, statusRef, blockerRefs: [] }
}

export async function submitAssignmentProgress(
  summary: BootstrapSummary,
  progress: AssignmentProgress,
  options: AssignmentClientOptions,
) {
  const state = await ensurePylonLocalState(summary)
  const response = await postJson(
    options,
    `/api/pylons/${encodeURIComponent(state.identity.pylonRef)}/assignments/${encodeURIComponent(progress.leaseRef)}/progress`,
    progress,
    state,
  )
  return { progressRef: String(response.progressRef ?? stableRef("assignment.progress", `${progress.leaseRef}:${progress.sequence}`)) }
}

export async function submitAssignmentArtifacts(
  summary: BootstrapSummary,
  input: Readonly<{
    artifactRefs: ReadonlyArray<string>
    assignmentRef: string
    leaseRef: string
    proofRefs: ReadonlyArray<string>
  }>,
  options: AssignmentClientOptions,
) {
  const state = await ensurePylonLocalState(summary)
  const response = await postJson(
    options,
    `/api/pylons/${encodeURIComponent(state.identity.pylonRef)}/assignments/${encodeURIComponent(input.leaseRef)}/artifacts`,
    {
      artifactRefs: [...input.artifactRefs],
      proofRefs: [...input.proofRefs],
      status: "submitted",
      storageRefs: [
        stableRef("assignment.storage", `${input.assignmentRef}:${input.leaseRef}`),
      ],
    },
    state,
  )
  return { artifactRef: String(response.artifactRef ?? stableRef("assignment.artifacts", input.leaseRef)) }
}

export async function submitAssignmentCloseout(
  summary: BootstrapSummary,
  closeout: AssignmentCloseout,
  options: AssignmentClientOptions,
) {
  const state = await ensurePylonLocalState(summary)
  const response = await postJson(
    options,
    `/api/pylons/${encodeURIComponent(state.identity.pylonRef)}/assignments/${encodeURIComponent(closeout.leaseRef)}/closeout`,
    closeout,
    state,
  )
  const closeoutRef = String(response.closeoutRef ?? stableRef("assignment.closeout", closeout.leaseRef))
  const store = await loadAssignmentStore(state)
  store.leases[closeout.leaseRef] = {
    assignmentRef: closeout.assignmentRef,
    status: closeout.status === "accepted" ? "closed" : closeout.status,
    acceptedAt: store.leases[closeout.leaseRef]?.acceptedAt,
    leaseExpiresAt: store.leases[closeout.leaseRef]?.leaseExpiresAt,
    paymentMode: closeout.paymentMode,
    closedAt: closeout.completedAt,
    serverCloseoutRef: closeoutRef,
    serverCloseoutSubmittedAt: closeout.completedAt,
  }
  await writeAssignmentStore(state, store)
  await writeTrainingWorkerReceiptsBundle(state, closeout, closeoutRef)
  return { closeoutRef }
}

async function recordLocalLeaseHeartbeat(
  state: PylonLocalState,
  leaseRef: string,
  now: Date,
  status: "accepted" | "running" = "running",
): Promise<void> {
  const store = await loadAssignmentStore(state)
  const existing = store.leases[leaseRef]
  if (existing === undefined || locallyTerminalAssignmentStatuses.has(existing.status)) {
    return
  }
  store.leases[leaseRef] = {
    ...existing,
    status,
    ownerHeartbeatAt: now.toISOString(),
    ownerHeartbeatSequence: (existing.ownerHeartbeatSequence ?? 0) + 1,
    ownerProcessId: process.pid,
    ownerStartedAt: existing.ownerStartedAt ?? existing.acceptedAt ?? now.toISOString(),
  }
  await writeAssignmentStore(state, store)
}

function startLocalLeaseHeartbeat(
  state: PylonLocalState,
  leaseRef: string,
  options: AssignmentClientOptions,
): { stop: () => void } {
  let stopped = false
  const touch = async () => {
    if (stopped) return
    await recordLocalLeaseHeartbeat(
      state,
      leaseRef,
      options.now?.() ?? new Date(),
      "running",
    )
  }
  void touch().catch(() => {})
  const interval = setInterval(() => {
    void touch().catch(() => {})
  }, LOCAL_ASSIGNMENT_HEARTBEAT_INTERVAL_MS)
  ;(interval as { unref?: () => void }).unref?.()
  return {
    stop: () => {
      stopped = true
      clearInterval(interval)
    },
  }
}

function deterministicIndexFromRef(value: string, size: number): number {
  if (size <= 0) return 0
  return createHash("sha256").update(value).digest().readUInt32BE(0) % size
}

function runtimeFailureCloseoutRecord(
  lease: PylonAssignmentLease,
  error: unknown,
): AgentRunnerCloseoutRecord {
  const message = error instanceof Error ? error.message : String(error)
  const timedOut = /\b(timed?\s*out|timeout)\b/i.test(message)
  const status = timedOut ? "timed-out" : "rejected"
  const reason = timedOut ? "timed_out" : "failed"
  return {
    artifactRefs: [stableRef("assignment.artifact.runtime_failure", `${lease.leaseRef}:${reason}`)],
    blockerRefs: [`blocker.assignment.runtime_${reason}`],
    buildRefs: [],
    message: timedOut
      ? "Assignment runtime timed out before producing a usable closeout."
      : "Assignment runtime failed before producing a usable closeout.",
    previewRefs: [],
    proofRefs: [stableRef("assignment.proof.runtime_failure", `${lease.leaseRef}:${reason}`)],
    resultRefs: [stableRef("assignment.result.runtime_failure", `${lease.assignmentRef}:${reason}`)],
    runRefs: [],
    status,
    summaryRefs: [stableRef("assignment.summary.runtime_failure", `${lease.assignmentRef}:${reason}`)],
    testRefs: [],
  }
}

function interruptedLocalLeaseCloseout(
  leaseRef: string,
  record: AssignmentStoreLeaseRecord,
  now: Date,
): AssignmentCloseout {
  const closeoutRef = stableRef(
    "assignment.closeout.local_interrupted",
    `${leaseRef}:${record.acceptedAt ?? "unknown"}:${now.toISOString()}`,
  )
  const proofRef = stableRef(
    "assignment.proof.local_interrupted",
    `${leaseRef}:${record.status}:${record.acceptedAt ?? "unknown"}`,
  )
  return {
    schema: "openagents.pylon.assignment_closeout.v0.3",
    assignmentRef: record.assignmentRef,
    leaseRef,
    status: "stale",
    paymentMode: "no-spend",
    settlementState: "not_applicable",
    payoutClaimAllowed: false,
    artifactRefs: [],
    blockerRefs: ["blocker.assignment.local_run_interrupted"],
    buildRefs: [],
    closeoutRefs: [closeoutRef],
    previewRefs: [],
    proofRefs: [proofRef],
    receiptRefs: ["receipt.pylon.assignment.local_interrupted_no_spend"],
    resultRefs: ["result.public.pylon.assignment.local_interrupted"],
    summaryRefs: ["summary.public.pylon.assignment.local_interrupted"],
    testRefs: [proofRef],
    redacted: true,
    completedAt: now.toISOString(),
  }
}

async function closeoutInterruptedNoSpendLeases(
  summary: BootstrapSummary,
  state: PylonLocalState,
  options: AssignmentClientOptions,
  now: Date,
): Promise<void> {
  const store = await loadAssignmentStore(state)
  const interrupted = interruptedNoSpendLeaseEntriesNeedingCloseout(store, now, options)
  for (const [leaseRef, record] of interrupted) {
    try {
      await submitAssignmentCloseout(
        summary,
        interruptedLocalLeaseCloseout(leaseRef, record, now),
        options,
      )
    } catch {
      // If the server already expired or closed the lease, local pruning below
      // still prevents the interrupted run from poisoning future dispatch.
    }
  }
}

// #6421: resolve the linked account for an assignment, provider-aware by the
// lease's coding service. A claude_agent_task lease resolves a `claude_agent`
// account (its isolated home + per-account OAuth token); a codex lease resolves
// a `codex` account, exactly as before. This is what lets the claude-supervisor
// pass `--account-ref <claude account>` without hitting "Pylon account ref is
// not registered for this provider" (the selector previously hardcoded codex).
async function resolveAgentAccountForAssignment(
  summary: BootstrapSummary,
  lease: PylonAssignmentLease,
  options: AssignmentClientOptions,
  now: Date,
): Promise<AssignmentCodexAccountSelection> {
  const provider: PylonAccountProvider =
    agentRunnerForLease(lease)?.accountProvider ??
    (codingRunServiceForLease(lease) === "claude" ? "claude_agent" : "codex")

  if (options.accountRef !== undefined || options.accountHome !== undefined) {
    const account = await resolvePylonAccountSelection(summary, {
      provider,
      ...(options.accountRef === undefined ? {} : { accountRef: options.accountRef }),
      ...(options.accountHome === undefined ? {} : { accountHome: options.accountHome }),
    })
    return account === null ? { account: null } : { account, accountRefHash: account.accountRefHash }
  }

  // Auto-select (no explicit ref/home): deterministically round-robin across the
  // quota-available, login-ready accounts of the lease's provider. Mirrors the
  // Codex auto-select; the Claude probe reads each account's isolated home env.
  const probeOptions =
    provider === "claude_agent" ? options.claudeAgentProbe : options.codexAgentProbe
  const env = probeOptions?.env ?? (Bun.env as Record<string, string | undefined>)
  const { env: _ignoredProbeEnv, ...probeOverrides } = probeOptions ?? {}
  const codexConfig =
    provider === "codex" ? await loadCodexAgentConfig(summary) : undefined
  const claudeConfig =
    provider === "claude_agent" ? await loadClaudeAgentConfig(summary) : undefined
  const targets = await resolvePylonAccountUsageRefreshTargets(
    summary,
    { accountRef: null, all: true, provider: null },
    { env },
  )
  const pinnedAccountRefHash = codingRunAccountRefHashForLease(lease)
  const readyTargets: PylonAccountUsageRefreshTarget[] = []
  for (const target of targets) {
    if (target.provider !== provider) continue
    if (pinnedAccountRefHash !== null && target.accountRefHash !== pinnedAccountRefHash) continue
    const quotaRecord = await loadQuotaRecord(summary, target.accountRefHash)
    if (!isAccountAvailable(quotaRecord, now)) continue
    if (provider === "codex") {
      const health = await loadCodexAccountHealthRecord(summary, target.accountRefHash)
      if (codexAccountHealthBlocksReadiness(health)) continue
    }
    const targetEnv = pylonAccountEnvironment(env, target.account)
    const readiness =
      provider === "claude_agent"
        ? await probeClaudeAgentReadiness({
            ...(probeOverrides as Partial<ClaudeAgentProbeOptions>),
            ...(claudeConfig === undefined ? {} : { config: claudeConfig }),
            env: targetEnv,
          })
        : await probeCodexAgentReadiness({
            ...(probeOverrides as Partial<CodexAgentProbeOptions>),
            ...(codexConfig === undefined ? {} : { config: codexConfig }),
            env: targetEnv,
          })
    if (readiness.state !== "ready") continue
    if (provider === "codex") {
      const authHealth = await probeAndRecordCodexAccountAuthHealth(summary, {
        account: target.account,
        env,
        now,
        ...(options.codexAuthValidityProbe === undefined ? {} : { probe: options.codexAuthValidityProbe }),
      })
      if (authHealth.state !== "valid") continue
    }
    readyTargets.push(target)
  }
  if (readyTargets.length === 0) return { account: null }

  const selected = readyTargets[
    deterministicIndexFromRef(`${lease.assignmentRef}:${lease.leaseRef}`, readyTargets.length)
  ]
  return {
    account: selected.account,
    accountRefHash: selected.accountRefHash,
  }
}

export async function runNoSpendAssignment(summary: BootstrapSummary, options: AssignmentClientOptions) {
  const runtimeHeartbeatIntervalMs =
    typeof options.runtimeHeartbeatIntervalMs === "number" &&
    Number.isFinite(options.runtimeHeartbeatIntervalMs) &&
    options.runtimeHeartbeatIntervalMs > 0
      ? Math.max(1, Math.floor(options.runtimeHeartbeatIntervalMs))
      : 30_000
  const runtimeProgressIntervalMs =
    typeof options.runtimeProgressIntervalMs === "number" &&
    Number.isFinite(options.runtimeProgressIntervalMs) &&
    options.runtimeProgressIntervalMs > 0
      ? Math.max(1, Math.floor(options.runtimeProgressIntervalMs))
      : 10_000
  let lastProgressEvent: AssignmentRunLifecycleEvent["event"] | undefined
  const emitLifecycleEvent = async (
    event: Omit<AssignmentRunLifecycleEvent, "schema" | "observedAt">,
  ) => {
    if (options.onLifecycleEvent === undefined) return
    try {
      const lifecycleEvent: AssignmentRunLifecycleEvent = {
        schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
        observedAt: (options.now?.() ?? new Date()).toISOString(),
        ...event,
      }
      assertPublicProjectionSafe(lifecycleEvent)
      await options.onLifecycleEvent(lifecycleEvent)
      if (event.event !== "assignment_run.runtime_progress") {
        lastProgressEvent = event.event
      }
    } catch {
      // Lifecycle output is operator observability; assignment execution must
      // remain fail-soft if stderr or an injected reporter is unavailable.
    }
  }
  const withRuntimeProgress = async <T>(
    input: {
      assignmentRef: string
      accountRefHash?: string
      leaseRef: string
      startedAtMs: number
      run: () => Promise<T>
    },
  ): Promise<T> => {
    if (options.onLifecycleEvent === undefined) return input.run()
    let stopped = false
    const tick = async () => {
      if (stopped) return
      await emitLifecycleEvent({
        event: "assignment_run.runtime_progress",
        assignmentRef: input.assignmentRef,
        ...(input.accountRefHash === undefined ? {} : { accountRefHash: input.accountRefHash }),
        leaseRef: input.leaseRef,
        phase: "runtime_active",
        elapsedMs: Math.max(0, Date.now() - input.startedAtMs),
        ...(lastProgressEvent === undefined ? {} : { lastProgressEvent }),
      })
    }
    const interval = setInterval(() => {
      void tick()
    }, runtimeProgressIntervalMs)
    try {
      await tick()
      return await input.run()
    } finally {
      stopped = true
      clearInterval(interval)
    }
  }
  const heartbeatRefresh = async () => {
    await sendHeartbeat(summary, {
      ...(options.agentToken === undefined ? {} : { agentToken: options.agentToken }),
      baseUrl: options.baseUrl,
      ...(options.fetch === undefined ? {} : { fetch: options.fetch }),
      ...(options.now === undefined ? {} : { now: options.now }),
    })
  }
  const state = await ensurePylonLocalState(summary)
  const observedAtDate = options.now?.() ?? new Date()
  await closeoutInterruptedNoSpendLeases(summary, state, options, observedAtDate)
  const store = await loadPrunedAssignmentStore(state, observedAtDate, options)
  let presenceRefreshError: unknown
  try {
    await heartbeatRefresh()
  } catch (error) {
    presenceRefreshError = error
  }
  const leases = await pollAssignments(summary, options)
  const candidates = leases.filter((candidate) =>
    candidate.paymentMode === "no-spend" &&
    (options.assignmentRef === undefined ||
      candidate.assignmentRef === options.assignmentRef ||
      candidate.leaseRef === options.assignmentRef) &&
    !isExpired(candidate, observedAtDate) &&
    !localLeaseIsTerminal(store, candidate.leaseRef)
  )
  await emitLifecycleEvent({
    event: "assignment_run.poll_complete",
    leaseCount: leases.length,
    candidateCount: candidates.length,
  })
  let claimed:
    | { acceptance: AssignmentAcceptance; lease: PylonAssignmentLease }
    | undefined
  let lastAcceptance: AssignmentAcceptance | undefined
  for (const candidate of candidates) {
    const result = await acceptAssignment(summary, candidate, options)
    if (result.accepted) {
      claimed = { acceptance: result, lease: candidate }
      break
    }
    lastAcceptance = result
  }
  if (claimed === undefined) {
    const diagnostic =
      lastAcceptance !== undefined &&
      presenceRefreshError !== undefined &&
      lastAcceptance.blockerRefs.some(ref =>
        ref === "blocker.assignment.presence_stale" ||
        ref === "blocker.assignment.presence_never_heartbeat"
      )
        ? {
            schema: "openagents.pylon.assignment_recovery_diagnostic.v0.1" as const,
            blockerRefs: lastAcceptance.blockerRefs.filter(ref =>
              ref === "blocker.assignment.presence_stale" ||
              ref === "blocker.assignment.presence_never_heartbeat"
            ),
            diagnosticRef: "diagnostic.assignment.presence_heartbeat_required",
            ...(presenceRefreshError instanceof PresenceRequestError
              ? { heartbeatStatus: presenceRefreshError.status }
              : {}),
            recoveryCommand: `pylon presence heartbeat --base-url ${options.baseUrl}`,
          } satisfies AssignmentRecoveryDiagnostic
        : undefined
    if (diagnostic !== undefined) assertPublicProjectionSafe(diagnostic)
    await emitLifecycleEvent({
      event: "assignment_run.no_assignment",
      leaseCount: leases.length,
      candidateCount: candidates.length,
      ...(lastAcceptance === undefined
        ? {}
        : {
            assignmentRef: lastAcceptance.assignmentRef,
            leaseRef: lastAcceptance.leaseRef,
            statusRef: lastAcceptance.statusRef,
            blockerRefs: lastAcceptance.blockerRefs,
          }),
    })
    return lastAcceptance === undefined
      ? { ok: false, reason: "no no-spend assignment lease available", leases }
      : { ok: false, acceptance: lastAcceptance, ...(diagnostic === undefined ? {} : { diagnostic }), leases }
  }
  const { acceptance, lease } = claimed
  await emitLifecycleEvent({
    event: "assignment_run.accepted",
    assignmentRef: lease.assignmentRef,
    leaseRef: lease.leaseRef,
    statusRef: acceptance.statusRef,
  })
  const localLeaseHeartbeat = startLocalLeaseHeartbeat(state, lease.leaseRef, options)
  try {

  const observedAt = observedAtDate.toISOString()
  const agentAccount = await resolveAgentAccountForAssignment(
    summary,
    lease,
    options,
    observedAtDate,
  )
  await emitLifecycleEvent({
    event: "assignment_run.runtime_started",
    assignmentRef: lease.assignmentRef,
    ...(agentAccount.accountRefHash === undefined ? {} : { accountRefHash: agentAccount.accountRefHash }),
    leaseRef: lease.leaseRef,
  })

  const runtimeStartedAtMs = Date.now()
  const codingRunService = codingRunServiceForLease(lease)
  const activeRun = codingRunService === null
    ? null
    : await registerActiveCodingRun(state.paths, {
        assignmentRef: lease.assignmentRef,
        ...(agentAccount.accountRefHash === undefined ? {} : { accountRefHash: agentAccount.accountRefHash }),
        leaseRef: lease.leaseRef,
        now: observedAtDate,
        service: codingRunService,
      })
  if (activeRun !== null) {
    try {
      await heartbeatRefresh()
    } catch {
      // Active-run heartbeats are load projection. Assignment execution remains
      // local and bounded even if the projection endpoint is briefly down.
    }
  }
  let stopActiveRunRefresh = false
  const activeRunRefreshInterval = activeRun === null
    ? undefined
    : setInterval(() => {
        if (stopActiveRunRefresh) return
        void refreshActiveCodingRun(state.paths, activeRun.runRef)
          .then(() => heartbeatRefresh())
          .catch(() => {})
      }, runtimeHeartbeatIntervalMs)
  ;(activeRunRefreshInterval as { unref?: () => void } | undefined)?.unref?.()
  let runtimeGate:
    | Awaited<ReturnType<typeof executeTassadarAssignment>>
    | AgentRunnerCloseoutRecord
    | Awaited<ReturnType<typeof executeRuntimeGate>>
  try {
    runtimeGate = await withRuntimeProgress({
      assignmentRef: lease.assignmentRef,
      ...(agentAccount.accountRefHash === undefined ? {} : { accountRefHash: agentAccount.accountRefHash }),
      leaseRef: lease.leaseRef,
      startedAtMs: runtimeStartedAtMs,
      run: async () =>
        (await executeTassadarAssignment(lease, observedAtDate)) ??
        (await executeRegisteredAgentRunner(state, lease, observedAtDate, {
          // agentToken + baseUrl + fetch let agent executors post exact
          // own-capacity turn token usage. The provider-aware account is
          // resolved from the selected registry entry so Claude credentials are
          // never crossed with Codex credentials.
          ...(options.agentToken === undefined ? {} : { agentToken: options.agentToken }),
          ...(agentAccount.account === null ? {} : { account: agentAccount.account }),
          baseUrl: options.baseUrl,
          ...(options.claudeAgentCheckoutRunner === undefined ? {} : { claudeAgentCheckoutRunner: options.claudeAgentCheckoutRunner }),
          ...(options.claudeAgentProbe === undefined ? {} : { claudeAgentProbe: options.claudeAgentProbe }),
          ...(options.claudeAgentRunner === undefined ? {} : { claudeAgentRunner: options.claudeAgentRunner }),
          ...(options.codexAgentRunner === undefined ? {} : { codexAgentRunner: options.codexAgentRunner }),
          ...(options.codexAuthValidityProbe === undefined
            ? {}
            : { codexAuthValidityProbe: options.codexAuthValidityProbe }),
          ...(options.codexAgentProbe === undefined ? {} : { codexAgentProbe: options.codexAgentProbe }),
          ...(options.fetch === undefined ? {} : { fetch: options.fetch }),
          onCodexProgress: async (progress) => {
            const elapsedMs = Math.max(0, Date.now() - runtimeStartedAtMs)
            await emitLifecycleEvent({
              event: "assignment_run.runtime_progress",
              assignmentRef: lease.assignmentRef,
              ...(agentAccount.accountRefHash === undefined ? {} : { accountRefHash: agentAccount.accountRefHash }),
              leaseRef: lease.leaseRef,
              phase: progress.phase,
              elapsedMs,
              ...(progress.tokensSoFar === undefined ? {} : { tokensSoFar: progress.tokensSoFar }),
              ...(progress.lastProgressEvent === undefined ? {} : { lastProgressEvent: progress.lastProgressEvent }),
            })
            await submitAssignmentProgress(
              summary,
              {
                schema: "openagents.pylon.assignment_progress.v0.3",
                assignmentRef: lease.assignmentRef,
                leaseRef: lease.leaseRef,
                sequence: Math.max(1, Math.floor(elapsedMs / 1000)),
                status: "running",
                message: `Runtime phase: ${progress.phase}.`,
                artifactRefs: [],
                proofRefs: [],
                observedAt: (options.now?.() ?? new Date()).toISOString(),
                elapsedMs,
                phase: progress.phase,
                ...(progress.tokensSoFar === undefined ? {} : { tokensSoFar: progress.tokensSoFar }),
                ...(progress.lastProgressEvent === undefined ? {} : { lastProgressEvent: progress.lastProgressEvent }),
              },
              options,
            ).catch(() => {})
          },
        })) ??
        (await executeRuntimeGate(state, lease, observedAtDate)),
    })
  } catch (error) {
    runtimeGate = runtimeFailureCloseoutRecord(lease, error)
    await emitLifecycleEvent({
      event: "assignment_run.runtime_failed",
      assignmentRef: lease.assignmentRef,
      ...(agentAccount.accountRefHash === undefined ? {} : { accountRefHash: agentAccount.accountRefHash }),
      leaseRef: lease.leaseRef,
      status: runtimeGate.status,
      blockerRefs: runtimeGate.blockerRefs,
    })
  } finally {
    stopActiveRunRefresh = true
    if (activeRunRefreshInterval !== undefined) clearInterval(activeRunRefreshInterval)
    if (activeRun !== null) {
      await finishActiveCodingRun(state.paths, activeRun.runRef)
      try {
        await heartbeatRefresh()
      } catch {
        // Best-effort final load projection; closeout remains source of truth.
      }
    }
  }
  const artifactRefs = runtimeGate?.artifactRefs ?? [stableRef("assignment.artifact", `${lease.assignmentRef}:${lease.goal}`)]
  const proofRefs = runtimeGate?.proofRefs ?? [stableRef("assignment.proof", `${lease.leaseRef}:${artifactRefs[0]}`)]
  const progress: AssignmentProgress = {
    schema: "openagents.pylon.assignment_progress.v0.3",
    assignmentRef: lease.assignmentRef,
    leaseRef: lease.leaseRef,
    sequence: 1,
    status: "proof-ready",
    message: runtimeGate?.message ?? "No-spend assignment executed in bounded local Pylon runtime.",
    artifactRefs,
    proofRefs,
    observedAt,
  }
  let progressReceipt: { progressRef: string }
  let artifactReceipt: { artifactRef: string } | null = null
  try {
    progressReceipt = await submitAssignmentProgress(summary, progress, options)
    await emitLifecycleEvent({
      event: "assignment_run.progress_submitted",
      assignmentRef: lease.assignmentRef,
      leaseRef: lease.leaseRef,
      status: progress.status,
      progressRef: progressReceipt.progressRef,
    })
    artifactReceipt = await submitAssignmentArtifacts(
      summary,
      {
        artifactRefs,
        assignmentRef: lease.assignmentRef,
        leaseRef: lease.leaseRef,
        proofRefs,
      },
      options,
    )
    await emitLifecycleEvent({
      event: "assignment_run.artifacts_submitted",
      assignmentRef: lease.assignmentRef,
      leaseRef: lease.leaseRef,
      artifactRef: artifactReceipt.artifactRef,
    })
  } catch (error) {
    const message = String(error)
    const status = message.includes("(410)") ? "cancelled" : message.includes("(408)") ? "timed-out" : "rejected"
    const failureProofRef = stableRef("assignment.proof.failure", `${lease.leaseRef}:${status}:${message}`)
    const closeout: AssignmentCloseout = {
      schema: "openagents.pylon.assignment_closeout.v0.3",
      assignmentRef: lease.assignmentRef,
      leaseRef: lease.leaseRef,
      status,
      paymentMode: "no-spend",
      settlementState: "not_applicable",
      payoutClaimAllowed: false,
      artifactRefs: [],
      blockerRefs: ["blocker.assignment.progress_or_artifact_rejected"],
      buildRefs: [],
      closeoutRefs: [stableRef("assignment.closeout.failure", `${lease.leaseRef}:${status}`)],
      previewRefs: [],
      proofRefs: [failureProofRef],
      receiptRefs: [acceptance.statusRef, ...psionicCloseoutReceiptRefs(lease, options)],
      resultRefs: [],
      summaryRefs: [stableRef("assignment.summary.failure", `${lease.leaseRef}:${status}`)],
      testRefs: [],
      redacted: true,
      completedAt: observedAt,
    }
    const closeoutReceipt = await submitAssignmentCloseout(summary, closeout, options)
    await emitLifecycleEvent({
      event: "assignment_run.closeout_submitted",
      assignmentRef: lease.assignmentRef,
      leaseRef: lease.leaseRef,
      status: closeout.status,
      closeoutRef: closeoutReceipt.closeoutRef,
      blockerRefs: closeout.blockerRefs,
    })
    await emitLifecycleEvent({
      event: "assignment_run.completed",
      assignmentRef: lease.assignmentRef,
      leaseRef: lease.leaseRef,
      status: closeout.status,
      closeoutRef: closeoutReceipt.closeoutRef,
      blockerRefs: closeout.blockerRefs,
    })
    localLeaseHeartbeat.stop()
    return { ok: false, lease, acceptance, closeout, closeoutReceipt }
  }
  const closeout: AssignmentCloseout = {
    schema: "openagents.pylon.assignment_closeout.v0.3",
    assignmentRef: lease.assignmentRef,
    leaseRef: lease.leaseRef,
    status: runtimeGate?.status ?? "accepted",
    paymentMode: "no-spend",
    settlementState: "not_applicable",
    payoutClaimAllowed: false,
    artifactRefs,
    blockerRefs: runtimeGate?.blockerRefs ?? [],
    buildRefs: runtimeGate?.buildRefs ?? [stableRef("assignment.build.not_required", lease.leaseRef)],
    closeoutRefs: [stableRef("assignment.closeout.summary", lease.leaseRef)],
    previewRefs: runtimeGate?.previewRefs ?? [stableRef("assignment.preview.not_required", lease.leaseRef)],
    proofRefs,
    receiptRefs: [
      acceptance.statusRef,
      progressReceipt.progressRef,
      ...(artifactReceipt === null ? [] : [artifactReceipt.artifactRef]),
      ...(runtimeGate?.runRefs ?? []),
      ...psionicCloseoutReceiptRefs(lease, options),
    ],
    resultRefs: runtimeGate?.resultRefs ?? [stableRef("assignment.result.public_safe", lease.assignmentRef)],
    summaryRefs: runtimeGate?.summaryRefs ?? [stableRef("assignment.summary.public_safe", lease.assignmentRef)],
    testRefs: runtimeGate?.testRefs ?? [stableRef("assignment.test.not_required", lease.leaseRef)],
    redacted: true,
    completedAt: observedAt,
  }
  assertPublicProjectionSafe(closeout)
  const closeoutReceipt = await submitAssignmentCloseout(summary, closeout, options)
  await emitLifecycleEvent({
    event: "assignment_run.closeout_submitted",
    assignmentRef: lease.assignmentRef,
    leaseRef: lease.leaseRef,
    status: closeout.status,
    closeoutRef: closeoutReceipt.closeoutRef,
    blockerRefs: closeout.blockerRefs,
  })
  await emitLifecycleEvent({
    event: "assignment_run.completed",
    assignmentRef: lease.assignmentRef,
    leaseRef: lease.leaseRef,
    status: closeout.status,
    closeoutRef: closeoutReceipt.closeoutRef,
    blockerRefs: closeout.blockerRefs,
  })
  localLeaseHeartbeat.stop()
  return {
    ok: closeout.status === "accepted",
    lease,
    acceptance,
    progress,
    closeout,
    progressReceipt,
    closeoutReceipt,
  }
  } finally {
    localLeaseHeartbeat.stop()
  }
}

function psionicAdmissionFromCapabilityRefs(capabilityRefs: string[]): PsionicQwenModelAdmission {
  const admittedModelRefs = capabilityRefs.filter((ref) =>
    ref === PSIONIC_QWEN_MODEL_REFS.qwen35_0_8b || ref === PSIONIC_QWEN_MODEL_REFS.qwen35_2b
  ) as PsionicQwenModelAdmission["admittedModelRefs"]

  return {
    rows: [],
    admittedModelRefs,
    observedModelRefs: admittedModelRefs,
    blockerRefs: admittedModelRefs.length === 0 ? ["blocker.psionic_qwen35.qwen35_model_missing"] : [],
  }
}

function psionicCloseoutReceiptRefs(lease: PylonAssignmentLease, options: AssignmentClientOptions): string[] {
  if (!lease.psionicQwenRequirements) return []
  const admission = options.psionicQwenAdmission ?? {
    rows: [],
    admittedModelRefs: [],
    observedModelRefs: [],
    blockerRefs: [],
  }
  const selection = selectPsionicQwenModel(admission, lease.psionicQwenRequirements.mode)
  const refs = new Set<string>([
    "backend.psionic.qwen35",
    ...(lease.psionicQwenRequirements.receiptRefs ?? []),
  ])
  if (selection.selectedModelRef) refs.add(selection.selectedModelRef)
  return [...refs]
}
