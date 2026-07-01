import type { BootstrapSummary } from "./bootstrap.js"
import type { AssignmentClientOptions, AssignmentRunLifecycleEvent } from "./assignment.js"
import { runNoSpendAssignment } from "./assignment.js"
import type { PylonAccountsListProjection } from "./account-usage.js"
import type { TipsNetworkOptions } from "./tips.js"
import {
  issuePylonKhalaRequest,
  readPylonKhalaProof,
  type PylonKhalaGitCheckoutWorkspace,
  type PylonKhalaProofResult,
  type PylonKhalaRequestInput,
  type PylonKhalaRequestResult,
} from "./khala-requester.js"
import { assertPublicProjectionSafe } from "./state.js"

export const PYLON_KHALA_SPAWN_PLAN_SCHEMA = "openagents.pylon.khala_spawn_plan.v0.1"
export const PYLON_KHALA_SPAWN_RUN_SCHEMA = "openagents.pylon.khala_spawn_run.v0.1"
export const PYLON_KHALA_SPAWN_WORKER_EVENT_SCHEMA = "openagents.pylon.khala_spawn_worker_event.v0.1"

export type PylonKhalaSpawnWorkerState =
  | "queued"
  | "requesting"
  | "assignment_created"
  | "running"
  | "closeout_submitted"
  | "proof_checked"
  | "accepted"
  | "rejected"
  | "failed"
  | "cancelled"

export type PylonKhalaSpawnAccount = {
  accountRef: string | null
  accountRefHash: string
}

export type PylonKhalaSpawnAdvertisedCodexAccount = {
  accountKey: string
  accountRefHash: string
  available: number
  busy: number
  queued: number
  ready: number
}

export type PylonKhalaSpawnObjective = {
  objective: string
  objectiveRef: string
}

export type PylonKhalaSpawnCommands = {
  proof: string
  request: string
  runNoSpend: string
}

export type PylonKhalaSpawnSlot = {
  account: PylonKhalaSpawnAccount
  commands: PylonKhalaSpawnCommands
  objective: PylonKhalaSpawnObjective
  requestInput: PylonKhalaRequestInput
  slotIndex: number
}

export type PylonKhalaSpawnSlotLike = {
  account: PylonKhalaSpawnAccount
  requestInput: PylonKhalaRequestInput
  slotIndex: number
}

export type PylonKhalaSpawnPlan = {
  schema: typeof PYLON_KHALA_SPAWN_PLAN_SCHEMA
  advertisedCodexAccounts: readonly PylonKhalaSpawnAdvertisedCodexAccount[]
  advertisedCodexAvailability: number
  baseUrl: string
  blockerRefs: string[]
  maxParallel: number
  objectiveCount: number
  readyCodexAccountCount: number
  requestedCount: number
  slots: readonly PylonKhalaSpawnSlot[]
  targetPylonRef: string
}

export type PylonKhalaSpawnPlanLike<Slot extends PylonKhalaSpawnSlotLike = PylonKhalaSpawnSlot> =
  Omit<PylonKhalaSpawnPlan, "slots"> & {
    slots: readonly Slot[]
  }

export type PylonKhalaSpawnWorkerEvent = {
  schema: typeof PYLON_KHALA_SPAWN_WORKER_EVENT_SCHEMA
  assignmentEvent?: AssignmentRunLifecycleEvent["event"]
  assignmentRef?: string
  closeoutRef?: string
  leaseRef?: string
  message: string
  observedAt: string
  slotIndex: number
  state: PylonKhalaSpawnWorkerState
  status?: string
}

export type PylonKhalaSpawnProofProjection = {
  cacheReadTokens: number
  demandKind: PylonKhalaProofResult["tokenUsage"]["demandKind"]
  demandSource: PylonKhalaProofResult["tokenUsage"]["demandSource"]
  inputTokens: number
  model: PylonKhalaProofResult["tokenUsage"]["model"]
  outputTokens: number
  provider: PylonKhalaProofResult["tokenUsage"]["provider"]
  rawEventCount: number
  rawEventRows: number
  reasoningTokens: number
  tokenRows: number
  totalTokens: number
  traceCount: number
  usageTruth: PylonKhalaProofResult["tokenUsage"]["usageTruth"]
}

export type PylonKhalaSpawnSlotResult = {
  accountRefHash: string
  assignmentRef: string | null
  blockerRefs: string[]
  closeoutStatus: string | null
  durableRequestId: string | null
  failure: {
    message: string
    phase: PylonKhalaSpawnWorkerState
    ref: string
  } | null
  lifecycleEvents: PylonKhalaSpawnWorkerEvent[]
  ok: boolean
  proof: PylonKhalaSpawnProofProjection | null
  runAccepted: boolean | null
  slotIndex: number
  state: PylonKhalaSpawnWorkerState
}

