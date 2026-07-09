import { existsSync } from "node:fs"
import { readFile, writeFile } from "node:fs/promises"
import { createHash } from "node:crypto"
import { schnorr } from "@noble/curves/secp256k1"
import { sha256 } from "@noble/hashes/sha256"
import {
  KIND_JOB_FEEDBACK,
  KIND_JOB_LABOR_CODE_TASK,
  KIND_JOB_LABOR_DOCUMENT_WORK,
  KIND_JOB_LABOR_REVIEW,
  KIND_JOB_TEXT_GENERATION,
  createJobFeedbackEvent,
  createJobResultEvent,
  isLaborJobKind,
  laborJobResultToTags,
  makeJobFeedback,
  makeJobResult,
  makeLaborJobResult,
  parseJobRequestEvent,
  parseLaborJobRequestEvent,
  type JobRequest,
  type LaborJobRequest,
} from "@openagentsinc/nip90"
import { Effect } from "effect"
import type { BootstrapSummary } from "./bootstrap.js"
import { loadOrCreateNostrIdentity, type PylonNostrPrivateIdentity } from "./nostr-identity.js"
import {
  assertPublicProjectionSafe,
  ensurePylonLocalState,
  ensureStateDirectories,
  type PylonLocalState,
  type PylonPaths,
} from "./state.js"
import { appendLedgerEvent, defaultWalletCommandRunner, type WalletCommandRunner } from "./wallet.js"
import { makeAppleFmClient, type AppleFmClient } from "../packages/runtime/src/index.js"
import {
  handleLaborMarketEventOnce,
  type LaborMarketHandleResult,
  type LaborMarketOptions,
} from "./labor-market.js"
import {
  assertLaborPublicSafe,
  detectConfiguredLaborAgent,
  evaluateLaborRequestSafety,
  hasLaborFirstRunApproval,
  laborResultContent,
  makeConfiguredLaborRuntime,
  requestedLaborWorkspacePath,
  resolveLaborWorkspace,
  type LaborLocalAgentKind,
  type LaborRuntime,
  type LaborWorkspace,
} from "./labor.js"
// #8578 (PY-1): the lane-ref/relay/capability-ref helpers below are the
// single source of truth in `@openagentsinc/pylon-core/presence`, which
// `presence.ts` depends on directly. Re-export (don't redefine) so both
// modules stay in sync.
import {
  OPENAGENTS_MARKET_RELAY_URL,
  PYLON_NIP90_PROVIDER_CAPABILITY_REF,
  providerNip90LaneRefs,
  providerSupportedKinds,
  relaysFromEnv,
} from "@openagentsinc/pylon-core/presence/nip90-lane-refs"

export { OPENAGENTS_MARKET_RELAY_URL, PYLON_NIP90_PROVIDER_CAPABILITY_REF, providerNip90LaneRefs, relaysFromEnv }

export const NIP89_HANDLER_INFO_KIND = 31990
export const DEFAULT_PROVIDER_PRICE_MSATS = 1_000
export const DEFAULT_PROVIDER_REQUEST_TTL_SECONDS = 365 * 24 * 60 * 60
export const DEFAULT_PROVIDER_MAX_INFLIGHT = 1
export const DEFAULT_PROVIDER_PER_BUYER_MAX_INFLIGHT = 1
export const DEFAULT_PROVIDER_LOOP_IDLE_MS = 250
export const DEFAULT_PROVIDER_RECONNECT_DELAY_MS = 5_000

export type NostrEvent = {
  id: string
  pubkey: string
  created_at: number
  kind: number
  tags: ReadonlyArray<readonly string[]>
  content: string
  sig: string
}

export type UnsignedNostrEvent = Omit<NostrEvent, "id" | "sig">

export type ProviderRelayMessage =
  | ["EVENT", string, NostrEvent]
  | ["EOSE", string]
  | ["OK", string, boolean, string]
  | ["NOTICE", string]
  | ["CLOSED", string, string]

export type ProviderRelayTransport = {
  relayUrl: string
  publish(event: NostrEvent): Promise<ProviderPublishReceipt>
  subscribe(filters: ReadonlyArray<Record<string, unknown>>, options?: { subscriptionId?: string }): AsyncIterable<NostrEvent>
  close?(): Promise<void> | void
}

export type ProviderPublishReceipt = {
  relayUrl: string
  accepted: boolean
  message: string
}

export type ProviderRuntimeCompletion = {
  text: string
  model: string
  receiptRefs: string[]
}

export type ProviderTextRuntime = {
  complete(prompt: string): Promise<ProviderRuntimeCompletion>
  runLabor?: LaborRuntime["runLabor"]
}

export type ProviderAdmissionPolicy = {
  priceMsats: number
  requestTtlSeconds: number
  maxInflight: number
  perBuyerMaxInflight: number
}

export type ProviderAdmissionStore = {
  schema: "openagents.pylon.nip90_provider_state.v0.3"
  admissionLeases: Record<string, ProviderAdmissionLease>
  handledRequests: Record<string, ProviderHandledRequest>
  earnings: ProviderEarningRecord[]
}

export type ProviderAdmissionLease = {
  requestEventId: string
  requesterPubkey: string
  status: "admitted" | "processing"
  expiresAtMs: number
}

export type ProviderHandledRequest = {
  requestEventId: string
  requesterPubkey: string
  status: "rejected" | "completed" | "error"
  reasonRef?: string
  resultEventId?: string
  completedAt: string
}

export type ProviderEarningRecord = {
  requestEventId: string
  requesterPubkey: string
  amountMsats: number
  amountSats: number
  receiptRef: string
  resultEventId: string
  recordedAt: string
}

export type ProviderRequestEntry = {
  requestEventId: string
  requesterPubkey: string
  relayUrl?: string
  targeted: boolean
  jobFamily: "text_generation" | "labor"
  decision: "match" | "drop"
  dropReason?: ProviderDropReason
  prompt?: string
  promptPreview?: string
  model?: string
  bidMsats?: number
  output?: string
  request?: JobRequest
  laborRequest?: LaborJobRequest
}

