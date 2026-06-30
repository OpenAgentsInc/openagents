import { createHash } from "node:crypto"

import type { TipsNetworkOptions } from "./tips.js"
import { assertPublicSafe } from "./work-requester.js"

export const KHALA_REQUEST_MODEL = "openagents/khala"

export type PylonKhalaWorkflow =
  | "claude_agent_task"
  | "cloud_coding_session"
  | "codex_agent_task"

export type PylonKhalaRequestInput = {
  prompt: string
  objectiveSummary?: string
  // #6354: public-safe Codex account-ref hash (`account.pylon.codex.<hex>`) so
  // the server gate admits against THAT account's per-account capacity instead
  // of pooling all accounts on the Pylon. The caller's Pylon computes it from a
  // local account ref; the raw ref/email/home never goes on the wire.
  targetAccountRefHash?: string
  targetPylonRef?: string
  workflow?: PylonKhalaWorkflow
  workspace?: PylonKhalaGitCheckoutWorkspace
}

export type PylonKhalaGitCheckoutWorkspace = {
  kind: "git_checkout"
  repository: {
    branch: string
    commitSha: string
    fullName: string
    provider: "github"
    visibility: "public"
  }
  verificationCommand: {
    args: string[]
    commandRef: string
  }
}

export type PylonKhalaResumeInput = {
  durableRequestId: string
  offset?: string | number
}

export type PylonKhalaSseFrame = {
  data: string
  parsed: unknown | null
}

export type PylonKhalaStreamProjection = {
  durableRequestId: string | null
  durableStreamUrl: string | null
  frames: PylonKhalaSseFrame[]
  nextOffset: string
  rawSse: string
  streamClosed: boolean
  streamUpToDate: boolean
  text: string
}

export type PylonKhalaRequestResult = PylonKhalaStreamProjection & {
  assignmentRef: string | null
  model: typeof KHALA_REQUEST_MODEL
  ok: true
  schema: "openagents.pylon.khala_request.v1"
  workflow: PylonKhalaWorkflow | null
}

export type PylonKhalaResumeResult = PylonKhalaStreamProjection & {
  ok: true
  schema: "openagents.pylon.khala_resume.v1"
}

export type PylonKhalaStatusResult = PylonKhalaStreamProjection & {
  ok: true
  schema: "openagents.pylon.khala_status.v1"
  state: "closed" | "streaming" | "up_to_date"
}

export type PylonKhalaAssignmentTraceStatusResult = {
  assignmentRef: string
  closeoutPolicy?: {
    paymentMode: "no-spend" | "paid" | "unknown"
    payoutClaimAllowed: boolean | null
    settlementState: "not_applicable" | "pending" | "settled" | "unknown"
    source: "worker_closeout_event" | "unavailable"
  }
  events: {
    count: number
    latestEventKind: string | null
    latestObservedAt: string | null
    latestStatus: string | null
    progressCount: number
  }
  generatedAt: string
  lifecycle: {
    acceptedWorkRefs: string[]
    artifactRefs: string[]
    closeoutRefs: string[]
    createdAt: string
    proofRefs: string[]
    rejectionRefs: string[]
    state: string
    updatedAt: string
  }
  ok: true
  owner: {
    agentUserRef: string
    openauthUserRef: string
  }
  progress: {
    closeoutReady: boolean
    hasFinalTrace: boolean
    hasLiveChunks: boolean
    hasTokenUsage: boolean
    state:
      | "assignment_created"
      | "streaming_chunks"
      | "final_trace_recorded"
      | "tokens_recorded"
      | "closed_out"
      | "rejected"
  }
  pylonRef: string
  rawEventChunks: {
    byteLength: number
    count: number
    eventCount: number
    latestChunkRef: string | null
    latestObservedAt: string | null
    visibility: "owner_only"
  }
  rawEvents: {
    byteLength: number
    count: number
    eventCount: number
    latestObservedAt: string | null
    latestRawEventRef: string | null
    refs: string[]
    visibility: "owner_only"
  }
  schemaVersion: "openagents.pylon.codex_assignment_trace_status.v1"
  tokenUsage: {
    cacheReadTokens: number
    demandKind: "own_capacity"
    demandSource: "khala_coding_delegation"
    inputTokens: number
    model: "openagents/pylon-codex"
    outputTokens: number
    provider: "pylon-codex-own-capacity"
    reasoningTokens: number
    rowCount: number
    status: "pending" | "recorded"
    totalTokens: number
    usageTruth: "exact"
  }
  traces: {
    count: number
    finalTraceUuid: string | null
    latestTraceUuid: string | null
    refs: string[]
    schemaVersion: string
    visibility: "owner_only"
  }
}

