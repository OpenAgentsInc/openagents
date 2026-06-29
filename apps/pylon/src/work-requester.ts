import { createHash } from "node:crypto"

import type { TipsNetworkOptions } from "./tips.js"

export type PylonWorkRequestInput = {
  objective: string
  budgetSats: number
  repository?: string
  verificationCommand?: string
  deadline?: string
}

export type PylonAutopilotWorkInput = {
  objective: string
  budgetCents: number
  repository?: string
  branch?: string
  commit?: string
  adapter?: "claude_agent" | "codex" | "fable"
  verificationCommand?: string
}

export type PylonWorkRequestBody = {
  budgetSats: number
  deadlineRef: string
  objectiveRef: string
  repositoryRefs: string[]
  requiredCapabilityRefs: string[]
  title: string
  verificationCommandRef: string
}

export type PylonWorkMemoryEntry = {
  at: string
  kind: "work_request" | "work_acceptance"
  summary: string
  refs: Record<string, unknown>
}

const DEFAULT_REQUIRED_CAPABILITY_REF = "capability.pylon.local_claude_agent"
const DEFAULT_REPOSITORY_REF = "repo.public.openagents"
const DEFAULT_VERIFY_REF = "command.public.pylon.labor.bun_test"
const DEFAULT_BRANCH = "main"
const DEFAULT_VERIFICATION_COMMAND = "bun test"
const AUTOPILOT_WORK_PROMISE_ID = "autopilot.mission_briefing.v1"
const commitShaPattern = /^[a-f0-9]{40}$/i
const placeholderCommitShaPattern = /^(0{40}|1{40})$/i

const unsafeWorkRequestPattern =
  /(\/Users\/|\/home\/|access[_-]?token|bearer\s+|cookie|file:\/\/|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage)|preimage|private[_-]?(key|repo)|provider[_-]?(credential|grant|payload|secret|token)|raw[_-]?(command|content|invoice|payment|payload|prompt|repo|runner|state)|secret|seed[_-]?phrase|\bsk-[A-Za-z0-9_-]{16,}\b|ssh:\/\/|wallet[._-]?(key|material|mnemonic|preimage|secret|seed)|xprv)/i

function stableRef(prefix: string, input: string) {
  return `${prefix}.${createHash("sha256").update(input).digest("hex").slice(0, 24)}`
}

export function assertPublicSafe(value: unknown, field: string) {
  if (unsafeWorkRequestPattern.test(JSON.stringify(value) ?? "")) {
    throw new Error(`${field} contains private, payment, credential, wallet, or raw material`)
  }
}

function cleanObjective(objective: string) {
  const trimmed = objective.trim()
  if (trimmed.length < 3 || trimmed.length > 160) {
    throw new Error("work request objective must be 3-160 characters")
  }
  assertPublicSafe(trimmed, "work request objective")
  return trimmed
}

