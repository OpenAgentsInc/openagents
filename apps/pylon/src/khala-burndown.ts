import { readFile } from "node:fs/promises"
import type { BootstrapSummary } from "./bootstrap.js"
import type { TipsNetworkOptions } from "./tips.js"
import {
  buildPylonKhalaGitCheckoutWorkspace,
  issuePylonKhalaRequest,
  type PylonKhalaProofResult,
  type PylonKhalaRequestInput,
  type PylonKhalaRequestResult,
} from "./khala-requester.js"
import {
  runNoSpendAssignment,
  type AssignmentClientOptions,
} from "./assignment.js"
import {
  assertPublicProjectionSafe,
  type PylonLocalState,
} from "./state.js"
import type { PylonAccountsListProjection } from "./account-usage.js"
import {
  PYLON_KHALA_SPAWN_PLAN_SCHEMA,
  readPublicKhalaTokensServed,
  pylonKhalaSpawnWorkflowForWorkerKind,
  readyKhalaSpawnAccounts,
  runPylonKhalaSpawnPlan,
  weightedKhalaAccountPool,
  type PylonKhalaSpawnAdvertisedCodexAccount,
  type PylonKhalaSpawnProofProjection,
  type PylonKhalaSpawnWorkerKind,
} from "./khala-spawn.js"

export { readPublicKhalaTokensServed } from "./khala-spawn.js"

export const PYLON_KHALA_BURNDOWN_PLAN_SCHEMA = "openagents.pylon.khala_burndown_plan.v0.1"
export const PYLON_KHALA_BURNDOWN_RUN_SCHEMA = "openagents.pylon.khala_burndown_run.v0.1"
export type PylonKhalaBurndownWorkerKind = PylonKhalaSpawnWorkerKind

export type PylonKhalaBurndownIssue = {
  issueNumber: number
  issueRef: string
  objective: string
}

export type PylonKhalaBurndownAccount = {
  accountRef: string | null
  accountRefHash: string
}

export type PylonKhalaBurndownSlot = {
  account: PylonKhalaBurndownAccount
  commands: {
    proof: string
    request: string
    runNoSpend: string
  }
  issue: PylonKhalaBurndownIssue
  iteration: number
  requestInput: PylonKhalaRequestInput
  slotIndex: number
}

export type PylonKhalaBurndownPlan = {
  schema: typeof PYLON_KHALA_BURNDOWN_PLAN_SCHEMA
  baseUrl: string
  maxParallel: number
  iterations: number
  repository: string
  commit: string
  verificationCommand: string
  targetPylonRef: string
  advertisedCodexAccounts: readonly PylonKhalaSpawnAdvertisedCodexAccount[]
  advertisedCodexAvailability: number
  advertisedWorkerAvailability: number
  readyCodexAccountCount: number
  readyWorkerAccountCount: number
  issueCount: number
  slots: PylonKhalaBurndownSlot[]
  mergePolicy: "operator_review_required"
  blockerRefs: string[]
  workerKind: PylonKhalaBurndownWorkerKind
  workflow: "claude_agent_task" | "codex_agent_task"
}

export type PylonKhalaBurndownSlotResult = {
  accountRefHash: string
  assignmentRef: string | null
  blockerRefs: string[]
  durableRequestId: string | null
  issueRef: string
  ok: boolean
  proof: {
    rawEventCount: number
    tokenRows: number
    totalTokens: number
    traceCount: number
    usageTruth: PylonKhalaProofResult["tokenUsage"]["usageTruth"]
  } | null
  runAccepted: boolean | null
  slotIndex: number
}

type PylonKhalaBurndownProofProjection = Pick<
  PylonKhalaSpawnProofProjection,
  "rawEventCount" | "tokenRows" | "totalTokens" | "traceCount" | "usageTruth"
>

export type PylonKhalaBurndownCounterEvidence = {
  after: number | null
  before: number | null
  blockerRefs: string[]
  delta: number | null
  expectedMinimumDelta: number
  state: "increment_observed" | "not_checked" | "unchanged" | "unavailable"
}

export type PylonKhalaBurndownRunResult = {
  schema: typeof PYLON_KHALA_BURNDOWN_RUN_SCHEMA
  ok: boolean
  plan: PylonKhalaBurndownPlan
  results: PylonKhalaBurndownSlotResult[]
  totalVerifiedTokens: number
  counter: PylonKhalaBurndownCounterEvidence
  blockerRefs: string[]
}

