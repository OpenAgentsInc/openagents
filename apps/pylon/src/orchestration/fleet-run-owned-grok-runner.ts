import { createHash } from "node:crypto"
import { lstat, mkdir, readFile, rename, rm, writeFile } from "node:fs/promises"
import { join, relative, resolve, sep } from "node:path"

import {
  createGrokHeadlessWorkerExecutor,
  type GrokWorkerExecutorPort,
} from "@openagentsinc/grok-harness/worker-executor"
import type { PylonAssignmentRunLifecycleEvent } from "@openagentsinc/agent-runtime-schema"
import { Effect } from "effect"

import {
  hashPylonAccountRef,
  isDefaultGrokAccountHome,
  loadPylonAccountRegistryEffect,
  normalizeAccountHome,
  pylonAccountEnvironment,
  PylonAccountRegistryError,
  type PylonAccountRegistryEntry,
  type ResolvedPylonAccountSelection,
} from "../account-registry.js"
import type { BootstrapSummary } from "../bootstrap.js"
import { buildPylonKhalaGitCheckoutWorkspace } from "../khala-requester.js"
import { assertPublicProjectionSafe } from "../state.js"
import {
  materializeGitCheckoutWorkspaceWithLease,
  type GitCheckoutWorkspace,
  type WorkspaceCheckoutRunner,
} from "../workspace-materializer.js"
import type {
  PylonOwnedFleetRunDispatchResult,
  PylonOwnedFleetRunReconcileResult,
  PylonOwnedGrokClaimedWorkPort,
} from "./fleet-run-owned-runner.js"
import {
  notMeasuredPylonFleetRunUsageEvidence,
  pylonGrokUsageEvidenceRefs,
  type PylonFleetRunUsageEvidenceCarrier,
} from "./fleet-run-usage-evidence.js"
import type {
  FleetRunSupervisorActiveAssignment,
  FleetRunSupervisorDispatchInput,
} from "./fleet-run-supervisor.js"
import type { PylonOrchestrationStore } from "./store.js"

export const PYLON_OWNED_GROK_RUNNER_BLOCKERS = {
  accountExhausted: "blocker.pylon.fleet_runner.grok_account_exhausted",
  accountRateLimited: "blocker.pylon.fleet_runner.grok_account_rate_limited",
  accountUnavailable: "blocker.pylon.fleet_runner.grok_account_unavailable",
  custodyInvalid: "blocker.pylon.fleet_runner.grok_custody_invalid",
  dispatchConflict: "blocker.pylon.fleet_runner.grok_dispatch_conflict",
  executionFailed: "blocker.pylon.fleet_runner.grok_execution_failed",
  executionInterrupted: "blocker.pylon.fleet_runner.grok_execution_interrupted",
  executionTimedOut: "blocker.pylon.fleet_runner.grok_execution_timed_out",
  receiptInvalid: "blocker.pylon.fleet_runner.grok_receipt_invalid",
  receiptUpgradeRequired: "blocker.pylon.fleet_runner.grok_receipt_upgrade_required",
  readinessUnavailable: "blocker.pylon.fleet_runner.grok_readiness_unavailable",
  verificationFailed: "blocker.pylon.fleet_runner.grok_verification_failed",
  workspaceInvalid: "blocker.pylon.fleet_runner.grok_workspace_invalid",
} as const

const GROK_RECEIPT_SCHEMA = "openagents.pylon.grok_claimed_work_receipt.v1" as const
const GROK_RECEIPT_SCHEMA_V2 = "openagents.pylon.grok_claimed_work_receipt.v2" as const
const GROK_ASSIGNMENT_PREFIX = "assignment.pylon.grok."
const GROK_RECEIPT_MAX_BYTES = 32 * 1_024
const PUBLIC_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._:/#-]{0,255}$/u
const ACCOUNT_REF_HASH_PATTERN = /^account\.pylon\.grok\.[a-f0-9]{24}$/u
const FINGERPRINT_PATTERN = /^[a-f0-9]{64}$/u
const DEFAULT_GROK_CLAIM_TIMEOUT_MS = 30 * 60 * 1_000
const MAX_GROK_CLAIM_TIMEOUT_MS = 4 * 60 * 60 * 1_000
const DEFAULT_GROK_VERIFY_TIMEOUT_MS = 15 * 60 * 1_000
const MAX_GROK_VERIFY_TIMEOUT_MS = 60 * 60 * 1_000
const GROK_LIFECYCLE_HEARTBEAT_MS = 10_000

type GrokExecutionReceiptV1 = {
  readonly schema: typeof GROK_RECEIPT_SCHEMA
  readonly accountRefHash: string
  readonly assignmentRef: string
  readonly closeoutRef: string
  readonly claimRef: string
  readonly failureRef: string | null
  readonly fingerprint: string
  readonly observedAt: string
  readonly runRef: string
  readonly receiptRef: string
  readonly state: "running" | "completed" | "failed"
  readonly taskId: string
  readonly usageTruth: "not_measured"
  readonly workUnitRef: string
  readonly workspaceRef: string | null
}