export type PylonKhalaProofResult = {
  assignmentRef: string
  closeoutPolicy?: PylonKhalaAssignmentTraceStatusResult["closeoutPolicy"]
  generatedAt: string
  ok: true
  owner: {
    agentUserRef: string
    openauthUserRef: string
  }
  pylonRef: string
  rawEvents: {
    byteLength: number
    count: number
    eventCount: number
    refs: string[]
    visibility: "owner_only"
  }
  proofChecklist: PylonKhalaProofChecklist
  schemaVersion: "openagents.pylon.codex_assignment_proof.v1"
  tokenUsage: {
    cacheReadTokens: number
    demandKind: "own_capacity"
    demandSource: "khala_coding_delegation"
    inputTokens: number
    model: "openagents/pylon-codex"
    outputTokens: number
    provider: "pylon-codex-own-capacity"
    reasoningTokens: number
    rowCount: number
    totalTokens: number
    usageTruth: "exact"
  }
  traces: {
    count: number
    refs: string[]
    schemaVersion: string
    visibility: "owner_only"
  }
}

export type PylonKhalaProofChecklistItem = {
  ok: boolean
  ref: string
}

export type PylonKhalaProofChecklist = {
  blockerRefs: string[]
  items: PylonKhalaProofChecklistItem[]
  ok: boolean
  schema: "openagents.pylon.khala_proof_checklist.v0.1"
}

export type PylonKhalaCloseoutChecklist = {
  blockerRefs: string[]
  caveatRefs: string[]
  items: PylonKhalaProofChecklistItem[]
  ok: boolean
  schema: "openagents.pylon.khala_closeout_checklist.v0.1"
}

export type PylonKhalaCloseoutResult = {
  assignmentRef: string
  closeoutChecklist: PylonKhalaCloseoutChecklist
  ok: true
  proof: PylonKhalaProofResult
  schema: "openagents.pylon.khala_closeout.v0.1"
  status: PylonKhalaAssignmentTraceStatusResult
}

const durablePrefix = "/v1/chat/completions/durable/"
const codexAssignmentProofPath = "/api/pylon/codex/proof"
const codexAssignmentTraceStatusPath = "/api/pylon/codex/trace-status"
const publicSafeDiagnosticRefPattern = /^[A-Za-z0-9_.:/=-]{1,200}$/

function requireAgentToken(options: TipsNetworkOptions): string {
  const token = options.agentToken ?? process.env.OPENAGENTS_AGENT_TOKEN
  if (!token) {
    throw new Error("OPENAGENTS_AGENT_TOKEN or --agent-token is required for Khala requests")
  }
  return token
}

function publicSafeDiagnosticRefs(value: unknown): string[] {
  if (!Array.isArray(value)) return []
  return value
    .filter((item): item is string => typeof item === "string")
    .map(item => item.trim())
    .filter(item => item !== "" && publicSafeDiagnosticRefPattern.test(item))
    .slice(0, 20)
}

