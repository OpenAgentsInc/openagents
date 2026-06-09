import { readFile } from "node:fs/promises"
import { createHash, sign } from "node:crypto"
import type { BootstrapSummary } from "./bootstrap"
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
  identityPath: string
  now?: Date
}) {
  const privateIdentity = JSON.parse(await readFile(input.identityPath, "utf8")) as { privateKeyPem: string; npub: string }
  const createdAt = (input.now ?? new Date()).toISOString()
  const bodyHash = sha256Base64Url(input.body)
  const payload = [input.method.toUpperCase(), input.url, bodyHash, createdAt, input.pylonRef].join("\n")
  const signature = sign(null, Buffer.from(payload), privateIdentity.privateKeyPem).toString("base64url")

  return {
    "content-type": "application/json",
    "x-pylon-ref": input.pylonRef,
    "x-nip98-pubkey": privateIdentity.npub,
    "x-nip98-created-at": createdAt,
    "x-nip98-body-sha256": bodyHash,
    "x-nip98-signature": signature,
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
    identityPath: state.paths.identity,
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
  }
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
  }
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