export type PylonKhalaSpawnCounterEvidence = {
  after: number | null
  before: number | null
  blockerRefs: string[]
  delta: number | null
  expectedMinimumDelta: number
  state: "increment_observed" | "not_checked" | "unchanged" | "unavailable"
}

export type PylonKhalaSpawnAggregate = {
  acceptedCount: number
  assignmentRefs: string[]
  closeoutAcceptedCount: number
  durableRequestIds: string[]
  rejectedWithVerifiedTokensCount: number
  failedWithVerifiedTokensCount: number
  ownerOnlyRawEventCount: number
  ownerOnlyTraceCount: number
  totalTokenRows: number
  totalVerifiedTokens: number
  verifiedTokenAssignmentCount: number
}

export type PylonKhalaSpawnRunResult = {
  schema: typeof PYLON_KHALA_SPAWN_RUN_SCHEMA
  aggregate: PylonKhalaSpawnAggregate
  blockerRefs: string[]
  counter: PylonKhalaSpawnCounterEvidence
  ok: boolean
  plan: PylonKhalaSpawnPlan
  results: PylonKhalaSpawnSlotResult[]
}

export type PylonKhalaSpawnRunResultForPlan<Slot extends PylonKhalaSpawnSlotLike = PylonKhalaSpawnSlot> =
  Omit<PylonKhalaSpawnRunResult, "plan"> & {
    plan: PylonKhalaSpawnPlanLike<Slot>
  }

export type PylonKhalaSpawnAssignmentRunResult = {
  acceptance?: { blockerRefs?: readonly string[] }
  closeout?: { blockerRefs?: readonly string[]; status?: string }
  ok: boolean
}

export type PylonKhalaSpawnRunDeps<Slot extends PylonKhalaSpawnSlotLike = PylonKhalaSpawnSlot> = {
  onWorkerLifecycle?: (event: PylonKhalaSpawnWorkerEvent, slot: Slot) => void | Promise<void>
  readProof?: (
    network: TipsNetworkOptions,
    assignmentRef: string,
    slot: Slot,
  ) => Promise<PylonKhalaProofResult>
  readTokensServed?: (network: TipsNetworkOptions) => Promise<number | null>
  sleep?: (ms: number) => Promise<void>
  requestAssignment?: (
    network: TipsNetworkOptions,
    input: PylonKhalaRequestInput,
    slot: Slot,
  ) => Promise<PylonKhalaRequestResult>
  runAssignment?: (
    summary: BootstrapSummary,
    options: AssignmentClientOptions,
    slot: Slot,
  ) => Promise<PylonKhalaSpawnAssignmentRunResult>
}

const quoteArg = (value: string): string => JSON.stringify(value)

const accountCommandArgs = (account: PylonKhalaSpawnAccount): string =>
  account.accountRef === null ? "" : ` --account ${quoteArg(account.accountRef)}`

const requestAccountArgs = (account: PylonKhalaSpawnAccount): string[] =>
  account.accountRef === null ? [] : [`--account-ref ${quoteArg(account.accountRef)}`]

const blocker = (namespace: string, suffix: string) => `blocker.${namespace}.${suffix}`
const failureRef = (suffix: string) => `failure.khala_spawn.${suffix}`

const defaultProofRetryDelaysMs = [500, 1_500, 3_000] as const
const defaultProofBackfillRetryDelaysMs = [5_000, 10_000, 20_000, 30_000] as const
const defaultAssignment409RetryDelaysMs = [750, 2_000, 4_000] as const

const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms))

function nonNegativeInteger(value: number | undefined, fallback: number): number {
  if (value === undefined || !Number.isFinite(value)) return fallback
  return Math.max(0, Math.floor(value))
}

export function readyCodexAccounts(
  accounts: PylonAccountsListProjection,
): PylonKhalaSpawnAccount[] {
  return accounts.accounts
    .filter((account) =>
      account.provider === "codex" &&
      account.homeState === "present" &&
      account.readiness.state === "ready" &&
      account.blockerRefs.length === 0
    )
    .map((account) => ({
      accountRef: account.accountRef,
      accountRefHash: account.accountRefHash,
    }))
}

export function repeatedKhalaSpawnObjectives(input: {
  count: number
  objective: string
}): PylonKhalaSpawnObjective[] {
  const count = Math.max(1, Math.floor(input.count))
  return Array.from({ length: count }, (_, index) => ({
    objective: [
      `Worker ${index + 1}/${count}.`,
      input.objective.trim(),
      "Work independently and return a concise public-safe closeout with evidence refs, blockers, and next step.",
    ].join(" "),
    objectiveRef: `objective.khala_spawn.${String(index + 1).padStart(2, "0")}`,
  }))
}

const KHALA_REQUEST_OBJECTIVE_SUMMARY_MAX_LENGTH = 1_000

