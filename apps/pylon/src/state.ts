import { mkdir, readFile, writeFile } from "node:fs/promises"
import { existsSync } from "node:fs"
import { createHash, randomUUID } from "node:crypto"
import { hostname } from "node:os"
import { dirname } from "node:path"
import { PYLON_DEFAULT_CAPABILITY_REFS, type BootstrapSummary } from "./bootstrap"
import type { PylonHostInventoryProjection } from "./inventory"
import type { PsionicConnectorState } from "./psionic-connector"
import { loadOrCreateNostrIdentity } from "./nostr-identity"

export type PylonLifecycleState = "offline" | "online" | "paused" | "degraded" | "assignment-ready"

export type PylonPaths = BootstrapSummary["paths"] & {
  identity: string
  identityMnemonic: string
  runtimeState: string
  presenceState: string
  assignmentState: string
  ledger: string
}

export type PylonIdentity = {
  nodeId: string
  pylonRef: string
  nodeLabel: string
  publicKey: string
  npub: string
  createdAt: string
}

export type PylonIdentityRecord = PylonIdentity & {
  legacyLocalNpub?: string
}

export type PylonRuntimeState = {
  lifecycle: PylonLifecycleState
  displayName: string | null
  resourceMode: string
  capabilityRefs: string[]
  blockerRefs: string[]
  updatedAt: string
}

export type PylonLocalState = {
  schema: "openagents.pylon.local_state.v0.3"
  packageName: "@openagentsinc/pylon"
  version: "0.3.0-rc2"
  paths: PylonPaths
  identity: PylonIdentity
  runtime: PylonRuntimeState
  presence: PylonPresenceState
}

export type PylonPresenceState = {
  registered: boolean
  linked: boolean
  stale: boolean
  pylonRef: string
  registrationRef: string | null
  linkRef: string | null
  lastHeartbeatAt: string | null
  heartbeatSequence: number
  blockerRefs: string[]
  updatedAt: string
}

export type PublicProjection =
  | {
      kind: "identity"
      identity: PylonIdentity
    }
  | {
      kind: "availability"
      pylonRef: string
      lifecycle: PylonLifecycleState
      resourceMode: string
      capabilityRefs: string[]
      blockerRefs: string[]
    }
  | {
      kind: "status"
      state: PylonLocalState
    }
  | Record<string, unknown>

const forbiddenKeyPattern =
  /(^|[._-])(wallet_seed|seed|mnemonic|private_key|privatekey|preimage|bearer|access_token|api_key|apikey|provider_token|provider_auth|raw_prompt|raw_prompts|private_repo|repo_content|private_topology|cache_path|cachepath|env_dump|environment_dump|capacity_pool_secret|internal_accounting_credential|invoice|offer|payment_hash|payment_preimage|secret|password|xprv)([._-]|$)/i

const forbiddenExactKeyPattern =
  /^(walletSeed|seed|mnemonic|privateKey|private_key|preimage|bearer|accessToken|apiKey|providerToken|providerAuth|rawPrompt|rawPrompts|privateRepo|repoContent|privateTopology|cachePath|envDump|environmentDump|capacityPoolSecret|internalAccountingCredential|invoice|offer|paymentHash|paymentPreimage|secret|password|xprv)$/i

const forbiddenStringPattern =
  /\b(wallet seed|mnemonic|private key|payment preimage|bearer\s+[a-z0-9._-]+|sk-[a-z0-9_-]+|lnbc[a-z0-9]+|lntb[a-z0-9]+|lno[a-z0-9]+|private-repo:\/\/|private_repo|raw prompt|capacity pool secret|internal accounting credential|xprv|\/Users\/[^\s]+\/\.cache|\/home\/[^\s]+\/\.cache)\b/i

function stableHash(input: string, length = 24) {
  return createHash("sha256").update(input).digest("hex").slice(0, length)
}

function sanitizeLabel(value: string) {
  const sanitized = value.toLowerCase().replace(/[^a-z0-9._-]+/g, "-").replace(/^-+|-+$/g, "")
  return sanitized || "pylon-node"
}

