import { createHash } from "node:crypto"
import { homedir } from "node:os"
import { join } from "node:path"

import type { PylonAssignmentRunLifecycleEvent } from "@openagentsinc/agent-runtime-schema"
import { Effect } from "effect"

import {
  hashPylonAccountRef,
  loadPylonAccountRegistryEffect,
  normalizeAccountHome,
  PylonAccountRegistryError,
  type PylonAccountProvider,
  type PylonAccountRegistryEntry,
} from "../account-registry.js"
import {
  runNoSpendAssignment,
  type AssignmentClientOptions,
  type AssignmentCloseout,
} from "../assignment.js"
import type { BootstrapSummary } from "../bootstrap.js"
import {
  buildPylonKhalaGitCheckoutWorkspace,
  issuePylonKhalaRequest,
  readPylonKhalaCloseout,
  readPylonKhalaAssignmentTraceStatus,
  type PylonKhalaCloseoutResult,
  type PylonKhalaAssignmentTraceStatusResult,
  type PylonKhalaRequestInput,
  type PylonKhalaRequestResult,
  type PylonKhalaWorkflow,
} from "../khala-requester.js"
import { assertPublicProjectionSafe } from "../state.js"
import type { TipsNetworkOptions } from "../tips.js"
import { assertPublicSafe } from "../work-requester.js"
import type {
  FleetRunSupervisorActiveAssignment,
  FleetRunSupervisorDispatchInput,
  FleetRunSupervisorDispatchResult,
  FleetRunSupervisorReconcileResult,
  FleetRunSupervisorRunner,
} from "./fleet-run-supervisor.js"
import type { FleetRunOwnerLocalLiveness } from "./fleet-run-recovery.js"
import {
  exactPylonFleetRunUsageEvidence,
  type PylonFleetRunUsageEvidenceCarrier,
} from "./fleet-run-usage-evidence.js"
import type {
  PylonFleetRunAttemptControl,
  PylonFleetRunExactAttempt,
} from "./fleet-run-steering-follow-up-dispatcher.js"

export const PYLON_OWNED_FLEET_RUNNER_BLOCKERS = {
  accountMismatch: "blocker.pylon.fleet_runner.account_mismatch",
  accountUnavailable: "blocker.pylon.fleet_runner.account_unavailable",
  assignmentMismatch: "blocker.pylon.fleet_runner.assignment_mismatch",
  assignmentMissing: "blocker.pylon.fleet_runner.assignment_missing",
  dispatchConflict: "blocker.pylon.fleet_runner.dispatch_conflict",
  dispatchInvalid: "blocker.pylon.fleet_runner.dispatch_invalid",
  delegationMismatch: "blocker.pylon.fleet_runner.delegation_mismatch",
  grokCustodyUnavailable: "blocker.pylon.fleet_runner.grok_custody_unavailable",
  requestFailed: "blocker.pylon.fleet_runner.request_failed",
  runFailed: "blocker.pylon.fleet_runner.run_failed",
  usageEvidenceInvalid: "blocker.pylon.fleet_runner.usage_evidence_invalid",
  workspaceInvalid: "blocker.pylon.fleet_runner.workspace_invalid",
} as const

export const PYLON_OWNED_FLEET_RUNNER_MAX_TERMINAL_RETENTION = 1_024

export type PylonOwnedFleetRunRequestReceipt = Pick<
  PylonKhalaRequestResult,
  "assignmentRef" | "frames" | "workflow"
>

export type PylonOwnedFleetRunAssignmentReceipt = {
  readonly accountRefHash: string | null
  readonly assignmentRef: string | null
  readonly closeout: Pick<
    AssignmentCloseout,
    "paymentMode" | "payoutClaimAllowed" | "settlementState" | "status"
  > | null
  readonly lifecycle: readonly PylonAssignmentRunLifecycleEvent[]
  readonly ok: boolean
}

export type PylonOwnedFleetRunRequestPort = (
  input: PylonKhalaRequestInput,
) => Promise<PylonOwnedFleetRunRequestReceipt>

export type PylonOwnedFleetRunAssignmentPort = (input: {
  readonly accountRef: string
  readonly assignmentRef: string
  readonly onLifecycle?: ((event: PylonAssignmentRunLifecycleEvent) => void | Promise<void>) | undefined
}) => Promise<PylonOwnedFleetRunAssignmentReceipt>

export type PylonOwnedFleetRunInspectionPort = (
  assignmentRef: string,
) => Promise<PylonKhalaAssignmentTraceStatusResult>

export type PylonOwnedFleetRunCloseoutPort = (
  assignmentRef: string,
) => Promise<PylonKhalaCloseoutResult>

export type PylonOwnedFleetRunDispatchResult = FleetRunSupervisorDispatchResult &
  PylonFleetRunUsageEvidenceCarrier

export type PylonOwnedFleetRunReconcileResult = FleetRunSupervisorReconcileResult &
  PylonFleetRunUsageEvidenceCarrier

/** Grok stays a separate local claimed-work port because it has no Khala assignment wire kind. */
export type PylonOwnedGrokClaimedWorkPort = {
  readonly dispatch: (
    input: FleetRunSupervisorDispatchInput,
  ) => Promise<PylonOwnedFleetRunDispatchResult>
  readonly reconcile: (input: {
    readonly active: FleetRunSupervisorActiveAssignment
    readonly now: Date
    readonly runRef: string
  }) => Promise<PylonOwnedFleetRunReconcileResult>
  /** Terminal local receipts remain available for supervisor reconciliation after restart. */
  readonly probeLiveness: (assignmentRef: string) => Promise<FleetRunOwnerLocalLiveness>
}

type AssignmentOptionOverrides = Omit<
  AssignmentClientOptions,
  | "accountHome"
  | "accountRef"
  | "agentToken"
  | "assignmentRef"
  | "baseUrl"
  | "fetch"
  | "now"
  | "onLifecycleEvent"
>

export type CreatePylonOwnedFleetRunSupervisorRunnerInput = {
  readonly summary: BootstrapSummary
  readonly pylonRef: string
  readonly baseUrl: string
  readonly agentToken?: string | undefined
  readonly fetch?: typeof fetch | undefined
  readonly now?: (() => Date) | undefined
  readonly assignmentOptions?: AssignmentOptionOverrides | undefined
  readonly defaultHomes?: {
    readonly claudeAgent: string
    readonly codex: string
  } | undefined
  readonly loadRegistry?: (() => Promise<readonly PylonAccountRegistryEntry[]>) | undefined
  readonly request?: PylonOwnedFleetRunRequestPort | undefined
  readonly runAssignment?: PylonOwnedFleetRunAssignmentPort | undefined
  readonly inspectAssignment?: PylonOwnedFleetRunInspectionPort | undefined
  readonly readCloseout?: PylonOwnedFleetRunCloseoutPort | undefined
  readonly grok?: PylonOwnedGrokClaimedWorkPort | undefined
}