type GrokExecutionReceiptV2 = Omit<GrokExecutionReceiptV1, "schema"> & {
  readonly schema: typeof GROK_RECEIPT_SCHEMA_V2
  readonly executionPlane: "cli_session"
  readonly marginalCostClass: PylonAccountRegistryEntry["marginalCostClass"]
  readonly wallClockMs: number | null
  readonly verification: {
    readonly truth: "pending" | "passed" | "failed"
    readonly verifierRef: string
    readonly evidenceRefs: readonly string[]
  }
  readonly artifactRefs: readonly string[]
  readonly proofRefs: readonly string[]
  readonly authorityReceiptRefs: readonly string[]
}

type GrokExecutionReceipt = GrokExecutionReceiptV1 | GrokExecutionReceiptV2

export type PylonOwnedGrokWorkspace = {
  readonly checkout: GitCheckoutWorkspace | null
  readonly verificationArgs: readonly string[] | null
  /** Local-only. Never return this value from a public result. */
  readonly workingDirectory: string
  readonly workspaceRef: string
}

export type PylonOwnedGrokWorkspacePort = (input: {
  readonly assignmentRef: string
  readonly dispatch: FleetRunSupervisorDispatchInput
  readonly now: Date
}) => Promise<PylonOwnedGrokWorkspace>

export type PylonOwnedGrokVerifierPort = (input: {
  readonly args: readonly string[]
  readonly cwd: string
  readonly timeoutMs: number
}) => Promise<{ readonly exitCode: number; readonly timedOut: boolean }>

export type CreatePylonOwnedGrokClaimedWorkPortInput = {
  readonly summary: BootstrapSummary
  readonly env?: NodeJS.ProcessEnv | undefined
  readonly now?: (() => Date) | undefined
  readonly loadRegistry?: (() => Promise<readonly PylonAccountRegistryEntry[]>) | undefined
  readonly createExecutor?: ((input: {
    readonly account: PylonAccountRegistryEntry
    readonly env: NodeJS.ProcessEnv
  }) => GrokWorkerExecutorPort) | undefined
  readonly materializeWorkspace?: PylonOwnedGrokWorkspacePort | undefined
  readonly checkoutRunner?: WorkspaceCheckoutRunner | undefined
  readonly runVerifier?: PylonOwnedGrokVerifierPort | undefined
  readonly workerTimeoutMs?: number | undefined
  readonly verifierTimeoutMs?: number | undefined
  readonly lifecycleHeartbeatMs?: number | undefined
  /** Canonical claim authority. The standing composition always supplies this store. */
  readonly store?: PylonOrchestrationStore | undefined
}

const boundedTimeout = (value: number | undefined, fallback: number, maximum: number): number =>
  value === undefined || !Number.isFinite(value)
    ? fallback
    : Math.max(100, Math.min(maximum, Math.trunc(value)))

const stableDigest = (value: string, length = 32): string =>
  createHash("sha256").update(value).digest("hex").slice(0, length)

const assignmentRefFor = (fingerprint: string): string =>
  `${GROK_ASSIGNMENT_PREFIX}${stableDigest(fingerprint, 24)}`

const stablePublicRef = (prefix: string, seed: string): string =>
  `${prefix}.${stableDigest(seed, 24)}`

const validDate = (value: Date): boolean => value instanceof Date && Number.isFinite(value.getTime())

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