export type ProviderDropReason =
  | "malformed_request"
  | "unsupported_kind"
  | "encrypted_request"
  | "target_mismatch"
  | "missing_prompt"
  | "unsupported_output"

export type ProviderAdmissionDecision =
  | { admitted: true; amountMsats: number }
  | { admitted: false; action: "drop" | "defer"; reasonRef: ProviderAdmissionReason }

export type ProviderAdmissionReason =
  | ProviderDropReason
  | "provider_not_online"
  | "stale_request"
  | "missing_bid"
  | "bid_below_price_floor"
  | "duplicate_request"
  | "active_admission_lease"
  | "max_inflight"
  | "buyer_limit"
  | "labor_auth_exfiltration_blocked"
  | "labor_first_run_approval_required"
  | "labor_policy_mismatch"
  | "labor_workspace_out_of_bounds"
  | "runtime_error"

export type ProviderPaymentQuote = {
  amountMsats: number
  amountSats: number
  bolt11: string
  receiptRef: string
}

export type ProviderRunResult = {
  requestEventId: string
  status: "accepted" | "dropped" | "deferred" | "completed" | "error"
  reasonRef?: ProviderAdmissionReason
  feedbackEventIds: string[]
  resultEventId?: string
  earning?: ProviderEarningRecord
  laborMarket?: LaborMarketHandleResult
}

export type ProviderLoopOptions = {
  relays?: readonly string[]
  policy?: Partial<ProviderAdmissionPolicy>
  runtime?: ProviderTextRuntime
  walletRunner?: WalletCommandRunner
  laborRuntime?: LaborRuntime
  laborWorkspaceRoot?: string
  laborAgentKind?: LaborLocalAgentKind
  laborMarket?: LaborMarketOptions
  now?: () => Date
  idleMs?: number
  once?: boolean
  // #4866 persistence knobs: delay between resubscribe attempts after a
  // relay failure, and an optional attempt cap (tests use a small cap; the
  // production loop resubscribes indefinitely).
  reconnectDelayMs?: number
  maxSubscribeAttempts?: number
  log?: (message: string) => void
}

type ProviderLoopRuntime = Required<Pick<ProviderLoopOptions, "now" | "idleMs">> &
  Omit<ProviderLoopOptions, "now" | "idleMs">

const textEncoder = new TextEncoder()

function bytesToHex(bytes: Uint8Array) {
  return Buffer.from(bytes).toString("hex")
}

function hexToBytes(hex: string) {
  if (!/^[0-9a-f]+$/i.test(hex) || hex.length % 2 !== 0) throw new Error("invalid hex")
  return Uint8Array.from(Buffer.from(hex, "hex"))
}

function stableRef(prefix: string, input: string) {
  return `${prefix}.${createHash("sha256").update(input).digest("hex").slice(0, 24)}`
}

function nowSeconds(now: Date) {
  return Math.floor(now.getTime() / 1000)
}

function serializeEvent(event: UnsignedNostrEvent) {
  return JSON.stringify([0, event.pubkey, event.created_at, event.kind, event.tags, event.content])
}

function unsignedEvent(input: unknown): UnsignedNostrEvent {
  return input as UnsignedNostrEvent
}

export function signNostrEvent(event: UnsignedNostrEvent, identity: PylonNostrPrivateIdentity): NostrEvent {
  const unsigned = { ...event, pubkey: identity.publicKey }
  const id = bytesToHex(sha256(textEncoder.encode(serializeEvent(unsigned))))
  const sig = bytesToHex(schnorr.sign(hexToBytes(id), identity.privateKeyBytes))
  return { ...unsigned, id, sig }
}

export function buildNip89HandlerInfoEvent(input: {
  identity: PylonNostrPrivateIdentity
  relayUrls: readonly string[]
  priceMsats: number
  now: Date
}) {
  const content = {
    name: "Pylon",
    about: "OpenAgents Pylon local NIP-90 text inference and labor provider.",
    picture: "https://openagents.com/pylon.png",
    nip90: {
      kinds: providerSupportedKinds(),
      backend: "pylon_local",
      pricing: input.priceMsats > 0 ? { amount: input.priceMsats, unit: "msats" } : { amount: 0, unit: "msats" },
    },
  }
  return signNostrEvent(
    {
      pubkey: input.identity.publicKey,
      created_at: nowSeconds(input.now),
      kind: NIP89_HANDLER_INFO_KIND,
      tags: [
        ["d", String(KIND_JOB_TEXT_GENERATION)],
        ...providerSupportedKinds().map((kind) => ["k", String(kind)]),
        ["t", "openagents"],
        ["t", "pylon"],
        ["t", "labor"],
        ["status", "healthy"],
        ["pricing", String(input.priceMsats), "msats"],
        ...input.relayUrls.map((relay) => ["relay", relay]),
      ],
      content: JSON.stringify(content),
    },
    input.identity,
  )
}

export function buildProviderReqFilters(input: {
  providerPubkey: string
  since: number
  limit?: number
}) {
  return [
    {
      kinds: providerSupportedKinds(),
      "#p": [input.providerPubkey],
      since: input.since,
      limit: input.limit ?? 64,
    },
    {
      kinds: providerSupportedKinds(),
      since: input.since,
      limit: input.limit ?? 64,
    },
    // Labor-market acceptances: kind-7000 feedback addressed to this
    // provider (the LBR quote-acceptance handshake).
    {
      kinds: [KIND_JOB_FEEDBACK],
      "#p": [input.providerPubkey],
      since: input.since,
      limit: input.limit ?? 64,
    },
  ]
}

function firstTagValue(tags: ReadonlyArray<readonly string[]>, name: string) {
  return tags.find((tag) => tag[0] === name)?.[1]
}

function requestPrompt(request: JobRequest) {
  const textInput = request.inputs.find((input) => input.inputType === "text")
  return textInput?.data.trim() || request.content.trim()
}

function requestModel(request: JobRequest) {
  return request.params.find((param) => param.key === "model")?.value
}