export type PylonOwnedFleetRunSupervisorRunner = Omit<
  FleetRunSupervisorRunner,
  "dispatch" | "reconcile"
> & {
  readonly dispatch: (
    input: FleetRunSupervisorDispatchInput,
  ) => Promise<PylonOwnedFleetRunDispatchResult>
  readonly reconcile: (input: {
    readonly activeAssignments: readonly FleetRunSupervisorActiveAssignment[]
    readonly now: Date
    readonly run: FleetRunSupervisorDispatchInput["run"]
  }) => Promise<readonly PylonOwnedFleetRunReconcileResult[]>
  /** Process-local single-flight/terminal replay cache; durable task state remains the restart authority. */
  readonly retainedDispatchCount: () => number
  /** Exact, production-owned attempt observation/control seam for Sarah follow-ups. */
  readonly steeringControl: PylonFleetRunAttemptControl
}

const assignmentRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:-]{2,180}$/u
const accountRefPattern = /^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$/u
const orchestrationRefPattern = /^[A-Za-z0-9][A-Za-z0-9._:/#-]{0,180}$/u
const pylonRefPattern = /^[a-z0-9][a-z0-9_.:-]{2,119}$/u

const providerForWorker = (
  workerKind: FleetRunSupervisorDispatchInput["workerKind"],
): Exclude<PylonAccountProvider, "grok"> | null =>
  workerKind === "codex" ? "codex" : workerKind === "claude" ? "claude_agent" : null

const workflowForWorker = (
  workerKind: FleetRunSupervisorDispatchInput["workerKind"],
): PylonKhalaWorkflow | null =>
  workerKind === "codex" ? "codex_agent_task" : workerKind === "claude" ? "claude_agent_task" : null

const fixedFailure = (
  status: "blocked" | "failed",
  summary: string,
  assignmentRef: string | null = null,
  lifecycle: readonly PylonAssignmentRunLifecycleEvent[] = [],
): PylonOwnedFleetRunDispatchResult => {
  const result = {
    accountRefHash: null,
    assignmentRef,
    closeoutRef: null,
    lifecycle: [...lifecycle],
    status,
    summary,
    usageEvidence: null,
  } satisfies PylonOwnedFleetRunDispatchResult
  assertPublicProjectionSafe(result, "pylonOwnedFleetRunFailure")
  return result
}

const failureLifecycle = (input: {
  readonly assignmentRef: string | null
  readonly blockerRef: string
  readonly now: Date
}): PylonAssignmentRunLifecycleEvent => ({
  schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
  event: input.assignmentRef === null ? "assignment_run.no_assignment" : "assignment_run.runtime_failed",
  observedAt: input.now.toISOString(),
  ...(input.assignmentRef === null ? {} : { assignmentRef: input.assignmentRef }),
  status: "rejected",
  blockerRefs: [input.blockerRef],
})

const terminalLifecycle = (input: {
  readonly assignmentRef: string
  readonly blockerRef?: string | undefined
  readonly now: Date
  readonly status: "closed" | "rejected"
}): PylonAssignmentRunLifecycleEvent => ({
  schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
  event: "assignment_run.completed",
  observedAt: input.now.toISOString(),
  assignmentRef: input.assignmentRef,
  status: input.status,
  ...(input.blockerRef === undefined ? {} : { blockerRefs: [input.blockerRef] }),
})

const stablePublicRef = (prefix: string, seed: string): string =>
  `${prefix}.${createHash("sha256").update(seed).digest("hex").slice(0, 24)}`

const workerEvidenceRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/=-]{2,259}$/u
const attemptProjectionRefPattern = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,179}$/u

const projectedWorkerEvidenceGroups = <const Groups extends readonly {
  readonly refs: readonly string[]
  readonly prefix: string
  readonly maximum: number
}[]>(groups: Groups): { readonly [Index in keyof Groups]: string[] } => {
  for (const group of groups) {
    if (
      group.refs.length > group.maximum ||
      new Set(group.refs).size !== group.refs.length
    ) {
      throw new Error("worker evidence ref cardinality is invalid")
    }
  }
  const projectedBySource = new Map<string, string>()
  const sourceByProjected = new Map<string, string>()
  const projected = groups.map(group => group.refs.map(ref => {
    const existing = projectedBySource.get(ref)
    if (existing !== undefined) return existing
    const next = attemptProjectionRefPattern.test(ref)
      ? ref
      : stablePublicRef(group.prefix, ref)
    const existingSource = sourceByProjected.get(next)
    if (existingSource !== undefined && existingSource !== ref) {
      throw new Error("worker evidence projection collided")
    }
    projectedBySource.set(ref, next)
    sourceByProjected.set(next, ref)
    return next
  }))
  return projected as { readonly [Index in keyof Groups]: string[] }
}

type ExactWorkerCloseoutEvidence = {
  readonly artifactRefs: readonly string[]
  readonly authorityReceiptRefs: readonly string[]
  readonly closeoutRefs: readonly string[]
  readonly eventRef: string
  readonly proofRefs: readonly string[]
  readonly resultRefs: readonly string[]
  readonly testRefs: readonly string[]
  readonly verificationRefs: readonly string[]
}

// Worker-closeout intake retains up to 100 refs per source role. The narrower
// FleetAttempt roles are enforced only when those specific arrays are lowered
// into the v2 terminal contract below; source-only closeout/result roles are
// not subjected to an invented global 64-ref cap.
const stringRefs = (value: unknown): readonly string[] | null =>
  Array.isArray(value) &&
    value.length <= 100 &&
    value.every(ref => typeof ref === "string" && workerEvidenceRefPattern.test(ref)) &&
    new Set(value).size === value.length
    ? value as readonly string[]
    : null

