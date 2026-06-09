import { createHash } from "node:crypto"
import type { BootstrapSummary } from "./bootstrap"
import { createNip98Event, encodeNip98Authorization, loadOrCreateNostrIdentity } from "./nostr-identity"
import {
  assertPublicProjectionSafe,
  ensurePylonLocalState,
  loadOrCreatePresenceState,
  type PylonLocalState,
  type PylonPresenceState,
  type PylonRuntimeState,
  writePresenceState,
} from "./state"

export type PresenceClientOptions = {
  baseUrl: string
  fetch?: typeof fetch
  now?: () => Date
}

export type PylonRegistrationRequest = {
  schema: "openagents.pylon.register.v0.3"
  pylonRef: string
  identity: PylonLocalState["identity"]
  lifecycle: PylonRuntimeState["lifecycle"]
  resourceMode: string
  capabilityRefs: string[]
  blockerRefs: string[]
}

export type PylonHeartbeatRequest = {
  schema: "openagents.pylon.heartbeat.v0.3"
  pylonRef: string
  sequence: number
  sentAt: string
  lifecycle: PylonRuntimeState["lifecycle"]
  walletReadiness: "unknown" | "offline" | "receive-ready" | "send-ready" | "blocked"
  assignmentReadiness: "not-ready" | "ready" | "blocked"
  capabilityRefs: string[]
  blockerRefs: string[]
}

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

async function postJson(options: PresenceClientOptions, path: string, body: JsonRecord, state: PylonLocalState) {
  assertPublicProjectionSafe(body)
  const fetchImpl = options.fetch ?? fetch
  const url = new URL(path, options.baseUrl).toString()
  const text = JSON.stringify(body)
  const headers = await createSignedHeaders({
    method: "POST",
    url,
    body: text,
    pylonRef: state.identity.pylonRef,
    paths: state.paths,
    now: options.now?.(),
  })
  const response = await fetchImpl(url, { method: "POST", headers, body: text })
  const responseText = await response.text()
  let json: JsonRecord = {}
  if (responseText.trim()) {
    json = JSON.parse(responseText) as JsonRecord
    assertPublicProjectionSafe(json)
  }
  if (!response.ok) {
    throw new Error(`OpenAgents presence request failed (${response.status}): ${responseText}`)
  }
  return json
}

export async function registerPylon(summary: BootstrapSummary, options: PresenceClientOptions) {
  const state = await ensurePylonLocalState(summary)
  const body: PylonRegistrationRequest = {
    schema: "openagents.pylon.register.v0.3",
    pylonRef: state.identity.pylonRef,
    identity: state.identity,
    lifecycle: state.runtime.lifecycle,
    resourceMode: state.runtime.resourceMode,
    capabilityRefs: state.runtime.capabilityRefs,
    blockerRefs: state.runtime.blockerRefs,
  }
  const response = await postJson(options, "/api/pylons/register", body, state)
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
  const body: PylonHeartbeatRequest = {
    schema: "openagents.pylon.heartbeat.v0.3",
    pylonRef: state.identity.pylonRef,
    sequence,
    sentAt,
    lifecycle: state.runtime.lifecycle,
    walletReadiness: "unknown",
    assignmentReadiness: state.runtime.lifecycle === "assignment-ready" ? "ready" : "not-ready",
    capabilityRefs: state.runtime.capabilityRefs,
    blockerRefs: [...state.runtime.blockerRefs, ...presence.blockerRefs],
  }
  await postJson(options, `/api/pylons/${encodeURIComponent(state.identity.pylonRef)}/heartbeat`, body, state)
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
  const response = await postJson(options, "/api/pylon-links/complete", body, state)
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
  const response = await postJson(options, "/api/pylon-links/refresh", body, state)
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
