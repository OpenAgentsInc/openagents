import { createHash } from "node:crypto"
import { stat } from "node:fs/promises"
import { homedir } from "node:os"
import { join } from "node:path"
import { PYLON_CLIENT_VERSION, type PylonClientVersion } from "./version.js"
import type { BootstrapSummary } from "./bootstrap.js"
import {
  hashPylonAccountRef,
  discoverPylonSiblingAccountHomes,
  loadPylonAccountRegistry,
  normalizeAccountHome,
  pylonClaudeAccountHomeHasAuth,
} from "./account-registry.js"
import { isAccountAvailable, loadQuotaRecord } from "./account-quota-ledger.js"
import {
  codexAccountHealthBlocksReadiness,
  loadCodexAccountHealthRecord,
} from "./codex-account-health-ledger.js"
import {
  PYLON_NIP90_PROVIDER_CAPABILITY_REF,
  providerNip90LaneRefs,
  relaysFromEnv,
} from "./provider-nip90.js"
import { CODEX_AGENT_CAPABILITY_REF } from "./codex-agent.js"
import { CLAUDE_AGENT_CAPABILITY_REF } from "./claude-agent.js"
import {
  appleFmBackendCapacityRefs,
  collectPylonAppleFmStatus,
  withAppleFmBackendCapabilities,
  type PylonAppleFmStatusProjection,
} from "./node/apple-fm-status.js"
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
import {
  activeCodingRunCounts,
  activeCodingRunCountsByAccount,
  maxActiveCodingRunCounts,
  UNKEYED_ACTIVE_RUN_ACCOUNT,
  type PylonActiveCodingRunAccountCounts,
  type PylonActiveCodingRunCounts,
} from "./active-assignment-runs.js"

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
  // Best-effort server-side active assignment lease counts, computed by CLI
  // orchestration from the assignment poll route. This keeps heartbeat capacity
  // honest when a prior no-spend lease exists but no fresh local marker remains.
  activeRunCounts?: PylonActiveCodingRunCounts
  // Test and node-internal seam for the live Apple FM bridge readiness report.
  // When omitted, heartbeat probes the local /health endpoint through the shared
  // Probe runtime capability reporter.
  appleFmStatusProbe?: () => Promise<PylonAppleFmStatusProjection>
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

export type PylonCodingServiceCapacity = {
  available: number
  busy: number
  queued: number
  ready: number
  service: "claude" | "codex"
}

export type PylonCodingServiceReadyCounts = Partial<Record<PylonCodingServiceCapacity["service"], number>>

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

const nonNegativeEnvInteger = (
  env: NodeJS.ProcessEnv,
  key: string,
  fallback: number,
) => {
  const raw = env[key]?.trim()
  if (raw === undefined || raw === "") return fallback
  const parsed = Number.parseInt(raw, 10)
  return Number.isSafeInteger(parsed) && parsed >= 0
    ? Math.min(parsed, 10_000)
    : fallback
}

export function codingServiceCapacityFromRuntime(
  state: PylonLocalState,
  env: NodeJS.ProcessEnv = process.env,
  readyCounts: PylonCodingServiceReadyCounts = {},
  activeRunCounts: PylonActiveCodingRunCounts = {},
): PylonCodingServiceCapacity[] {
  const capabilityRefs = publishableCapabilityRefs(state.runtime.capabilityRefs)
  const serviceConfig = [
    {
      busyKey: "OPENAGENTS_PYLON_CODEX_BUSY",
      capabilityRef: CODEX_AGENT_CAPABILITY_REF,
      concurrencyKey: "OPENAGENTS_PYLON_CODEX_CONCURRENCY",
      queuedKey: "OPENAGENTS_PYLON_CODEX_QUEUED",
      service: "codex" as const,
    },
    {
      busyKey: "OPENAGENTS_PYLON_CLAUDE_BUSY",
      capabilityRef: CLAUDE_AGENT_CAPABILITY_REF,
      concurrencyKey: "OPENAGENTS_PYLON_CLAUDE_CONCURRENCY",
      queuedKey: "OPENAGENTS_PYLON_CLAUDE_QUEUED",
      service: "claude" as const,
    },
  ]

  return serviceConfig
    .filter(config => capabilityRefs.includes(config.capabilityRef))
    .map(config => {
      const observedReady = Math.max(0, readyCounts[config.service] ?? 0)
      const fallbackReady = observedReady > 0 ? observedReady : 1
      const ready = nonNegativeEnvInteger(env, config.concurrencyKey, fallbackReady)
      const busy = Math.min(
        nonNegativeEnvInteger(env, config.busyKey, 0) + Math.max(0, activeRunCounts[config.service] ?? 0),
        ready,
      )
      const queued = nonNegativeEnvInteger(env, config.queuedKey, 0)
      return {
        available: Math.max(0, ready - busy),
        busy,
        queued,
        ready,
        service: config.service,
      }
    })
}