export function resolveStatePaths(paths: BootstrapSummary["paths"]): PylonPaths {
  return {
    ...paths,
    identity: `${paths.home}/identity.json`,
    identityMnemonic: `${paths.home}/identity.mnemonic`,
    runtimeState: `${paths.home}/runtime-state.json`,
    presenceState: `${paths.home}/presence-state.json`,
    assignmentState: `${paths.home}/assignment-state.json`,
    ledger: `${paths.home}/ledger.jsonl`,
  }
}

export async function ensureStateDirectories(paths: PylonPaths) {
  await mkdir(paths.home, { recursive: true })
  await mkdir(paths.cache, { recursive: true })
  await mkdir(paths.releases, { recursive: true })
  await mkdir(dirname(paths.ledger), { recursive: true })
}

export async function createPylonIdentity(input: { paths: PylonPaths; nodeLabel?: string; pylonRef?: string; now?: Date }) {
  const nostrIdentity = await loadOrCreateNostrIdentity(input.paths)
  const publicKey = nostrIdentity.publicKey
  const nodeId = `pylon_${stableHash(publicKey)}`
  const nodeLabel = sanitizeLabel(input.nodeLabel ?? hostname())
  const pylonRef = input.pylonRef ?? `pylon.${stableHash(`${nodeLabel}:${publicKey}`, 20)}`
  const npub = nostrIdentity.npub
  const createdAt = (input.now ?? new Date()).toISOString()

  return {
    nodeId,
    pylonRef,
    nodeLabel,
    publicKey,
    npub,
    createdAt,
  } satisfies PylonIdentity
}

function publicIdentity(record: PylonIdentityRecord): PylonIdentity {
  return {
    nodeId: record.nodeId,
    pylonRef: record.pylonRef,
    nodeLabel: record.nodeLabel,
    publicKey: record.publicKey,
    npub: record.npub,
    createdAt: record.createdAt,
  }
}

async function readJsonFile<T>(path: string): Promise<T | null> {
  if (!existsSync(path)) return null
  return JSON.parse(await readFile(path, "utf8")) as T
}

export async function loadOrCreateIdentity(paths: PylonPaths, input: { nodeLabel?: string; pylonRef?: string } = {}) {
  await ensureStateDirectories(paths)
  const existing = await readJsonFile<PylonIdentityRecord>(paths.identity)
  const nostrIdentity = await loadOrCreateNostrIdentity(paths)
  const nodeLabel = sanitizeLabel(input.nodeLabel ?? existing?.nodeLabel ?? hostname())
  const pylonRef = input.pylonRef ?? existing?.pylonRef ?? `pylon.${stableHash(`${nodeLabel}:${nostrIdentity.publicKey}`, 20)}`
  const identity = {
    nodeId: `pylon_${stableHash(nostrIdentity.publicKey)}`,
    pylonRef,
    nodeLabel,
    publicKey: nostrIdentity.publicKey,
    npub: nostrIdentity.npub,
    createdAt: existing?.createdAt ?? new Date().toISOString(),
    ...(existing?.npub && existing.npub !== nostrIdentity.npub ? { legacyLocalNpub: existing.npub } : {}),
  } satisfies PylonIdentityRecord
  await writeFile(paths.identity, `${JSON.stringify(identity, null, 2)}\n`, { mode: 0o600 })
  return publicIdentity(identity)
}

export async function loadOrCreateRuntimeState(
  paths: PylonPaths,
  input: Partial<Pick<PylonRuntimeState, "displayName" | "resourceMode" | "capabilityRefs">> = {},
) {
  await ensureStateDirectories(paths)
  const existing = await readJsonFile<PylonRuntimeState>(paths.runtimeState)
  const requestedCapabilityRefs = input.capabilityRefs ?? []
  const defaultCapabilityRefSet = new Set(PYLON_DEFAULT_CAPABILITY_REFS)
  const defaultOnly =
    requestedCapabilityRefs.length > 0 &&
    requestedCapabilityRefs.every(ref => defaultCapabilityRefSet.has(ref))
  const capabilityRefs =
    requestedCapabilityRefs.length === 0
      ? existing?.capabilityRefs ?? []
      : defaultOnly && existing?.capabilityRefs
        ? [...new Set([...existing.capabilityRefs, ...requestedCapabilityRefs])]
        : requestedCapabilityRefs
  const state: PylonRuntimeState = {
    lifecycle: existing?.lifecycle ?? "offline",
    displayName: input.displayName ?? existing?.displayName ?? null,
    resourceMode: input.resourceMode ?? existing?.resourceMode ?? "background_20",
    capabilityRefs,
    blockerRefs: existing?.blockerRefs ?? [],
    updatedAt: new Date().toISOString(),
  }
  await writeFile(paths.runtimeState, `${JSON.stringify(state, null, 2)}\n`)
  return state
}