function objectiveSummaryForKhalaRequest(objective: string): string {
  const trimmed = objective.trim()
  if (trimmed.length <= KHALA_REQUEST_OBJECTIVE_SUMMARY_MAX_LENGTH) return trimmed
  return `${trimmed.slice(0, KHALA_REQUEST_OBJECTIVE_SUMMARY_MAX_LENGTH - 3).trimEnd()}...`
}

export function buildPylonKhalaSpawnPlan(input: {
  accounts: PylonAccountsListProjection
  advertisedCodexAccounts?: readonly PylonKhalaSpawnAdvertisedCodexAccount[]
  advertisedCodexAvailability?: number
  baseUrl: string
  branch?: string
  commit?: string
  fixture?: boolean
  maxParallel?: number
  objectives: readonly PylonKhalaSpawnObjective[]
  repository?: string
  targetPylonRef: string
  verificationCommand?: string
  workspace?: PylonKhalaGitCheckoutWorkspace
}): PylonKhalaSpawnPlan {
  const readyAccounts = readyCodexAccounts(input.accounts)
  const advertisedCodexAccounts = (input.advertisedCodexAccounts ?? [])
    .map((account) => ({
      accountKey: account.accountKey,
      accountRefHash: account.accountRefHash,
      available: nonNegativeInteger(account.available, 0),
      busy: nonNegativeInteger(account.busy, 0),
      queued: nonNegativeInteger(account.queued, 0),
      ready: nonNegativeInteger(account.ready, 0),
    }))
  const advertisedAccountCapacity = new Map(
    advertisedCodexAccounts.map((account) => [account.accountRefHash, account]),
  )
  const accounts = advertisedCodexAccounts.length === 0
    ? readyAccounts
    : readyAccounts.filter((account) => (advertisedAccountCapacity.get(account.accountRefHash)?.available ?? 0) > 0)
  const advertisedCodexAvailability = nonNegativeInteger(
    input.advertisedCodexAvailability,
    advertisedCodexAccounts.length === 0
      ? accounts.length
      : advertisedCodexAccounts.reduce((sum, account) => sum + account.available, 0),
  )
  const requestedCount = input.objectives.length
  const requestedExceedsAdvertisedAvailability =
    requestedCount > 0 &&
    advertisedCodexAvailability > 0 &&
    requestedCount > advertisedCodexAvailability
  const defaultMaxParallel = Math.max(
    1,
    Math.min(
      requestedCount || 1,
      Math.max(accounts.length, advertisedCodexAvailability),
    ),
  )
  const requestedMaxParallel = Math.max(1, Math.floor(input.maxParallel ?? defaultMaxParallel))
  const selectedParallel =
    accounts.length === 0 ||
    advertisedCodexAvailability === 0 ||
    requestedCount === 0 ||
    requestedExceedsAdvertisedAvailability
      ? 0
      : Math.min(requestedMaxParallel, advertisedCodexAvailability, requestedCount)
  const selectedAccounts = weightedKhalaAccountPool(
    accounts,
    advertisedAccountCapacity,
    Math.min(accounts.length, Math.max(1, selectedParallel)),
  )
  const blockerRefs = [
    ...(readyAccounts.length === 0 ? [blocker("khala_spawn", "no_ready_codex_accounts")] : []),
    ...(readyAccounts.length > 0 && accounts.length === 0
      ? [blocker("khala_spawn", "no_ready_codex_account_slots")]
      : []),
    ...(advertisedCodexAvailability === 0 ? [blocker("khala_spawn", "no_advertised_codex_availability")] : []),
    ...(requestedExceedsAdvertisedAvailability
      ? [blocker("khala_spawn", "requested_count_exceeds_advertised_availability")]
      : []),
    ...(requestedCount === 0 ? [blocker("khala_spawn", "no_objectives")] : []),
  ]

  const slots: PylonKhalaSpawnSlot[] = selectedParallel === 0
    ? []
    : input.objectives.map((objective, index) => {
        const account = selectedAccounts[index % selectedAccounts.length]!
        const requestInput: PylonKhalaRequestInput = {
          objectiveSummary: objectiveSummaryForKhalaRequest(objective.objective),
          prompt: objective.objective,
          targetAccountRefHash: account.accountRefHash,
          targetPylonRef: input.targetPylonRef,
          workflow: "codex_agent_task",
          ...(input.workspace === undefined ? {} : { workspace: input.workspace }),
        }
        const workspaceArgs = input.workspace === undefined || input.fixture === true
          ? ["--fixture"]
          : [
              `--repo ${quoteArg(input.repository ?? input.workspace.repository.fullName)}`,
              `--commit ${quoteArg(input.commit ?? input.workspace.repository.commitSha)}`,
              `--verify ${quoteArg(input.verificationCommand ?? input.workspace.verificationCommand.args.join(" "))}`,
              `--branch ${quoteArg(input.branch ?? input.workspace.repository.branch)}`,
            ]
        return {
          account,
          commands: {
            proof: "pylon khala proof --assignment-ref <assignmentRef> --json",
            request: [
              "pylon khala request",
              "--workflow codex_agent_task",
              `--pylon-ref ${quoteArg(input.targetPylonRef)}`,
              ...requestAccountArgs(account),
              `--prompt ${quoteArg(objective.objective)}`,
              ...workspaceArgs,
              "--json",
            ].join(" "),
            runNoSpend: `pylon assignment run-no-spend --base-url ${quoteArg(input.baseUrl)}${accountCommandArgs(account)} --assignment-ref <assignmentRef> --lifecycle-ndjson --json`,
          },
          objective,
          requestInput,
          slotIndex: index,
        }
      })

  const plan: PylonKhalaSpawnPlan = {
    schema: PYLON_KHALA_SPAWN_PLAN_SCHEMA,
    advertisedCodexAccounts,
    advertisedCodexAvailability,
    baseUrl: input.baseUrl,
    blockerRefs,
    maxParallel: selectedParallel,
    objectiveCount: requestedCount,
    readyCodexAccountCount: readyAccounts.length,
    requestedCount,
    slots,
    targetPylonRef: input.targetPylonRef,
  }
  assertPublicProjectionSafe(plan)
  return plan
}