function previewText(value: string, maxChars = 96) {
  const normalized = value.replace(/\s+/g, " ").trim()
  return normalized.length <= maxChars ? normalized : `${normalized.slice(0, maxChars)}...`
}

export function classifyProviderRequestEvent(input: {
  event: NostrEvent
  providerPubkey: string
  relayUrl?: string
}): ProviderRequestEntry {
  const targeted = input.event.tags.some((tag) => tag[0] === "p")
  if (isLaborJobKind(input.event.kind)) {
    return classifyLaborRequestEvent(input, targeted)
  }
  if (input.event.kind !== KIND_JOB_TEXT_GENERATION) {
    return dropEntry(input, targeted, "unsupported_kind")
  }

  let request: JobRequest
  try {
    request = parseJobRequestEvent(input.event)
  } catch {
    return dropEntry(input, targeted, "malformed_request")
  }

  if (request.encrypted) {
    return dropEntry(input, targeted, "encrypted_request", request)
  }
  if (request.serviceProviders.length > 0 && !request.serviceProviders.includes(input.providerPubkey as never)) {
    return dropEntry(input, targeted, "target_mismatch", request)
  }
  if (request.output !== undefined && request.output !== "text/plain" && request.output !== "text/markdown") {
    return dropEntry(input, targeted, "unsupported_output", request)
  }

  const prompt = requestPrompt(request)
  if (!prompt) {
    return dropEntry(input, targeted, "missing_prompt", request)
  }

  return {
    requestEventId: input.event.id,
    requesterPubkey: input.event.pubkey,
    relayUrl: input.relayUrl,
    targeted,
    jobFamily: "text_generation",
    decision: "match",
    prompt,
    promptPreview: previewText(prompt),
    model: requestModel(request),
    bidMsats: request.bid,
    output: request.output,
    request,
  }
}

function classifyLaborRequestEvent(
  input: { event: NostrEvent; providerPubkey: string; relayUrl?: string },
  targeted: boolean,
): ProviderRequestEntry {
  let laborRequest: LaborJobRequest
  try {
    laborRequest = parseLaborJobRequestEvent(input.event)
  } catch {
    return dropEntry(input, targeted, "malformed_request")
  }

  const request = laborRequest.request
  if (request.encrypted) {
    return dropEntry(input, targeted, "encrypted_request", request)
  }
  if (request.serviceProviders.length > 0 && !request.serviceProviders.includes(input.providerPubkey as never)) {
    return dropEntry(input, targeted, "target_mismatch", request)
  }

  const prompt = request.content.trim() || laborRequest.acceptanceCriteria.join("\n")
  return {
    requestEventId: input.event.id,
    requesterPubkey: input.event.pubkey,
    relayUrl: input.relayUrl,
    targeted,
    jobFamily: "labor",
    decision: "match",
    prompt,
    promptPreview: previewText(prompt || laborRequest.inputRefs.join(" ")),
    bidMsats: request.bid,
    output: request.output,
    request,
    laborRequest,
  }
}

function dropEntry(
  input: { event: NostrEvent; relayUrl?: string },
  targeted: boolean,
  reason: ProviderDropReason,
  request?: JobRequest,
): ProviderRequestEntry {
  const prompt = request === undefined ? undefined : requestPrompt(request)
  return {
    requestEventId: input.event.id,
    requesterPubkey: input.event.pubkey,
    relayUrl: input.relayUrl,
    targeted,
    jobFamily: isLaborJobKind(input.event.kind) ? "labor" : "text_generation",
    decision: "drop",
    dropReason: reason,
    prompt: prompt || undefined,
    promptPreview: prompt ? previewText(prompt) : undefined,
    model: request === undefined ? undefined : requestModel(request),
    bidMsats: request?.bid,
    output: request?.output,
    request,
  }
}

export function defaultProviderAdmissionPolicy(input: Partial<ProviderAdmissionPolicy> = {}): ProviderAdmissionPolicy {
  return {
    priceMsats: input.priceMsats ?? DEFAULT_PROVIDER_PRICE_MSATS,
    requestTtlSeconds: input.requestTtlSeconds ?? DEFAULT_PROVIDER_REQUEST_TTL_SECONDS,
    maxInflight: input.maxInflight ?? DEFAULT_PROVIDER_MAX_INFLIGHT,
    perBuyerMaxInflight: input.perBuyerMaxInflight ?? DEFAULT_PROVIDER_PER_BUYER_MAX_INFLIGHT,
  }
}

export function emptyProviderAdmissionStore(): ProviderAdmissionStore {
  return {
    schema: "openagents.pylon.nip90_provider_state.v0.3",
    admissionLeases: {},
    handledRequests: {},
    earnings: [],
  }
}

export async function loadProviderAdmissionStore(paths: PylonPaths): Promise<ProviderAdmissionStore> {
  await ensureStateDirectories(paths)
  const path = providerStatePath(paths)
  if (!existsSync(path)) return emptyProviderAdmissionStore()
  const parsed = JSON.parse(await readFile(path, "utf8")) as ProviderAdmissionStore
  return {
    ...emptyProviderAdmissionStore(),
    ...parsed,
    admissionLeases: parsed.admissionLeases ?? {},
    handledRequests: parsed.handledRequests ?? {},
    earnings: parsed.earnings ?? [],
  }
}

export async function writeProviderAdmissionStore(paths: PylonPaths, store: ProviderAdmissionStore) {
  assertProviderStorePublicSafe(store)
  await writeFile(providerStatePath(paths), `${JSON.stringify(store, null, 2)}\n`)
}

export function providerStatePath(paths: PylonPaths) {
  return `${paths.home}/nip90-provider-state.json`
}

export function assertProviderStorePublicSafe(store: ProviderAdmissionStore) {
  assertPublicProjectionSafe(store)
  const serialized = JSON.stringify(store)
  if (/lnbc|lntb|lnbcrt|lno1|mnemonic|preimage|payment_hash|OPENAGENTS_AGENT_TOKEN|Bearer\s+/i.test(serialized)) {
    throw new Error("provider admission store contains private payment or auth material")
  }
}