const fixedResult = (input: {
  readonly assignmentRef: string | null
  readonly blockerRef?: string | undefined
  readonly now: Date
  readonly status: "blocked" | "failed" | "completed"
  readonly summary: string
  readonly accountRefHash?: string | undefined
  readonly workspaceRef?: string | undefined
  readonly evidence?: PylonFleetRunUsageEvidenceCarrier | undefined
  readonly marginalCostClass?: PylonAccountRegistryEntry["marginalCostClass"] | undefined
  readonly verification?: PylonOwnedFleetRunDispatchResult["verification"] | undefined
  readonly artifactRefs?: readonly string[] | undefined
  readonly proofRefs?: readonly string[] | undefined
  readonly authorityReceiptRefs?: readonly string[] | undefined
}): PylonOwnedFleetRunDispatchResult => {
  const lifecycle: PylonAssignmentRunLifecycleEvent[] = input.assignmentRef === null
    ? [{
        schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
        event: "assignment_run.no_assignment",
        observedAt: input.now.toISOString(),
        status: "rejected",
        ...(input.blockerRef === undefined ? {} : { blockerRefs: [input.blockerRef] }),
      }]
    : [{
        schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
        event: "assignment_run.completed",
        observedAt: input.now.toISOString(),
        assignmentRef: input.assignmentRef,
        status: input.status === "completed" ? "closed" : "rejected",
        ...(input.accountRefHash === undefined ? {} : { accountRefHash: input.accountRefHash }),
        ...(input.workspaceRef === undefined ? {} : { artifactRef: input.workspaceRef }),
        ...(input.blockerRef === undefined ? {} : { blockerRefs: [input.blockerRef] }),
      }]
  const result = {
    accountRefHash: input.evidence?.accountRefHash ?? null,
    assignmentRef: input.assignmentRef,
    closeoutRef: input.evidence?.closeoutRef ?? null,
    lifecycle,
    ...(input.marginalCostClass === undefined
      ? {}
      : { marginalCostClass: input.marginalCostClass }),
    ...(input.verification === undefined ? {} : { verification: input.verification }),
    ...(input.artifactRefs === undefined ? {} : { artifactRefs: input.artifactRefs }),
    ...(input.proofRefs === undefined ? {} : { proofRefs: input.proofRefs }),
    ...(input.authorityReceiptRefs === undefined
      ? {}
      : { authorityReceiptRefs: input.authorityReceiptRefs }),
    status: input.status,
    summary: input.summary,
    usageEvidence: input.evidence?.usageEvidence ?? null,
  } satisfies PylonOwnedFleetRunDispatchResult
  assertPublicProjectionSafe(result, "pylonOwnedGrokClaimedWorkResult")
  return result
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

const exactNamedGrokAccount = async (
  summary: Pick<BootstrapSummary, "paths">,
  registry: readonly PylonAccountRegistryEntry[],
  accountRef: string,
): Promise<PylonAccountRegistryEntry | null> => {
  const matches = registry.filter(account => account.ref === accountRef)
  if (matches.length !== 1) return null
  const account = matches[0]
  if (account.provider !== "grok" || account.paused === true) return null
  const expectedHome = normalizeAccountHome(join(summary.paths.home, "accounts", "grok", accountRef))
  if (
    /^(?:\(default\)|default)$/iu.test(account.ref.trim()) ||
    isDefaultGrokAccountHome(account.home, {}) ||
    normalizeAccountHome(account.home) !== expectedHome
  ) return null
  try {
    const info = await lstat(expectedHome)
    if (!info.isDirectory() || info.isSymbolicLink()) return null
  } catch {
    return null
  }
  return account
}

const checkoutFor = (input: FleetRunSupervisorDispatchInput): GitCheckoutWorkspace | null => {
  if (input.workUnit.kind === "fixture") return null
  if (
    input.workUnit.repo === undefined ||
    input.workUnit.branch === undefined ||
    input.workUnit.baseCommit === undefined ||
    input.workUnit.verify === undefined
  ) throw new Error("Grok real work lacks a pinned checkout")
  return buildPylonKhalaGitCheckoutWorkspace({
    repository: input.workUnit.repo,
    branch: input.workUnit.branch,
    commit: input.workUnit.baseCommit,
    verificationCommand: input.workUnit.verify,
  })
}

const defaultWorkspacePort = (
  input: CreatePylonOwnedGrokClaimedWorkPortInput,
): PylonOwnedGrokWorkspacePort => async request => {
  const checkout = checkoutFor(request.dispatch)
  if (checkout === null) {
    const workspaceRef = `workspace.pylon.grok.fixture.${stableDigest(request.assignmentRef, 24)}`
    const workingDirectory = join(input.summary.paths.cache, "grok-fleet-fixtures", workspaceRef)
    await rm(workingDirectory, { force: true, recursive: true })
    await mkdir(workingDirectory, { mode: 0o700, recursive: true })
    return { checkout: null, verificationArgs: null, workingDirectory, workspaceRef }
  }
  const materialized = await materializeGitCheckoutWorkspaceWithLease({
    cacheRoot: join(input.summary.paths.cache, "grok-fleet-workspaces"),
    checkout,
    ...(input.checkoutRunner === undefined ? {} : { checkoutRunner: input.checkoutRunner }),
    leaseRef: request.assignmentRef,
    now: request.now,
    refPrefix: "workspace.pylon.grok",
    repositoryCacheRoot: join(input.summary.paths.cache, "workspace-git-cache"),
    retentionPolicy: "retain_until_ttl",
    workspaceStateRoot: join(input.summary.paths.cache, "workspace-leases"),
  })
  return {
    checkout,
    verificationArgs: checkout.verificationCommand.args,
    workingDirectory: materialized.workingDirectory,
    workspaceRef: materialized.workspaceRef,
  }
}

const defaultVerifier: PylonOwnedGrokVerifierPort = async input => {
  const child = Bun.spawn([...input.args], {
    cwd: input.cwd,
    stdin: "ignore",
    stdout: "ignore",
    stderr: "ignore",
  })
  let timedOut = false
  let forceKillTimer: ReturnType<typeof setTimeout> | undefined
  const timer = setTimeout(() => {
    timedOut = true
    try {
      child.kill()
    } catch {
      // The verifier already exited.
    }
    forceKillTimer = setTimeout(() => {
      try {
        child.kill("SIGKILL")
      } catch {
        // The verifier already exited.
      }
    }, 250)
  }, input.timeoutMs)
  let exitCode = 1
  try {
    exitCode = await child.exited
  } catch {
    exitCode = 1
  } finally {
    clearTimeout(timer)
    if (forceKillTimer !== undefined) clearTimeout(forceKillTimer)
  }
  return { exitCode: timedOut ? 1 : exitCode, timedOut }
}

const receiptDirectory = (summary: Pick<BootstrapSummary, "paths">): string =>
  join(summary.paths.cache, "grok-fleet-receipts")

const receiptPath = (summary: Pick<BootstrapSummary, "paths">, assignmentRef: string): string =>
  join(receiptDirectory(summary), `${stableDigest(assignmentRef, 40)}.json`)

const receiptFrom = (value: unknown): GrokExecutionReceipt | null => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return null
  const record = value as Record<string, unknown>
  if (
    (record.schema !== GROK_RECEIPT_SCHEMA && record.schema !== GROK_RECEIPT_SCHEMA_V2) ||
    typeof record.accountRefHash !== "string" ||
    typeof record.assignmentRef !== "string" ||
    typeof record.closeoutRef !== "string" ||
    typeof record.claimRef !== "string" ||
    !(record.failureRef === null || typeof record.failureRef === "string") ||
    typeof record.fingerprint !== "string" ||
    typeof record.observedAt !== "string" ||
    typeof record.runRef !== "string" ||
    typeof record.receiptRef !== "string" ||
    !(record.state === "running" || record.state === "completed" || record.state === "failed") ||
    typeof record.taskId !== "string" ||
    record.usageTruth !== "not_measured" ||
    typeof record.workUnitRef !== "string" ||
    !(record.workspaceRef === null || typeof record.workspaceRef === "string")
  ) return null
  const observedAt = new Date(record.observedAt)
  const stableEvidenceRefs = isPylonOwnedGrokAssignmentRef(record.assignmentRef)
    ? pylonGrokUsageEvidenceRefs(record.assignmentRef)
    : null
  const refs = [
    record.claimRef,
    record.closeoutRef,
    record.receiptRef,
    record.runRef,
    record.taskId,
    record.workUnitRef,
  ]
  if (
    !validDate(observedAt) ||
    !isPylonOwnedGrokAssignmentRef(record.assignmentRef) ||
    stableEvidenceRefs === null ||
    record.closeoutRef !== stableEvidenceRefs.closeoutRef ||
    record.receiptRef !== stableEvidenceRefs.receiptRef ||
    !ACCOUNT_REF_HASH_PATTERN.test(record.accountRefHash) ||
    !FINGERPRINT_PATTERN.test(record.fingerprint) ||
    refs.some(ref => !PUBLIC_REF_PATTERN.test(ref)) ||
    (record.workspaceRef !== null && !PUBLIC_REF_PATTERN.test(record.workspaceRef)) ||
    (record.failureRef !== null && !Object.values(PYLON_OWNED_GROK_RUNNER_BLOCKERS).includes(
      record.failureRef as typeof PYLON_OWNED_GROK_RUNNER_BLOCKERS[keyof typeof PYLON_OWNED_GROK_RUNNER_BLOCKERS],
    )) ||
    (record.state === "failed" ? record.failureRef === null : record.failureRef !== null)
  ) return null
  if (record.schema === GROK_RECEIPT_SCHEMA_V2) {
    const verification = record.verification
    if (
      record.executionPlane !== "cli_session" ||
      !(record.marginalCostClass === "free" ||
        record.marginalCostClass === "subscription" ||
        record.marginalCostClass === "api_metered" ||
        record.marginalCostClass === "not_measured") ||
      !(record.wallClockMs === null ||
        (typeof record.wallClockMs === "number" &&
          Number.isSafeInteger(record.wallClockMs) &&
          record.wallClockMs >= 0)) ||
      verification === null ||
      typeof verification !== "object" ||
      Array.isArray(verification) ||
      !(
        (verification as Record<string, unknown>).truth === "pending" ||
        (verification as Record<string, unknown>).truth === "passed" ||
        (verification as Record<string, unknown>).truth === "failed"
      ) ||
      typeof (verification as Record<string, unknown>).verifierRef !== "string" ||
      !Array.isArray((verification as Record<string, unknown>).evidenceRefs) ||
      !Array.isArray(record.artifactRefs) ||
      !Array.isArray(record.proofRefs) ||
      !Array.isArray(record.authorityReceiptRefs)
    ) return null
    const evidenceRefs = (verification as Record<string, unknown>).evidenceRefs as unknown[]
    const allEvidenceRefs = [
      (verification as Record<string, unknown>).verifierRef,
      ...evidenceRefs,
      ...record.artifactRefs,
      ...record.proofRefs,
      ...record.authorityReceiptRefs,
    ]
    if (
      evidenceRefs.length > 64 ||
      record.artifactRefs.length > 64 ||
      record.proofRefs.length > 64 ||
      record.authorityReceiptRefs.length > 64 ||
      allEvidenceRefs.some(ref => typeof ref !== "string" || !PUBLIC_REF_PATTERN.test(ref)) ||
      (record.state === "running"
        ? (verification as Record<string, unknown>).truth !== "pending" ||
          record.wallClockMs !== null
        : record.state === "completed"
          ? (verification as Record<string, unknown>).truth !== "passed" ||
            record.wallClockMs === null ||
            evidenceRefs.length === 0 ||
            record.artifactRefs.length === 0 ||
            record.proofRefs.length === 0 ||
            record.authorityReceiptRefs.length === 0
          : (verification as Record<string, unknown>).truth === "passed")
    ) return null
  }
  return record as GrokExecutionReceipt
}