export function weightedKhalaAccountPool<Account extends { accountRefHash: string }>(
  accounts: readonly Account[],
  advertisedAccountCapacity: ReadonlyMap<string, Pick<PylonKhalaSpawnAdvertisedCodexAccount, "available">>,
  maxDistinctAccounts: number,
): Account[] {
  const selected = accounts.slice(0, Math.max(0, maxDistinctAccounts))
  if (advertisedAccountCapacity.size === 0) return selected
  const remaining = selected
    .map((account) => ({
      account,
      remaining: nonNegativeInteger(advertisedAccountCapacity.get(account.accountRefHash)?.available, 0),
    }))
    .filter((slot) => slot.remaining > 0)
  const pool: Account[] = []
  while (remaining.some((slot) => slot.remaining > 0)) {
    for (const slot of remaining) {
      if (slot.remaining <= 0) continue
      pool.push(slot.account)
      slot.remaining -= 1
    }
  }
  return pool.length === 0 ? selected : pool
}

export async function readPublicKhalaTokensServed(network: TipsNetworkOptions): Promise<number | null> {
  const fetcher = network.fetch ?? fetch
  const response = await fetcher(new URL("/api/public/khala-tokens-served", network.baseUrl))
  if (!response.ok) return null
  const body = (await response.json()) as { tokensServed?: unknown }
  return typeof body.tokensServed === "number" && Number.isFinite(body.tokensServed)
    ? body.tokensServed
    : null
}

export function pylonKhalaProofProjection(proof: PylonKhalaProofResult): PylonKhalaSpawnProofProjection {
  return {
    cacheReadTokens: proof.tokenUsage.cacheReadTokens,
    demandKind: proof.tokenUsage.demandKind,
    demandSource: proof.tokenUsage.demandSource,
    inputTokens: proof.tokenUsage.inputTokens,
    model: proof.tokenUsage.model,
    outputTokens: proof.tokenUsage.outputTokens,
    provider: proof.tokenUsage.provider,
    rawEventCount: proof.rawEvents.eventCount,
    rawEventRows: proof.rawEvents.count,
    reasoningTokens: proof.tokenUsage.reasoningTokens,
    tokenRows: proof.tokenUsage.rowCount,
    totalTokens: proof.tokenUsage.totalTokens,
    traceCount: proof.traces.count,
    usageTruth: proof.tokenUsage.usageTruth,
  }
}

function proofBlockerRefs(
  proof: PylonKhalaSpawnProofProjection,
  namespace: string,
): string[] {
  const exactOwnCapacity =
    proof.provider === "pylon-codex-own-capacity" &&
    proof.model === "openagents/pylon-codex" &&
    proof.usageTruth === "exact" &&
    proof.demandKind === "own_capacity" &&
    proof.demandSource === "khala_coding_delegation"
  return [
    ...(!exactOwnCapacity ? [blocker(namespace, "proof_not_exact_own_capacity")] : []),
    ...(proof.tokenRows <= 0 || proof.totalTokens <= 0 ? [blocker(namespace, "proof_token_rows_missing")] : []),
    ...(proof.traceCount <= 0 ? [blocker(namespace, "owner_trace_missing")] : []),
    ...(proof.rawEventCount <= 0 ? [blocker(namespace, "raw_events_missing")] : []),
  ]
}

