import { createHash } from "node:crypto"
import { PYLON_CLIENT_VERSION, type PylonClientVersion } from "./version.js"
import type { BootstrapSummary } from "./bootstrap.js"
import {
  PYLON_NIP90_PROVIDER_CAPABILITY_REF,
  providerNip90LaneRefs,
  relaysFromEnv,
} from "./provider-nip90.js"
import { publishableCapabilityRefs } from "./tassadar-capability.js"
import { createNip98Event, encodeNip98Authorization, loadOrCreateNostrIdentity } from "./nostr-identity.js"
import {
  assertPublicProjectionSafe,
  ensurePylonLocalState,
  loadOrCreatePresenceState,
  type PylonLocalState,
  type PylonPresenceState,
  type PylonRuntimeState,
  writePresenceState,
} from "./state.js"
import type { WalletStatusProjection } from "./wallet.js"
import { PresenceRequestError } from "./presence-error.js"

// The fields of the local wallet probe the heartbeat needs to publish
// receive-readiness (openagents #5151). A full WalletStatusProjection satisfies
// this. Live Pylon entry points inject the Spark-primary probe. When no probe is
// provided, heartbeat omits walletReady rather than spawning any wallet backend.
export type HeartbeatWalletProbe = Pick<
  WalletStatusProjection,
  "configured" | "daemonOnline" | "receiveReady" | "sendReady"
>

export type PresenceClientOptions = {
  agentToken?: string
  baseUrl: string
  env?: NodeJS.ProcessEnv
  fetch?: typeof fetch
  now?: () => Date
  // Local wallet probe used by `sendHeartbeat` to publish live receive-readiness
  // so an online, receive-ready node is not shown `walletReadyNow=false` until a
  // separate `wallet report-readiness` (openagents #5151). The live node injects
  // the Spark-primary probe; no default probe is run.
  walletProbe?: () => Promise<HeartbeatWalletProbe>
}

// Map the local wallet probe to the heartbeat's public readiness fields. The
// boolean `walletReady` is what the server projects into `walletReadyNow`; the
// `walletReadiness` enum is the richer public-safe label. Receive-ready is the
// bar for "can receive a tip/payout".
export const heartbeatWalletReadiness = (
  status: HeartbeatWalletProbe,
): {
  walletReadiness: PylonHeartbeatRequest["walletReadiness"]
  walletReady: boolean
} => {
  if (!status.configured || !status.daemonOnline) {
    return { walletReadiness: "offline", walletReady: false }
  }
  if (status.sendReady) return { walletReadiness: "send-ready", walletReady: true }
  if (status.receiveReady)
    return { walletReadiness: "receive-ready", walletReady: true }
  return { walletReadiness: "blocked", walletReady: false }
}

export function presenceClientOptionsFromEnv(input: {
  baseUrl: string
  env?: NodeJS.ProcessEnv
}): PresenceClientOptions {
  const env = input.env ?? process.env
  const agentToken = env.OPENAGENTS_AGENT_TOKEN?.trim()
  return {
    baseUrl: input.baseUrl,
    env,
    ...(agentToken ? { agentToken } : {}),
  }
}

export type PylonProviderDiscoveryFields = {
  providerNostrPubkey: string
  providerNostrNpub: string
  providerMarketRelayRefs: string[]
  providerNip90LaneRefs: string[]
}

export type PylonRegistrationRequest = {
  schema: "openagents.pylon.register.v0.3"
  pylonRef: string
  identity: PylonLocalState["identity"]
  lifecycle: PylonRuntimeState["lifecycle"]
  clientProtocolVersion: "0.3.0"
  clientVersion: PylonClientVersion
  resourceMode: string
  capabilityRefs: string[]
  blockerRefs: string[]
  statusRefs: string[]
} & Partial<PylonProviderDiscoveryFields>

