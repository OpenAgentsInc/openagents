import { createHash } from "node:crypto"

import type { TipsNetworkOptions } from "./tips"

export type PylonWorkRequestInput = {
  objective: string
  budgetSats: number
  repository?: string
  verificationCommand?: string
  deadline?: string
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

const unsafeWorkRequestPattern =
  /(\/Users\/|\/home\/|access[_-]?token|bearer\s+|cookie|file:\/\/|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage)|preimage|private[_-]?(key|repo)|provider[_-]?(credential|grant|payload|secret|token)|raw[_-]?(command|content|invoice|payment|payload|prompt|repo|runner|state)|secret|seed[_-]?phrase|sk-[a-z0-9]|ssh:\/\/|wallet[._-]?(key|material|mnemonic|preimage|secret|seed)|xprv)/i

function stableRef(prefix: string, input: string) {
  return `${prefix}.${createHash("sha256").update(input).digest("hex").slice(0, 24)}`
}

function assertPublicSafe(value: unknown, field: string) {
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
  if (!response.ok) {
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