export const codingServiceCapacityRefs = (
  capacity: ReadonlyArray<PylonCodingServiceCapacity>,
): { capacityRefs: string[]; loadRefs: string[] } => ({
  capacityRefs: capacity.flatMap(item => [
    `capacity.coding.${item.service}.ready=${item.ready}`,
    `capacity.coding.${item.service}.available=${item.available}`,
  ]),
  loadRefs: capacity.flatMap(item => [
    `load.coding.${item.service}.busy=${item.busy}`,
    `load.coding.${item.service}.queued=${item.queued}`,
  ]),
})

async function codexHomeHasAuth(home: string): Promise<boolean> {
  try {
    const info = await stat(join(home, "auth.json"))
    return info.isFile() && info.size > 0
  } catch {
    return false
  }
}

export async function localCodingServiceReadyCounts(
  summary: Pick<BootstrapSummary, "paths">,
  env: NodeJS.ProcessEnv = process.env,
): Promise<PylonCodingServiceReadyCounts> {
  const codexHomes = new Set<string>()
  const configuredCodexHome = env.CODEX_HOME?.trim()
  codexHomes.add(
    configuredCodexHome && configuredCodexHome.length > 0
      ? normalizeAccountHome(configuredCodexHome)
      : join(homedir(), ".codex"),
  )
  for (const entry of await loadPylonAccountRegistry(summary)) {
    if (entry.provider === "codex") {
      codexHomes.add(entry.home)
    }
  }

  let codex = 0
  for (const home of codexHomes) {
    if (await codexHomeHasAuth(home)) {
      codex += 1
    }
  }
  return codex > 0 ? { codex } : {}
}

// #6354: per-Codex-account capacity so multiple linked accounts on one owner
// Pylon each advertise their own concurrent slots. The capacity-ref account key
// is the public-safe trailing hex of the account-ref hash; the wire never
// carries a raw account ref, email, or home path.
export type PylonCodexAccountReadiness = {
  accountRefHash: string
  ready: boolean
  reason?: "credentials_revoked" | "usage_limited" | "rate_limited" | "network" | "timeout" | "other"
}

export type PylonCodexAccountCapacity = {
  accountKey: string
  accountRefHash: string
  available: number
  busy: number
  queued: number
  ready: number
}

export const codexAccountCapacityKey = (
  accountRefHash: string,
): string | null => {
  const match = /^account\.pylon\.[a-z0-9_]+\.([a-z0-9]{6,64})$/.exec(
    accountRefHash.trim(),
  )
  return match === null ? null : match[1]!
}