export type PylonHeartbeatRequest = {
  schema: "openagents.pylon.heartbeat.v0.3"
  pylonRef: string
  sequence: number
  sentAt: string
  lifecycle: PylonRuntimeState["lifecycle"]
  capacityRefs: string[]
  clientProtocolVersion: "0.3.0"
  clientVersion: PylonClientVersion
  healthRefs: string[]
  loadRefs: string[]
  resourceMode: string
  status: "online"
  walletReadiness: "unknown" | "offline" | "receive-ready" | "send-ready" | "blocked"
  // Boolean the server projects into public `walletReadyNow` (#5151). Omitted
  // when the probe could not run, so the server keeps the last known value.
  walletReady?: boolean
  assignmentReadiness: "not-ready" | "ready" | "blocked"
  capabilityRefs: string[]
  blockerRefs: string[]
} & Partial<PylonProviderDiscoveryFields>

export type PylonLinkRequest = {
  schema: "openagents.pylon.link.v0.3"
  pylonRef: string
  npub: string
  publicKey: string
  bodyHash: string
}

type JsonRecord = Record<string, unknown>

export function sha256Base64Url(input: string) {
  return createHash("sha256").update(input).digest("base64url")
}

const compactTimestamp = (now: Date) =>
  now.toISOString().replace(/\D/g, "").slice(0, 14)

const makeIdempotencyKey = (
  pylonRef: string,
  action: "register" | "heartbeat" | "link-complete" | "link-refresh",
  now: Date,
) => `pylon-presence:${pylonRef}:${action}:${compactTimestamp(now)}`

export async function createSignedHeaders(input: {
  method: string
  url: string
  body: string
  pylonRef: string
  paths: BootstrapSummary["paths"]
  now?: Date
}) {
  const identity = await loadOrCreateNostrIdentity(input.paths)
  const event = createNip98Event({
    method: input.method,
    url: input.url,
    body: input.body,
    identity,
    now: input.now,
  })

  return {
    "content-type": "application/json",
    "x-pylon-ref": input.pylonRef,
    authorization: encodeNip98Authorization(event),
  }
}

async function postJson(
  options: PresenceClientOptions,
  input: {
    action: "register" | "heartbeat" | "link-complete" | "link-refresh"
    body: JsonRecord
    path: string
  },
  state: PylonLocalState,
) {
  const { body, path } = input
  assertPublicProjectionSafe(body)
  const fetchImpl = options.fetch ?? fetch
  const url = new URL(path, options.baseUrl).toString()
  const text = JSON.stringify(body)
  const now = options.now?.() ?? new Date()
  const headers = options.agentToken
    ? {
        "content-type": "application/json",
        "Idempotency-Key": makeIdempotencyKey(state.identity.pylonRef, input.action, now),
        "x-pylon-ref": state.identity.pylonRef,
        authorization: `Bearer ${options.agentToken}`,
      }
    : await createSignedHeaders({
        method: "POST",
        url,
        body: text,
        pylonRef: state.identity.pylonRef,
        paths: state.paths,
        now,
      })
  const response = await fetchImpl(url, { method: "POST", headers, body: text })
  const responseText = await response.text()
  let json: JsonRecord = {}
  if (responseText.trim()) {
    // #5268: parse the server response, but do NOT run the OUTBOUND public-projection
    // guard against it. That guard's contract is "don't PUBLISH my private data" — it
    // belongs on the outbound `body` only (above). Applying it to an INBOUND response
    // fail-closes the node's own register/heartbeat whenever the server's error
    // envelope carries a path-shaped `reason` (e.g. `/Users/<user>/.cache/...`): the
    // node would reject data it merely received and stay offline indefinitely. We
    // never re-publish this response, so it must never take the node offline.
    json = JSON.parse(responseText) as JsonRecord
  }
  if (!response.ok) {
    throw new PresenceRequestError(response.status, responseText)
  }
  return json
}

