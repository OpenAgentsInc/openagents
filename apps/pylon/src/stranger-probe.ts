import { createHash } from "node:crypto"
import type { Brand } from "effect"
import { generateMnemonic } from "@scure/bip39"
import { wordlist } from "@scure/bip39/wordlists/english"
import {
  KIND_JOB_FEEDBACK,
  KIND_JOB_TEXT_GENERATION,
  createJobRequestEvent,
  getResultKind,
  jobInput,
  makeJobRequest,
} from "@openagentsinc/nip90"
import { deriveNip06Identity, type PylonNostrPrivateIdentity } from "./nostr-identity.js"
import {
  OPENAGENTS_MARKET_RELAY_URL,
  WebSocketRelayTransport,
  signNostrEvent,
  type NostrEvent,
  type ProviderPublishReceipt,
} from "./provider-nip90.js"
import { assertPublicProjectionSafe } from "./state.js"
import type { WalletCommandRunner } from "./wallet.js"

// #4866: repeatable stranger-buyer NIP-90 probe smoke, replaying the Orrery
// probe shape (forum topic 499cec6e, post 7be6aa0a) against the canonical
// market relay with registered-capacity mapping via the #4864 /api/pylons
// provider fields. No-spend by default; the paid leg is operator-gated.

export const STRANGER_PROBE_SCHEMA = "openagents.pylon.stranger_probe_smoke.v0.1"
export const ENV_STRANGER_PROBE_ALLOW_SPEND = "PYLON_STRANGER_PROBE_ALLOW_SPEND"
export const DEFAULT_STRANGER_PROBE_BID_MSATS = 21_000
export const DEFAULT_STRANGER_PROBE_COLLECT_BUDGET_MS = 30_000
export const DEFAULT_STRANGER_PROBE_BASE_URL = "https://openagents.com"
export const DEFAULT_STRANGER_PROBE_PROMPT =
  "OpenAgents stranger-buyer probe: reply with one sentence confirming receipt."

export const STRANGER_PROBE_PROVENANCE = {
  issueRef: "openagents#4866",
  probeShapeSourceRef: "probe.orrery.stranger_buyer.2026_06_12",
  forumTopicRef: "forum.topic.499cec6e-c09e-45a7-8c24-4bcee8fc87dc",
  forumPostRef: "forum.post.7be6aa0a-c64a-466f-b90e-45e1d24ef93f",
} as const

const JOB_RESULT_KIND = getResultKind(KIND_JOB_TEXT_GENERATION) ?? 6_050

function stableRef(prefix: string, input: string) {
  return `${prefix}.${createHash("sha256").update(input).digest("hex").slice(0, 24)}`
}

// --- registered-capacity mapping (the #4864 fields are the point) ---------

export type RegisteredProviderEntry = {
  pylonRef: string
  providerNostrPubkey: string
  providerNostrNpub?: string
  providerNip90LaneRefs: string[]
  providerMarketRelayRefs: string[]
  latestHeartbeatStatus?: string
  capabilityRefs: string[]
}

export type RegisteredProviderMap = {
  providers: RegisteredProviderEntry[]
  byPubkey: Map<string, RegisteredProviderEntry[]>
  skippedWithoutPubkey: number
}

function stringArray(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : []
}