// Enumerate the linked Codex accounts and whether each one's isolated home holds
// a usable login. Registry accounts are keyed by their ref (matching
// `resolvePylonAccountSelection`'s registry_ref hash and the per-assignment
// account hash). The default `~/.codex` home is only included when no registry
// Codex account exists, so a normal multi-account Pylon does not advertise a
// phantom default-account slot the supervisor never targets.
export async function localCodexAccountReadiness(
  summary: Pick<BootstrapSummary, "paths">,
  env: NodeJS.ProcessEnv = process.env,
): Promise<PylonCodexAccountReadiness[]> {
  const readiness = new Map<string, boolean>()
  const reasons = new Map<string, PylonCodexAccountReadiness["reason"]>()
  const registry = await loadPylonAccountRegistry(summary)
  const codexEntries = registry.filter(entry => entry.provider === "codex")
  for (const entry of codexEntries) {
    const accountRefHash = hashPylonAccountRef("codex", entry.ref)
    let ready = await codexHomeHasAuth(entry.home)
    const health = await loadCodexAccountHealthRecord(summary, accountRefHash)
    if (codexAccountHealthBlocksReadiness(health)) {
      ready = false
      reasons.set(accountRefHash, health.reason)
    }
    const quotaRecord = await loadQuotaRecord(summary, accountRefHash)
    if (!isAccountAvailable(quotaRecord, new Date())) {
      ready = false
      reasons.set(accountRefHash, "usage_limited")
    }
    readiness.set(accountRefHash, ready)
  }
  if (codexEntries.length === 0) {
    const configuredCodexHome = env.CODEX_HOME?.trim()
    const defaultHome =
      configuredCodexHome && configuredCodexHome.length > 0
        ? normalizeAccountHome(configuredCodexHome)
        : join(homedir(), ".codex")
    const accountRefHash = hashPylonAccountRef("codex", "default")
    let ready = await codexHomeHasAuth(defaultHome)
    const health = await loadCodexAccountHealthRecord(summary, accountRefHash)
    if (codexAccountHealthBlocksReadiness(health)) {
      ready = false
      reasons.set(accountRefHash, health.reason)
    }
    const quotaRecord = await loadQuotaRecord(summary, accountRefHash)
    if (!isAccountAvailable(quotaRecord, new Date())) {
      ready = false
      reasons.set(accountRefHash, "usage_limited")
    }
    readiness.set(accountRefHash, ready)
  }
  return [...readiness.entries()].map(([accountRefHash, ready]) => ({
    accountRefHash,
    ready,
    ...(reasons.get(accountRefHash) === undefined ? {} : { reason: reasons.get(accountRefHash)! }),
  }))
}

// Per-account Codex capacity: each ready account advertises `perAccountConcurrency`
// ready slots, minus its own active local runs (busy). One account's busy load
// never lowers another account's available slots.
export function codexAccountCapacities(input: {
  busyByAccount?: Record<string, number>
  perAccountConcurrency: number
  queuedByAccount?: Record<string, number>
  readiness: ReadonlyArray<PylonCodexAccountReadiness>
}): PylonCodexAccountCapacity[] {
  const ready = Math.max(0, Math.min(input.perAccountConcurrency, 10_000))
  const out: PylonCodexAccountCapacity[] = []
  for (const account of input.readiness) {
    if (!account.ready) continue
    const accountKey = codexAccountCapacityKey(account.accountRefHash)
    if (accountKey === null) continue
    const busy = Math.min(input.busyByAccount?.[account.accountRefHash] ?? 0, ready)
    const queued = Math.max(0, input.queuedByAccount?.[account.accountRefHash] ?? 0)
    out.push({
      accountKey,
      accountRefHash: account.accountRefHash,
      available: Math.max(0, ready - busy),
      busy,
      queued,
      ready,
    })
  }
  return out.sort((left, right) => left.accountKey.localeCompare(right.accountKey))
}

export const codexAccountCapacityRefs = (
  accounts: ReadonlyArray<PylonCodexAccountCapacity>,
): { capacityRefs: string[]; loadRefs: string[] } => ({
  capacityRefs: accounts.flatMap(account => [
    `capacity.coding.codex.account.${account.accountKey}.ready=${account.ready}`,
    `capacity.coding.codex.account.${account.accountKey}.available=${account.available}`,
  ]),
  loadRefs: accounts.flatMap(account => [
    `load.coding.codex.account.${account.accountKey}.busy=${account.busy}`,
    `load.coding.codex.account.${account.accountKey}.queued=${account.queued}`,
  ]),
})