function khalaApiErrorReason(status: number, text: string): string {
  const rawReason = text.trim() || String(status)
  try {
    const payload = JSON.parse(text) as {
      dispatchGate?: { blockerRefs?: unknown }
      error?: unknown
      evidenceRefs?: unknown
      reason?: unknown
      requestedPylonRef?: unknown
    }
    const reason =
      typeof payload.reason === "string"
        ? payload.reason
        : typeof payload.error === "string"
          ? payload.error
          : rawReason
    const evidenceRefs = [
      ...publicSafeDiagnosticRefs(payload.evidenceRefs),
      ...publicSafeDiagnosticRefs(payload.dispatchGate?.blockerRefs),
    ]
    const uniqueEvidenceRefs = [...new Set(evidenceRefs)]
    const requestedPylonRef =
      typeof payload.requestedPylonRef === "string" &&
      pylonRefPattern.test(payload.requestedPylonRef)
        ? payload.requestedPylonRef
        : null

    return [
      reason,
      requestedPylonRef === null ? null : `requestedPylonRef=${requestedPylonRef}`,
      uniqueEvidenceRefs.length === 0
        ? null
        : `evidenceRefs=${uniqueEvidenceRefs.join(",")}`,
    ]
      .filter((item): item is string => item !== null)
      .join("; ")
  } catch {
    return rawReason
  }
}

const byteLength = (value: string): number => new TextEncoder().encode(value).byteLength
const pylonRefPattern = /^[a-z0-9][a-z0-9_.:-]{2,119}$/
const githubFullNamePattern = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/
const gitCommitShaPattern = /^[a-f0-9]{40}$/i
const placeholderCommitShaPattern = /^(0{40}|1{40})$/i
const verificationCommandArgPattern = /^[A-Za-z0-9_./:=@+-]{1,120}$/
const unsafeVerificationCommandArgPattern =
  /(^|[._/:=@+-])(access[_-]?token|bearer|cookie|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage)|preimage|private[_-]?(key|repo)|provider[_-]?(credential|grant|payload|secret|token)|raw[_-]?(command|content|invoice|payment|payload|prompt|repo|runner|state)|secret|seed[_-]?phrase|ssh:|wallet[._-]?(key|material|mnemonic|preimage|secret|seed)|xprv)([._/:=@+-]|$)|\bsk-[A-Za-z0-9_-]{16,}\b|\bln(?:bc|tb|bcrt)[A-Za-z0-9]{20,}\b/i

