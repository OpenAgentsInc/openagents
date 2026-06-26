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
import type { ClaudeAgentProbeOptions } from "./claude-agent.js"
import {
  executeClaudeAgentAssignment,
  type ClaudeAgentCheckoutRunner,
  type ClaudeAgentRunner,
} from "./claude-agent-executor.js"
import type { CodexAgentProbeOptions } from "./codex-agent.js"
import { executeCodexAgentAssignment, type CodexAgentRunner } from "./codex-agent-executor.js"
import { createSignedHeaders } from "./presence.js"
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
}

type AssignmentStore = {
  schema: "openagents.pylon.assignment_state.v0.3"
  leases: Record<string, { assignmentRef: string; status: AssignmentStatus; acceptedAt?: string; closedAt?: string }>
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

async function loadAssignmentStore(state: PylonLocalState): Promise<AssignmentStore> {
  await ensureStateDirectories(state.paths)
  if (!existsSync(state.paths.assignmentState)) {
    return { schema: "openagents.pylon.assignment_state.v0.3", leases: {} }
  }
  return JSON.parse(await readFile(state.paths.assignmentState, "utf8")) as AssignmentStore
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

function localLeaseIsTerminal(store: AssignmentStore, leaseRef: string): boolean {
  const local = store.leases[leaseRef]
  return local !== undefined && locallyTerminalAssignmentStatuses.has(local.status)
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
    expiresAt: new Date(now.getTime() + expiresInSeconds * 1000).toISOString(),
  }
}

function normalizePollResponse(value: unknown, now: Date): PylonAssignmentLease[] {
  const response = value as AssignmentPollResponse
  const leases = response.leases ?? (response.assignment ? [response.assignment] : [])
  const assignments = (value as { assignments?: unknown }).assignments

  if (Array.isArray(assignments)) {
    return assignments.flatMap((assignment) => {
      if (isLegacyLease(assignment)) {
        return [assignment]
      }

      const normalized = normalizeProjectedAssignment(
        assignment as PublicPylonAssignmentProjection,
        now,
      )

      return normalized === null ? [] : [normalized]
    })
  }

  return leases.filter(isLegacyLease)
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
  if (lease.backendRef && !state.runtime.capabilityRefs.includes(lease.backendRef)) {
    blockerRefs.add("blocker.assignment.unsupported_backend")
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
  const response = await fetchImpl(url, { method: "POST", headers, body: text })
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
  const response = await fetchImpl(url, { method: "GET", headers })
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
  const store = await loadAssignmentStore(state)
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
    acceptedAt: (options.now?.() ?? new Date()).toISOString(),
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
  const store = await loadAssignmentStore(state)
  store.leases[closeout.leaseRef] = {
    assignmentRef: closeout.assignmentRef,
    status: closeout.status === "accepted" ? "closed" : closeout.status,
    acceptedAt: store.leases[closeout.leaseRef]?.acceptedAt,
    closedAt: closeout.completedAt,
  }
  await writeAssignmentStore(state, store)
  const closeoutRef = String(response.closeoutRef ?? stableRef("assignment.closeout", closeout.leaseRef))
  await writeTrainingWorkerReceiptsBundle(state, closeout, closeoutRef)
  return { closeoutRef }
}

export async function runNoSpendAssignment(summary: BootstrapSummary, options: AssignmentClientOptions) {
  const state = await ensurePylonLocalState(summary)
  const store = await loadAssignmentStore(state)
  const leases = await pollAssignments(summary, options)
  const candidates = leases.filter((candidate) =>
    candidate.paymentMode === "no-spend" &&
    (options.assignmentRef === undefined ||
      candidate.assignmentRef === options.assignmentRef ||
      candidate.leaseRef === options.assignmentRef) &&
    !localLeaseIsTerminal(store, candidate.leaseRef)
  )
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
    return lastAcceptance === undefined
      ? { ok: false, reason: "no no-spend assignment lease available", leases }
      : { ok: false, acceptance: lastAcceptance, leases }
  }
  const { acceptance, lease } = claimed

  const observedAtDate = options.now?.() ?? new Date()
  const observedAt = observedAtDate.toISOString()
  const runtimeGate =
    (await executeTassadarAssignment(lease, observedAtDate)) ??
    (await executeClaudeAgentAssignment(state, lease, observedAtDate, {
      ...(options.claudeAgentCheckoutRunner === undefined ? {} : { checkoutRunner: options.claudeAgentCheckoutRunner }),
      ...(options.claudeAgentRunner === undefined ? {} : { claudeAgentRunner: options.claudeAgentRunner }),
      ...(options.claudeAgentProbe === undefined ? {} : { claudeAgentProbe: options.claudeAgentProbe }),
    })) ??
    (await executeCodexAgentAssignment(state, lease, observedAtDate, {
      ...(options.codexAgentRunner === undefined ? {} : { codexAgentRunner: options.codexAgentRunner }),
      ...(options.codexAgentProbe === undefined ? {} : { codexAgentProbe: options.codexAgentProbe }),
    })) ??
    (await executeRuntimeGate(state, lease, observedAtDate))
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
  return {
    ok: closeout.status === "accepted",
    lease,
    acceptance,
    progress,
    closeout,
    progressReceipt,
    closeoutReceipt,
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