export function buildRegisteredProviderMap(payload: unknown): RegisteredProviderMap {
  const pylons = payload !== null && typeof payload === "object" && Array.isArray((payload as { pylons?: unknown }).pylons)
    ? (payload as { pylons: unknown[] }).pylons
    : []
  const providers: RegisteredProviderEntry[] = []
  const byPubkey = new Map<string, RegisteredProviderEntry[]>()
  let skippedWithoutPubkey = 0
  for (const raw of pylons) {
    if (raw === null || typeof raw !== "object") continue
    const record = raw as Record<string, unknown>
    const pylonRef = typeof record.pylonRef === "string" ? record.pylonRef : undefined
    const pubkey = typeof record.providerNostrPubkey === "string" && /^[0-9a-f]{64}$/i.test(record.providerNostrPubkey)
      ? record.providerNostrPubkey.toLowerCase()
      : undefined
    if (pylonRef === undefined) continue
    if (pubkey === undefined) {
      skippedWithoutPubkey += 1
      continue
    }
    const entry: RegisteredProviderEntry = {
      pylonRef,
      providerNostrPubkey: pubkey,
      ...(typeof record.providerNostrNpub === "string" ? { providerNostrNpub: record.providerNostrNpub } : {}),
      providerNip90LaneRefs: stringArray(record.providerNip90LaneRefs),
      providerMarketRelayRefs: stringArray(record.providerMarketRelayRefs),
      ...(typeof record.latestHeartbeatStatus === "string"
        ? { latestHeartbeatStatus: record.latestHeartbeatStatus }
        : {}),
      capabilityRefs: stringArray(record.capabilityRefs),
    }
    providers.push(entry)
    byPubkey.set(pubkey, [...(byPubkey.get(pubkey) ?? []), entry])
  }
  return { providers, byPubkey, skippedWithoutPubkey }
}

export async function fetchRegisteredProviderMap(baseUrl: string): Promise<RegisteredProviderMap> {
  const response = await fetch(new URL("/api/pylons", baseUrl))
  if (!response.ok) {
    throw new Error(`/api/pylons failed (${response.status})`)
  }
  return buildRegisteredProviderMap(await response.json())
}

// --- throwaway customer identity -------------------------------------------

// The throwaway customer key lives only in process memory: a fresh BIP-39
// mnemonic is derived to a NIP-06 identity with an in-memory path marker and
// is never written to disk, the artifact, or logs.
export function makeThrowawayCustomerIdentity(): PylonNostrPrivateIdentity {
  return deriveNip06Identity(generateMnemonic(wordlist, 128), "memory:stranger-probe-throwaway")
}

// --- probe request and response filters ------------------------------------

// `createJobRequestEvent` takes a branded UnixTimestamp. The nip90 dependency
// intentionally does not re-export the `UnixTimestamp` schema/type, so we
// reconstruct its structural brand locally. The brand is purely nominal — the
// value here is already a validated integer unix-seconds timestamp produced by
// the probe — so this carries no runtime risk and stays type-sound.
type UnixTimestamp = number & Brand.Brand<"UnixTimestamp">

export function buildProbeJobRequestEvent(input: {
  identity: PylonNostrPrivateIdentity
  prompt: string
  bidMsats: number
  createdAtSeconds: number
}): NostrEvent {
  const request = makeJobRequest({
    kind: KIND_JOB_TEXT_GENERATION,
    inputs: [jobInput.text(input.prompt)],
    bid: input.bidMsats,
    output: "text/plain",
    // The stranger shape is untargeted on purpose: any responder is mapped
    // afterwards, instead of pre-selecting a provider.
    serviceProviders: [],
  })
  const createdAt = input.createdAtSeconds as UnixTimestamp
  return signNostrEvent(createJobRequestEvent(request, createdAt), input.identity)
}

export function buildProbeResponseFilters(input: {
  requestEventId: string
  customerPubkey: string
  sinceSeconds: number
  limit?: number
}) {
  const kinds = [KIND_JOB_FEEDBACK, JOB_RESULT_KIND]
  const limit = input.limit ?? 64
  return [
    { kinds, "#e": [input.requestEventId], since: input.sinceSeconds, limit },
    { kinds, "#p": [input.customerPubkey], since: input.sinceSeconds, limit },
  ]
}

// --- bounded response collection --------------------------------------------

export type ProbeCollectionResult = {
  events: NostrEvent[]
  relayClosedEarly: boolean
}

export type ProbeResponseCollector = (input: {
  relayUrl: string
  filters: ReadonlyArray<Record<string, unknown>>
  budgetMs: number
  subscriptionId?: string
}) => Promise<ProbeCollectionResult>