type PylonKhalaBurndownRunDeps = {
  issueRequest?: (
    network: TipsNetworkOptions,
    input: PylonKhalaRequestInput,
    slot: PylonKhalaBurndownSlot,
  ) => Promise<PylonKhalaRequestResult>
  readProof?: (
    network: TipsNetworkOptions,
    assignmentRef: string,
    slot: PylonKhalaBurndownSlot,
  ) => Promise<PylonKhalaProofResult>
  readTokensServed?: (network: TipsNetworkOptions) => Promise<number | null>
  runAssignment?: (
    summary: BootstrapSummary,
    options: AssignmentClientOptions,
    slot: PylonKhalaBurndownSlot,
  ) => Promise<{ ok: boolean; closeout?: { status?: string; blockerRefs?: string[] }; acceptance?: { blockerRefs?: string[] } }>
}

const issueRefPattern = /#([1-9][0-9]{0,9})/g

const uniqueIssueNumbers = (values: number[]): number[] => {
  const seen = new Set<number>()
  const out: number[] = []
  for (const value of values) {
    if (seen.has(value)) continue
    seen.add(value)
    out.push(value)
  }
  return out
}

export function parseKhalaBurndownIssueNumbers(input: string): number[] {
  return uniqueIssueNumbers(
    input
      .split(",")
      .flatMap((part) => {
        const trimmed = part.trim().replace(/^#/, "")
        const parsed = Number.parseInt(trimmed, 10)
        return Number.isSafeInteger(parsed) && parsed > 0 ? [parsed] : []
      }),
  )
}

export function parseKhalaRoadmapActiveIssueNumbers(roadmapText: string): number[] {
  const start = roadmapText.indexOf("Remaining active sequence")
  const remaining = start >= 0 ? roadmapText.slice(start) : roadmapText
  const end = remaining.search(/\n## Dependency rationale|\n## Notes|\nRunning \*\*continuously/)
  const section = end >= 0 ? remaining.slice(0, end) : remaining
  return uniqueIssueNumbers([...section.matchAll(issueRefPattern)].map((match) => Number.parseInt(match[1], 10)))
}

export async function readKhalaRoadmapIssueNumbers(path: string): Promise<number[]> {
  return parseKhalaRoadmapActiveIssueNumbers(await readFile(path, "utf8"))
}

const quoteArg = (value: string): string =>
  JSON.stringify(value)

const accountCommandArgs = (account: PylonKhalaBurndownAccount): string =>
  account.accountRef === null ? "" : ` --account ${quoteArg(account.accountRef)}`

const requestAccountArgs = (account: PylonKhalaBurndownAccount): string[] =>
  account.accountRef === null ? [] : [`--account-ref ${quoteArg(account.accountRef)}`]

const burndownBlocker = (
  workerKind: PylonKhalaBurndownWorkerKind,
  suffix: "no_advertised_availability" | "no_ready_account_slots" | "no_ready_accounts",
): string => {
  const kind = workerKind === "claude" ? "claude" : "codex"
  return `blocker.khala_burndown.${suffix.replace("_account", `_${kind}_account`).replace("_advertised_", `_advertised_${kind}_`)}`
}

export function buildKhalaBurndownObjective(issueNumber: number): string {
  return [
    `Implement OpenAgents issue #${issueNumber} from the Khala roadmap.`,
    "Read the GitHub issue and docs/khala/2026-06-26-khala-open-issues-master-roadmap.md.",
    "Keep edits scoped, run the requested verification, update relevant docs, commit, and prepare a concise closeout summary.",
  ].join(" ")
}

export function buildPylonKhalaBurndownPlan(input: {
  accounts: PylonAccountsListProjection
  advertisedCodexAccounts?: readonly PylonKhalaSpawnAdvertisedCodexAccount[]
  baseUrl: string
  branch?: string
  commit: string
  issueNumbers: number[]
  iterations?: number
  maxParallel?: number
  advertisedCodexAvailability?: number
  repository: string
  targetPylonRef: string
  verificationCommand: string
  workerKind?: PylonKhalaBurndownWorkerKind
}): PylonKhalaBurndownPlan {
  const workerKind = input.workerKind ?? "codex"
  const workflow = pylonKhalaSpawnWorkflowForWorkerKind(workerKind)
  const readyAccounts = readyKhalaSpawnAccounts(input.accounts, workflow)
  const advertisedCodexAccounts = (input.advertisedCodexAccounts ?? [])
    .map(account => ({
      accountKey: account.accountKey,
      accountRefHash: account.accountRefHash,
      available: Math.max(0, Math.floor(account.available)),
      busy: Math.max(0, Math.floor(account.busy)),
      queued: Math.max(0, Math.floor(account.queued)),
      ready: Math.max(0, Math.floor(account.ready)),
    }))
  const advertisedAccountCapacity = new Map(
    advertisedCodexAccounts.map(account => [account.accountRefHash, account]),
  )
  const accounts = advertisedCodexAccounts.length === 0
    ? readyAccounts
    : readyAccounts.filter(account => (advertisedAccountCapacity.get(account.accountRefHash)?.available ?? 0) > 0)
  const maxParallel = Math.max(1, Math.floor(input.maxParallel ?? accounts.length))
  const advertisedCodexAvailability = Math.max(
    0,
    Math.floor(
      input.advertisedCodexAvailability ??
        (
          advertisedCodexAccounts.length === 0
            ? accounts.length
            : advertisedCodexAccounts.reduce((sum, account) => sum + account.available, 0)
        ),
    ),
  )
  const iterations = Math.max(1, Math.floor(input.iterations ?? 1))
  const selectedParallel = accounts.length === 0 || advertisedCodexAvailability === 0
    ? 0
    : Math.min(maxParallel, advertisedCodexAvailability)
  const selectedAccounts = weightedKhalaAccountPool(accounts, advertisedAccountCapacity, selectedParallel)
  const requestedIssueNumbers = uniqueIssueNumbers(input.issueNumbers)
  const issueNumbers = requestedIssueNumbers.slice(0, selectedParallel * iterations)
  const blockerRefs = [
    ...(readyAccounts.length === 0 ? [burndownBlocker(workerKind, "no_ready_accounts")] : []),
    ...(readyAccounts.length > 0 && accounts.length === 0
      ? [burndownBlocker(workerKind, "no_ready_account_slots")]
      : []),
    ...(advertisedCodexAvailability === 0 ? [burndownBlocker(workerKind, "no_advertised_availability")] : []),
    ...(requestedIssueNumbers.length === 0 ? ["blocker.khala_burndown.no_issue_numbers"] : []),
  ]
  const slots: PylonKhalaBurndownSlot[] = selectedParallel === 0
    ? []
    : issueNumbers.map((issueNumber, index) => {
        const account = selectedAccounts[index % selectedAccounts.length]!
        const iteration = Math.floor(index / selectedParallel) + 1
        const slotIndex = index % selectedParallel
        const objective = buildKhalaBurndownObjective(issueNumber)
        const workspace = buildPylonKhalaGitCheckoutWorkspace({
          branch: input.branch,
          commit: input.commit,
          repository: input.repository,
          verificationCommand: input.verificationCommand,
        })
        const requestInput: PylonKhalaRequestInput = {
          objectiveSummary: objective,
          prompt: objective,
          targetAccountRefHash: account.accountRefHash,
          targetPylonRef: input.targetPylonRef,
          workflow,
          workspace,
        }
        const requestCommand = [
          "pylon khala request",
          `--workflow ${workflow}`,
          `--pylon-ref ${quoteArg(input.targetPylonRef)}`,
          ...requestAccountArgs(account),
          `--repo ${quoteArg(input.repository)}`,
          `--commit ${quoteArg(input.commit)}`,
          `--verify ${quoteArg(input.verificationCommand)}`,
          `--prompt ${quoteArg(objective)}`,
          "--json",
        ].join(" ")
        return {
          account,
          commands: {
            proof: "pylon khala proof --assignment-ref <assignmentRef> --json",
            request: requestCommand,
            runNoSpend: `pylon assignment run-no-spend --base-url ${quoteArg(input.baseUrl)}${accountCommandArgs(account)} --assignment-ref <assignmentRef> --json`,
          },
          issue: {
            issueNumber,
            issueRef: `#${issueNumber}`,
            objective,
          },
          iteration,
          requestInput,
          slotIndex,
        }
      })
  const plan: PylonKhalaBurndownPlan = {
    schema: PYLON_KHALA_BURNDOWN_PLAN_SCHEMA,
    baseUrl: input.baseUrl,
    maxParallel: selectedParallel,
    iterations,
    repository: input.repository,
    commit: input.commit,
    verificationCommand: input.verificationCommand,
    targetPylonRef: input.targetPylonRef,
    advertisedCodexAccounts,
    advertisedCodexAvailability,
    advertisedWorkerAvailability: advertisedCodexAvailability,
    readyCodexAccountCount: readyAccounts.length,
    readyWorkerAccountCount: readyAccounts.length,
    issueCount: requestedIssueNumbers.length,
    slots,
    mergePolicy: "operator_review_required",
    blockerRefs,
    workerKind,
    workflow,
  }
  assertPublicProjectionSafe(plan)
  return plan
}

export async function runPylonKhalaBurndownPlan(input: {
  network: TipsNetworkOptions
  plan: PylonKhalaBurndownPlan
  summary: BootstrapSummary
  deps?: PylonKhalaBurndownRunDeps
}): Promise<PylonKhalaBurndownRunResult> {
  const spawnRun = await runPylonKhalaSpawnPlan<PylonKhalaBurndownSlot>({
    blockerNamespace: "khala_burndown",
    deps: {
      ...(input.deps?.issueRequest === undefined ? {} : { requestAssignment: input.deps.issueRequest }),
      ...(input.deps?.readProof === undefined ? {} : { readProof: input.deps.readProof }),
      ...(input.deps?.readTokensServed === undefined ? {} : { readTokensServed: input.deps.readTokensServed }),
      ...(input.deps?.runAssignment === undefined ? {} : { runAssignment: input.deps.runAssignment }),
    },
    network: input.network,
    plan: {
      schema: PYLON_KHALA_SPAWN_PLAN_SCHEMA,
      advertisedCodexAccounts: input.plan.advertisedCodexAccounts,
      advertisedCodexAvailability: input.plan.advertisedCodexAvailability,
      advertisedWorkerAvailability: input.plan.advertisedWorkerAvailability,
      baseUrl: input.plan.baseUrl,
      blockerRefs: input.plan.blockerRefs,
      maxParallel: input.plan.maxParallel,
      objectiveCount: input.plan.issueCount,
      readyCodexAccountCount: input.plan.readyCodexAccountCount,
      readyWorkerAccountCount: input.plan.readyWorkerAccountCount,
      requestedCount: input.plan.issueCount,
      slots: input.plan.slots,
      targetPylonRef: input.plan.targetPylonRef,
      workerKind: input.plan.workerKind,
      workflow: input.plan.workflow,
    },
    summary: input.summary,
  })
  const results: PylonKhalaBurndownSlotResult[] = spawnRun.results.map((result, index) => {
    const slot = input.plan.slots[index]
    const proof: PylonKhalaBurndownProofProjection | null = result.proof === null
      ? null
      : {
          rawEventCount: result.proof.rawEventCount,
          tokenRows: result.proof.tokenRows,
          totalTokens: result.proof.totalTokens,
          traceCount: result.proof.traceCount,
          usageTruth: result.proof.usageTruth,
        }
    return {
      accountRefHash: result.accountRefHash,
      assignmentRef: result.assignmentRef,
      blockerRefs: result.blockerRefs,
      durableRequestId: result.durableRequestId,
      issueRef: slot?.issue.issueRef ?? `slot.${result.slotIndex}`,
      ok: result.ok,
      proof,
      runAccepted: result.runAccepted,
      slotIndex: result.slotIndex,
    }
  })
  const totalVerifiedTokens = spawnRun.aggregate.totalVerifiedTokens
  const counter = spawnRun.counter
  const blockerRefs = [
    ...input.plan.blockerRefs,
    ...results.flatMap((result) => result.blockerRefs),
    ...counter.blockerRefs,
  ]
  const run: PylonKhalaBurndownRunResult = {
    schema: PYLON_KHALA_BURNDOWN_RUN_SCHEMA,
    ok: blockerRefs.length === 0 && results.length > 0,
    plan: input.plan,
    results,
    totalVerifiedTokens,
    counter,
    blockerRefs,
  }
  assertPublicProjectionSafe(run)
  return run
}

export function localPylonTargetRef(state: Pick<PylonLocalState, "identity">): string {
  return state.identity.pylonRef
}