function spawnFailureProjection(input: {
  error: unknown
  namespace: string
  phase: PylonKhalaSpawnWorkerState
}) {
  const raw = input.error instanceof Error ? input.error.message : String(input.error)
  if (/\bcontains private, payment, credential, wallet, or raw material\b/iu.test(raw)) {
    return {
      blockerRef: blocker(input.namespace, "request_public_safety_blocked"),
      failure: {
        message: "worker request was blocked by the public-safety guard",
        phase: input.phase,
        ref: failureRef("request_public_safety_blocked"),
      },
    }
  }
  if (/\b(?:timed out|timeout|AbortError)\b/iu.test(raw)) {
    return {
      blockerRef: blocker(input.namespace, "slot_timeout"),
      failure: {
        message: "worker failed because a bounded operation timed out",
        phase: input.phase,
        ref: failureRef("timeout"),
      },
    }
  }
  const httpStatus = /\((\d{3})\)/u.exec(raw)?.[1]
  if (httpStatus !== undefined) {
    return {
      blockerRef: blocker(input.namespace, `slot_http_${httpStatus}`),
      failure: {
        message: `worker failed because the OpenAgents API returned HTTP ${httpStatus}`,
        phase: input.phase,
        ref: failureRef(`http_${httpStatus}`),
      },
    }
  }
  if (/\bproof\b/iu.test(raw)) {
    return {
      blockerRef: blocker(input.namespace, "proof_unavailable"),
      failure: {
        message: "worker closeout finished but proof was not readable in time",
        phase: input.phase,
        ref: failureRef("proof_unavailable"),
      },
    }
  }
  return {
    blockerRef: blocker(input.namespace, "slot_failed"),
    failure: {
      message: "worker failed with a public-safe internal error",
      phase: input.phase,
      ref: failureRef("internal"),
    },
  }
}

function isHttp409AssignmentConflict(error: unknown): boolean {
  const raw = error instanceof Error ? error.message : String(error)
  const status = /\((\d{3})\)/u.exec(raw)?.[1]
  return status === "409" || /\bHTTP\s+409\b/iu.test(raw)
}

async function requestAssignmentWith409Retry<Slot extends PylonKhalaSpawnSlotLike>(input: {
  emit: (
    state: PylonKhalaSpawnWorkerState,
    message: string,
    patch?: Partial<PylonKhalaSpawnWorkerEvent>,
  ) => Promise<void>
  network: TipsNetworkOptions
  requestAssignment: NonNullable<PylonKhalaSpawnRunDeps<Slot>["requestAssignment"]>
  sleep: (ms: number) => Promise<void>
  slot: Slot
}): Promise<{ request: PylonKhalaRequestResult; retried409: boolean }> {
  let last409: unknown
  for (let attempt = 0; attempt <= defaultAssignment409RetryDelaysMs.length; attempt += 1) {
    try {
      const request = await input.requestAssignment(input.network, input.slot.requestInput, input.slot)
      if (attempt > 0) {
        await input.emit("requesting", "Khala assignment creation recovered after transient HTTP 409", {
          status: "retry.khala_spawn.assignment_http_409_recovered",
        })
      }
      return { request, retried409: attempt > 0 }
    } catch (error) {
      if (!isHttp409AssignmentConflict(error)) throw error
      last409 = error
      const delay = defaultAssignment409RetryDelaysMs[attempt]
      if (delay === undefined) break
      await input.emit("requesting", "Khala assignment creation hit transient HTTP 409; retrying", {
        status: "retry.khala_spawn.assignment_http_409",
      })
      await input.sleep(delay)
    }
  }
  await input.emit("requesting", "Khala assignment creation exhausted HTTP 409 retries", {
    status: "failure.khala_spawn.assignment_http_409_retry_exhausted",
  })
  throw last409 ?? new Error("pylon khala request failed (409): assignment creation retry exhausted")
}

async function readProofWithRetry<Slot extends PylonKhalaSpawnSlotLike>(input: {
  delaysMs?: readonly number[]
  network: TipsNetworkOptions
  readProof: NonNullable<PylonKhalaSpawnRunDeps<Slot>["readProof"]>
  sleep: (ms: number) => Promise<void>
  slot: Slot
  assignmentRef: string
}): Promise<PylonKhalaProofResult> {
  let lastError: unknown
  const delaysMs = input.delaysMs ?? defaultProofRetryDelaysMs
  for (let attempt = 0; attempt <= delaysMs.length; attempt += 1) {
    try {
      return await input.readProof(input.network, input.assignmentRef, input.slot)
    } catch (error) {
      lastError = error
      const delay = delaysMs[attempt]
      if (delay === undefined) break
      await input.sleep(delay)
    }
  }
  throw lastError
}