// Per-account Codex concurrency. Reuses the existing
// `OPENAGENTS_PYLON_CODEX_CONCURRENCY` (pooled today) as the per-account default
// so an operator who already runs "N concurrent" gets N per account, while
// `OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY` can override it explicitly.
export function codexPerAccountConcurrency(
  env: NodeJS.ProcessEnv = process.env,
): number {
  const explicit = env.OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY?.trim()
  if (explicit !== undefined && explicit !== "") {
    return nonNegativeEnvInteger(env, "OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY", 1)
  }
  return nonNegativeEnvInteger(env, "OPENAGENTS_PYLON_CODEX_CONCURRENCY", 1)
}

// Build the per-account Codex capacity for the heartbeat from live readiness and
// active-run busy load. Returns [] when the node is not Codex-capable.
export async function localCodexAccountCapacities(
  state: PylonLocalState,
  summary: Pick<BootstrapSummary, "paths">,
  env: NodeJS.ProcessEnv = process.env,
  accountBusyCounts: Record<string, number> = {},
): Promise<PylonCodexAccountCapacity[]> {
  const capabilityRefs = publishableCapabilityRefs(state.runtime.capabilityRefs)
  if (!capabilityRefs.includes(CODEX_AGENT_CAPABILITY_REF)) {
    return []
  }
  return codexAccountCapacities({
    busyByAccount: accountBusyCounts,
    perAccountConcurrency: codexPerAccountConcurrency(env),
    readiness: await localCodexAccountReadiness(summary, env),
  })
}

// Flatten per-service per-account active-run counts into a single
// accountRefHash->count map for the Codex service (dropping the unkeyed bucket,
// which cannot be attributed to a specific account).
export function codexBusyByAccount(
  counts: PylonActiveCodingRunAccountCounts,
): Record<string, number> {
  const out: Record<string, number> = {}
  for (const [accountRefHash, count] of Object.entries(counts.codex ?? {})) {
    if (accountRefHash === UNKEYED_ACTIVE_RUN_ACCOUNT) continue
    out[accountRefHash] = count
  }
  return out
}

// #6421: per-Claude-account capacity, mirroring the Codex per-account lane
// (#6354) so the claude-supervisor can run several distinct Claude accounts on
// one owner Pylon, each advertising its own concurrent slots. A linked Claude
// account's isolated home carries a `claude-oauth-token` file (the per-account
// analogue of a Codex home's `auth.json`); a non-empty token file is the
// public-safe readiness signal. The wire only carries the trailing hex of the
// account-ref hash, never a raw ref, email, home path, or token.
// Enumerate the linked Claude accounts and whether each one's isolated home
// holds a usable per-account login. Registry accounts are keyed by their ref
// (matching `resolvePylonAccountSelection`'s registry_ref hash and the
// per-assignment account hash). Unlike Codex, no default-home account is
// synthesized: the default Claude home authenticates via the macOS Keychain,
// which cannot be cleanly probed per-account, and is already covered by the
// pooled `capacity.coding.claude` projection. Only registry accounts that carry
// a token file advertise a per-account slot, so the gate never sees a phantom.
export async function localClaudeAccountReadiness(
  summary: Pick<BootstrapSummary, "paths">,
  env: NodeJS.ProcessEnv = process.env,
): Promise<PylonCodexAccountReadiness[]> {
  const registry = await loadPylonAccountRegistry(summary)
  const readiness: PylonCodexAccountReadiness[] = []
  const seen = new Set<string>()
  const seenHomes = new Set<string>()
  for (const entry of registry) {
    if (entry.provider !== "claude_agent") continue
    const accountRefHash = hashPylonAccountRef("claude_agent", entry.ref)
    seen.add(accountRefHash)
    seenHomes.add(entry.home)
    readiness.push({
      accountRefHash,
      ready: await pylonClaudeAccountHomeHasAuth(entry.home),
    })
  }
  for (const sibling of await discoverPylonSiblingAccountHomes(env)) {
    if (sibling.provider !== "claude_agent") continue
    if (seenHomes.has(sibling.home)) continue
    const accountRefHash = hashPylonAccountRef("claude_agent", sibling.home)
    if (seen.has(accountRefHash)) continue
    seen.add(accountRefHash)
    readiness.push({
      accountRefHash,
      ready: await pylonClaudeAccountHomeHasAuth(sibling.home),
    })
  }
  return readiness
}