// #4864 provider discovery fields, included only for Pylons that have
// declared the NIP-90 provider lane (the go-online path adds
// PYLON_NIP90_PROVIDER_CAPABILITY_REF). Consent semantics: a provider
// that goes online IS announcing publicly — its provider loop publishes
// NIP-89 handler info signed with this same pubkey on these same relays.
// Carrying pubkey + relay refs + lane refs into the worker registry adds
// discoverability for stranger buyers, not exposure. The relay refs are
// the values this Pylon actually listens on (relaysFromEnv), never a
// worker-side constant, so the #4863 relay-domain cutover follows the
// provider configuration automatically.
export function providerDiscoveryFields(
  state: PylonLocalState,
  env: NodeJS.ProcessEnv = process.env,
): Partial<PylonProviderDiscoveryFields> {
  if (!state.runtime.capabilityRefs.includes(PYLON_NIP90_PROVIDER_CAPABILITY_REF)) {
    return {}
  }
  return {
    providerNostrPubkey: state.identity.publicKey,
    providerNostrNpub: state.identity.npub,
    providerMarketRelayRefs: relaysFromEnv(env),
    providerNip90LaneRefs: providerNip90LaneRefs(),
  }
}

export async function registerPylon(summary: BootstrapSummary, options: PresenceClientOptions) {
  const state = await ensurePylonLocalState(summary)
  const body: PylonRegistrationRequest = {
    schema: "openagents.pylon.register.v0.3",
    pylonRef: state.identity.pylonRef,
    identity: state.identity,
    lifecycle: state.runtime.lifecycle,
    clientProtocolVersion: "0.3.0",
    clientVersion: PYLON_CLIENT_VERSION,
    resourceMode: state.runtime.resourceMode,
    // W4.1 (#4750): an executor-capability claim without its self-test
    // receipt never leaves the device.
    capabilityRefs: publishableCapabilityRefs(state.runtime.capabilityRefs),
    blockerRefs: state.runtime.blockerRefs,
    statusRefs: ["status.public.pylon_cli.registered"],
    ...providerDiscoveryFields(state, options.env ?? process.env),
  }
  const response = await postJson(options, {
    action: "register",
    body,
    path: "/api/pylons/register",
  }, state)
  const presence = await loadOrCreatePresenceState(state.paths, state.identity)
  const next: PylonPresenceState = {
    ...presence,
    registered: true,
    stale: false,
    registrationRef: String(response.registrationRef ?? `registration.${state.identity.pylonRef}`),
    blockerRefs: [],
  }
  await writePresenceState(state.paths, next)
  return next
}

export async function sendHeartbeat(summary: BootstrapSummary, options: PresenceClientOptions) {
  const state = await ensurePylonLocalState(summary)
  const presence = await loadOrCreatePresenceState(state.paths, state.identity)
  const sequence = presence.heartbeatSequence + 1
  const sentAt = (options.now?.() ?? new Date()).toISOString()
  // Publish live receive-readiness so an online, receive-ready node is not shown
  // `walletReadyNow=false` until a separate `wallet report-readiness` (#5151).
  // Best-effort: a probe failure leaves `walletReadiness: "unknown"` and omits
  // `walletReady`, so the server keeps the last known value (no flap to false).
  let walletReadiness: PylonHeartbeatRequest["walletReadiness"] = "unknown"
  let walletReady: boolean | undefined
  if (options.walletProbe !== undefined) {
    try {
      const probe = heartbeatWalletReadiness(await options.walletProbe())
      walletReadiness = probe.walletReadiness
      walletReady = probe.walletReady
    } catch {
      walletReadiness = "unknown"
      walletReady = undefined
    }
  }
  const body: PylonHeartbeatRequest = {
    schema: "openagents.pylon.heartbeat.v0.3",
    pylonRef: state.identity.pylonRef,
    sequence,
    sentAt,
    lifecycle: state.runtime.lifecycle,
    capacityRefs: ["capacity.public.pylon_cli.available"],
    clientProtocolVersion: "0.3.0",
    clientVersion: PYLON_CLIENT_VERSION,
    healthRefs: ["health.public.pylon_cli.ok"],
    loadRefs: ["load.public.pylon_cli.low"],
    resourceMode: state.runtime.resourceMode,
    status: "online",
    walletReadiness,
    ...(walletReady === undefined ? {} : { walletReady }),
    assignmentReadiness: state.runtime.lifecycle === "assignment-ready" ? "ready" : "not-ready",
    capabilityRefs: publishableCapabilityRefs(state.runtime.capabilityRefs),
    blockerRefs: [...state.runtime.blockerRefs, ...presence.blockerRefs],
    ...providerDiscoveryFields(state, options.env ?? process.env),
  }
  await postJson(options, {
    action: "heartbeat",
    body,
    path: `/api/pylons/${encodeURIComponent(state.identity.pylonRef)}/heartbeat`,
  }, state)
  const next: PylonPresenceState = {
    ...presence,
    stale: false,
    lastHeartbeatAt: sentAt,
    heartbeatSequence: sequence,
    blockerRefs: [],
  }
  await writePresenceState(state.paths, next)
  return next
}