async function backfillMissingProofs<Slot extends PylonKhalaSpawnSlotLike>(input: {
  deps: PylonKhalaSpawnRunDeps<Slot> | undefined
  namespace: string
  network: TipsNetworkOptions
  readProof: NonNullable<PylonKhalaSpawnRunDeps<Slot>["readProof"]>
  results: readonly PylonKhalaSpawnSlotResult[]
  sleep: (ms: number) => Promise<void>
  slots: readonly Slot[]
}): Promise<PylonKhalaSpawnSlotResult[]> {
  return Promise.all(input.results.map(async (result, index) => {
    if (result.proof !== null || result.assignmentRef === null) return result
    if (result.failure !== null && result.failure.ref !== failureRef("proof_unavailable")) {
      return result
    }
    const slot = input.slots[result.slotIndex] ?? input.slots[index]
    if (slot === undefined) return result
    try {
      const proof = pylonKhalaProofProjection(
        await readProofWithRetry({
          assignmentRef: result.assignmentRef,
          delaysMs: defaultProofBackfillRetryDelaysMs,
          network: input.network,
          readProof: input.readProof,
          sleep: input.sleep,
          slot,
        }),
      )
      const recoveredProofUnavailable = result.failure?.ref === failureRef("proof_unavailable")
      const proofBlockers = proofBlockerRefs(proof, input.namespace)
      const blockerRefs = [
        ...result.blockerRefs.filter((ref) => ref !== blocker(input.namespace, "proof_unavailable")),
        ...proofBlockers,
      ]
      const event: PylonKhalaSpawnWorkerEvent = {
        schema: PYLON_KHALA_SPAWN_WORKER_EVENT_SCHEMA,
        assignmentRef: result.assignmentRef,
        message: "assignment proof backfilled",
        observedAt: new Date().toISOString(),
        slotIndex: result.slotIndex,
        state: "proof_checked",
        status: "proof.khala_spawn.backfilled",
      }
      assertPublicProjectionSafe(event)
      await input.deps?.onWorkerLifecycle?.(event, slot)
      const state =
        proofBlockers.length > 0
          ? "failed"
          : result.runAccepted === true && blockerRefs.length === 0
            ? "accepted"
            : result.runAccepted === false && result.state === "failed"
              ? "rejected"
              : result.state
      return {
        ...result,
        blockerRefs,
        failure: recoveredProofUnavailable ? null : result.failure,
        lifecycleEvents: [...result.lifecycleEvents, event],
        ok: blockerRefs.length === 0,
        proof,
        state,
      }
    } catch {
      return result
    }
  }))
}

export async function runPylonKhalaSpawnPlan<Slot extends PylonKhalaSpawnSlotLike>(input: {
  blockerNamespace?: string
  deps?: PylonKhalaSpawnRunDeps<Slot>
  network: TipsNetworkOptions
  plan: PylonKhalaSpawnPlanLike<Slot>
  summary: BootstrapSummary
}): Promise<PylonKhalaSpawnRunResultForPlan<Slot>> {
  const namespace = input.blockerNamespace ?? "khala_spawn"
  const requestAssignment = input.deps?.requestAssignment ?? issuePylonKhalaRequest
  const runAssignment = input.deps?.runAssignment ?? runNoSpendAssignment
  const readProof = input.deps?.readProof ?? readPylonKhalaProof
  const readTokensServed = input.deps?.readTokensServed ?? readPublicKhalaTokensServed
  const counterBefore = await readTokensServed(input.network).catch(() => null)
  const results: PylonKhalaSpawnSlotResult[] = new Array(input.plan.slots.length)
  let nextIndex = 0

  const runNext = async (): Promise<void> => {
    while (nextIndex < input.plan.slots.length) {
      const slot = input.plan.slots[nextIndex]
      const resultIndex = nextIndex
      nextIndex += 1
      if (slot === undefined) return
      results[resultIndex] = await runPylonKhalaSpawnSlot({
        deps: input.deps,
        namespace,
        network: input.network,
        readProof,
        requestAssignment,
        runAssignment,
        slot,
        summary: input.summary,
      })
    }
  }

  const laneCount = Math.min(input.plan.maxParallel, input.plan.slots.length)
  await Promise.all(Array.from({ length: laneCount }, () => runNext()))
  const completedResults = await backfillMissingProofs({
    deps: input.deps,
    namespace,
    network: input.network,
    readProof,
    results: results.filter((result): result is PylonKhalaSpawnSlotResult => result !== undefined),
    sleep: input.deps?.sleep ?? sleep,
    slots: input.plan.slots,
  })
  const aggregate = aggregateSpawnResults(completedResults)
  const counterAfter = aggregate.totalVerifiedTokens > 0
    ? await readTokensServed(input.network).catch(() => null)
    : counterBefore
  const counterDelta = counterBefore === null || counterAfter === null
    ? null
    : counterAfter - counterBefore
  const counterBlockerRefs =
    aggregate.totalVerifiedTokens > 0 &&
    counterBefore !== null &&
    counterAfter !== null &&
    counterAfter <= counterBefore
      ? [blocker(namespace, "counter_not_incremented")]
      : []
  const counter: PylonKhalaSpawnCounterEvidence = {
    after: counterAfter,
    before: counterBefore,
    blockerRefs: counterBlockerRefs,
    delta: counterDelta,
    expectedMinimumDelta: aggregate.totalVerifiedTokens,
    state: aggregate.totalVerifiedTokens <= 0
      ? "not_checked"
      : counterBefore === null || counterAfter === null
        ? "unavailable"
        : counterAfter > counterBefore
          ? "increment_observed"
          : "unchanged",
  }
  const blockerRefs = [
    ...input.plan.blockerRefs,
    ...completedResults.flatMap((result) => result.blockerRefs),
    ...counterBlockerRefs,
  ]
  const run: PylonKhalaSpawnRunResultForPlan<Slot> = {
    schema: PYLON_KHALA_SPAWN_RUN_SCHEMA,
    aggregate,
    blockerRefs,
    counter,
    ok: blockerRefs.length === 0 && completedResults.length > 0,
    plan: input.plan,
    results: completedResults,
  }
  assertPublicProjectionSafe(run)
  return run
}