const readReceipt = async (
  summary: Pick<BootstrapSummary, "paths">,
  assignmentRef: string,
): Promise<GrokExecutionReceipt | null | "invalid"> => {
  const path = receiptPath(summary, assignmentRef)
  try {
    const info = await lstat(path)
    if (!info.isFile() || info.isSymbolicLink() || info.size > GROK_RECEIPT_MAX_BYTES) return "invalid"
    const value = await readFile(path, "utf8")
    return receiptFrom(JSON.parse(value)) ?? "invalid"
  } catch (error) {
    const code = error !== null && typeof error === "object" && "code" in error
      ? String(error.code)
      : ""
    return code === "ENOENT" ? null : "invalid"
  }
}

const writeReceipt = async (
  summary: Pick<BootstrapSummary, "paths">,
  receipt: GrokExecutionReceipt,
): Promise<void> => {
  const directory = receiptDirectory(summary)
  await mkdir(directory, { mode: 0o700, recursive: true })
  const path = receiptPath(summary, receipt.assignmentRef)
  const temp = `${path}.${process.pid}.${stableDigest(`${receipt.fingerprint}:${Date.now()}`, 12)}.tmp`
  await writeFile(temp, `${JSON.stringify(receipt)}\n`, { mode: 0o600 })
  await rename(temp, path)
}