const exactWorkerCloseout = (
  closeout: PylonKhalaCloseoutResult,
): ExactWorkerCloseoutEvidence => {
  const status = (closeout.status as unknown as { workerCloseout?: unknown }).workerCloseout
  const proof = (closeout.proof as unknown as { workerCloseout?: unknown }).workerCloseout
  const decode = (value: unknown): ExactWorkerCloseoutEvidence & {
    readonly projectionBlockerRefs: readonly string[]
    readonly source: string
    readonly status: string | null
  } => {
    if (value === null || typeof value !== "object" || Array.isArray(value)) {
      throw new Error("worker closeout evidence is unavailable")
    }
    const record = value as Record<string, unknown>
    const artifactRefs = stringRefs(record.artifactRefs)
    const authorityReceiptRefs = stringRefs(record.authorityReceiptRefs)
    const closeoutRefs = stringRefs(record.closeoutRefs)
    const proofRefs = stringRefs(record.proofRefs)
    const resultRefs = stringRefs(record.resultRefs)
    const testRefs = stringRefs(record.testRefs)
    const verificationRefs = stringRefs(record.verificationRefs)
    const projectionBlockerRefs = stringRefs(record.projectionBlockerRefs)
    if (
      record.source !== "worker_closeout_event" ||
      typeof record.eventRef !== "string" ||
      !workerEvidenceRefPattern.test(record.eventRef) ||
      record.status !== "closeout_submitted" ||
      artifactRefs === null ||
      authorityReceiptRefs === null ||
      closeoutRefs === null ||
      proofRefs === null ||
      resultRefs === null ||
      testRefs === null ||
      verificationRefs === null ||
      projectionBlockerRefs === null ||
      artifactRefs.length === 0 ||
      authorityReceiptRefs.length === 0 ||
      closeoutRefs.length === 0 ||
      proofRefs.length === 0 ||
      resultRefs.length === 0 ||
      testRefs.length === 0 ||
      verificationRefs.length === 0 ||
      projectionBlockerRefs.length !== 0
    ) throw new Error("worker closeout evidence is incomplete")
    return {
      artifactRefs,
      authorityReceiptRefs,
      closeoutRefs,
      eventRef: record.eventRef,
      proofRefs,
      resultRefs,
      testRefs,
      verificationRefs,
      projectionBlockerRefs,
      source: record.source,
      status: record.status,
    }
  }
  const statusEvidence = decode(status)
  const proofEvidence = decode(proof)
  if (JSON.stringify(statusEvidence) !== JSON.stringify(proofEvidence)) {
    throw new Error("worker closeout status and proof evidence diverged")
  }
  return statusEvidence
}

const projectedTerminalEvidenceRefs = (
  worker: ExactWorkerCloseoutEvidence,
): {
  readonly artifactRefs: readonly string[]
  readonly authorityReceiptRefs: readonly string[]
  readonly proofRefs: readonly string[]
  readonly verificationRefs: readonly string[]
} => {
  // FleetAttempt verification evidence is a set of refs rather than two
  // separate test/verification role arrays. Collapse only this explicit union;
  // the worker closeout itself retains both exact role arrays.
  const verificationSourceRefs = [...new Set([
    ...worker.testRefs,
    ...worker.verificationRefs,
  ])]
  if (verificationSourceRefs.length > 64) {
    throw new Error("worker verification evidence cardinality is invalid")
  }
  const [verificationRefs, artifactRefs, proofRefs, authorityReceiptRefs] =
    projectedWorkerEvidenceGroups([
      {
        refs: verificationSourceRefs,
        prefix: "verification.public.pylon.opaque",
        maximum: 64,
      },
      {
        refs: worker.artifactRefs,
        prefix: "artifact.public.pylon.opaque",
        maximum: 64,
      },
      {
        refs: worker.proofRefs,
        prefix: "proof.public.pylon.opaque",
        maximum: 64,
      },
      {
        refs: worker.authorityReceiptRefs,
        prefix: "receipt.public.pylon.authority.opaque",
        maximum: 64,
      },
    ] as const)
  return { artifactRefs, authorityReceiptRefs, proofRefs, verificationRefs }
}

const terminalEvidenceFromCloseout = (
  dispatch: FleetRunSupervisorDispatchInput,
  closeout: PylonKhalaCloseoutResult,
): {
  readonly verification: {
    readonly truth: "passed"
    readonly verifierRef: string
    readonly evidenceRefs: readonly string[]
  }
  readonly artifactRefs: readonly string[]
  readonly proofRefs: readonly string[]
  readonly authorityReceiptRefs: readonly string[]
} => {
  const worker = exactWorkerCloseout(closeout)
  const projected = projectedTerminalEvidenceRefs(worker)
  const verifierSeed =
    `worker_closeout:${worker.eventRef}:${projected.verificationRefs.join(":")}`
  return {
    verification: {
      truth: "passed",
      verifierRef: stablePublicRef("verifier.public.pylon.fleet_run", verifierSeed),
      evidenceRefs: projected.verificationRefs,
    },
    artifactRefs: projected.artifactRefs,
    proofRefs: projected.proofRefs,
    authorityReceiptRefs: projected.authorityReceiptRefs,
  }
}

const reconciledTerminalEvidenceFromCloseout = (
  active: FleetRunSupervisorActiveAssignment,
  closeout: PylonKhalaCloseoutResult,
): ReturnType<typeof terminalEvidenceFromCloseout> => {
  const worker = exactWorkerCloseout(closeout)
  const projected = projectedTerminalEvidenceRefs(worker)
  return {
    verification: {
      truth: "passed",
      verifierRef: stablePublicRef(
        "verifier.public.pylon.fleet_run",
        `worker_closeout:${worker.eventRef}:${projected.verificationRefs.join(":")}`,
      ),
      evidenceRefs: projected.verificationRefs,
    },
    artifactRefs: projected.artifactRefs,
    proofRefs: projected.proofRefs,
    authorityReceiptRefs: projected.authorityReceiptRefs,
  }
}

const delegationProjection = (
  receipt: PylonOwnedFleetRunRequestReceipt,
): { assignmentRef: string; pylonRef: string; workflowClass: string } | null => {
  for (const frame of receipt.frames) {
    if (frame.parsed === null || typeof frame.parsed !== "object" || Array.isArray(frame.parsed)) continue
    const openagents = (frame.parsed as { openagents?: unknown }).openagents
    if (openagents === null || typeof openagents !== "object" || Array.isArray(openagents)) continue
    const delegation = (openagents as { coding_delegation?: unknown }).coding_delegation
    if (delegation === null || typeof delegation !== "object" || Array.isArray(delegation)) continue
    const candidate = delegation as Record<string, unknown>
    if (
      typeof candidate.assignmentRef === "string" &&
      typeof candidate.pylonRef === "string" &&
      typeof candidate.workflowClass === "string"
    ) {
      return {
        assignmentRef: candidate.assignmentRef,
        pylonRef: candidate.pylonRef,
        workflowClass: candidate.workflowClass,
      }
    }
  }
  return null
}