// A dedicated bounded collector: unlike the provider loop's long-lived
// subscribe iterator, this opens one socket, gathers matching events until
// the hard deadline, then closes. It never blocks past the budget.
export const collectProbeResponsesOverWebSocket: ProbeResponseCollector = (input) => {
  const subscriptionId = input.subscriptionId ?? `stranger-probe-${Date.now()}`
  return new Promise((resolve, reject) => {
    const events: NostrEvent[] = []
    const seen = new Set<string>()
    let settled = false
    let opened = false
    const ws = new WebSocket(input.relayUrl)

    const finish = (relayClosedEarly: boolean) => {
      if (settled) return
      settled = true
      clearTimeout(deadline)
      try {
        if (ws.readyState === WebSocket.OPEN) {
          ws.send(JSON.stringify(["CLOSE", subscriptionId]))
        }
        ws.close()
      } catch {
        // closing is best-effort; the deadline already bounds the probe
      }
      resolve({ events, relayClosedEarly })
    }

    const fail = (error: Error) => {
      if (settled) return
      settled = true
      clearTimeout(deadline)
      try {
        ws.close()
      } catch {
        // ignore close failures on an already-failed socket
      }
      reject(error)
    }

    const deadline = setTimeout(() => finish(false), input.budgetMs)

    ws.addEventListener("open", () => {
      opened = true
      ws.send(JSON.stringify(["REQ", subscriptionId, ...input.filters]))
    })
    ws.addEventListener("message", (message) => {
      try {
        const parsed = JSON.parse(String(message.data)) as unknown[]
        if (!Array.isArray(parsed)) return
        if (parsed[0] === "EVENT" && parsed[1] === subscriptionId && isProbeNostrEvent(parsed[2])) {
          const event = parsed[2]
          if (!seen.has(event.id)) {
            seen.add(event.id)
            events.push(event)
          }
        }
        if (parsed[0] === "CLOSED" && parsed[1] === subscriptionId) {
          finish(true)
        }
      } catch {
        // ignore malformed relay frames; the deadline still bounds the probe
      }
    })
    ws.addEventListener("error", () => {
      if (!opened) fail(new Error("stranger probe relay websocket failed before open"))
      else finish(true)
    })
    ws.addEventListener("close", () => {
      if (!opened) fail(new Error("stranger probe relay websocket closed before open"))
      else finish(true)
    })
  })
}

function isProbeNostrEvent(value: unknown): value is NostrEvent {
  const event = value as NostrEvent
  return (
    event !== null &&
    typeof event === "object" &&
    typeof event.id === "string" &&
    typeof event.pubkey === "string" &&
    typeof event.created_at === "number" &&
    typeof event.kind === "number" &&
    Array.isArray(event.tags) &&
    typeof event.content === "string"
  )
}

// --- responder classification -----------------------------------------------

export type ProbeResponderRecord = {
  eventId: string
  responderPubkey: string
  kind: number
  kindLabel: "job_feedback" | "job_result" | "other"
  status?: string
  amountMsats?: number
  // Redaction-safe by construction: the artifact records only whether a
  // bolt11 invoice was attached, never the invoice string itself.
  hasBolt11Invoice: boolean
  contentLength: number
  classification: "registered" | "unregistered"
  registeredPylonRefs: string[]
  observedAt: string
}

function firstTag(tags: ReadonlyArray<readonly string[]>, name: string) {
  return tags.find((tag) => tag[0] === name)
}

export function probeResponseReferencesRequest(event: NostrEvent, requestEventId: string, customerPubkey: string) {
  return event.tags.some(
    (tag) => (tag[0] === "e" && tag[1] === requestEventId) || (tag[0] === "p" && tag[1] === customerPubkey),
  )
}