const receiptMatchesDispatch = (
  receipt: GrokExecutionReceipt,
  dispatch: FleetRunSupervisorDispatchInput,
  fingerprint: string,
  accountRefHash: string,
): boolean =>
  receipt.fingerprint === fingerprint &&
  receipt.accountRefHash === accountRefHash &&
  receipt.claimRef === dispatch.claim.claimRef &&
  receipt.runRef === dispatch.run.runRef &&
  receipt.taskId === dispatch.taskId &&
  receipt.workUnitRef === dispatch.workUnit.workUnitRef

const resultFromReceipt = (
  receipt: GrokExecutionReceipt,
  now: Date,
): PylonOwnedFleetRunDispatchResult => {
  if (receipt.state === "completed") {
    if (receipt.schema !== GROK_RECEIPT_SCHEMA_V2) {
      return fixedResult({
        assignmentRef: receipt.assignmentRef,
        accountRefHash: receipt.accountRefHash,
        ...(receipt.workspaceRef === null ? {} : { workspaceRef: receipt.workspaceRef }),
        blockerRef: PYLON_OWNED_GROK_RUNNER_BLOCKERS.receiptUpgradeRequired,
        now,
        status: "failed",
        summary: "The legacy Grok receipt lacks v2 verification evidence and was not promoted.",
      })
    }
    const evidence = notMeasuredPylonFleetRunUsageEvidence({
      accountRefHash: receipt.accountRefHash,
      assignmentRef: receipt.assignmentRef,
      closeoutRef: receipt.closeoutRef,
      receiptRef: receipt.receiptRef,
    })
    return fixedResult({
      assignmentRef: receipt.assignmentRef,
      accountRefHash: receipt.accountRefHash,
      ...(receipt.workspaceRef === null ? {} : { workspaceRef: receipt.workspaceRef }),
      now,
      status: "completed",
      summary: "The exact named Grok claimed work completed with not_measured usage.",
      evidence,
      marginalCostClass: receipt.marginalCostClass,
      verification: {
        truth: "passed",
        verifierRef: receipt.verification.verifierRef,
        evidenceRefs: receipt.verification.evidenceRefs,
      },
      artifactRefs: receipt.artifactRefs,
      proofRefs: receipt.proofRefs,
      authorityReceiptRefs: receipt.authorityReceiptRefs,
    })
  }
  return fixedResult({
    assignmentRef: receipt.assignmentRef,
    accountRefHash: receipt.accountRefHash,
    ...(receipt.workspaceRef === null ? {} : { workspaceRef: receipt.workspaceRef }),
    blockerRef: receipt.state === "running"
      ? PYLON_OWNED_GROK_RUNNER_BLOCKERS.executionInterrupted
      : receipt.failureRef ?? PYLON_OWNED_GROK_RUNNER_BLOCKERS.executionFailed,
    now,
    status: "failed",
    summary: receipt.state === "running"
      ? "The exact Grok claimed work was interrupted and was not rerun."
      : "The exact named Grok claimed work failed safely.",
  })
}

const failureRefForClass = (failureClass: string | undefined): string =>
  failureClass === "timeout"
    ? PYLON_OWNED_GROK_RUNNER_BLOCKERS.executionTimedOut
    : failureClass === "account_rate_limited"
      ? PYLON_OWNED_GROK_RUNNER_BLOCKERS.accountRateLimited
      : failureClass === "account_exhausted" || failureClass === "account_quota_exhausted"
        ? PYLON_OWNED_GROK_RUNNER_BLOCKERS.accountExhausted
        : failureClass === "auth_required"
          ? PYLON_OWNED_GROK_RUNNER_BLOCKERS.custodyInvalid
          : PYLON_OWNED_GROK_RUNNER_BLOCKERS.executionFailed

export function isPylonOwnedGrokAssignmentRef(value: string | null | undefined): value is string {
  return typeof value === "string" && /^assignment\.pylon\.grok\.[a-f0-9]{24}$/u.test(value)
}