function pruneAdmissionLeases(store: ProviderAdmissionStore, nowMs: number) {
  for (const [requestEventId, lease] of Object.entries(store.admissionLeases)) {
    if (lease.expiresAtMs <= nowMs || store.handledRequests[requestEventId]) {
      delete store.admissionLeases[requestEventId]
    }
  }
}

function activeCounts(store: ProviderAdmissionStore, requesterPubkey: string, nowMs: number) {
  pruneAdmissionLeases(store, nowMs)
  let total = 0
  let buyer = 0
  for (const lease of Object.values(store.admissionLeases)) {
    if (lease.expiresAtMs <= nowMs) continue
    total += 1
    if (lease.requesterPubkey === requesterPubkey) buyer += 1
  }
  return { total, buyer }
}

export function evaluateProviderAdmission(input: {
  entry: ProviderRequestEntry
  eventCreatedAtSeconds: number
  store: ProviderAdmissionStore
  policy?: Partial<ProviderAdmissionPolicy>
  now: Date
  online?: boolean
}): ProviderAdmissionDecision {
  if (input.entry.decision === "drop") {
    return { admitted: false, action: "drop", reasonRef: input.entry.dropReason ?? "malformed_request" }
  }
  if (input.online === false) {
    return { admitted: false, action: "drop", reasonRef: "provider_not_online" }
  }

  const policy = defaultProviderAdmissionPolicy(input.policy)
  const nowMs = input.now.getTime()
  const eventAgeSeconds = Math.max(0, Math.floor(nowMs / 1000) - input.eventCreatedAtSeconds)
  if (eventAgeSeconds > policy.requestTtlSeconds) {
    return { admitted: false, action: "drop", reasonRef: "stale_request" }
  }
  if (input.store.handledRequests[input.entry.requestEventId]) {
    return { admitted: false, action: "drop", reasonRef: "duplicate_request" }
  }
  const existingLease = input.store.admissionLeases[input.entry.requestEventId]
  if (existingLease && existingLease.expiresAtMs > nowMs) {
    return { admitted: false, action: "defer", reasonRef: "active_admission_lease" }
  }
  if (policy.priceMsats > 0) {
    if (input.entry.bidMsats === undefined) {
      return { admitted: false, action: "drop", reasonRef: "missing_bid" }
    }
    if (input.entry.bidMsats < policy.priceMsats) {
      return { admitted: false, action: "drop", reasonRef: "bid_below_price_floor" }
    }
  }

  const counts = activeCounts(input.store, input.entry.requesterPubkey, nowMs)
  if (counts.total >= policy.maxInflight) {
    return { admitted: false, action: "defer", reasonRef: "max_inflight" }
  }
  if (counts.buyer >= policy.perBuyerMaxInflight) {
    return { admitted: false, action: "defer", reasonRef: "buyer_limit" }
  }
  return { admitted: true, amountMsats: input.entry.bidMsats ?? policy.priceMsats }
}

export async function evaluateLaborAdmission(input: {
  entry: ProviderRequestEntry
  paths: PylonPaths
  workspaceRoot: string
}) {
  if (input.entry.laborRequest === undefined) {
    return { admitted: true as const }
  }

  const safetyBlocker = evaluateLaborRequestSafety(input.entry.laborRequest)[0]
  if (safetyBlocker !== undefined) {
    return { admitted: false as const, action: "drop" as const, reasonRef: safetyBlocker }
  }

  const workspace = resolveLaborWorkspace({
    root: input.workspaceRoot,
    requestedPath: requestedLaborWorkspacePath(input.entry.laborRequest),
  })
  if (workspace === undefined) {
    return { admitted: false as const, action: "drop" as const, reasonRef: "labor_workspace_out_of_bounds" as const }
  }

  const approved = await hasLaborFirstRunApproval(input.paths, input.entry.laborRequest)
  if (!approved) {
    return { admitted: false as const, action: "defer" as const, reasonRef: "labor_first_run_approval_required" as const }
  }

  return { admitted: true as const, workspace }
}

export function rememberAdmissionLease(input: {
  store: ProviderAdmissionStore
  entry: ProviderRequestEntry
  now: Date
  ttlMs?: number
}) {
  const expiresAtMs = input.now.getTime() + (input.ttlMs ?? 15 * 60 * 1000)
  input.store.admissionLeases[input.entry.requestEventId] = {
    requestEventId: input.entry.requestEventId,
    requesterPubkey: input.entry.requesterPubkey,
    status: "admitted",
    expiresAtMs,
  }
}

export async function createMdkBolt11Quote(input: {
  amountMsats: number
  runner?: WalletCommandRunner
}): Promise<ProviderPaymentQuote> {
  const amountSats = Math.max(1, Math.ceil(input.amountMsats / 1000))
  const runner = input.runner ?? defaultWalletCommandRunner
  const result = await runner(["receive", String(amountSats)])
  if (result.exitCode !== 0) {
    throw new Error("provider wallet invoice failed")
  }
  const parsed = result.stdout.trim() ? JSON.parse(result.stdout) as Record<string, unknown> : {}
  const invoice = typeof parsed.invoice === "string"
    ? parsed.invoice
    : typeof parsed.bolt11 === "string"
      ? parsed.bolt11
      : typeof parsed.payment_request === "string"
        ? parsed.payment_request
        : null
  if (!invoice) {
    throw new Error("provider wallet invoice response missing bolt11 invoice")
  }
  return {
    amountMsats: input.amountMsats,
    amountSats,
    bolt11: invoice,
    receiptRef: stableRef("receipt.public.pylon.nip90.invoice", `${amountSats}:${invoice}`),
  }
}