const cleanObjective = (input: FleetRunSupervisorDispatchInput): string => {
  const value = (input.workUnit.body ?? input.run.objective).trim()
  if (value.length < 3 || value.length > 8_000) {
    throw new Error("fleet work objective must be 3-8000 characters")
  }
  assertPublicSafe(value, "Pylon fleet work objective")
  return value
}

const cleanObjectiveSummary = (input: FleetRunSupervisorDispatchInput): string => {
  const title = input.workUnit.title.trim()
  const value = title.length >= 3
    ? `Implement ${input.workUnit.workUnitRef}: ${title}`
    : `Implement public work unit ${input.workUnit.workUnitRef}.`
  const bounded = value.slice(0, 1_000)
  assertPublicSafe(bounded, "Pylon fleet work objective summary")
  return bounded
}

const requestInputFor = (
  input: FleetRunSupervisorDispatchInput,
  target: {
    readonly accountRefHash: string
    readonly pylonRef: string
    readonly workflow: PylonKhalaWorkflow
  },
): PylonKhalaRequestInput => {
  const prompt = cleanObjective(input)
  const shared = {
    objectiveSummary: cleanObjectiveSummary(input),
    prompt,
    targetAccountRefHash: target.accountRefHash,
    targetPylonRef: target.pylonRef,
    workflow: target.workflow,
  } satisfies PylonKhalaRequestInput
  if (input.workUnit.kind === "fixture") return shared
  if (
    input.workUnit.repo === undefined ||
    input.workUnit.branch === undefined ||
    input.workUnit.baseCommit === undefined ||
    input.workUnit.verify === undefined
  ) {
    throw new Error("real fleet work requires a pinned repository, branch, commit, and verifier")
  }
  return {
    ...shared,
    workspace: buildPylonKhalaGitCheckoutWorkspace({
      branch: input.workUnit.branch,
      commit: input.workUnit.baseCommit,
      repository: input.workUnit.repo,
      verificationCommand: input.workUnit.verify,
    }),
  }
}

const strictLoadRegistry = async (
  summary: Pick<BootstrapSummary, "paths">,
): Promise<readonly PylonAccountRegistryEntry[]> => {
  try {
    return await Effect.runPromise(loadPylonAccountRegistryEffect(summary))
  } catch (error) {
    if (error instanceof PylonAccountRegistryError && error.kind === "not_found") return []
    throw error
  }
}

const exactRegistryAccount = (
  registry: readonly PylonAccountRegistryEntry[],
  input: {
    readonly accountRef: string
    readonly defaultHomes: { readonly claudeAgent: string; readonly codex: string }
    readonly provider: Exclude<PylonAccountProvider, "grok">
  },
): PylonAccountRegistryEntry | null => {
  // Bare account refs are supervisor authority keys. A duplicate anywhere in
  // the registry is ambiguous even when the providers differ, matching the
  // capacity adapter's fail-closed rule.
  const bareRefMatches = registry.filter(account => account.ref === input.accountRef)
  if (bareRefMatches.length !== 1) return null
  const account = bareRefMatches[0]
  if (account.provider !== input.provider || account.paused === true) return null
  const defaultHome = input.provider === "codex"
    ? input.defaultHomes.codex
    : input.defaultHomes.claudeAgent
  if (/^(?:\(default\)|default)$/iu.test(account.ref.trim())) return null
  if (normalizeAccountHome(account.home) === normalizeAccountHome(defaultHome)) return null
  return account
}

const closeoutIsNoSpend = (
  closeout: PylonOwnedFleetRunAssignmentReceipt["closeout"],
): boolean =>
  closeout !== null &&
  closeout.paymentMode === "no-spend" &&
  closeout.settlementState === "not_applicable" &&
  closeout.payoutClaimAllowed === false

const lifecycleMatchesAssignment = (
  lifecycle: readonly PylonAssignmentRunLifecycleEvent[],
  assignmentRef: string,
): boolean => lifecycle.every(event => event.assignmentRef === undefined || event.assignmentRef === assignmentRef)

const traceIsAcceptedCloseout = (trace: PylonKhalaAssignmentTraceStatusResult): boolean => {
  const workerCloseout = (trace as unknown as {
    workerCloseout?: { source?: unknown; status?: unknown; projectionBlockerRefs?: unknown }
  }).workerCloseout
  return trace.progress.state === "closed_out" &&
  trace.closeoutPolicy?.source === "worker_closeout_event" &&
  trace.closeoutPolicy.paymentMode === "no-spend" &&
  trace.closeoutPolicy.settlementState === "not_applicable" &&
  trace.closeoutPolicy.payoutClaimAllowed === false &&
  trace.lifecycle.rejectionRefs.length === 0 &&
  workerCloseout?.source === "worker_closeout_event" &&
  workerCloseout.status === "closeout_submitted" &&
  Array.isArray(workerCloseout.projectionBlockerRefs) &&
  workerCloseout.projectionBlockerRefs.length === 0
}

const traceIsRejected = (trace: PylonKhalaAssignmentTraceStatusResult): boolean =>
  trace.progress.state === "rejected" ||
  trace.progress.state === "closed_out" ||
  trace.lifecycle.rejectionRefs.length > 0

const validDate = (value: Date): boolean => value instanceof Date && Number.isFinite(value.getTime())

const validateBaseUrl = (value: string): void => {
  if (value.length < 8 || value.length > 2_048) throw new Error("Pylon fleet runner base URL is invalid")
  assertPublicSafe(value, "Pylon fleet runner base URL")
  let parsed: URL
  try {
    parsed = new URL(value)
  } catch {
    throw new Error("Pylon fleet runner base URL is invalid")
  }
  if (
    (parsed.protocol !== "https:" && parsed.protocol !== "http:") ||
    parsed.username !== "" ||
    parsed.password !== "" ||
    parsed.hash !== ""
  ) {
    throw new Error("Pylon fleet runner base URL is invalid")
  }
}

const validateConstructorInput = (input: CreatePylonOwnedFleetRunSupervisorRunnerInput): void => {
  if (!pylonRefPattern.test(input.pylonRef)) throw new Error("Pylon fleet runner pylonRef is invalid")
  assertPublicSafe(input.pylonRef, "Pylon fleet runner pylonRef")
  validateBaseUrl(input.baseUrl)
  const now = input.now?.() ?? new Date()
  if (!validDate(now)) throw new Error("Pylon fleet runner clock is invalid")
}

