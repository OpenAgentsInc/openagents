import { readFile } from "node:fs/promises"
import type { BootstrapSummary } from "./bootstrap.js"
import type { TipsNetworkOptions } from "./tips.js"
import {
  buildPylonKhalaGitCheckoutWorkspace,
  issuePylonKhalaRequest,
  readPylonKhalaProof,
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

export const PYLON_KHALA_BURNDOWN_PLAN_SCHEMA = "openagents.pylon.khala_burndown_plan.v0.1"
export const PYLON_KHALA_BURNDOWN_RUN_SCHEMA = "openagents.pylon.khala_burndown_run.v0.1"

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
  readyCodexAccountCount: number
  issueCount: number
  slots: PylonKhalaBurndownSlot[]
  mergePolicy: "operator_review_required"
  blockerRefs: string[]
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

type PylonKhalaBurndownProofProjection = NonNullable<PylonKhalaBurndownSlotResult["proof"]>

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

const readyCodexAccounts = (
  accounts: PylonAccountsListProjection,
): PylonKhalaBurndownAccount[] =>
  accounts.accounts
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

export function buildKhalaBurndownObjective(issueNumber: number): string {
  return [
    `Implement OpenAgents issue #${issueNumber} from the Khala roadmap.`,
    "Read the GitHub issue and docs/khala/2026-06-26-khala-open-issues-master-roadmap.md.",
    "Keep edits scoped, run the requested verification, update relevant docs, commit, and prepare a concise closeout summary.",
  ].join(" ")
}

export function buildPylonKhalaBurndownPlan(input: {
  accounts: PylonAccountsListProjection
  baseUrl: string
  branch?: string
  commit: string
  issueNumbers: number[]
  iterations?: number
  maxParallel?: number
  repository: string
  targetPylonRef: string
  verificationCommand: string
}): PylonKhalaBurndownPlan {
  const accounts = readyCodexAccounts(input.accounts)
  const maxParallel = Math.max(1, Math.floor(input.maxParallel ?? accounts.length))
  const iterations = Math.max(1, Math.floor(input.iterations ?? 1))
  const selectedParallel = Math.min(maxParallel, accounts.length)
  const issueNumbers = uniqueIssueNumbers(input.issueNumbers).slice(0, selectedParallel * iterations)
  const blockerRefs = [
    ...(accounts.length === 0 ? ["blocker.khala_burndown.no_ready_codex_accounts"] : []),
    ...(issueNumbers.length === 0 ? ["blocker.khala_burndown.no_issue_numbers"] : []),
  ]
  const slots: PylonKhalaBurndownSlot[] = selectedParallel === 0
    ? []
    : issueNumbers.map((issueNumber, index) => {
        const account = accounts[index % selectedParallel]
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
          targetPylonRef: input.targetPylonRef,
          workflow: "codex_agent_task",
          workspace,
        }
        const requestCommand = [
          "pylon khala request",
          "--workflow codex_agent_task",
          `--pylon-ref ${quoteArg(input.targetPylonRef)}`,
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
    readyCodexAccountCount: accounts.length,
    issueCount: issueNumbers.length,
    slots,
    mergePolicy: "operator_review_required",
    blockerRefs,
  }
  assertPublicProjectionSafe(plan)
  return plan
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

const proofProjection = (proof: PylonKhalaProofResult): PylonKhalaBurndownProofProjection => ({
  rawEventCount: proof.rawEvents.eventCount,
  tokenRows: proof.tokenUsage.rowCount,
  totalTokens: proof.tokenUsage.totalTokens,
  traceCount: proof.traces.count,
  usageTruth: proof.tokenUsage.usageTruth,
})

export async function runPylonKhalaBurndownPlan(input: {
  network: TipsNetworkOptions
  plan: PylonKhalaBurndownPlan
  summary: BootstrapSummary
  deps?: PylonKhalaBurndownRunDeps
}): Promise<PylonKhalaBurndownRunResult> {
  const issueRequest = input.deps?.issueRequest ?? issuePylonKhalaRequest
  const runAssignment = input.deps?.runAssignment ?? runNoSpendAssignment
  const readProof = input.deps?.readProof ?? readPylonKhalaProof
  const readTokensServed = input.deps?.readTokensServed ?? readPublicKhalaTokensServed
  const results: PylonKhalaBurndownSlotResult[] = []
  const counterBefore = await readTokensServed(input.network).catch(() => null)

  for (let iteration = 1; iteration <= input.plan.iterations; iteration += 1) {
    const batch = input.plan.slots.filter((slot) => slot.iteration === iteration)
    const batchResults = await Promise.all(
      batch.map(async (slot): Promise<PylonKhalaBurndownSlotResult> => {
        const blockerRefs: string[] = []
        let assignmentRef: string | null = null
        let durableRequestId: string | null = null
        let runAccepted: boolean | null = null
        let proof: PylonKhalaBurndownSlotResult["proof"] = null
        try {
          const request = await issueRequest(input.network, slot.requestInput, slot)
          assignmentRef = request.assignmentRef
          durableRequestId = request.durableRequestId
          if (assignmentRef === null) {
            blockerRefs.push("blocker.khala_burndown.assignment_ref_missing")
          } else {
            const run = await runAssignment(
              input.summary,
              {
                ...(input.network.agentToken === undefined ? {} : { agentToken: input.network.agentToken }),
                ...(slot.account.accountRef === null ? {} : { accountRef: slot.account.accountRef }),
                assignmentRef,
                baseUrl: input.network.baseUrl,
                ...(input.network.fetch === undefined ? {} : { fetch: input.network.fetch }),
              },
              slot,
            )
            runAccepted = run.ok === true || run.closeout?.status === "accepted"
            if (!runAccepted) {
              blockerRefs.push(...(run.closeout?.blockerRefs ?? run.acceptance?.blockerRefs ?? ["blocker.khala_burndown.assignment_not_accepted"]))
            } else {
              const slotProof = proofProjection(await readProof(input.network, assignmentRef, slot))
              proof = slotProof
              if (slotProof.usageTruth !== "exact" || slotProof.tokenRows <= 0 || slotProof.totalTokens <= 0) {
                blockerRefs.push("blocker.khala_burndown.proof_not_exact")
              }
            }
          }
        } catch {
          blockerRefs.push("blocker.khala_burndown.slot_failed")
        }
        return {
          accountRefHash: slot.account.accountRefHash,
          assignmentRef,
          blockerRefs,
          durableRequestId,
          issueRef: slot.issue.issueRef,
          ok: blockerRefs.length === 0,
          proof,
          runAccepted,
          slotIndex: slot.slotIndex,
        }
      }),
    )
    results.push(...batchResults)
  }

  const totalVerifiedTokens = results.reduce((sum, result) => sum + (result.proof?.totalTokens ?? 0), 0)
  const counterAfter = totalVerifiedTokens > 0
    ? await readTokensServed(input.network).catch(() => null)
    : counterBefore
  const counterDelta = counterBefore === null || counterAfter === null
    ? null
    : counterAfter - counterBefore
  const counterBlockerRefs =
    totalVerifiedTokens > 0 &&
    counterBefore !== null &&
    counterAfter !== null &&
    counterAfter <= counterBefore
      ? ["blocker.khala_burndown.counter_not_incremented"]
      : []
  const counter: PylonKhalaBurndownCounterEvidence = {
    after: counterAfter,
    before: counterBefore,
    blockerRefs: counterBlockerRefs,
    delta: counterDelta,
    expectedMinimumDelta: totalVerifiedTokens,
    state: totalVerifiedTokens <= 0
      ? "not_checked"
      : counterBefore === null || counterAfter === null
        ? "unavailable"
        : counterAfter > counterBefore
          ? "increment_observed"
          : "unchanged",
  }
  const blockerRefs = [
    ...input.plan.blockerRefs,
    ...results.flatMap((result) => result.blockerRefs),
    ...counterBlockerRefs,
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