export const claudeAccountCapacityRefs = (
  accounts: ReadonlyArray<PylonCodexAccountCapacity>,
): { capacityRefs: string[]; loadRefs: string[] } => ({
  capacityRefs: accounts.flatMap(account => [
    `capacity.coding.claude.account.${account.accountKey}.ready=${account.ready}`,
    `capacity.coding.claude.account.${account.accountKey}.available=${account.available}`,
  ]),
  loadRefs: accounts.flatMap(account => [
    `load.coding.claude.account.${account.accountKey}.busy=${account.busy}`,
    `load.coding.claude.account.${account.accountKey}.queued=${account.queued}`,
  ]),
})

// Per-account Claude concurrency. Reuses the existing
// `OPENAGENTS_PYLON_CLAUDE_CONCURRENCY` (pooled today) as the per-account default
// so an operator who already runs "N concurrent" gets N per account, while
// `OPENAGENTS_PYLON_CLAUDE_ACCOUNT_CONCURRENCY` can override it explicitly.
export function claudePerAccountConcurrency(
  env: NodeJS.ProcessEnv = process.env,
): number {
  const explicit = env.OPENAGENTS_PYLON_CLAUDE_ACCOUNT_CONCURRENCY?.trim()
  if (explicit !== undefined && explicit !== "") {
    return nonNegativeEnvInteger(env, "OPENAGENTS_PYLON_CLAUDE_ACCOUNT_CONCURRENCY", 1)
  }
  return nonNegativeEnvInteger(env, "OPENAGENTS_PYLON_CLAUDE_CONCURRENCY", 1)
}

// Build the per-account Claude capacity for the heartbeat from live readiness
// and active-run busy load. Returns [] when the node is not Claude-capable.
export async function localClaudeAccountCapacities(
  state: PylonLocalState,
  summary: Pick<BootstrapSummary, "paths">,
  env: NodeJS.ProcessEnv = process.env,
  accountBusyCounts: Record<string, number> = {},
): Promise<PylonCodexAccountCapacity[]> {
  const capabilityRefs = publishableCapabilityRefs(state.runtime.capabilityRefs)
  if (!capabilityRefs.includes(CLAUDE_AGENT_CAPABILITY_REF)) {
    return []
  }
  return codexAccountCapacities({
    busyByAccount: accountBusyCounts,
    perAccountConcurrency: claudePerAccountConcurrency(env),
    readiness: await localClaudeAccountReadiness(summary, env),
  })
}