export async function runProviderJobOnce(input: {
  state: PylonLocalState
  event: NostrEvent
  identity: PylonNostrPrivateIdentity
  relay: ProviderRelayTransport
  store?: ProviderAdmissionStore
  runtime?: ProviderTextRuntime
  walletRunner?: WalletCommandRunner
  policy?: Partial<ProviderAdmissionPolicy>
  laborRuntime?: LaborRuntime
  laborWorkspaceRoot?: string
  laborAgentKind?: LaborLocalAgentKind
  laborMarket?: LaborMarketOptions
  now?: () => Date
  online?: boolean
}): Promise<ProviderRunResult> {
  const now = input.now?.() ?? new Date()

  // The labor-market negotiation lane routes first: LBR requests are
  // quoted, never auto-executed, and LBR acceptances trigger execution
  // of previously quoted jobs. Everything else falls through unchanged.
  const market = await handleLaborMarketEventOnce({
    state: input.state,
    event: input.event,
    identity: input.identity,
    relay: input.relay,
    options: { ...(input.laborMarket ?? {}), now: input.now ?? (() => now) },
  })
  if (market.handled) {
    return {
      requestEventId: input.event.id,
      status:
        market.action === "delivered"
          ? "completed"
          : market.action === "quoted"
            ? "accepted"
            : market.action === "deferred"
              ? "deferred"
              : market.action === "verification_failed"
                ? "error"
                : "dropped",
      feedbackEventIds: market.action === "quoted" ? [market.quoteEventId] : [],
      ...(market.action === "delivered" ? { resultEventId: market.resultEventId } : {}),
      laborMarket: market,
    }
  }

  const store = input.store ?? await loadProviderAdmissionStore(input.state.paths)
  const entry = classifyProviderRequestEvent({
    event: input.event,
    providerPubkey: input.identity.publicKey,
    relayUrl: input.relay.relayUrl,
  })
  const decision = evaluateProviderAdmission({
    entry,
    eventCreatedAtSeconds: input.event.created_at,
    store,
    policy: input.policy,
    now,
    online: input.online ?? (input.state.runtime.lifecycle === "online" || input.state.runtime.lifecycle === "assignment-ready"),
  })
  if (!decision.admitted) {
    if (decision.action === "drop") {
      store.handledRequests[input.event.id] = {
        requestEventId: input.event.id,
        requesterPubkey: input.event.pubkey,
        status: "rejected",
        reasonRef: decision.reasonRef,
        completedAt: now.toISOString(),
      }
      await writeProviderAdmissionStore(input.state.paths, store)
    }
    return {
      requestEventId: input.event.id,
      status: decision.action === "defer" ? "deferred" : "dropped",
      reasonRef: decision.reasonRef,
      feedbackEventIds: [],
    }
  }

  const laborAdmission = await evaluateLaborAdmission({
    entry,
    paths: input.state.paths,
    workspaceRoot: input.laborWorkspaceRoot ?? process.cwd(),
  })
  if (!laborAdmission.admitted) {
    if (laborAdmission.action === "drop") {
      store.handledRequests[input.event.id] = {
        requestEventId: input.event.id,
        requesterPubkey: input.event.pubkey,
        status: "rejected",
        reasonRef: laborAdmission.reasonRef,
        completedAt: now.toISOString(),
      }
      await writeProviderAdmissionStore(input.state.paths, store)
    }
    return {
      requestEventId: input.event.id,
      status: laborAdmission.action === "defer" ? "deferred" : "dropped",
      reasonRef: laborAdmission.reasonRef,
      feedbackEventIds: [],
    }
  }

  rememberAdmissionLease({ store, entry, now })
  await writeProviderAdmissionStore(input.state.paths, store)

  const feedbackEventIds: string[] = []
  let quote: ProviderPaymentQuote | undefined
  if (decision.amountMsats > 0) {
    quote = await createMdkBolt11Quote({ amountMsats: decision.amountMsats, runner: input.walletRunner })
    const paymentRequired = signNostrEvent(
      unsignedEvent(createJobFeedbackEvent(makeJobFeedback({
        status: "payment-required",
        requestId: input.event.id,
        requestRelay: input.relay.relayUrl,
        customerPubkey: input.event.pubkey,
        amount: quote.amountMsats,
        bolt11: quote.bolt11,
        statusExtra: "lightning settlement required",
        content: "",
      }))),
      input.identity,
    )
    await input.relay.publish(paymentRequired)
    feedbackEventIds.push(paymentRequired.id)
  }

  const processing = signNostrEvent(
    unsignedEvent(createJobFeedbackEvent(makeJobFeedback({
      status: "processing",
      requestId: input.event.id,
      requestRelay: input.relay.relayUrl,
      customerPubkey: input.event.pubkey,
      statusExtra: "processing with local Pylon runtime",
      content: "",
    }))),
    input.identity,
  )
  await input.relay.publish(processing)
  feedbackEventIds.push(processing.id)

  try {
    const result = entry.laborRequest === undefined
      ? await runTextJobResult({
          entry,
          event: input.event,
          identity: input.identity,
          quote,
          relayUrl: input.relay.relayUrl,
          runtime: input.runtime,
        })
      : await (async () => {
          // A defined laborRequest only reaches here via the admitted-with-
          // workspace branch of evaluateLaborAdmission; assert the invariant so
          // the workspace type is sound.
          if (laborAdmission.workspace === undefined) {
            throw new Error("labor admission accepted without a resolved workspace")
          }
          return runLaborResult({
            agentKind: input.laborAgentKind ?? detectConfiguredLaborAgent() ?? "codex",
            entry: entry as ProviderRequestEntry & { laborRequest: LaborJobRequest },
            event: input.event,
            identity: input.identity,
            quote,
            relayUrl: input.relay.relayUrl,
            runtime: input.laborRuntime ?? input.runtime ?? makeConfiguredLaborRuntime(),
            workspace: laborAdmission.workspace,
          })
        })()
    await input.relay.publish(result)

    const success = signNostrEvent(
      unsignedEvent(createJobFeedbackEvent(makeJobFeedback({
        status: "success",
        requestId: input.event.id,
        requestRelay: input.relay.relayUrl,
        customerPubkey: input.event.pubkey,
        statusExtra: "result published",
        content: "",
      }))),
      input.identity,
    )
    await input.relay.publish(success)
    feedbackEventIds.push(success.id)

    const earning = quote === undefined ? undefined : await recordProviderEarning(input.state.paths, {
      requestEventId: input.event.id,
      requesterPubkey: input.event.pubkey,
      amountMsats: quote.amountMsats,
      amountSats: quote.amountSats,
      receiptRef: stableRef("receipt.public.pylon.nip90.result", `${result.id}:${quote.amountMsats}`),
      resultEventId: result.id,
      recordedAt: (input.now?.() ?? new Date()).toISOString(),
    })
    if (earning !== undefined) {
      store.earnings = [
        ...store.earnings.filter((existing) => existing.requestEventId !== earning.requestEventId),
        earning,
      ].slice(-100)
    }
    store.handledRequests[input.event.id] = {
      requestEventId: input.event.id,
      requesterPubkey: input.event.pubkey,
      status: "completed",
      resultEventId: result.id,
      completedAt: (input.now?.() ?? new Date()).toISOString(),
    }
    delete store.admissionLeases[input.event.id]
    await writeProviderAdmissionStore(input.state.paths, store)
    return {
      requestEventId: input.event.id,
      status: "completed",
      feedbackEventIds,
      resultEventId: result.id,
      ...(earning === undefined ? {} : { earning }),
    }
  } catch (error) {
    const errorFeedback = signNostrEvent(
      unsignedEvent(createJobFeedbackEvent(makeJobFeedback({
        status: "error",
        requestId: input.event.id,
        requestRelay: input.relay.relayUrl,
        customerPubkey: input.event.pubkey,
        statusExtra: "local runtime failed",
        content: error instanceof Error ? error.message : String(error),
      }))),
      input.identity,
    )
    await input.relay.publish(errorFeedback)
    feedbackEventIds.push(errorFeedback.id)
    store.handledRequests[input.event.id] = {
      requestEventId: input.event.id,
      requesterPubkey: input.event.pubkey,
      status: "error",
      reasonRef: "runtime_error",
      completedAt: (input.now?.() ?? new Date()).toISOString(),
    }
    delete store.admissionLeases[input.event.id]
    await writeProviderAdmissionStore(input.state.paths, store)
    return {
      requestEventId: input.event.id,
      status: "error",
      reasonRef: "runtime_error",
      feedbackEventIds,
    }
  }
}