function repositoryRefFromInput(repository: string | undefined) {
  const value = repository?.trim()
  if (!value) return DEFAULT_REPOSITORY_REF
  assertPublicSafe(value, "work request repository")
  if (value.startsWith("repo.public.")) return value
  const github = /^https:\/\/github\.com\/([^/\s]+)\/([^/\s#?]+)(?:[/?#].*)?$/.exec(value)
  if (github) {
    return `repo.public.github.${github[1]}.${github[2].replace(/\.git$/, "")}`
  }
  throw new Error("work request --repo must be a repo.public ref or public GitHub URL")
}

function verificationRefFromInput(command: string | undefined) {
  const value = command?.trim()
  if (!value) return DEFAULT_VERIFY_REF
  assertPublicSafe(value, "work request verification command")
  if (value.startsWith("command.public.")) return value
  if (value.length > 120) {
    throw new Error("work request --verify must be 120 characters or less")
  }
  return stableRef("command.public.pylon_work", value)
}

function cleanRefSegment(value: string) {
  return value.trim().replace(/[^A-Za-z0-9_-]+/g, "_").slice(0, 80) || "request"
}

function githubFullNameFromInput(repository: string | undefined) {
  const value = repository?.trim()
  if (!value) return "OpenAgentsInc/openagents"
  assertPublicSafe(value, "autopilot work repository")
  if (/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(value)) return value
  const github = /^https:\/\/github\.com\/([^/\s]+)\/([^/\s#?]+)(?:[/?#].*)?$/.exec(value)
  if (github) return `${github[1]}/${github[2].replace(/\.git$/, "")}`
  throw new Error("work submit --repo must be owner/repo or a public GitHub URL")
}

function verificationArgsFromInput(command: string | undefined) {
  const value = command?.trim() || DEFAULT_VERIFICATION_COMMAND
  assertPublicSafe(value, "autopilot work verification command")
  const args = value.split(/\s+/).filter(Boolean)
  if (args.length === 0 || args.length > 12 || args.some((arg) => arg.length > 120 || arg.startsWith("/") || arg.includes(".."))) {
    throw new Error("work submit --verify must be bounded argv tokens without absolute paths or traversal")
  }
  return args
}

function commitShaFromInput(commit: string | undefined) {
  const value = commit?.trim()
  if (!value) {
    throw new Error("work submit --commit <40-char-sha> is required for git checkout tasks")
  }
  if (!commitShaPattern.test(value) || placeholderCommitShaPattern.test(value)) {
    throw new Error("work submit --commit must be a real pinned 40-character commit SHA, not a placeholder")
  }
  return value.toLowerCase()
}

function requestedAdapterFromInput(adapter: PylonAutopilotWorkInput["adapter"]) {
  if (adapter === undefined) return {}
  if (adapter === "codex") return { requestedAdapter: "codex" as const }
  if (adapter === "claude_agent") return { requestedAdapter: "claude_agent" as const }
  if (adapter === "fable") {
    return {
      requestedAdapter: "claude_agent" as const,
      requestedAdapterProfileRef: "profile.claude_agent.fable",
    }
  }
  throw new Error("work submit --adapter must be codex, claude_agent, or fable")
}

function firstAutopilotTask(body: Record<string, unknown>) {
  return Array.isArray(body.tasks)
    ? body.tasks[0] as Record<string, unknown> | undefined
    : undefined
}

function pinnedCheckoutFromBody(body: Record<string, unknown>) {
  const task = firstAutopilotTask(body)
  const repository = task?.repository as Record<string, unknown> | undefined
  const checkout = task?.checkout as Record<string, unknown> | undefined
  const fullName = typeof repository?.fullName === "string" ? repository.fullName : ""
  const commitSha = typeof checkout?.commitSha === "string" ? checkout.commitSha : ""
  return { checkout, commitSha, repository, fullName }
}

export function buildPylonAutopilotWorkRequestBody(input: PylonAutopilotWorkInput): Record<string, unknown> {
  const objective = cleanObjective(input.objective)
  if (!Number.isInteger(input.budgetCents) || input.budgetCents < 0) {
    throw new Error("work submit --budget-cents must be a non-negative integer cent amount")
  }
  const repository = githubFullNameFromInput(input.repository)
  const branch = input.branch?.trim() || DEFAULT_BRANCH
  assertPublicSafe(branch, "autopilot work branch")
  const args = verificationArgsFromInput(input.verificationCommand)
  const commitSha = commitShaFromInput(input.commit)
  const requestedAdapter = requestedAdapterFromInput(input.adapter)
  const taskRef = `task.autopilot_coder.pylon.${cleanRefSegment(repository)}.${cleanRefSegment(objective)}`
  const paid = input.budgetCents > 0
  const body = {
    caller: { agentId: "oa_agent.pylon_cli", kind: "registered_agent", ownerRef: "owner_ref.pylon_cli" },
    clientRequestRef: `client.pylon.${taskRef}`,
    intent: "delegate_to_autopilot",
    mode: paid ? "free_slice_or_paid_quote_or_l402" : "free_slice_or_paid_quote",
    paymentPolicy: {
      buyerPaymentMode: paid ? "l402" : "free_slice",
      maxSpendCents: input.budgetCents,
      quoteRef: paid ? `quote.${taskRef}` : null,
      quotedAmountCents: paid ? input.budgetCents : null,
      settlementMode: paid ? "no_worker_payout_until_accepted_work" : "no_worker_payout",
    },
    placementPolicy: {
      allowedRunnerKinds: ["requester_pylon", "openagents_shc"],
      disallowedRunnerKinds: [],
      localOnlyAllowed: false,
      preferredRunnerKinds: ["requester_pylon", "openagents_shc"],
      privacyTier: paid ? "openagents_shc" : "public_beta",
      publicTraceAllowed: !paid,
      requiresSecretBroker: false,
    },
    promiseRef: { blockerRefs: [], promiseId: AUTOPILOT_WORK_PROMISE_ID, registryVersion: "2026-06-11.1" },
    schema: "openagents.autopilot_work_request.v1",
    tasks: [{
      acceptanceCriteriaRefs: ["acceptance.pylon_cli.customer_review"],
      accessRequests: [{ kind: "github_repo_read", reasonRef: "access.github.public_read" }],
      checkout: {
        commitSha,
        kind: "git_checkout",
        verificationCommand: { args, commandRef: `command.${cleanRefSegment(args.join("_"))}` },
      },
      forumReporting: { mode: "operator_approved_only" },
      kind: "code_change",
      objective,
      ...requestedAdapter,
      repository: { branch, fullName: repository, provider: "github", visibility: "public" },
      taskRef,
    }],
  }
  return body
}

async function assertResolvablePublicGitHubCommit(
  options: TipsNetworkOptions,
  input: Readonly<{ commitSha: string; fullName: string }>,
): Promise<void> {
  const [owner, repo] = input.fullName.split("/")
  if (!owner || !repo) {
    throw new Error("work submit --repo must resolve to a public GitHub owner/repo before commit preflight")
  }
  const url = new URL(
    `https://api.github.com/repos/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}/commits/${input.commitSha}`,
  )
  const response = await (options.fetch ?? fetch)(url, {
    headers: {
      Accept: "application/vnd.github+json",
      "User-Agent": "openagents-pylon",
    },
    method: "GET",
  })
  if (response.status === 404) {
    throw new Error(`work submit --commit ${input.commitSha} was not found in ${input.fullName}`)
  }
  if (!response.ok) {
    throw new Error(`work submit commit preflight failed (${response.status}) for ${input.fullName}@${input.commitSha}`)
  }
}

function deadlineRefFromInput(deadline: string | undefined) {
  const value = deadline?.trim()
  if (!value) return "deadline.public.pylon_work.unspecified"
  assertPublicSafe(value, "work request deadline")
  const parsed = Date.parse(value)
  if (!Number.isFinite(parsed)) {
    throw new Error("work request --deadline must be an ISO-like date")
  }
  return stableRef("deadline.public.pylon_work", new Date(parsed).toISOString())
}

export function buildPylonWorkRequestBody(input: PylonWorkRequestInput): PylonWorkRequestBody {
  const objective = cleanObjective(input.objective)
  if (!Number.isInteger(input.budgetSats) || input.budgetSats <= 0) {
    throw new Error("work request --budget must be a positive integer sat amount")
  }
  const body = {
    budgetSats: input.budgetSats,
    deadlineRef: deadlineRefFromInput(input.deadline),
    objectiveRef: stableRef("objective.public.pylon_work", objective),
    repositoryRefs: [repositoryRefFromInput(input.repository)],
    requiredCapabilityRefs: [DEFAULT_REQUIRED_CAPABILITY_REF],
    title: `Pylon work: ${objective}`.slice(0, 160),
    verificationCommandRef: verificationRefFromInput(input.verificationCommand),
  }
  assertPublicSafe(body, "work request body")
  return body
}

async function workApiRequest(
  options: TipsNetworkOptions,
  input: {
    path: string
    method: "GET" | "POST"
    body?: Record<string, unknown>
    idempotencyKey?: string
    okStatuses?: number[]
  },
): Promise<Record<string, unknown>> {
  const headers: Record<string, string> = { "Content-Type": "application/json" }
  const token = options.agentToken ?? process.env.OPENAGENTS_AGENT_TOKEN
  if (token) headers.Authorization = `Bearer ${token}`
  if (input.method === "POST") {
    if (!token) throw new Error("OPENAGENTS_AGENT_TOKEN or --agent-token is required for work writes")
    if (input.idempotencyKey) headers["Idempotency-Key"] = input.idempotencyKey
  }
  const response = await (options.fetch ?? fetch)(new URL(input.path, options.baseUrl), {
    ...(input.body === undefined ? {} : { body: JSON.stringify(input.body) }),
    headers,
    method: input.method,
  })
  const text = await response.text()
  const payload = text.trim() ? JSON.parse(text) as Record<string, unknown> : {}
  if (!response.ok && !(input.okStatuses ?? []).includes(response.status)) {
    const reason = typeof payload.reason === "string" ? payload.reason : typeof payload.error === "string" ? payload.error : String(response.status)
    throw new Error(`pylon work request failed (${response.status}): ${reason}`)
  }
  return payload
}

export async function createPylonWorkRequest(
  options: TipsNetworkOptions,
  input: PylonWorkRequestInput,
): Promise<Record<string, unknown>> {
  const body = buildPylonWorkRequestBody(input)
  const now = options.now?.() ?? new Date()
  return workApiRequest(options, {
    body,
    idempotencyKey: `pylon-work-request:${body.objectiveRef}:${now.toISOString().slice(0, 16)}`,
    method: "POST",
    path: "/api/forum/work-requests",
  })
}

export async function submitPylonAutopilotWork(
  options: TipsNetworkOptions,
  input: PylonAutopilotWorkInput,
): Promise<Record<string, unknown>> {
  const body = buildPylonAutopilotWorkRequestBody(input)
  const pinnedCheckout = pinnedCheckoutFromBody(body)
  await assertResolvablePublicGitHubCommit(options, {
    commitSha: pinnedCheckout.commitSha,
    fullName: pinnedCheckout.fullName,
  })
  const now = options.now?.() ?? new Date()
  const result = await workApiRequest(options, {
    body,
    idempotencyKey: `pylon-work-submit:${String(body.clientRequestRef)}:${now.toISOString().slice(0, 16)}`,
    method: "POST",
    okStatuses: [402],
    path: "/api/autopilot/work",
  })
  return {
    ...result,
    pylonSubmission: {
      adapter: input.adapter ?? null,
      pinnedCheckout: {
        branch: pinnedCheckout.repository?.branch ?? null,
        commitSha: pinnedCheckout.checkout?.commitSha ?? null,
        repository: pinnedCheckout.repository?.fullName ?? null,
      },
    },
  }
}

export async function readPylonAutopilotWorkStatus(
  options: TipsNetworkOptions,
  workOrderRef: string,
): Promise<Record<string, unknown>> {
  assertPublicSafe(workOrderRef, "autopilot work order ref")
  return workApiRequest(options, {
    method: "GET",
    path: `/api/autopilot/work/${encodeURIComponent(workOrderRef)}`,
  })
}

export async function readPylonAutopilotWorkEvents(
  options: TipsNetworkOptions,
  workOrderRef: string,
): Promise<Record<string, unknown>> {
  assertPublicSafe(workOrderRef, "autopilot work order ref")
  return workApiRequest(options, {
    method: "GET",
    path: `/api/autopilot/work/${encodeURIComponent(workOrderRef)}/events`,
  })
}

export async function reviewPylonAutopilotWork(
  options: TipsNetworkOptions,
  input: { action: "accept" | "reject" | "request_changes"; workOrderRef: string },
): Promise<Record<string, unknown>> {
  assertPublicSafe(input, "autopilot work review input")
  const now = options.now?.() ?? new Date()
  const ref = `review.pylon_cli.${input.action}.${cleanRefSegment(input.workOrderRef)}`
  return workApiRequest(options, {
    body: {
      action: input.action,
      ...(input.action === "accept"
        ? { decisionRefs: [ref] }
        : input.action === "reject"
          ? { rejectionRefs: [ref] }
          : { revisionRequestRefs: [ref] }),
    },
    idempotencyKey: `pylon-work-review:${input.workOrderRef}:${input.action}:${now.toISOString().slice(0, 16)}`,
    method: "POST",
    path: `/api/autopilot/work/${encodeURIComponent(input.workOrderRef)}/review`,
  })
}

export async function listPylonWorkOffers(
  options: TipsNetworkOptions,
  requestRef: string,
): Promise<Record<string, unknown>> {
  assertPublicSafe(requestRef, "work request ref")
  return workApiRequest(options, {
    method: "GET",
    path: `/api/forum/work-requests/${encodeURIComponent(requestRef)}/offers`,
  })
}

export async function acceptPylonWorkOffer(
  options: TipsNetworkOptions,
  input: { requestRef: string; quoteRef: string },
): Promise<Record<string, unknown>> {
  assertPublicSafe(input, "work acceptance input")
  const now = options.now?.() ?? new Date()
  return workApiRequest(options, {
    body: { quoteRef: input.quoteRef },
    idempotencyKey: `pylon-work-accept:${input.requestRef}:${input.quoteRef}:${now.toISOString().slice(0, 16)}`,
    method: "POST",
    path: `/api/forum/work-requests/${encodeURIComponent(input.requestRef)}/acceptances`,
  })
}

export async function readPylonWorkStatus(
  options: TipsNetworkOptions,
  requestRef: string,
): Promise<Record<string, unknown>> {
  assertPublicSafe(requestRef, "work request ref")
  return workApiRequest(options, {
    method: "GET",
    path: `/api/forum/work-requests/${encodeURIComponent(requestRef)}`,
  })
}

export function workRequestMemoryEntry(input: {
  at: string
  result: Record<string, unknown>
}): PylonWorkMemoryEntry {
  const workRequest = input.result.workRequest as { workRequestId?: string; jobEventId?: string; topicId?: string } | undefined
  return {
    at: input.at,
    kind: "work_request",
    refs: {
      jobEventId: workRequest?.jobEventId ?? null,
      topicId: workRequest?.topicId ?? null,
      workRequestId: workRequest?.workRequestId ?? null,
    },
    summary: `requested work ${String(workRequest?.workRequestId ?? "unknown").slice(0, 16)}`,
  }
}

export function workAcceptanceMemoryEntry(input: {
  at: string
  quoteRef: string
  requestRef: string
  result: Record<string, unknown>
}): PylonWorkMemoryEntry {
  return {
    at: input.at,
    kind: "work_acceptance",
    refs: {
      quoteRef: input.quoteRef,
      requestRef: input.requestRef,
      receiptRefs: input.result.receiptRefs ?? null,
    },
    summary: `accepted work quote ${input.quoteRef.slice(0, 32)}`,
  }
}