function stableRef(prefix: string, value: string) {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`
}

function khalaPublicSafetyValue(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(khalaPublicSafetyValue)
  if (value === null || typeof value !== "object") return value
  const record = value as Record<string, unknown>
  if (
    Array.isArray(record.args) &&
    typeof record.commandRef === "string" &&
    Object.keys(record).every((key) => key === "args" || key === "commandRef")
  ) {
    return {
      args: ["validated-public-verify-argv"],
      commandRef: record.commandRef,
    }
  }
  return Object.fromEntries(
    Object.entries(record).map(([key, entry]) => [key, khalaPublicSafetyValue(entry)]),
  )
}

function githubFullNameFromInput(repository: string | undefined): string {
  const value = repository?.trim()
  if (!value) return "OpenAgentsInc/openagents"
  assertPublicSafe(value, "khala request repository")
  if (githubFullNamePattern.test(value)) return value
  const github = /^https:\/\/github\.com\/([^/\s]+)\/([^/\s#?]+)(?:[/?#].*)?$/.exec(value)
  if (github) return `${github[1]}/${github[2].replace(/\.git$/, "")}`
  throw new Error("khala request --repo must be owner/repo or a public GitHub URL")
}

function verificationArgsFromInput(command: string | undefined): string[] {
  const value = command?.trim()
  if (!value) {
    throw new Error("khala request --verify <command> is required for workspace-backed coding requests")
  }
  const args = value.split(/\s+/).filter(Boolean)
  if (
    args.length === 0 ||
    args.length > 20 ||
    args.some((arg) =>
      !verificationCommandArgPattern.test(arg) ||
      arg.includes("..") ||
      arg.startsWith("/"),
    )
  ) {
    throw new Error("khala request --verify must be bounded argv tokens without absolute paths or traversal")
  }
  if (args.some((arg) => unsafeVerificationCommandArgPattern.test(arg))) {
    throw new Error("khala request verification command contains private, payment, credential, wallet, or raw material")
  }
  return args
}

function commitShaFromInput(commit: string | undefined): string {
  const value = commit?.trim()
  if (!value) {
    throw new Error("khala request --commit <40-char-sha> is required for workspace-backed coding requests")
  }
  if (!gitCommitShaPattern.test(value) || placeholderCommitShaPattern.test(value)) {
    throw new Error("khala request --commit must be a real pinned 40-character commit SHA, not a placeholder")
  }
  return value.toLowerCase()
}

function cleanBranch(branch: string | undefined): string {
  const value = branch?.trim() || "main"
  assertPublicSafe(value, "khala request branch")
  if (value.includes("..") || value.startsWith("/") || value.length > 120) {
    throw new Error("khala request --branch must be a bounded public branch name")
  }
  return value
}

export function buildPylonKhalaGitCheckoutWorkspace(input: {
  branch?: string
  commit?: string
  repository?: string
  verificationCommand?: string
}): PylonKhalaGitCheckoutWorkspace {
  const fullName = githubFullNameFromInput(input.repository)
  const commitSha = commitShaFromInput(input.commit)
  const args = verificationArgsFromInput(input.verificationCommand)
  const workspace: PylonKhalaGitCheckoutWorkspace = {
    kind: "git_checkout",
    repository: {
      branch: cleanBranch(input.branch),
      commitSha,
      fullName,
      provider: "github",
      visibility: "public",
    },
    verificationCommand: {
      args,
      commandRef: `command.public.pylon_khala.verify.${stableRef("argv", args.join("\0")).slice("argv.".length)}`,
    },
  }
  assertPublicSafe(khalaPublicSafetyValue(workspace), "khala request workspace")
  return workspace
}

function cleanObjectiveSummary(value: string | undefined, fallback: string): string {
  const summary = (value ?? fallback).trim()
  if (summary.length < 3 || summary.length > 1000) {
    throw new Error("khala request objective summary must be 3-1000 characters")
  }
  assertPublicSafe(summary, "khala request objective summary")
  return summary
}

export function durableRequestIdFromUrl(value: string | null | undefined): string | null {
  if (value === null || value === undefined || value.trim() === "") {
    return null
  }

  const parsed = new URL(value, "https://openagents.invalid")
  if (!parsed.pathname.startsWith(durablePrefix)) {
    return null
  }
  const encoded = parsed.pathname.slice(durablePrefix.length)
  return encoded === "" || encoded.includes("/") ? null : decodeURIComponent(encoded)
}

function cleanAssignmentRef(value: string): string {
  const assignmentRef = value.trim()
  if (!/^[A-Za-z0-9][A-Za-z0-9_.:-]{2,180}$/.test(assignmentRef)) {
    throw new Error("khala proof assignmentRef must be a bounded public-safe ref")
  }
  assertPublicSafe(assignmentRef, "khala proof assignment ref")
  return assignmentRef
}

const checklistItem = (ref: string, ok: boolean): PylonKhalaProofChecklistItem => ({
  ok,
  ref,
})

export function evaluatePylonKhalaProofChecklist(
  proof: Omit<PylonKhalaProofResult, "ok" | "proofChecklist">,
): PylonKhalaProofChecklist {
  const items = [
    checklistItem(
      "check.khala_proof.schema.codex_assignment_proof_v1",
      proof.schemaVersion === "openagents.pylon.codex_assignment_proof.v1",
    ),
    checklistItem(
      "check.khala_proof.token_usage.exact_own_capacity",
      proof.tokenUsage.provider === "pylon-codex-own-capacity" &&
        proof.tokenUsage.model === "openagents/pylon-codex" &&
        proof.tokenUsage.usageTruth === "exact" &&
        proof.tokenUsage.demandKind === "own_capacity" &&
        proof.tokenUsage.demandSource === "khala_coding_delegation",
    ),
    checklistItem(
      "check.khala_proof.token_usage.rows_and_tokens_present",
      proof.tokenUsage.rowCount > 0 &&
        proof.tokenUsage.totalTokens > 0 &&
        proof.tokenUsage.totalTokens >= proof.tokenUsage.inputTokens + proof.tokenUsage.outputTokens,
    ),
    checklistItem(
      "check.khala_proof.traces.owner_only_present",
      proof.traces.visibility === "owner_only" &&
        proof.traces.count > 0 &&
        proof.traces.refs.length >= proof.traces.count,
    ),
    checklistItem(
      "check.khala_proof.raw_events.owner_only_present",
      proof.rawEvents.visibility === "owner_only" &&
        proof.rawEvents.count > 0 &&
        proof.rawEvents.eventCount > 0 &&
        proof.rawEvents.byteLength > 0 &&
        proof.rawEvents.refs.length >= proof.rawEvents.count,
    ),
    checklistItem(
      "check.khala_proof.generated_at.iso_timestamp",
      !Number.isNaN(Date.parse(proof.generatedAt)),
    ),
  ]
  const blockerRefs = items
    .filter((item) => !item.ok)
    .map((item) => item.ref.replace(/^check\./, "blocker."))
  return {
    blockerRefs,
    items,
    ok: blockerRefs.length === 0,
    schema: "openagents.pylon.khala_proof_checklist.v0.1",
  }
}

const unavailableCloseoutPolicy = {
  paymentMode: "unknown",
  payoutClaimAllowed: null,
  settlementState: "unknown",
  source: "unavailable",
} as const satisfies NonNullable<
  PylonKhalaAssignmentTraceStatusResult["closeoutPolicy"]
>

export function evaluatePylonKhalaCloseoutChecklist(
  status: PylonKhalaAssignmentTraceStatusResult,
  proof: PylonKhalaProofResult,
): PylonKhalaCloseoutChecklist {
  const statusCloseoutPolicy =
    status.closeoutPolicy ?? unavailableCloseoutPolicy
  const proofCloseoutPolicy = proof.closeoutPolicy ?? unavailableCloseoutPolicy
  const items = [
    checklistItem(
      "check.khala_closeout.status_schema.codex_assignment_trace_status_v1",
      status.schemaVersion === "openagents.pylon.codex_assignment_trace_status.v1",
    ),
    checklistItem(
      "check.khala_closeout.proof_schema.codex_assignment_proof_v1",
      proof.schemaVersion === "openagents.pylon.codex_assignment_proof.v1",
    ),
    checklistItem(
      "check.khala_closeout.assignment_ref_consistent",
      status.assignmentRef === proof.assignmentRef,
    ),
    checklistItem(
      "check.khala_closeout.pylon_ref_consistent",
      status.pylonRef === proof.pylonRef,
    ),
    checklistItem(
      "check.khala_closeout.owner_consistent",
      status.owner.agentUserRef === proof.owner.agentUserRef &&
        status.owner.openauthUserRef === proof.owner.openauthUserRef,
    ),
    checklistItem(
      "check.khala_closeout.lifecycle.closed_out",
      status.progress.state === "closed_out" &&
        status.progress.closeoutReady &&
        status.lifecycle.closeoutRefs.length > 0 &&
        status.lifecycle.rejectionRefs.length === 0,
    ),
    checklistItem(
      "check.khala_closeout.trace_status.final_owner_trace_present",
      status.progress.hasFinalTrace &&
        status.traces.visibility === "owner_only" &&
        status.traces.count > 0 &&
        status.traces.finalTraceUuid !== null &&
        status.traces.refs.length >= status.traces.count,
    ),
    checklistItem(
      "check.khala_closeout.trace_status.raw_events_owner_only_present",
      status.rawEvents.visibility === "owner_only" &&
        status.rawEvents.count > 0 &&
        status.rawEvents.eventCount > 0 &&
        status.rawEvents.byteLength > 0 &&
        status.rawEvents.refs.length >= status.rawEvents.count,
    ),
    checklistItem(
      "check.khala_closeout.trace_status.token_usage_recorded",
      status.progress.hasTokenUsage &&
        status.tokenUsage.status === "recorded" &&
        status.tokenUsage.rowCount > 0 &&
        status.tokenUsage.totalTokens > 0 &&
        status.tokenUsage.provider === proof.tokenUsage.provider &&
        status.tokenUsage.model === proof.tokenUsage.model &&
        status.tokenUsage.usageTruth === proof.tokenUsage.usageTruth &&
        status.tokenUsage.demandKind === proof.tokenUsage.demandKind &&
        status.tokenUsage.demandSource === proof.tokenUsage.demandSource,
    ),
    checklistItem(
      "check.khala_closeout.proof_checklist.ok",
      proof.proofChecklist.ok,
    ),
    checklistItem(
      "check.khala_closeout.no_spend_payout_false",
      statusCloseoutPolicy.source === "worker_closeout_event" &&
        proofCloseoutPolicy.source === "worker_closeout_event" &&
        statusCloseoutPolicy.paymentMode === "no-spend" &&
        proofCloseoutPolicy.paymentMode === "no-spend" &&
        statusCloseoutPolicy.settlementState === "not_applicable" &&
        proofCloseoutPolicy.settlementState === "not_applicable" &&
        statusCloseoutPolicy.payoutClaimAllowed === false &&
        proofCloseoutPolicy.payoutClaimAllowed === false,
    ),
    checklistItem(
      "check.khala_closeout.generated_at.iso_timestamps",
      !Number.isNaN(Date.parse(status.generatedAt)) &&
        !Number.isNaN(Date.parse(proof.generatedAt)),
    ),
  ]
  const blockerRefs = [
    ...items
      .filter((item) => !item.ok)
      .map((item) => item.ref.replace(/^check\./, "blocker.")),
    ...proof.proofChecklist.blockerRefs,
  ]
  return {
    blockerRefs: [...new Set(blockerRefs)].sort(),
    caveatRefs: [
      "caveat.khala_closeout.public_token_counter_is_supporting_not_assignment_proof",
    ],
    items,
    ok: blockerRefs.length === 0,
    schema: "openagents.pylon.khala_closeout_checklist.v0.1",
  }
}

export function buildPylonKhalaChatRequestBody(
  input: PylonKhalaRequestInput,
): Record<string, unknown> {
  const prompt = input.prompt.trim()
  if (prompt.length < 3 || prompt.length > 8_000) {
    throw new Error("khala request --prompt/--objective must be 3-8000 characters")
  }
  assertPublicSafe(prompt, "khala request prompt")

  const targetPylonRef = input.targetPylonRef?.trim()
  if (targetPylonRef !== undefined && targetPylonRef !== "") {
    if (!pylonRefPattern.test(targetPylonRef)) {
      throw new Error("khala request --pylon-ref must be a public-safe Pylon ref")
    }
    assertPublicSafe(targetPylonRef, "khala request target pylon ref")
  }

  const targetAccountRefHash = input.targetAccountRefHash?.trim()
  if (targetAccountRefHash !== undefined && targetAccountRefHash !== "") {
    // #6421: accept both the Codex and Claude per-account hash shapes
    // (`account.pylon.codex.<hex>` / `account.pylon.claude_agent.<hex>`) so the
    // claude-supervisor can pin a Claude account; the wire never carries a raw
    // ref, email, or home path.
    if (!/^account\.pylon\.(codex|claude_agent)\.[a-f0-9]{6,64}$/.test(targetAccountRefHash)) {
      throw new Error(
        "khala request --account-ref must resolve to a public-safe account.pylon.<codex|claude_agent>.<hex> hash",
      )
    }
    assertPublicSafe(targetAccountRefHash, "khala request target account ref hash")
  }
  const codingBase = {
    ...(targetPylonRef === undefined || targetPylonRef === ""
      ? {}
      : { targetPylonRef }),
    ...(targetAccountRefHash === undefined || targetAccountRefHash === ""
      ? {}
      : { targetAccountRefHash }),
  }
  const coding = Object.keys(codingBase).length === 0 ? undefined : codingBase
  const workspaceCoding =
    input.workspace === undefined
      ? coding
      : {
          ...(coding ?? {}),
          objectiveSummary: cleanObjectiveSummary(input.objectiveSummary, prompt),
          workspace: input.workspace,
        }
  const openagents =
    input.workflow === undefined && workspaceCoding === undefined
      ? undefined
      : {
          ...(input.workflow === undefined ? {} : { workflowClass: input.workflow }),
          ...(workspaceCoding === undefined ? {} : { coding: workspaceCoding }),
        }

  const body = {
    messages: [
      {
        content: prompt,
        role: "user",
      },
    ],
    model: KHALA_REQUEST_MODEL,
    ...(input.workflow === undefined ? {} : { workflowClass: input.workflow }),
    ...(openagents === undefined ? {} : { openagents }),
    stream: true,
    ...(targetPylonRef === undefined || targetPylonRef === ""
      ? {}
      : { targetPylonRef }),
  }
  assertPublicSafe(khalaPublicSafetyValue(body), "khala request body")
  return body
}

function parseSseFrames(rawSse: string): PylonKhalaSseFrame[] {
  return rawSse
    .split(/\n\n+/)
    .map((chunk) =>
      chunk
        .split(/\n/)
        .filter((line) => line.startsWith("data:"))
        .map((line) => line.slice("data:".length).trimStart())
        .join("\n"),
    )
    .filter((data) => data !== "")
    .map((data) => {
      if (data === "[DONE]") {
        return { data, parsed: null }
      }
      try {
        return { data, parsed: JSON.parse(data) }
      } catch {
        return { data, parsed: null }
      }
    })
}

function textFromFrames(frames: readonly PylonKhalaSseFrame[]): string {
  return frames
    .map((frame) => {
      const parsed = frame.parsed
      if (parsed === null || typeof parsed !== "object") {
        return ""
      }
      const choices = (parsed as { choices?: unknown }).choices
      if (!Array.isArray(choices)) {
        return ""
      }
      return choices
        .map((choice) => {
          const delta = (choice as { delta?: { content?: unknown } }).delta
          const message = (choice as { message?: { content?: unknown } }).message
          const content = delta?.content ?? message?.content
          return typeof content === "string" ? content : ""
        })
        .join("")
    })
    .join("")
}

function streamProjection(input: {
  durableStreamUrl: string | null
  fallbackRequestId?: string | null
  rawSse: string
  response: Response
}): PylonKhalaStreamProjection {
  const frames = parseSseFrames(input.rawSse)
  const durableRequestId =
    durableRequestIdFromUrl(input.durableStreamUrl) ?? input.fallbackRequestId ?? null
  return {
    durableRequestId,
    durableStreamUrl: input.durableStreamUrl,
    frames,
    nextOffset:
      input.response.headers.get("stream-next-offset") ??
      String(byteLength(input.rawSse)),
    rawSse: input.rawSse,
    streamClosed:
      input.response.headers.get("stream-closed") === "true" ||
      frames.some((frame) => frame.data === "[DONE]"),
    streamUpToDate: input.response.headers.get("stream-up-to-date") === "true",
    text: textFromFrames(frames),
  }
}

async function khalaApiRequest(
  options: TipsNetworkOptions,
  input: {
    body?: Record<string, unknown>
    method: "GET" | "POST"
    path: string
  },
): Promise<Response> {
  const token = requireAgentToken(options)
  const response = await (options.fetch ?? fetch)(new URL(input.path, options.baseUrl), {
    ...(input.body === undefined ? {} : { body: JSON.stringify(input.body) }),
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/json",
    },
    method: input.method,
  })
  if (!response.ok) {
    const text = await response.text()
    const reason = khalaApiErrorReason(response.status, text)
    throw new Error(`pylon khala request failed (${response.status}): ${reason}`)
  }
  return response
}

export async function issuePylonKhalaRequest(
  options: TipsNetworkOptions,
  input: PylonKhalaRequestInput,
): Promise<PylonKhalaRequestResult> {
  const body = buildPylonKhalaChatRequestBody(input)
  const response = await khalaApiRequest(options, {
    body,
    method: "POST",
    path: "/api/v1/chat/completions",
  })
  const rawSse = await response.text()
  const durableStreamUrl = response.headers.get("openagents-durable-stream-url")
  const projection = streamProjection({
    durableStreamUrl,
    fallbackRequestId: null,
    rawSse,
    response,
  })
  return {
    ...projection,
    assignmentRef: response.headers.get("openagents-coding-assignment-ref"),
    model: KHALA_REQUEST_MODEL,
    ok: true,
    schema: "openagents.pylon.khala_request.v1",
    workflow: input.workflow ?? null,
  }
}

export async function resumePylonKhalaRequest(
  options: TipsNetworkOptions,
  input: PylonKhalaResumeInput,
): Promise<PylonKhalaResumeResult> {
  assertPublicSafe(input.durableRequestId, "khala durable request id")
  const offset =
    input.offset === undefined
      ? undefined
      : String(input.offset).trim()
  const path =
    `${durablePrefix}${encodeURIComponent(input.durableRequestId)}` +
    (offset === undefined || offset === "" ? "" : `?offset=${encodeURIComponent(offset)}`)
  const response = await khalaApiRequest(options, { method: "GET", path })
  const rawSse = await response.text()
  return {
    ...streamProjection({
      durableStreamUrl: `${durablePrefix}${encodeURIComponent(input.durableRequestId)}`,
      fallbackRequestId: input.durableRequestId,
      rawSse,
      response,
    }),
    ok: true,
    schema: "openagents.pylon.khala_resume.v1",
  }
}

export async function readPylonKhalaStatus(
  options: TipsNetworkOptions,
  durableRequestId: string,
): Promise<PylonKhalaStatusResult> {
  const resumed = await resumePylonKhalaRequest(options, {
    durableRequestId,
    offset: 0,
  })
  return {
    ...resumed,
    schema: "openagents.pylon.khala_status.v1",
    state: resumed.streamClosed
      ? "closed"
      : resumed.streamUpToDate
        ? "up_to_date"
      : "streaming",
  }
}

export async function readPylonKhalaAssignmentTraceStatus(
  options: TipsNetworkOptions,
  assignmentRefInput: string,
): Promise<PylonKhalaAssignmentTraceStatusResult> {
  const assignmentRef = cleanAssignmentRef(assignmentRefInput)
  const response = await khalaApiRequest(options, {
    method: "GET",
    path: `${codexAssignmentTraceStatusPath}?assignmentRef=${encodeURIComponent(assignmentRef)}`,
  })
  const payload = (await response.json()) as Omit<PylonKhalaAssignmentTraceStatusResult, "ok">
  assertPublicSafe(payload, "khala assignment trace status response")
  return {
    ...payload,
    ok: true,
  }
}

export async function readPylonKhalaProof(
  options: TipsNetworkOptions,
  assignmentRefInput: string,
): Promise<PylonKhalaProofResult> {
  const assignmentRef = cleanAssignmentRef(assignmentRefInput)
  const response = await khalaApiRequest(options, {
    method: "GET",
    path: `${codexAssignmentProofPath}?assignmentRef=${encodeURIComponent(assignmentRef)}`,
  })
  const payload = (await response.json()) as Omit<PylonKhalaProofResult, "ok" | "proofChecklist">
  assertPublicSafe(payload, "khala proof response")
  return {
    ...payload,
    ok: true,
    proofChecklist: evaluatePylonKhalaProofChecklist(payload),
  }
}

export async function readPylonKhalaCloseout(
  options: TipsNetworkOptions,
  assignmentRefInput: string,
): Promise<PylonKhalaCloseoutResult> {
  const assignmentRef = cleanAssignmentRef(assignmentRefInput)
  const [status, proof] = await Promise.all([
    readPylonKhalaAssignmentTraceStatus(options, assignmentRef),
    readPylonKhalaProof(options, assignmentRef),
  ])
  const result: PylonKhalaCloseoutResult = {
    assignmentRef,
    closeoutChecklist: evaluatePylonKhalaCloseoutChecklist(status, proof),
    ok: true,
    proof,
    schema: "openagents.pylon.khala_closeout.v0.1",
    status,
  }
  assertPublicSafe(result, "khala closeout response")
  return result
}