export function classifyProbeResponse(input: {
  event: NostrEvent
  map: RegisteredProviderMap
  requestEventId: string
  customerPubkey: string
  observedAt: Date
}): ProbeResponderRecord | null {
  const { event } = input
  if (event.pubkey === input.customerPubkey) return null
  if (!probeResponseReferencesRequest(event, input.requestEventId, input.customerPubkey)) return null

  const statusTag = firstTag(event.tags, "status")
  const amountTag = firstTag(event.tags, "amount")
  const amountMsats = amountTag?.[1] !== undefined && /^\d+$/.test(amountTag[1]) ? Number(amountTag[1]) : undefined
  const hasBolt11Invoice =
    (amountTag?.[2] !== undefined && amountTag[2] !== "") || firstTag(event.tags, "bolt11")?.[1] !== undefined
  const registered = input.map.byPubkey.get(event.pubkey.toLowerCase()) ?? []

  return {
    eventId: event.id,
    responderPubkey: event.pubkey,
    kind: event.kind,
    kindLabel:
      event.kind === KIND_JOB_FEEDBACK ? "job_feedback" : event.kind === JOB_RESULT_KIND ? "job_result" : "other",
    ...(statusTag?.[1] !== undefined ? { status: statusTag[1].slice(0, 64) } : {}),
    ...(amountMsats !== undefined ? { amountMsats } : {}),
    hasBolt11Invoice,
    contentLength: event.content.length,
    classification: registered.length > 0 ? "registered" : "unregistered",
    registeredPylonRefs: registered.map((entry) => entry.pylonRef),
    observedAt: input.observedAt.toISOString(),
  }
}

// --- operator-gated paid leg --------------------------------------------------

export type PaidLegGate =
  | { authorized: true }
  | { authorized: false; reasonRef: string }

// The paid leg settles a real invoice with real sats. It refuses unless BOTH
// the explicit --paid flag AND the spend env guard are present, so neither a
// stray flag nor a stray environment variable can authorize spend alone.
export function evaluatePaidLegGate(input: { paidFlag: boolean; env?: NodeJS.ProcessEnv }): PaidLegGate {
  const env = input.env ?? process.env
  const envAllowed = env[ENV_STRANGER_PROBE_ALLOW_SPEND] === "1"
  if (!input.paidFlag && !envAllowed) {
    return { authorized: false, reasonRef: "blocker.pylon.stranger_probe.paid_leg_not_requested" }
  }
  if (input.paidFlag && !envAllowed) {
    return { authorized: false, reasonRef: "blocker.pylon.stranger_probe.spend_env_guard_missing" }
  }
  if (!input.paidFlag && envAllowed) {
    return { authorized: false, reasonRef: "blocker.pylon.stranger_probe.paid_flag_missing" }
  }
  return { authorized: true }
}

export type ProbePaidLegRecord =
  | { attempted: false; authorized: boolean; reasonRef: string }
  | {
      attempted: true
      authorized: true
      settled: boolean
      amountMsats: number
      quoteEventId: string
      providerPubkey: string
      registeredPylonRefs: string[]
      settlementReceiptRef: string
      resultEventId?: string
      settledAt?: string
      blockerRefs: string[]
    }

// --- artifact ------------------------------------------------------------------

export type StrangerProbeArtifact = {
  schema: typeof STRANGER_PROBE_SCHEMA
  mode: "no_spend" | "paid"
  relayUrl: string
  baseUrl: string
  probeStartedAt: string
  probeCompletedAt: string
  provenance: typeof STRANGER_PROBE_PROVENANCE
  request: {
    eventId: string
    kind: number
    bidMsats: number
    output: string
    customerPubkey: string
    publishedAt: string
    acceptedByRelay: boolean
    relayMessage: string
  }
  registeredCapacity: {
    sourceRoute: "/api/pylons"
    providerCount: number
    skippedWithoutPubkey: number
    providers: RegisteredProviderEntry[]
  }
  collection: {
    budgetMs: number
    collectedEventCount: number
    relayClosedEarly: boolean
    responderCount: number
    registeredResponderCount: number
    unregisteredResponderCount: number
    responses: ProbeResponderRecord[]
  }
  paidLeg: ProbePaidLegRecord
  verdict: {
    status: "passed" | "blocked"
    zeroRegisteredResponders: boolean
    blockerRefs: string[]
  }
}

export function assertStrangerProbeArtifactPublicSafe(artifact: StrangerProbeArtifact) {
  assertPublicProjectionSafe(artifact)
  const serialized = JSON.stringify(artifact)
  if (/lnbc|lntb|lnbcrt|lno1|mnemonic|preimage|payment_hash|nsec1|xprv|privateKey|Bearer\s+/i.test(serialized)) {
    throw new Error("stranger probe artifact contains private payment, key, or auth material")
  }
}