const validateDispatchInput = (input: FleetRunSupervisorDispatchInput): void => {
  const refs = [
    input.run.runRef,
    input.claim.claimRef,
    input.claim.runRef,
    input.claim.workUnitRef,
    input.taskId,
    input.workUnit.workUnitRef,
  ]
  if (refs.some(ref => !orchestrationRefPattern.test(ref))) {
    throw new Error("Pylon fleet dispatch contains an invalid public ref")
  }
  if (!accountRefPattern.test(input.accountRef) || !accountRefPattern.test(input.claim.workerAccountRef)) {
    throw new Error("Pylon fleet dispatch contains an invalid account ref")
  }
  if (
    input.claim.runRef !== input.run.runRef ||
    input.claim.workUnitRef !== input.workUnit.workUnitRef ||
    input.claim.workerAccountRef !== input.accountRef ||
    input.claim.assignmentRef !== null ||
    (input.claim.state !== "claimed" && input.claim.state !== "in_progress") ||
    input.run.state !== "running"
  ) {
    throw new Error("Pylon fleet dispatch authority does not match its durable claim")
  }
  assertPublicSafe({
    accountRef: input.accountRef,
    refs,
    workerKind: input.workerKind,
  }, "Pylon fleet dispatch envelope")
  cleanObjective(input)
  cleanObjectiveSummary(input)
  if (input.workUnit.kind !== "fixture") {
    if (
      input.workUnit.repo === undefined ||
      input.workUnit.branch === undefined ||
      input.workUnit.baseCommit === undefined ||
      input.workUnit.verify === undefined
    ) {
      throw new Error("Pylon fleet dispatch lacks a pinned public workspace")
    }
    buildPylonKhalaGitCheckoutWorkspace({
      branch: input.workUnit.branch,
      commit: input.workUnit.baseCommit,
      repository: input.workUnit.repo,
      verificationCommand: input.workUnit.verify,
    })
  }
}

const validateReconcileInput = (
  active: FleetRunSupervisorActiveAssignment,
  runRef: string,
): void => {
  const refs = [
    active.claim.claimRef,
    active.claim.runRef,
    active.claim.workUnitRef,
    active.contextId,
    active.taskId,
    runRef,
  ]
  if (refs.some(ref => !orchestrationRefPattern.test(ref))) {
    throw new Error("Pylon fleet reconcile contains an invalid public ref")
  }
  if (
    !accountRefPattern.test(active.accountRef) ||
    active.claim.workerAccountRef !== active.accountRef ||
    active.claim.runRef !== runRef ||
    active.claim.state !== "in_progress"
  ) {
    throw new Error("Pylon fleet reconcile authority does not match its durable claim")
  }
  assertPublicSafe({ accountRef: active.accountRef, refs }, "Pylon fleet reconcile envelope")
}

const dispatchFingerprint = (input: FleetRunSupervisorDispatchInput): string =>
  createHash("sha256").update(JSON.stringify({
    accountRef: input.accountRef,
    claimRef: input.claim.claimRef,
    runRef: input.run.runRef,
    taskId: input.taskId,
    workerKind: input.workerKind,
    workUnit: {
      baseCommit: input.workUnit.baseCommit ?? null,
      body: input.workUnit.body ?? null,
      branch: input.workUnit.branch ?? null,
      kind: input.workUnit.kind,
      repo: input.workUnit.repo ?? null,
      source: input.workUnit.source,
      title: input.workUnit.title,
      verify: input.workUnit.verify ?? null,
      workUnitRef: input.workUnit.workUnitRef,
    },
  })).digest("hex")