// Flatten per-service per-account active-run counts into a single
// accountRefHash->count map for the Claude service (dropping the unkeyed bucket,
// which cannot be attributed to a specific account).
export function claudeBusyByAccount(
  counts: PylonActiveCodingRunAccountCounts,
): Record<string, number> {
  const out: Record<string, number> = {}
  for (const [accountRefHash, count] of Object.entries(counts.claude ?? {})) {
    if (accountRefHash === UNKEYED_ACTIVE_RUN_ACCOUNT) continue
    out[accountRefHash] = count
  }
  return out
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
  const presenceEnv = options.env ?? process.env
  const codingRefs = codingServiceCapacityRefs(
    codingServiceCapacityFromRuntime(
      state,
      presenceEnv,
      await localCodingServiceReadyCounts(summary, presenceEnv),
      maxActiveCodingRunCounts(
        await activeCodingRunCounts(state.paths, { now: options.now?.() }),
        options.activeRunCounts,
      ),
    ),
  )
  // #6354/#6421: per-account capacity so each linked Codex or Claude account
  // dispatches its own concurrent assignments. Busy is the account's own fresh
  // active local runs, read once and split per service.
  const activeRunCountsByAccount = await activeCodingRunCountsByAccount(
    state.paths,
    { now: options.now?.() },
  )
  const codexAccountRefs = codexAccountCapacityRefs(
    await localCodexAccountCapacities(
      state,
      summary,
      presenceEnv,
      codexBusyByAccount(activeRunCountsByAccount),
    ),
  )
  // #6421: per-Claude-account capacity, mirroring Codex, so the claude-supervisor
  // can target distinct Claude accounts via `--account-ref` and the dispatch gate
  // admits each account against its own slots.
  const claudeAccountRefs = claudeAccountCapacityRefs(
    await localClaudeAccountCapacities(
      state,
      summary,
      presenceEnv,
      claudeBusyByAccount(activeRunCountsByAccount),
    ),
  )
  const appleFmStatus = options.appleFmStatusProbe === undefined
    ? await collectPylonAppleFmStatus({
        env: presenceEnv,
        now: options.now?.(),
        summary,
      })
    : await options.appleFmStatusProbe()
  const appleFmRefs = appleFmBackendCapacityRefs(appleFmStatus)
  const capabilityRefs = publishableCapabilityRefs(
    withAppleFmBackendCapabilities(state.runtime.capabilityRefs, appleFmStatus),
  )
  const body: PylonHeartbeatRequest = {
    schema: "openagents.pylon.heartbeat.v0.3",
    pylonRef: state.identity.pylonRef,
    sequence,
    sentAt,
    lifecycle: state.runtime.lifecycle,
    capacityRefs: [
      "capacity.public.pylon_cli.available",
      ...codingRefs.capacityRefs,
      ...codexAccountRefs.capacityRefs,
      ...claudeAccountRefs.capacityRefs,
      ...appleFmRefs.capacityRefs,
    ],
    clientProtocolVersion: "0.3.0",
    clientVersion: PYLON_CLIENT_VERSION,
    healthRefs: ["health.public.pylon_cli.ok", ...appleFmRefs.healthRefs],
    loadRefs: [
      "load.public.pylon_cli.low",
      ...codingRefs.loadRefs,
      ...codexAccountRefs.loadRefs,
      ...claudeAccountRefs.loadRefs,
      ...appleFmRefs.loadRefs,
    ],
    resourceMode: state.runtime.resourceMode,
    status: "online",
    walletReadiness,
    ...(walletReady === undefined ? {} : { walletReady }),
    assignmentReadiness: state.runtime.lifecycle === "assignment-ready" ? "ready" : "not-ready",
    capabilityRefs,
    blockerRefs: [...new Set([...state.runtime.blockerRefs, ...presence.blockerRefs, ...appleFmStatus.blockerRefs])],
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

/**
 * Persist an account-established link into the local presence state
 * (openagents #6331). `pylon accounts connect codex --openagents-link`
 * confirms a server-side account->OpenAuth-owner link (`pylonLink: linked`),
 * but that import never went through `completePylonLink`, so the presence state
 * the heartbeat reads stayed `linked: false`/`linkRef: null`. Operators (and
 * the runbook) expect the next heartbeat to reflect that the Pylon is linked.
 * This reconciles the two: it marks the presence state linked and records a
 * stable, public-safe account link ref so `presence heartbeat` reports
 * `linked: true` once a Codex account is connected with `--openagents-link`.
 */
export async function recordAccountLinkInPresence(
  summary: Pick<BootstrapSummary, "bootstrap" | "paths">,
  input: { providerAccountRef: string },
): Promise<PylonPresenceState> {
  const state = await ensurePylonLocalState(summary)
  const presence = await loadOrCreatePresenceState(state.paths, state.identity)
  const linkRef =
    presence.linkRef ??
    `link.account.${sha256Base64Url(`${state.identity.pylonRef}:${input.providerAccountRef}`)}`
  const next: PylonPresenceState = {
    ...presence,
    linked: true,
    linkRef,
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