async function runTextJobResult(input: {
  entry: ProviderRequestEntry
  event: NostrEvent
  identity: PylonNostrPrivateIdentity
  quote?: ProviderPaymentQuote
  relayUrl: string
  runtime?: ProviderTextRuntime
}) {
  const runtime = input.runtime ?? await makeAppleFmProviderRuntime()
  const completion = await runtime.complete(input.entry.prompt ?? input.event.content)
  return signNostrEvent(
    unsignedEvent(createJobResultEvent(makeJobResult({
      requestKind: input.event.kind,
      requestId: input.event.id,
      requestRelay: input.relayUrl,
      customerPubkey: input.event.pubkey,
      content: completion.text,
      inputs: input.entry.request?.inputs ?? [],
      amount: input.quote?.amountMsats,
      bolt11: input.quote?.bolt11,
    }))),
    input.identity,
  )
}

async function runLaborResult(input: {
  agentKind: LaborLocalAgentKind
  entry: ProviderRequestEntry & { laborRequest: LaborJobRequest }
  event: NostrEvent
  identity: PylonNostrPrivateIdentity
  quote?: ProviderPaymentQuote
  relayUrl: string
  runtime: ProviderTextRuntime | LaborRuntime
  workspace: LaborWorkspace
}) {
  const runLabor = "runLabor" in input.runtime && typeof input.runtime.runLabor === "function"
    ? input.runtime.runLabor.bind(input.runtime)
    : makeConfiguredLaborRuntime().runLabor
  const completion = await runLabor({
    agentKind: input.agentKind,
    request: input.entry.laborRequest,
    requestEventId: input.event.id,
    workspace: input.workspace,
  })
  assertLaborPublicSafe(completion)
  const content = completion.content.trim()
    ? completion.content
    : laborResultContent({
        agentKind: input.agentKind,
        request: input.entry.laborRequest,
        artifactRefs: completion.artifactRefs,
        receiptRefs: completion.receiptRefs,
        summary: "Local labor agent completed.",
        workspace: input.workspace,
      })
  const laborResult = makeLaborJobResult({
    jobType: input.entry.laborRequest.jobType,
    requestId: input.event.id,
    requestRelay: input.relayUrl,
    customerPubkey: input.event.pubkey,
    artifactRefs: completion.artifactRefs,
    content,
    amount: input.quote?.amountMsats,
    bolt11: input.quote?.bolt11,
    policyRef: input.entry.laborRequest.policyRef,
  })
  return signNostrEvent(
    {
      pubkey: input.identity.publicKey,
      created_at: nowSeconds(new Date()),
      kind: laborResult.result.kind,
      tags: laborJobResultToTags(laborResult),
      content: laborResult.result.content,
    },
    input.identity,
  )
}

export async function recordProviderEarning(paths: PylonPaths, earning: ProviderEarningRecord) {
  const store = await loadProviderAdmissionStore(paths)
  store.earnings = [
    ...store.earnings.filter((existing) => existing.requestEventId !== earning.requestEventId),
    earning,
  ].slice(-100)
  await writeProviderAdmissionStore(paths, store)
  await appendLedgerEvent(paths, {
    kind: "provider.nip90.earning",
    ref: earning.receiptRef,
    data: {
      requestEventId: earning.requestEventId,
      amountMsats: earning.amountMsats,
      amountSats: earning.amountSats,
      receiptRef: earning.receiptRef,
      resultEventId: earning.resultEventId,
    },
  })
  return earning
}