export function createPylonOwnedFleetRunSupervisorRunner(
  input: CreatePylonOwnedFleetRunSupervisorRunnerInput,
): PylonOwnedFleetRunSupervisorRunner {
  validateConstructorInput(input)
  const readNow = (): Date => {
    const value = input.now?.() ?? new Date()
    if (!validDate(value)) throw new Error("Pylon fleet runner clock is invalid")
    return value
  }
  const safeNow = (): Date => {
    try {
      return readNow()
    } catch {
      return new Date(0)
    }
  }
  const fail = (
    status: "blocked" | "failed",
    summary: string,
    blockerRef: string,
    assignmentRef: string | null = null,
    lifecycle: readonly PylonAssignmentRunLifecycleEvent[] = [],
  ): PylonOwnedFleetRunDispatchResult => fixedFailure(
    status,
    summary,
    assignmentRef,
    [...lifecycle, failureLifecycle({ assignmentRef, blockerRef, now: safeNow() })],
  )
  const network: TipsNetworkOptions = {
    baseUrl: input.baseUrl,
    ...(input.agentToken === undefined ? {} : { agentToken: input.agentToken }),
    ...(input.fetch === undefined ? {} : { fetch: input.fetch }),
    now: readNow,
  }
  const defaultHomes = {
    claudeAgent: normalizeAccountHome(input.defaultHomes?.claudeAgent ?? join(homedir(), ".claude")),
    codex: normalizeAccountHome(input.defaultHomes?.codex ?? join(homedir(), ".codex")),
  }
  const loadRegistry = input.loadRegistry ?? (() => strictLoadRegistry(input.summary))
  const request = input.request ?? ((requestInput) => issuePylonKhalaRequest(network, requestInput))
  const inspectAssignment = input.inspectAssignment ?? ((assignmentRef) =>
    readPylonKhalaAssignmentTraceStatus(network, assignmentRef))
  const readCloseout = input.readCloseout ?? ((assignmentRef) =>
    readPylonKhalaCloseout(network, assignmentRef))
  const runAssignment = input.runAssignment ?? (async (requestInput) => {
    const lifecycle: PylonAssignmentRunLifecycleEvent[] = []
    const result = await runNoSpendAssignment(input.summary, {
      ...input.assignmentOptions,
      ...network,
      accountRef: requestInput.accountRef,
      assignmentRef: requestInput.assignmentRef,
      strictAssignmentRef: true,
      onLifecycleEvent: async event => {
        lifecycle.push(event)
        await requestInput.onLifecycle?.(event)
      },
    })
    const lease = "lease" in result ? result.lease : undefined
    const closeout = "closeout" in result ? result.closeout : undefined
    const accountRefHash = [...lifecycle].reverse().find(event => event.accountRefHash !== undefined)?.accountRefHash ?? null
    return {
      accountRefHash,
      assignmentRef: lease?.assignmentRef ?? null,
      closeout: closeout === undefined
        ? null
        : {
            paymentMode: closeout.paymentMode,
            payoutClaimAllowed: closeout.payoutClaimAllowed,
            settlementState: closeout.settlementState,
            status: closeout.status,
          },
      lifecycle,
      ok: result.ok === true,
    }
  })

  // A durable claim is one execution authority. Coalesce duplicate in-process
  // dispatch calls so one supervisor race cannot mint two requests or run the
  // same assignment twice. Terminal replay entries are bounded because the
  // durable task/claim store is authoritative after this process hands a
  // terminal result back to the supervisor. Restart recovery goes through
  // inspect-only reconcile, never this cache.
  type DispatchCacheEntry = {
    readonly fingerprint: string
    readonly promise: Promise<PylonOwnedFleetRunDispatchResult>
  }
  const dispatchesByClaim = new Map<string, DispatchCacheEntry>()
  const terminalRetentionOrder: Array<{ readonly dispatchKey: string; readonly entry: DispatchCacheEntry }> = []

  const retainTerminalDispatch = (
    dispatchKey: string,
    entry: DispatchCacheEntry,
    result: PylonOwnedFleetRunDispatchResult,
  ): void => {
    if (result.status === "accepted") return
    terminalRetentionOrder.push({ dispatchKey, entry })
    while (terminalRetentionOrder.length > PYLON_OWNED_FLEET_RUNNER_MAX_TERMINAL_RETENTION) {
      const oldest = terminalRetentionOrder.shift()
      if (oldest === undefined) break
      if (dispatchesByClaim.get(oldest.dispatchKey) === oldest.entry) {
        dispatchesByClaim.delete(oldest.dispatchKey)
      }
    }
  }

  const dispatchExact = async (
    dispatchInput: FleetRunSupervisorDispatchInput,
  ): Promise<PylonOwnedFleetRunDispatchResult> => {
    if (dispatchInput.workerKind === "grok") {
      if (input.grok === undefined) {
        return fail(
          "blocked",
          "Grok claimed-work custody is unavailable on this Pylon.",
          PYLON_OWNED_FLEET_RUNNER_BLOCKERS.grokCustodyUnavailable,
        )
      }
      try {
        const result = await input.grok.dispatch(dispatchInput)
        assertPublicProjectionSafe(result, "pylonOwnedGrokFleetRunDispatchResult")
        return result
      } catch {
        return fail(
          "failed",
          "The Pylon-owned Grok claimed-work port failed safely.",
          PYLON_OWNED_FLEET_RUNNER_BLOCKERS.runFailed,
        )
      }
    }
    const provider = providerForWorker(dispatchInput.workerKind)
    const workflow = workflowForWorker(dispatchInput.workerKind)
    if (provider === null || workflow === null) {
      return fail(
        "blocked",
        "The claimed worker kind is unavailable on this Pylon.",
        PYLON_OWNED_FLEET_RUNNER_BLOCKERS.accountUnavailable,
      )
    }

    let registry: readonly PylonAccountRegistryEntry[]
    try {
      registry = await loadRegistry()
    } catch {
      return fail(
        "blocked",
        "The named Pylon account registry is unavailable.",
        PYLON_OWNED_FLEET_RUNNER_BLOCKERS.accountUnavailable,
      )
    }
    const account = exactRegistryAccount(registry, {
      accountRef: dispatchInput.accountRef,
      defaultHomes,
      provider,
    })
    if (account === null) {
      return fail(
        "blocked",
        "The named Pylon account is unavailable for this worker kind.",
        PYLON_OWNED_FLEET_RUNNER_BLOCKERS.accountUnavailable,
      )
    }
    const accountRefHash = hashPylonAccountRef(provider, account.ref)

    let khalaInput: PylonKhalaRequestInput
    try {
      khalaInput = requestInputFor(dispatchInput, {
        accountRefHash,
        pylonRef: input.pylonRef,
        workflow,
      })
    } catch {
      return fail(
        "blocked",
        "The claimed work unit lacks a valid public pinned workspace.",
        PYLON_OWNED_FLEET_RUNNER_BLOCKERS.workspaceInvalid,
      )
    }

    let receipt: PylonOwnedFleetRunRequestReceipt
    try {
      receipt = await request(khalaInput)
    } catch {
      return fail(
        "failed",
        "The exact Pylon coding request failed.",
        PYLON_OWNED_FLEET_RUNNER_BLOCKERS.requestFailed,
      )
    }
    const assignmentRef = receipt.assignmentRef?.trim() ?? ""
    if (!assignmentRefPattern.test(assignmentRef)) {
      return fail(
        "failed",
        "The coding request returned no valid assignment ref.",
        PYLON_OWNED_FLEET_RUNNER_BLOCKERS.assignmentMissing,
      )
    }
    const delegation = delegationProjection(receipt)
    if (
      receipt.workflow !== workflow ||
      delegation === null ||
      delegation.assignmentRef !== assignmentRef ||
      delegation.pylonRef !== input.pylonRef ||
      delegation.workflowClass !== workflow
    ) {
      return fail(
        "failed",
        "The coding delegation receipt did not match the exact requested target.",
        PYLON_OWNED_FLEET_RUNNER_BLOCKERS.delegationMismatch,
        assignmentRef,
      )
    }

    let assignment: PylonOwnedFleetRunAssignmentReceipt
    let lifecycleDeliveryTail = Promise.resolve<void>(undefined)
    const deliverLifecycle = (event: PylonAssignmentRunLifecycleEvent): Promise<void> => {
      const delivery = lifecycleDeliveryTail.then(async () => {
        await dispatchInput.onLifecycle?.(event)
      })
      // Keep the serial lane usable after one fail-soft projection failure;
      // the supervisor will replay the buffered event because it deduplicates
      // only successful live deliveries.
      lifecycleDeliveryTail = delivery.catch(() => undefined)
      return delivery
    }
    try {
      assignment = await runAssignment({
        accountRef: account.ref,
        assignmentRef,
        ...(dispatchInput.onLifecycle === undefined
          ? {}
          : { onLifecycle: deliverLifecycle }),
      })
    } catch {
      await lifecycleDeliveryTail
      return fail(
        "failed",
        "The exact no-spend assignment run failed.",
        PYLON_OWNED_FLEET_RUNNER_BLOCKERS.runFailed,
        assignmentRef,
      )
    }
    // `runNoSpendAssignment` clears its interval before returning, but a slow
    // callback may still be in flight. Join the serial delivery lane before a
    // terminal dispatch result can be projected into the ordered outbox.
    await lifecycleDeliveryTail
    if (
      assignment.assignmentRef !== assignmentRef ||
      !lifecycleMatchesAssignment(assignment.lifecycle, assignmentRef)
    ) {
      return fail(
        "failed",
        "The assignment runner returned evidence for a different assignment.",
        PYLON_OWNED_FLEET_RUNNER_BLOCKERS.assignmentMismatch,
        assignmentRef,
      )
    }
    if (assignment.accountRefHash !== accountRefHash) {
      return fail(
        "failed",
        "The assignment runner did not use the named Pylon account.",
        PYLON_OWNED_FLEET_RUNNER_BLOCKERS.accountMismatch,
        assignmentRef,
      )
    }
    if (!closeoutIsNoSpend(assignment.closeout)) {
      return fail(
        "failed",
        "The assignment did not produce an exact no-spend closeout.",
        PYLON_OWNED_FLEET_RUNNER_BLOCKERS.runFailed,
        assignmentRef,
        assignment.lifecycle,
      )
    }
    const completed = assignment.ok && assignment.closeout?.status === "accepted"
    if (!completed) {
      return fail(
        "failed",
        "The exact no-spend Pylon assignment closed unsuccessfully.",
        PYLON_OWNED_FLEET_RUNNER_BLOCKERS.runFailed,
        assignmentRef,
        assignment.lifecycle,
      )
    }
    let evidence: ReturnType<typeof exactPylonFleetRunUsageEvidence>
    let terminalEvidence: ReturnType<typeof terminalEvidenceFromCloseout>
    try {
      const closeout = await readCloseout(assignmentRef)
      evidence = exactPylonFleetRunUsageEvidence({
        accountRefHash,
        assignmentRef,
        closeout,
        harnessKind: dispatchInput.workerKind,
        pylonRef: input.pylonRef,
      })
      terminalEvidence = terminalEvidenceFromCloseout(dispatchInput, closeout)
    } catch {
      return fail(
        "failed",
        "The exact assignment closeout usage evidence was incomplete or mismatched.",
        PYLON_OWNED_FLEET_RUNNER_BLOCKERS.usageEvidenceInvalid,
        assignmentRef,
        assignment.lifecycle,
      )
    }
    const result = {
      ...evidence,
      assignmentRef,
      ...terminalEvidence,
      lifecycle: [...assignment.lifecycle],
      marginalCostClass: dispatchInput.claim.marginalCostClass ?? "not_measured",
      status: "completed",
      summary: "The exact no-spend Pylon assignment completed with exact usage evidence.",
    } satisfies PylonOwnedFleetRunDispatchResult
    assertPublicProjectionSafe(result, "pylonOwnedFleetRunDispatchResult")
    return result
  }

  const dispatch: PylonOwnedFleetRunSupervisorRunner["dispatch"] = (dispatchInput) => {
    try {
      readNow()
      validateDispatchInput(dispatchInput)
    } catch {
      return Promise.resolve(fail(
        "blocked",
        "The Pylon fleet dispatch envelope is invalid.",
        PYLON_OWNED_FLEET_RUNNER_BLOCKERS.dispatchInvalid,
      ))
    }
    const dispatchKey = `${dispatchInput.run.runRef}:${dispatchInput.claim.claimRef}`
    const fingerprint = dispatchFingerprint(dispatchInput)
    const existing = dispatchesByClaim.get(dispatchKey)
    if (existing !== undefined) {
      if (existing.fingerprint === fingerprint) return existing.promise
      return Promise.resolve(fail(
        "failed",
        "The durable claim ref was reused with conflicting dispatch authority.",
        PYLON_OWNED_FLEET_RUNNER_BLOCKERS.dispatchConflict,
      ))
    }
    const started = dispatchExact(dispatchInput).catch(() =>
      fail(
        "failed",
        "The Pylon fleet runner failed safely.",
        PYLON_OWNED_FLEET_RUNNER_BLOCKERS.runFailed,
      ))
    const entry = { fingerprint, promise: started } satisfies DispatchCacheEntry
    dispatchesByClaim.set(dispatchKey, entry)
    void started.then(result => retainTerminalDispatch(dispatchKey, entry, result))
    return started
  }

  const reconcileOne = async (
    active: FleetRunSupervisorActiveAssignment,
    now: Date,
    runRef: string,
  ): Promise<PylonOwnedFleetRunReconcileResult> => {
    try {
      validateReconcileInput(active, runRef)
    } catch {
      return {
        ...fail(
          "failed",
          "The Pylon fleet reconcile envelope is invalid.",
          PYLON_OWNED_FLEET_RUNNER_BLOCKERS.dispatchInvalid,
        ),
        taskId: active.taskId,
      }
    }
    const assignmentRef = active.claim.assignmentRef?.trim() ?? ""
    if (!assignmentRefPattern.test(assignmentRef)) {
      return {
        ...fail(
          "failed",
          "The durable claim has no valid assignment ref.",
          PYLON_OWNED_FLEET_RUNNER_BLOCKERS.assignmentMissing,
          null,
        ),
        taskId: active.taskId,
      }
    }
    if (assignmentRef.startsWith("assignment.pylon.grok.")) {
      if (input.grok === undefined) {
        return {
          ...fail(
            "failed",
            "The exact Grok claimed-work port is unavailable during reconciliation.",
            PYLON_OWNED_FLEET_RUNNER_BLOCKERS.grokCustodyUnavailable,
            assignmentRef,
          ),
          taskId: active.taskId,
        }
      }
      try {
        const result = await input.grok.reconcile({ active, now, runRef })
        if (result.taskId !== active.taskId) throw new Error("Grok reconcile task mismatch")
        assertPublicProjectionSafe(result, "pylonOwnedGrokFleetRunReconcileResult")
        return result
      } catch {
        return {
          ...fail(
            "failed",
            "The exact Grok claimed-work receipt could not be reconciled safely.",
            PYLON_OWNED_FLEET_RUNNER_BLOCKERS.runFailed,
            assignmentRef,
          ),
          taskId: active.taskId,
        }
      }
    }
    let trace: PylonKhalaAssignmentTraceStatusResult
    try {
      trace = await inspectAssignment(assignmentRef)
    } catch {
      return {
        accountRefHash: null,
        assignmentRef,
        closeoutRef: null,
        lifecycle: [{
          schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
          event: "assignment_run.runtime_progress",
          observedAt: now.toISOString(),
          assignmentRef,
          status: "running",
          phase: "runtime_active",
          blockerRefs: [PYLON_OWNED_FLEET_RUNNER_BLOCKERS.requestFailed],
        }],
        status: "accepted",
        summary: "The exact assignment is temporarily unavailable for inspection.",
        taskId: active.taskId,
        usageEvidence: null,
      }
    }
    if (trace.assignmentRef !== assignmentRef || trace.pylonRef !== input.pylonRef) {
      return {
        accountRefHash: null,
        assignmentRef,
        closeoutRef: null,
        lifecycle: [terminalLifecycle({
          assignmentRef,
          blockerRef: PYLON_OWNED_FLEET_RUNNER_BLOCKERS.assignmentMismatch,
          now,
          status: "rejected",
        })],
        status: "failed",
        summary: "The assignment inspection did not match the durable claim.",
        taskId: active.taskId,
        usageEvidence: null,
      }
    }
    if (traceIsAcceptedCloseout(trace)) {
      let evidence: ReturnType<typeof exactPylonFleetRunUsageEvidence>
      let terminalEvidence: ReturnType<typeof terminalEvidenceFromCloseout>
      try {
        const registry = await loadRegistry()
        const matches = registry.filter(account => account.ref === active.accountRef)
        const candidate = matches.length === 1 ? matches[0] : undefined
        if (candidate === undefined || (candidate.provider !== "codex" && candidate.provider !== "claude_agent")) {
          throw new Error("reconcile account custody is unavailable")
        }
        const account = exactRegistryAccount(registry, {
          accountRef: active.accountRef,
          defaultHomes,
          provider: candidate.provider,
        })
        if (account === null) throw new Error("reconcile account custody is invalid")
        const closeout = await readCloseout(assignmentRef)
        evidence = exactPylonFleetRunUsageEvidence({
          accountRefHash: hashPylonAccountRef(account.provider, account.ref),
          assignmentRef,
          closeout,
          harnessKind: account.provider === "codex" ? "codex" : "claude",
          pylonRef: input.pylonRef,
        })
        terminalEvidence = reconciledTerminalEvidenceFromCloseout(active, closeout)
      } catch {
        return {
          ...fail(
            "failed",
            "The reconciled assignment closeout usage evidence was incomplete or mismatched.",
            PYLON_OWNED_FLEET_RUNNER_BLOCKERS.usageEvidenceInvalid,
            assignmentRef,
          ),
          taskId: active.taskId,
        }
      }
      return {
        ...evidence,
        assignmentRef,
        ...terminalEvidence,
        lifecycle: [terminalLifecycle({ assignmentRef, now, status: "closed" })],
        marginalCostClass: active.claim.marginalCostClass ?? "not_measured",
        status: "completed",
        summary: "The exact no-spend Pylon assignment is closed out with exact usage evidence.",
        taskId: active.taskId,
      }
    }
    if (traceIsRejected(trace)) {
      return {
        accountRefHash: null,
        assignmentRef,
        closeoutRef: null,
        lifecycle: [terminalLifecycle({
          assignmentRef,
          blockerRef: PYLON_OWNED_FLEET_RUNNER_BLOCKERS.runFailed,
          now,
          status: "rejected",
        })],
        status: "failed",
        summary: "The exact Pylon assignment was rejected.",
        taskId: active.taskId,
        usageEvidence: null,
      }
    }
    return {
      accountRefHash: null,
      assignmentRef,
      closeoutRef: null,
      lifecycle: [],
      status: "accepted",
      summary: "The exact Pylon assignment remains active.",
      taskId: active.taskId,
      usageEvidence: null,
    }
  }

  const inspectExactAttempt = async (
    attempt: PylonFleetRunExactAttempt,
  ): Promise<"active" | "terminal" | "invalid"> => {
    if (attempt.pylonRef !== input.pylonRef) return "invalid"
    let trace: PylonKhalaAssignmentTraceStatusResult
    try {
      trace = await inspectAssignment(attempt.assignmentRef)
    } catch {
      return "active"
    }
    if (
      trace.assignmentRef !== attempt.assignmentRef ||
      trace.pylonRef !== attempt.pylonRef
    ) return "invalid"
    if (traceIsAcceptedCloseout(trace) || traceIsRejected(trace)) return "terminal"
    return "active"
  }

  const unsupportedAttemptControl = async (
    attempt: PylonFleetRunExactAttempt,
    failureRef: string,
  ) => {
    const state = await inspectExactAttempt(attempt)
    if (state === "invalid") {
      return {
        state: "failed" as const,
        failureRef: "blocker.pylon.fleet_steering.attempt_inspection_mismatch",
      }
    }
    if (state === "terminal") {
      return {
        state: "stale" as const,
        failureRef: "blocker.pylon.fleet_steering.attempt_terminal",
      }
    }
    // The production no-spend runner is deliberately unattended
    // (approvalPolicy=never) and its current assignment port has no live
    // next-turn/abort wire. Report that boundary instead of pretending the
    // command reached the worker.
    return { state: "failed" as const, failureRef }
  }

  return {
    dispatch,
    reconcile: async ({ activeAssignments, now, run }) => {
      if (!validDate(now)) {
        return activeAssignments.map(active => ({
          ...fail(
            "failed",
            "The Pylon fleet reconcile clock is invalid.",
            PYLON_OWNED_FLEET_RUNNER_BLOCKERS.dispatchInvalid,
          ),
          taskId: active.taskId,
        }))
      }
      const results = await Promise.all(activeAssignments.map(active => reconcileOne(active, now, run.runRef)))
      for (const result of results) {
        if (result.status === "accepted") continue
        const active = activeAssignments.find(candidate => candidate.taskId === result.taskId)
        if (active !== undefined) {
          dispatchesByClaim.delete(`${run.runRef}:${active.claim.claimRef}`)
        }
      }
      assertPublicProjectionSafe(results, "pylonOwnedFleetRunReconcileResults")
      return results
    },
    retainedDispatchCount: () => dispatchesByClaim.size,
    steeringControl: {
      applyApproval: async attempt => await unsupportedAttemptControl(
        attempt,
        "blocker.pylon.fleet_steering.approval_control_unavailable",
      ),
      applySteer: async attempt => await unsupportedAttemptControl(
        attempt,
        "blocker.pylon.fleet_steering.next_turn_control_unavailable",
      ),
      observeStop: async ({ attempts }) => {
        for (const attempt of attempts) {
          const state = await inspectExactAttempt(attempt)
          if (state === "invalid") {
            return {
              state: "failed",
              failureRef: "blocker.pylon.fleet_steering.attempt_inspection_mismatch",
            }
          }
        }
        return {
          state: "retry",
          failureRef: "blocker.pylon.fleet_steering.stop_waiting_for_terminal_attempts",
        }
      },
    },
  }
}