export function createPylonOwnedGrokClaimedWorkPort(
  input: CreatePylonOwnedGrokClaimedWorkPortInput,
): PylonOwnedGrokClaimedWorkPort {
  const baseEnv = input.env ?? process.env
  const loadRegistry = input.loadRegistry ?? (() => strictLoadRegistry(input.summary))
  const createExecutor = input.createExecutor ?? (request =>
    createGrokHeadlessWorkerExecutor({ env: request.env }))
  const materializeWorkspace = input.materializeWorkspace ?? defaultWorkspacePort(input)
  const runVerifier = input.runVerifier ?? defaultVerifier
  const workerTimeoutMs = boundedTimeout(
    input.workerTimeoutMs,
    DEFAULT_GROK_CLAIM_TIMEOUT_MS,
    MAX_GROK_CLAIM_TIMEOUT_MS,
  )
  const verifierTimeoutMs = boundedTimeout(
    input.verifierTimeoutMs,
    DEFAULT_GROK_VERIFY_TIMEOUT_MS,
    MAX_GROK_VERIFY_TIMEOUT_MS,
  )
  const lifecycleHeartbeatMs = boundedTimeout(
    input.lifecycleHeartbeatMs,
    GROK_LIFECYCLE_HEARTBEAT_MS,
    15_000,
  )
  const readNow = (): Date => {
    const value = input.now?.() ?? new Date()
    if (!validDate(value)) throw new Error("Grok claimed-work clock is invalid")
    return value
  }
  const safeNow = (): Date => {
    try {
      return readNow()
    } catch {
      return new Date(0)
    }
  }

  const dispatch = async (
    request: FleetRunSupervisorDispatchInput,
  ): Promise<PylonOwnedFleetRunDispatchResult> => {
    const now = safeNow()
    if (request.workerKind !== "grok") {
      return fixedResult({
        assignmentRef: null,
        blockerRef: PYLON_OWNED_GROK_RUNNER_BLOCKERS.dispatchConflict,
        now,
        status: "blocked",
        summary: "The Grok claimed-work port rejected a different worker kind.",
      })
    }
    let registry: readonly PylonAccountRegistryEntry[]
    try {
      registry = await loadRegistry()
    } catch {
      return fixedResult({
        assignmentRef: null,
        blockerRef: PYLON_OWNED_GROK_RUNNER_BLOCKERS.accountUnavailable,
        now,
        status: "blocked",
        summary: "The named Grok account registry is unavailable.",
      })
    }
    const account = await exactNamedGrokAccount(input.summary, registry, request.accountRef)
    if (account === null) {
      return fixedResult({
        assignmentRef: null,
        blockerRef: PYLON_OWNED_GROK_RUNNER_BLOCKERS.accountUnavailable,
        now,
        status: "blocked",
        summary: "The exact named Grok account is unavailable.",
      })
    }
    const accountRefHash = hashPylonAccountRef("grok", account.ref)
    const fingerprint = dispatchFingerprint(request)
    const assignmentRef = assignmentRefFor(fingerprint)
    const evidenceRefs = pylonGrokUsageEvidenceRefs(assignmentRef)
    const existing = await readReceipt(input.summary, assignmentRef)
    if (existing === "invalid") {
      return fixedResult({
        assignmentRef,
        accountRefHash,
        blockerRef: PYLON_OWNED_GROK_RUNNER_BLOCKERS.receiptInvalid,
        now,
        status: "failed",
        summary: "The durable Grok execution receipt is invalid.",
      })
    }
    if (existing !== null) {
      if (!receiptMatchesDispatch(existing, request, fingerprint, accountRefHash)) {
        return fixedResult({
          assignmentRef,
          accountRefHash,
          blockerRef: PYLON_OWNED_GROK_RUNNER_BLOCKERS.dispatchConflict,
          now,
          status: "failed",
          summary: "The Grok assignment ref has conflicting durable authority.",
        })
      }
      return resultFromReceipt(existing, now)
    }

    const selection: ResolvedPylonAccountSelection = {
      provider: "grok",
      selector: "registry_ref",
      accountRef: account.ref,
      accountRefHash,
      home: account.home,
      openAgentsProviderAccountRef: account.openAgentsProviderAccountRef,
    }
    const accountEnv = pylonAccountEnvironment(baseEnv, selection)
    if (
      normalizeAccountHome(accountEnv.GROK_HOME ?? "") !== normalizeAccountHome(account.home) ||
      Object.entries(accountEnv).some(([key, value]) =>
        typeof value === "string" && value.trim() !== "" &&
        (key === "XAI_API_KEY" || key === "GROK_CODE_XAI_API_KEY" || key.startsWith("GROK_AUTH"))
      )
    ) {
      return fixedResult({
        assignmentRef,
        accountRefHash,
        blockerRef: PYLON_OWNED_GROK_RUNNER_BLOCKERS.custodyInvalid,
        now,
        status: "blocked",
        summary: "The named Grok account environment failed isolated custody.",
      })
    }
    const executor = createExecutor({ account, env: accountEnv })
    let readiness: Awaited<ReturnType<GrokWorkerExecutorPort["readiness"]>>
    try {
      readiness = await executor.readiness()
    } catch {
      readiness = { ready: false, binary: "grok", plane: "unknown", models: [] }
    }
    if (!readiness.ready || readiness.plane !== "cli_session") {
      return fixedResult({
        assignmentRef,
        accountRefHash,
        blockerRef: PYLON_OWNED_GROK_RUNNER_BLOCKERS.readinessUnavailable,
        now,
        status: "blocked",
        summary: "The isolated named Grok CLI session is not ready.",
      })
    }

    let workspace: PylonOwnedGrokWorkspace
    try {
      workspace = await materializeWorkspace({ assignmentRef, dispatch: request, now })
      const relativeToAccountHome = relative(resolve(account.home), resolve(workspace.workingDirectory))
      if (
        relativeToAccountHome === "" ||
        (relativeToAccountHome !== ".." && !relativeToAccountHome.startsWith(`..${sep}`)) ||
        !workspace.workspaceRef.startsWith("workspace.")
      ) throw new Error("invalid Grok workspace")
    } catch {
      return fixedResult({
        assignmentRef,
        accountRefHash,
        blockerRef: PYLON_OWNED_GROK_RUNNER_BLOCKERS.workspaceInvalid,
        now,
        status: "failed",
        summary: "The pinned Grok workspace could not be materialized.",
      })
    }
    const verifierRef = stablePublicRef(
      "verifier.public.pylon.grok",
      workspace.verificationArgs === null
        ? `claimed_work:${assignmentRef}`
        : workspace.verificationArgs.join("\u0000"),
    )
    const verificationEvidenceRef = stablePublicRef(
      "verification.public.pylon.grok",
      `${assignmentRef}:${verifierRef}`,
    )
    const running: GrokExecutionReceiptV2 = {
      schema: GROK_RECEIPT_SCHEMA_V2,
      accountRefHash,
      assignmentRef,
      closeoutRef: evidenceRefs.closeoutRef,
      claimRef: request.claim.claimRef,
      failureRef: null,
      fingerprint,
      observedAt: now.toISOString(),
      runRef: request.run.runRef,
      receiptRef: evidenceRefs.receiptRef,
      state: "running",
      taskId: request.taskId,
      usageTruth: "not_measured",
      workUnitRef: request.workUnit.workUnitRef,
      workspaceRef: workspace.workspaceRef,
      executionPlane: "cli_session",
      marginalCostClass: account.marginalCostClass,
      wallClockMs: null,
      verification: {
        truth: "pending",
        verifierRef,
        evidenceRefs: [],
      },
      artifactRefs: [workspace.workspaceRef],
      proofRefs: [],
      authorityReceiptRefs: [request.claim.claimRef],
    }
    try {
      if (input.store !== undefined) {
        const current = input.store.getWorkClaim(request.claim.claimRef)
        if (
          current === null ||
          current.runRef !== request.run.runRef ||
          current.workUnitRef !== request.workUnit.workUnitRef ||
          current.workerAccountRef !== request.accountRef ||
          current.state !== "in_progress" ||
          (current.assignmentRef !== null && current.assignmentRef !== assignmentRef)
        ) throw new Error("canonical Grok claim changed before execution")
        // Persist the deterministic local assignment ref in the ONE canonical
        // claim registry before the CLI starts. A daemon restart can therefore
        // find the refs-only receipt instead of minting a second execution.
        input.store.updateWorkClaimAssignmentRef(request.claim.claimRef, assignmentRef, now)
      }
      await writeReceipt(input.summary, running)
    } catch {
      return fixedResult({
        assignmentRef,
        accountRefHash,
        blockerRef: PYLON_OWNED_GROK_RUNNER_BLOCKERS.receiptInvalid,
        now,
        status: "failed",
        summary: "The durable Grok execution receipt could not be written.",
      })
    }

    let failureRef: string | null = null
    let wallClockMs: number | null = null
    const emitExecutorLifecycle = async (
      event: "assignment_run.runtime_started" | "assignment_run.runtime_progress",
    ): Promise<void> => {
      if (request.onLifecycle === undefined) return
      try {
        await request.onLifecycle({
          schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
          event,
          observedAt: safeNow().toISOString(),
          assignmentRef,
          accountRefHash,
          status: "running",
          phase: "runtime_active",
        })
      } catch {
        // Projection is durable/fail-soft and cannot become execution authority.
      }
    }
    let lifecycleClosed = false
    let lifecycleTail = Promise.resolve<void>(undefined)
    const queueExecutorLifecycle = (
      event: "assignment_run.runtime_started" | "assignment_run.runtime_progress",
    ): Promise<void> => {
      const delivery = lifecycleTail.then(async () => {
        if (lifecycleClosed) return
        await emitExecutorLifecycle(event)
      })
      lifecycleTail = delivery.catch(() => undefined)
      return delivery
    }
    await queueExecutorLifecycle("assignment_run.runtime_started")
    const lifecycleHeartbeat = setInterval(() => {
      void queueExecutorLifecycle("assignment_run.runtime_progress")
    }, lifecycleHeartbeatMs)
    try {
      const closeout = await executor.runClaimedWork({
        pin: {
          claimRef: request.claim.claimRef,
          workUnitRef: request.workUnit.workUnitRef,
          runRef: request.run.runRef,
          accountRefHash,
          cwd: workspace.workingDirectory,
          ...(request.workUnit.repo === undefined ? {} : { repo: request.workUnit.repo }),
          ...(request.workUnit.baseCommit === undefined ? {} : { commit: request.workUnit.baseCommit }),
          ...(request.workUnit.branch === undefined ? {} : { branch: request.workUnit.branch }),
          ...(request.workUnit.verify === undefined ? {} : { verifyCommand: request.workUnit.verify }),
        },
        prompt: request.workUnit.body ?? request.run.objective,
        timeoutMs: workerTimeoutMs,
        plane: "cli_session",
        marginalCostClass: account.marginalCostClass,
      })
      if (
        !Number.isSafeInteger(closeout.usage.wallClockMs) ||
        closeout.usage.wallClockMs < 0
      ) {
        failureRef = PYLON_OWNED_GROK_RUNNER_BLOCKERS.executionFailed
      } else {
        wallClockMs = closeout.usage.wallClockMs
      }
      if (
        !closeout.ok ||
        closeout.claimRef !== request.claim.claimRef ||
        closeout.usage.metering !== "not_measured" ||
        closeout.usage.plane !== "cli_session" ||
        closeout.usage.marginalCostClass !== account.marginalCostClass
      ) {
        failureRef = failureRefForClass(closeout.failureClass)
      } else if (workspace.verificationArgs !== null) {
        try {
          const verification = await runVerifier({
            args: workspace.verificationArgs,
            cwd: workspace.workingDirectory,
            timeoutMs: verifierTimeoutMs,
          })
          if (verification.exitCode !== 0 || verification.timedOut) {
            failureRef = PYLON_OWNED_GROK_RUNNER_BLOCKERS.verificationFailed
          }
        } catch {
          failureRef = PYLON_OWNED_GROK_RUNNER_BLOCKERS.verificationFailed
        }
      }
    } catch {
      failureRef = PYLON_OWNED_GROK_RUNNER_BLOCKERS.executionFailed
    } finally {
      lifecycleClosed = true
      clearInterval(lifecycleHeartbeat)
      await lifecycleTail
    }
    const terminal: GrokExecutionReceiptV2 = {
      ...running,
      failureRef,
      observedAt: safeNow().toISOString(),
      state: failureRef === null ? "completed" : "failed",
      wallClockMs,
      verification: {
        truth: failureRef === null ? "passed" : "failed",
        verifierRef,
        evidenceRefs: [verificationEvidenceRef],
      },
      proofRefs: [evidenceRefs.receiptRef],
    }
    try {
      await writeReceipt(input.summary, terminal)
    } catch {
      return fixedResult({
        assignmentRef,
        accountRefHash,
        blockerRef: PYLON_OWNED_GROK_RUNNER_BLOCKERS.receiptInvalid,
        now: safeNow(),
        status: "failed",
        summary: "The durable Grok terminal receipt could not be written.",
      })
    }
    return resultFromReceipt(terminal, safeNow())
  }

  const reconcile = async (request: {
    readonly active: FleetRunSupervisorActiveAssignment
    readonly now: Date
    readonly runRef: string
  }): Promise<PylonOwnedFleetRunReconcileResult> => {
    const assignmentRef = request.active.claim.assignmentRef
    if (!isPylonOwnedGrokAssignmentRef(assignmentRef)) {
      return {
        ...fixedResult({
          assignmentRef: null,
          blockerRef: PYLON_OWNED_GROK_RUNNER_BLOCKERS.receiptInvalid,
          now: request.now,
          status: "failed",
          summary: "The durable Grok claim has no exact local assignment ref.",
        }),
        taskId: request.active.taskId,
      }
    }
    const receipt = await readReceipt(input.summary, assignmentRef)
    if (
      receipt === null ||
      receipt === "invalid" ||
      receipt.assignmentRef !== assignmentRef ||
      receipt.claimRef !== request.active.claim.claimRef ||
      receipt.runRef !== request.runRef ||
      receipt.taskId !== request.active.taskId ||
      receipt.accountRefHash !== hashPylonAccountRef("grok", request.active.accountRef)
    ) {
      return {
        ...fixedResult({
          assignmentRef,
          accountRefHash: hashPylonAccountRef("grok", request.active.accountRef),
          blockerRef: PYLON_OWNED_GROK_RUNNER_BLOCKERS.receiptInvalid,
          now: request.now,
          status: "failed",
          summary: "The durable Grok execution receipt does not match the active claim.",
        }),
        taskId: request.active.taskId,
      }
    }
    return { ...resultFromReceipt(receipt, request.now), taskId: request.active.taskId }
  }

  const probeLiveness: PylonOwnedGrokClaimedWorkPort["probeLiveness"] = async assignmentRef => {
    if (!isPylonOwnedGrokAssignmentRef(assignmentRef)) return "unknown"
    const receipt = await readReceipt(input.summary, assignmentRef)
    if (receipt === null || receipt === "invalid") return "unknown"
    // "live" here means "leave this durable task for supervisor reconcile",
    // not that a child PID is alive. Terminal refs-only receipts are the exact
    // recovery authority; a running receipt from a prior daemon is interrupted.
    return receipt.state === "running" ? "dead" : "live"
  }

  return { dispatch, reconcile, probeLiveness }
}