async function runPylonKhalaSpawnSlot<Slot extends PylonKhalaSpawnSlotLike>(input: {
  deps: PylonKhalaSpawnRunDeps<Slot> | undefined
  namespace: string
  network: TipsNetworkOptions
  readProof: NonNullable<PylonKhalaSpawnRunDeps<Slot>["readProof"]>
  requestAssignment: NonNullable<PylonKhalaSpawnRunDeps<Slot>["requestAssignment"]>
  runAssignment: NonNullable<PylonKhalaSpawnRunDeps<Slot>["runAssignment"]>
  slot: Slot
  summary: BootstrapSummary
}): Promise<PylonKhalaSpawnSlotResult> {
  const events: PylonKhalaSpawnWorkerEvent[] = []
  const emit = async (
    state: PylonKhalaSpawnWorkerState,
    message: string,
    patch: Partial<PylonKhalaSpawnWorkerEvent> = {},
  ) => {
    const event: PylonKhalaSpawnWorkerEvent = {
      schema: PYLON_KHALA_SPAWN_WORKER_EVENT_SCHEMA,
      message,
      observedAt: new Date().toISOString(),
      slotIndex: input.slot.slotIndex,
      state,
      ...patch,
    }
    assertPublicProjectionSafe(event)
    events.push(event)
    await input.deps?.onWorkerLifecycle?.(event, input.slot)
  }

  const blockerRefs: string[] = []
  let assignmentRef: string | null = null
  let durableRequestId: string | null = null
  let proof: PylonKhalaSpawnProofProjection | null = null
  let runAccepted: boolean | null = null
  let closeoutStatus: string | null = null
  let failure: PylonKhalaSpawnSlotResult["failure"] = null
  let state: PylonKhalaSpawnWorkerState = "queued"
  const pause = input.deps?.sleep ?? sleep

  try {
    await emit("queued", "worker queued")
    state = "requesting"
    await emit("requesting", "requesting Khala durable assignment")
    const { request, retried409 } = await requestAssignmentWith409Retry({
      emit,
      network: input.network,
      requestAssignment: input.requestAssignment,
      sleep: pause,
      slot: input.slot,
    })
    assignmentRef = request.assignmentRef
    durableRequestId = request.durableRequestId
    if (assignmentRef === null) {
      blockerRefs.push(blocker(input.namespace, "assignment_ref_missing"))
      state = "failed"
      await emit("failed", "Khala request did not return an assignment ref")
    } else {
      state = "assignment_created"
      await emit("assignment_created", "Khala assignment ref created", {
        assignmentRef,
        ...(retried409 ? { status: "retry.khala_spawn.assignment_http_409_succeeded" } : {}),
      })
      const run = await input.runAssignment(
        input.summary,
        {
          ...(input.network.agentToken === undefined ? {} : { agentToken: input.network.agentToken }),
          ...(input.slot.account.accountRef === null ? {} : { accountRef: input.slot.account.accountRef }),
          assignmentRef,
          baseUrl: input.network.baseUrl,
          ...(input.network.fetch === undefined ? {} : { fetch: input.network.fetch }),
          onLifecycleEvent: async (assignmentEvent) => {
            const mapped = lifecycleState(assignmentEvent)
            if (mapped !== null) {
              await emit(mapped, "assignment lifecycle event", {
                assignmentEvent: assignmentEvent.event,
                ...(assignmentEvent.assignmentRef === undefined ? {} : { assignmentRef: assignmentEvent.assignmentRef }),
                ...(assignmentEvent.closeoutRef === undefined ? {} : { closeoutRef: assignmentEvent.closeoutRef }),
                ...(assignmentEvent.leaseRef === undefined ? {} : { leaseRef: assignmentEvent.leaseRef }),
                ...(assignmentEvent.status === undefined ? {} : { status: assignmentEvent.status }),
              })
            }
          },
        },
        input.slot,
      )
      closeoutStatus = run.closeout?.status ?? null
      runAccepted = run.ok === true || run.closeout?.status === "accepted"
      if (!runAccepted) {
        const projectedProof = await readProofWithRetry({
          assignmentRef,
          network: input.network,
          readProof: input.readProof,
          sleep: pause,
          slot: input.slot,
        }).then(pylonKhalaProofProjection, () => null)
        if (projectedProof !== null) {
          proof = projectedProof
          blockerRefs.push(...proofBlockerRefs(projectedProof, input.namespace))
          await emit("proof_checked", "assignment proof checked", { assignmentRef })
        }
        blockerRefs.push(
          ...(run.closeout?.blockerRefs ?? run.acceptance?.blockerRefs ?? [blocker(input.namespace, "assignment_not_accepted")]),
        )
        state = closeoutStatus === "cancelled" ? "cancelled" : "rejected"
        await emit(state, "assignment did not close as accepted", {
          assignmentRef,
          ...(closeoutStatus === null ? {} : { status: closeoutStatus }),
        })
      } else {
        const projectedProof = pylonKhalaProofProjection(
          await readProofWithRetry({
            assignmentRef,
            network: input.network,
            readProof: input.readProof,
            sleep: pause,
            slot: input.slot,
          }),
        )
        proof = projectedProof
        blockerRefs.push(...proofBlockerRefs(projectedProof, input.namespace))
        state = blockerRefs.length === 0 ? "accepted" : "failed"
        await emit("proof_checked", "assignment proof checked", { assignmentRef })
        await emit(state, state === "accepted" ? "worker accepted" : "worker proof failed", { assignmentRef })
      }
    }
  } catch (error) {
    const projected = spawnFailureProjection({
      error,
      namespace: input.namespace,
      phase: events.at(-1)?.state ?? state,
    })
    failure = projected.failure
    blockerRefs.push(projected.blockerRef)
    state = "failed"
    await emit("failed", failure.message, { status: failure.ref })
  }

  return {
    accountRefHash: input.slot.account.accountRefHash,
    assignmentRef,
    blockerRefs,
    closeoutStatus,
    durableRequestId,
    failure,
    lifecycleEvents: events,
    ok: blockerRefs.length === 0,
    proof,
    runAccepted,
    slotIndex: input.slot.slotIndex,
    state,
  }
}