async function makeAppleFmProviderRuntime(): Promise<ProviderTextRuntime> {
  const client = await Effect.runPromise(makeAppleFmClient()) as AppleFmClient
  await Effect.runPromise(client.requireReady())
  return {
    async complete(prompt: string) {
      const completion = await Effect.runPromise(client.completePlainText([{ role: "user", content: prompt }]))
      return {
        text: completion.text,
        model: client.profile.model,
        receiptRefs: [
          stableRef(
            "receipt.public.apple_fm.transcript",
            JSON.stringify({
              profileId: completion.receipt.profileId,
              model: completion.receipt.model,
              observedAt: completion.receipt.observedAt,
            }),
          ),
        ],
      }
    },
  }
}

export function policyFromEnv(env: NodeJS.ProcessEnv = process.env): ProviderAdmissionPolicy {
  return defaultProviderAdmissionPolicy({
    priceMsats: positiveIntEnv(env.PYLON_NIP90_PRICE_MSATS, DEFAULT_PROVIDER_PRICE_MSATS),
    requestTtlSeconds: positiveIntEnv(env.PYLON_NIP90_REQUEST_TTL_SECONDS, DEFAULT_PROVIDER_REQUEST_TTL_SECONDS),
    maxInflight: positiveIntEnv(env.PYLON_NIP90_MAX_INFLIGHT, DEFAULT_PROVIDER_MAX_INFLIGHT),
    perBuyerMaxInflight: positiveIntEnv(env.PYLON_NIP90_PER_BUYER_MAX_INFLIGHT, DEFAULT_PROVIDER_PER_BUYER_MAX_INFLIGHT),
  })
}

function positiveIntEnv(value: string | undefined, fallback: number) {
  if (value === undefined || value.trim() === "") return fallback
  const parsed = Number(value)
  return Number.isInteger(parsed) && parsed >= 0 ? parsed : fallback
}

/**
 * Closes every transport, isolating each `close()` call so one relay's
 * rejection never hides whether the others closed cleanly (Promise.all
 * cron-landmine audit). `Promise.all` starts every close concurrently, so
 * they are all initiated regardless of ordering, but it also rejects the
 * instant any ONE of them rejects without waiting for the rest to settle —
 * called from a `finally` block, that rejection would replace whatever
 * result the caller's `try` block already computed. This never throws:
 * failures are logged per-transport and the function always resolves once
 * every close has settled.
 */
export async function closeProviderRelayTransports(
  transports: readonly ProviderRelayTransport[],
  log?: (message: string) => void,
): Promise<void> {
  const results = await Promise.allSettled(transports.map((transport) => transport.close?.()))
  for (const [index, result] of results.entries()) {
    if (result.status === "rejected") {
      const relayUrl = transports[index]?.relayUrl ?? `#${index}`
      log?.(
        `[NIP-90] Failed to close relay transport ${relayUrl}: ${
          result.reason instanceof Error ? result.reason.message : String(result.reason)
        }`,
      )
    }
  }
}

export async function startNip90ProviderLoop(summary: BootstrapSummary, options: ProviderLoopOptions = {}) {
  const state = await ensurePylonLocalState(summary)
  const loopOptions: ProviderLoopRuntime = {
    ...options,
    now: options.now ?? (() => new Date()),
    idleMs: options.idleMs ?? DEFAULT_PROVIDER_LOOP_IDLE_MS,
  }
  const online = state.runtime.lifecycle === "online" || state.runtime.lifecycle === "assignment-ready"
  if (!online) {
    loopOptions.log?.(`[NIP-90] Provider loop not started; lifecycle is ${state.runtime.lifecycle}.`)
    return { started: false as const, reasonRef: "provider_not_online" as const }
  }
  const identity = await loadOrCreateNostrIdentity(summary.paths)
  const relays = options.relays?.length ? [...options.relays] : relaysFromEnv()
  const policy = defaultProviderAdmissionPolicy(options.policy ?? policyFromEnv())
  const runtime = options.runtime
  const walletRunner = options.walletRunner
  const laborRuntime = options.laborRuntime
  const laborWorkspaceRoot = options.laborWorkspaceRoot
  const laborAgentKind = options.laborAgentKind
  const transports = relays.map((relay) => new WebSocketRelayTransport(relay))

  try {
    for (const transport of transports) {
      const handler = buildNip89HandlerInfoEvent({ identity, relayUrls: relays, priceMsats: policy.priceMsats, now: loopOptions.now() })
      await transport.publish(handler)
      loopOptions.log?.(`[NIP-90] Published handler info to ${transport.relayUrl}.`)
    }

    let handled = 0
    for (const transport of transports) {
      // #4866 root-cause fix: any relay hiccup (idle "relay message timed
      // out", a dropped socket, a transient publish/runtime failure)
      // previously escaped this loop and permanently killed the supervised
      // NIP-90 service ("Service stopped with error"), so registered
      // providers went dark within about a minute of going online. Outside
      // bounded `once` runs, each failure logs and resubscribes after a
      // short delay instead of propagating.
      let attempts = 0
      while (true) {
        attempts += 1
        const since = Math.max(0, nowSeconds(loopOptions.now()) - 30)
        const filters = buildProviderReqFilters({ providerPubkey: identity.publicKey, since })
        try {
          for await (const event of transport.subscribe(filters, {
            subscriptionId: `pylon-provider-${state.identity.nodeId}`,
            keepAliveOnIdle: !options.once,
          })) {
            const result = await runProviderJobOnce({
              state,
              event,
              identity,
              relay: transport,
              policy,
              runtime,
              walletRunner,
              laborRuntime,
              laborWorkspaceRoot,
              laborAgentKind,
              ...(options.laborMarket === undefined ? {} : { laborMarket: options.laborMarket }),
              now: loopOptions.now,
              online: true,
            })
            handled += result.status === "completed" ? 1 : 0
            loopOptions.log?.(`[NIP-90] ${result.status} ${event.id}${result.reasonRef ? ` (${result.reasonRef})` : ""}.`)
            if (options.once) {
              return { started: true as const, handled }
            }
          }
        } catch (error) {
          if (options.once) throw error
          loopOptions.log?.(
            `[NIP-90] Relay subscription to ${transport.relayUrl} interrupted: ${
              error instanceof Error ? error.message : String(error)
            }; resubscribing.`,
          )
        }
        if (options.once) break
        if (options.maxSubscribeAttempts !== undefined && attempts >= options.maxSubscribeAttempts) break
        await sleep(options.reconnectDelayMs ?? DEFAULT_PROVIDER_RECONNECT_DELAY_MS)
      }
      if (options.once) break
    }

    return { started: true as const, handled }
  } finally {
    await closeProviderRelayTransports(transports, loopOptions.log)
  }
}