export async function loadOrCreatePresenceState(paths: PylonPaths, identity: PylonIdentity) {
  await ensureStateDirectories(paths)
  const existing = await readJsonFile<PylonPresenceState>(paths.presenceState)
  const state: PylonPresenceState = {
    registered: existing?.registered ?? false,
    linked: existing?.linked ?? false,
    stale: existing?.stale ?? false,
    pylonRef: existing?.pylonRef ?? identity.pylonRef,
    registrationRef: existing?.registrationRef ?? null,
    linkRef: existing?.linkRef ?? null,
    lastHeartbeatAt: existing?.lastHeartbeatAt ?? null,
    heartbeatSequence: existing?.heartbeatSequence ?? 0,
    blockerRefs: existing?.blockerRefs ?? [],
    updatedAt: new Date().toISOString(),
  }
  await writeFile(paths.presenceState, `${JSON.stringify(state, null, 2)}\n`)
  return state
}

export async function writePresenceState(paths: PylonPaths, state: PylonPresenceState) {
  await ensureStateDirectories(paths)
  await writeFile(paths.presenceState, `${JSON.stringify({ ...state, updatedAt: new Date().toISOString() }, null, 2)}\n`)
}

export async function writeRuntimeState(paths: PylonPaths, state: PylonRuntimeState) {
  await ensureStateDirectories(paths)
  await writeFile(paths.runtimeState, `${JSON.stringify({ ...state, updatedAt: new Date().toISOString() }, null, 2)}\n`)
}

export async function ensurePylonLocalState(summary: BootstrapSummary): Promise<PylonLocalState> {
  const paths = resolveStatePaths(summary.paths)
  const identity = await loadOrCreateIdentity(paths, {
    nodeLabel: summary.bootstrap.displayName ?? undefined,
    pylonRef: summary.bootstrap.pylonRef ?? undefined,
  })
  const runtime = await loadOrCreateRuntimeState(paths, {
    displayName: summary.bootstrap.displayName,
    resourceMode: summary.bootstrap.resourceMode,
    capabilityRefs: summary.bootstrap.capabilityRefs,
  })
  const presence = await loadOrCreatePresenceState(paths, identity)

  return {
    schema: "openagents.pylon.local_state.v0.3",
    packageName: "@openagentsinc/pylon",
    version: "0.3.0-rc2",
    paths,
    identity,
    runtime,
    presence,
  }
}

export function assertPublicProjectionSafe(value: unknown, path = "projection"): asserts value is PublicProjection {
  if (value === null || value === undefined) return
  if (typeof value === "string") {
    if (forbiddenStringPattern.test(value)) {
      throw new Error(`${path} contains private-data-shaped text`)
    }
    return
  }
  if (typeof value !== "object") return

  for (const [key, child] of Object.entries(value as Record<string, unknown>)) {
    if (forbiddenKeyPattern.test(key) || forbiddenExactKeyPattern.test(key)) {
      throw new Error(`${path}.${key} is not public-safe`)
    }
    assertPublicProjectionSafe(child, `${path}.${key}`)
  }
}

export function projectPublicStatus(
  state: PylonLocalState,
  inventory?: PylonHostInventoryProjection,
  psionicConnector?: PsionicConnectorState,
) {
  const projection = {
    kind: "status",
    state: {
      schema: state.schema,
      packageName: state.packageName,
      version: state.version,
      paths: {
        home: state.paths.home,
        config: state.paths.config,
        cache: state.paths.cache,
        releases: state.paths.releases,
      },
      identity: state.identity,
      runtime: state.runtime,
      presence: state.presence,
      inventory,
      psionicConnector,
    },
  } as const

  assertPublicProjectionSafe(projection)
  return projection
}