function lifecycleState(event: AssignmentRunLifecycleEvent): PylonKhalaSpawnWorkerState | null {
  if (event.event === "assignment_run.runtime_started" || event.event === "assignment_run.runtime_progress") {
    return "running"
  }
  if (event.event === "assignment_run.closeout_submitted") return "closeout_submitted"
  if (event.event === "assignment_run.completed") {
    if (event.status === "accepted") return "accepted"
    if (event.status === "cancelled") return "cancelled"
    return "rejected"
  }
  return null
}

function aggregateSpawnResults(results: readonly PylonKhalaSpawnSlotResult[]): PylonKhalaSpawnAggregate {
  const proofs = results.flatMap((result) => result.proof === null ? [] : [result.proof])
  const resultsWithVerifiedTokens = results.filter((result) =>
    (result.proof?.totalTokens ?? 0) > 0
  )
  return {
    acceptedCount: results.filter((result) => result.ok && result.runAccepted).length,
    assignmentRefs: results.flatMap((result) => result.assignmentRef === null ? [] : [result.assignmentRef]),
    closeoutAcceptedCount: results.filter((result) => result.runAccepted === true).length,
    durableRequestIds: results.flatMap((result) => result.durableRequestId === null ? [] : [result.durableRequestId]),
    failedWithVerifiedTokensCount: resultsWithVerifiedTokens.filter((result) => result.state === "failed").length,
    ownerOnlyRawEventCount: proofs.reduce((sum, proof) => sum + proof.rawEventCount, 0),
    ownerOnlyTraceCount: proofs.reduce((sum, proof) => sum + proof.traceCount, 0),
    rejectedWithVerifiedTokensCount: resultsWithVerifiedTokens.filter((result) => result.state === "rejected").length,
    totalTokenRows: proofs.reduce((sum, proof) => sum + proof.tokenRows, 0),
    totalVerifiedTokens: proofs.reduce((sum, proof) => sum + proof.totalTokens, 0),
    verifiedTokenAssignmentCount: resultsWithVerifiedTokens.length,
  }
}