// --- orchestration ----------------------------------------------------------------

export type StrangerProbeOptions = {
  relayUrl?: string
  baseUrl?: string
  bidMsats?: number
  prompt?: string
  collectBudgetMs?: number
  paidFlag?: boolean
  env?: NodeJS.ProcessEnv
  identity?: PylonNostrPrivateIdentity
  fetchProviders?: (baseUrl: string) => Promise<RegisteredProviderMap>
  publishRequest?: (relayUrl: string, event: NostrEvent) => Promise<ProviderPublishReceipt>
  collectResponses?: ProbeResponseCollector
  walletRunner?: WalletCommandRunner
  now?: () => Date
}

async function defaultPublishRequest(relayUrl: string, event: NostrEvent) {
  return new WebSocketRelayTransport(relayUrl).publish(event)
}

export async function runStrangerProbe(options: StrangerProbeOptions = {}): Promise<StrangerProbeArtifact> {
  const now = options.now ?? (() => new Date())
  const relayUrl = options.relayUrl ?? OPENAGENTS_MARKET_RELAY_URL
  const baseUrl = options.baseUrl ?? DEFAULT_STRANGER_PROBE_BASE_URL
  const bidMsats = options.bidMsats ?? DEFAULT_STRANGER_PROBE_BID_MSATS
  const collectBudgetMs = options.collectBudgetMs ?? DEFAULT_STRANGER_PROBE_COLLECT_BUDGET_MS
  const gate = evaluatePaidLegGate({ paidFlag: options.paidFlag ?? false, env: options.env })
  const fetchProviders = options.fetchProviders ?? fetchRegisteredProviderMap
  const publishRequest = options.publishRequest ?? defaultPublishRequest
  const collectResponses = options.collectResponses ?? collectProbeResponsesOverWebSocket

  const probeStartedAt = now()
  const map = await fetchProviders(baseUrl)

  // Throwaway stranger identity: generated here, used to sign exactly one
  // bounded request, and dropped when this function returns.
  const identity = options.identity ?? makeThrowawayCustomerIdentity()
  const requestEvent = buildProbeJobRequestEvent({
    identity,
    prompt: options.prompt ?? DEFAULT_STRANGER_PROBE_PROMPT,
    bidMsats,
    createdAtSeconds: Math.floor(probeStartedAt.getTime() / 1000),
  })
  const publishReceipt = await publishRequest(relayUrl, requestEvent)
  const publishedAt = now()

  const collected = publishReceipt.accepted
    ? await collectResponses({
        relayUrl,
        filters: buildProbeResponseFilters({
          requestEventId: requestEvent.id,
          customerPubkey: identity.publicKey,
          sinceSeconds: Math.floor(probeStartedAt.getTime() / 1000) - 60,
        }),
        budgetMs: collectBudgetMs,
      })
    : { events: [], relayClosedEarly: false }

  const responses: ProbeResponderRecord[] = []
  for (const event of collected.events) {
    const record = classifyProbeResponse({
      event,
      map,
      requestEventId: requestEvent.id,
      customerPubkey: identity.publicKey,
      observedAt: now(),
    })
    if (record !== null) responses.push(record)
  }
  const registeredResponderCount = responses.filter((record) => record.classification === "registered").length

  const paidLeg = gate.authorized
    ? await runPaidLeg({
        collected: collected.events,
        responses,
        map,
        requestEventId: requestEvent.id,
        customerPubkey: identity.publicKey,
        walletRunner: options.walletRunner,
        now,
      })
    : { attempted: false as const, authorized: false, reasonRef: gate.reasonRef }

  const blockerRefs = [
    ...(publishReceipt.accepted ? [] : ["blocker.pylon.stranger_probe.request_rejected_by_relay"]),
  ]
  const artifact: StrangerProbeArtifact = {
    schema: STRANGER_PROBE_SCHEMA,
    mode: paidLeg.attempted ? "paid" : "no_spend",
    relayUrl,
    baseUrl,
    probeStartedAt: probeStartedAt.toISOString(),
    probeCompletedAt: now().toISOString(),
    provenance: STRANGER_PROBE_PROVENANCE,
    request: {
      eventId: requestEvent.id,
      kind: requestEvent.kind,
      bidMsats,
      output: "text/plain",
      customerPubkey: identity.publicKey,
      publishedAt: publishedAt.toISOString(),
      acceptedByRelay: publishReceipt.accepted,
      relayMessage: publishReceipt.message.slice(0, 160),
    },
    registeredCapacity: {
      sourceRoute: "/api/pylons",
      providerCount: map.providers.length,
      skippedWithoutPubkey: map.skippedWithoutPubkey,
      providers: map.providers,
    },
    collection: {
      budgetMs: collectBudgetMs,
      collectedEventCount: collected.events.length,
      relayClosedEarly: collected.relayClosedEarly,
      responderCount: responses.length,
      registeredResponderCount,
      unregisteredResponderCount: responses.length - registeredResponderCount,
      responses,
    },
    paidLeg,
    verdict: {
      status: blockerRefs.length === 0 ? "passed" : "blocked",
      zeroRegisteredResponders: registeredResponderCount === 0,
      blockerRefs,
    },
  }
  assertStrangerProbeArtifactPublicSafe(artifact)
  return artifact
}