export async function completePylonLink(summary: BootstrapSummary, options: PresenceClientOptions) {
  const state = await ensurePylonLocalState(summary)
  const bodyWithoutHash = {
    schema: "openagents.pylon.link.v0.3",
    pylonRef: state.identity.pylonRef,
    npub: state.identity.npub,
    publicKey: state.identity.publicKey,
  } as const
  const body: PylonLinkRequest = {
    ...bodyWithoutHash,
    bodyHash: sha256Base64Url(JSON.stringify(bodyWithoutHash)),
  }
  const response = await postJson(options, {
    action: "link-complete",
    body,
    path: "/api/pylon-links/complete",
  }, state)
  const presence = await loadOrCreatePresenceState(state.paths, state.identity)
  const next: PylonPresenceState = {
    ...presence,
    linked: true,
    linkRef: String(response.linkRef ?? `link.${state.identity.pylonRef}`),
    blockerRefs: [],
  }
  await writePresenceState(state.paths, next)
  return next
}

export async function refreshPylonLink(summary: BootstrapSummary, options: PresenceClientOptions) {
  const state = await ensurePylonLocalState(summary)
  const bodyWithoutHash = {
    schema: "openagents.pylon.link.v0.3",
    pylonRef: state.identity.pylonRef,
    npub: state.identity.npub,
    publicKey: state.identity.publicKey,
  } as const
  const body: PylonLinkRequest = {
    ...bodyWithoutHash,
    bodyHash: sha256Base64Url(JSON.stringify(bodyWithoutHash)),
  }
  const response = await postJson(options, {
    action: "link-refresh",
    body,
    path: "/api/pylon-links/refresh",
  }, state)
  const presence = await loadOrCreatePresenceState(state.paths, state.identity)
  const next: PylonPresenceState = {
    ...presence,
    linked: true,
    stale: false,
    linkRef: String(response.linkRef ?? presence.linkRef ?? `link.${state.identity.pylonRef}`),
    blockerRefs: [],
  }
  await writePresenceState(state.paths, next)
  return next
}

export function degradeStalePresence(presence: PylonPresenceState, input: { now: Date; staleAfterMs: number }) {
  if (!presence.lastHeartbeatAt) {
    return {
      ...presence,
      stale: true,
      blockerRefs: [...new Set([...presence.blockerRefs, "blocker.presence.never_heartbeat"])],
    }
  }
  const age = input.now.getTime() - new Date(presence.lastHeartbeatAt).getTime()
  if (age <= input.staleAfterMs) return presence
  return {
    ...presence,
    stale: true,
    blockerRefs: [...new Set([...presence.blockerRefs, "blocker.presence.stale_heartbeat"])],
  }
}

export async function withPresenceRetry<T>(
  run: () => Promise<T>,
  input: { attempts: number; delayMs?: number; onRetry?: (error: unknown, attempt: number) => void },
) {
  let lastError: unknown
  for (let attempt = 1; attempt <= input.attempts; attempt += 1) {
    try {
      return await run()
    } catch (error) {
      lastError = error
      if (attempt >= input.attempts) break
      input.onRetry?.(error, attempt)
      if (input.delayMs) {
        await new Promise((resolve) => setTimeout(resolve, input.delayMs))
      }
    }
  }
  throw lastError
}