export class WebSocketRelayTransport implements ProviderRelayTransport {
  readonly relayUrl: string

  constructor(relayUrl: string) {
    this.relayUrl = normalizeRelayUrl(relayUrl)
  }

  close() {
    return Promise.resolve()
  }

  async publish(event: NostrEvent): Promise<ProviderPublishReceipt> {
    const ws = await openRelaySocket(this.relayUrl)
    try {
      ws.send(JSON.stringify(["EVENT", event]))
      const message = await waitForRelayMessage(ws, (parsed) => {
        if (Array.isArray(parsed) && parsed[0] === "OK" && parsed[1] === event.id) return parsed
        if (Array.isArray(parsed) && parsed[0] === "NOTICE") return parsed
        return null
      })
      if (message[0] === "OK") {
        return { relayUrl: this.relayUrl, accepted: message[2] === true, message: String(message[3] ?? "") }
      }
      return { relayUrl: this.relayUrl, accepted: false, message: String(message[1] ?? "relay notice") }
    } finally {
      ws.close()
    }
  }

  async *subscribe(
    filters: ReadonlyArray<Record<string, unknown>>,
    options: { subscriptionId?: string; keepAliveOnIdle?: boolean; idleTimeoutMs?: number } = {},
  ) {
    const ws = await openRelaySocket(this.relayUrl)
    const subscriptionId = options.subscriptionId ?? `pylon-provider-${Date.now()}`
    const idleTimeoutMs = options.idleTimeoutMs ?? 60_000
    try {
      ws.send(JSON.stringify(["REQ", subscriptionId, ...filters]))
      while (true) {
        // A socket that died while a job was being processed (no listener
        // attached) fires no late close event; detect it here instead of
        // hanging until the idle timeout.
        if (ws.readyState !== WebSocket.OPEN) {
          throw new Error("relay websocket closed")
        }
        let message: unknown[]
        try {
          message = await waitForRelayMessage(ws, (parsed) => Array.isArray(parsed) ? parsed : null, idleTimeoutMs)
        } catch (error) {
          // #4866: a quiet relay is not a dead relay. Between jobs the
          // subscription legitimately sees no frames for minutes; with
          // keepAliveOnIdle the idle timeout keeps waiting on the open
          // socket instead of tearing the provider subscription down.
          if (
            options.keepAliveOnIdle === true &&
            ws.readyState === WebSocket.OPEN &&
            error instanceof Error &&
            error.message === "relay message timed out"
          ) {
            continue
          }
          throw error
        }
        if (message[0] === "EVENT" && message[1] === subscriptionId && isNostrEvent(message[2])) {
          yield message[2]
        }
        if (message[0] === "EOSE" && message[1] === subscriptionId) {
          await sleep(DEFAULT_PROVIDER_LOOP_IDLE_MS)
        }
        if (message[0] === "CLOSED" && message[1] === subscriptionId) {
          break
        }
      }
    } finally {
      ws.send(JSON.stringify(["CLOSE", subscriptionId]))
      ws.close()
    }
  }
}

function normalizeRelayUrl(value: string) {
  const url = /^[a-z]+:\/\//i.test(value) ? new URL(value) : new URL(`wss://${value}`)
  if (url.protocol !== "wss:" && url.protocol !== "ws:") {
    throw new Error(`Expected ws/wss relay URL, got ${value}`)
  }
  return url.toString()
}

function openRelaySocket(relayUrl: string): Promise<WebSocket> {
  const ws = new WebSocket(relayUrl)
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => {
      cleanup()
      ws.close()
      reject(new Error("relay websocket open timed out"))
    }, 10_000)
    const cleanup = () => {
      clearTimeout(timeout)
      ws.removeEventListener("open", onOpen)
      ws.removeEventListener("error", onError)
    }
    const onOpen = () => {
      cleanup()
      resolve(ws)
    }
    const onError = () => {
      cleanup()
      reject(new Error("relay websocket failed before open"))
    }
    ws.addEventListener("open", onOpen)
    ws.addEventListener("error", onError)
  })
}

function waitForRelayMessage<T>(
  ws: WebSocket,
  match: (parsed: unknown) => T | null,
  timeoutMs = 15_000,
): Promise<T> {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => {
      cleanup()
      reject(new Error("relay message timed out"))
    }, timeoutMs)
    const cleanup = () => {
      clearTimeout(timeout)
      ws.removeEventListener("message", onMessage)
      ws.removeEventListener("error", onError)
      ws.removeEventListener("close", onClose)
    }
    const onMessage = (event: MessageEvent) => {
      try {
        const parsed = JSON.parse(String(event.data))
        const matched = match(parsed)
        if (matched !== null) {
          cleanup()
          resolve(matched)
        }
      } catch (error) {
        cleanup()
        reject(error)
      }
    }
    const onError = () => {
      cleanup()
      reject(new Error("relay websocket error"))
    }
    const onClose = () => {
      cleanup()
      reject(new Error("relay websocket closed"))
    }
    ws.addEventListener("message", onMessage)
    ws.addEventListener("error", onError)
    ws.addEventListener("close", onClose)
  })
}

function isNostrEvent(value: unknown): value is NostrEvent {
  const event = value as NostrEvent
  return (
    event !== null &&
    typeof event === "object" &&
    typeof event.id === "string" &&
    typeof event.pubkey === "string" &&
    typeof event.created_at === "number" &&
    typeof event.kind === "number" &&
    Array.isArray(event.tags) &&
    typeof event.content === "string" &&
    typeof event.sig === "string"
  )
}

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms))
}