// The paid leg runs only after evaluatePaidLegGate authorized it. It settles
// the first registered payment-required quote via the MDK wallet runner. The
// bolt11 invoice stays in memory; the artifact records refs and amounts only.
async function runPaidLeg(input: {
  collected: NostrEvent[]
  responses: ProbeResponderRecord[]
  map: RegisteredProviderMap
  requestEventId: string
  customerPubkey: string
  walletRunner?: WalletCommandRunner
  now: () => Date
}): Promise<ProbePaidLegRecord> {
  const quoteRecord = input.responses.find(
    (record) =>
      record.classification === "registered" &&
      record.status === "payment-required" &&
      record.hasBolt11Invoice &&
      record.amountMsats !== undefined,
  )
  if (quoteRecord === undefined) {
    return {
      attempted: false,
      authorized: true,
      reasonRef: "blocker.pylon.stranger_probe.no_registered_payment_required_quote",
    }
  }
  const quoteEvent = input.collected.find((event) => event.id === quoteRecord.eventId)
  const amountTag = quoteEvent === undefined ? undefined : firstTag(quoteEvent.tags, "amount")
  const bolt11 = amountTag?.[2] ?? (quoteEvent === undefined ? undefined : firstTag(quoteEvent.tags, "bolt11")?.[1])
  if (input.walletRunner === undefined || bolt11 === undefined || bolt11 === "") {
    return {
      attempted: false,
      authorized: true,
      reasonRef: "blocker.pylon.stranger_probe.wallet_runner_or_invoice_unavailable",
    }
  }

  const payment = await input.walletRunner(["send", bolt11])
  const settled = payment.exitCode === 0
  const settledAt = input.now().toISOString()
  const resultEventId = input.responses.find(
    (record) => record.responderPubkey === quoteRecord.responderPubkey && record.kindLabel === "job_result",
  )?.eventId
  return {
    attempted: true,
    authorized: true,
    settled,
    amountMsats: quoteRecord.amountMsats ?? 0,
    quoteEventId: quoteRecord.eventId,
    providerPubkey: quoteRecord.responderPubkey,
    registeredPylonRefs: quoteRecord.registeredPylonRefs,
    settlementReceiptRef: stableRef(
      "receipt.public.pylon.stranger_probe.settlement",
      `${input.requestEventId}:${quoteRecord.eventId}:${quoteRecord.amountMsats ?? 0}`,
    ),
    ...(resultEventId === undefined ? {} : { resultEventId }),
    ...(settled ? { settledAt } : {}),
    blockerRefs: settled ? [] : ["blocker.pylon.stranger_probe.settlement_failed"],
  }
}
