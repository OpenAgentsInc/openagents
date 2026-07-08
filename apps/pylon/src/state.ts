import { mkdir, readFile, writeFile } from "node:fs/promises"
import { existsSync } from "node:fs"
import { createHash, randomUUID } from "node:crypto"
import { hostname } from "node:os"
import { dirname } from "node:path"
import { type BootstrapSummary } from "./bootstrap.js"
import { PYLON_VERSION, type PylonVersion } from "./version.js"
import type { PylonHostInventoryProjection } from "./inventory.js"
import { loadOrCreateNostrIdentity } from "./nostr-identity.js"

export type PylonLifecycleState = "offline" | "online" | "paused" | "degraded" | "assignment-ready"

export type PylonPaths = BootstrapSummary["paths"] & {
  identity: string
  identityMnemonic: string
  runtimeState: string
  presenceState: string
  activeAssignmentRuns: string
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
  version: PylonVersion
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
  // #5305: the redacted `payout.spark.<digest>` ref of this node's OWN Spark
  // address once it has been auto-registered as a payout target. Null until the
  // first successful auto-register. This is a digest (public-safe), NEVER the
  // raw `spark1…` address — that only ever rides the authenticated private
  // request body. Persisted so the auto-register is idempotent across reboots.
  sparkPayoutTargetRef: string | null
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
  /(^|[._-])(wallet_seed|seed|mnemonic|private_key|privatekey|preimage|bearer|access_token|api_key|apikey|provider_token|provider_auth|raw_prompt|raw_prompts|private_repo|repo_content|private_topology|cache_path|cachepath|env_dump|environment_dump|capacity_pool_secret|internal_accounting_credential|invoice|offer|payment_hash|payment_preimage|spark_address|spark_invoice|spark_request|secret|password|xprv)([._-]|$)/i

const forbiddenExactKeyPattern =
  /^(walletSeed|seed|mnemonic|privateKey|private_key|preimage|bearer|accessToken|apiKey|providerToken|providerAuth|rawPrompt|rawPrompts|privateRepo|repoContent|privateTopology|cachePath|envDump|environmentDump|capacityPoolSecret|internalAccountingCredential|invoice|offer|paymentHash|paymentPreimage|sparkAddress|sparkInvoice|sparkRequest|secret|password|xprv)$/i

const forbiddenStringPatterns = [
  /\b(?:wallet seed|mnemonic|private key|capacity pool secret|internal accounting credential)\s*[:=]\s*\S+/i,
  /\bpayment preimage\s+\S+/i,
  /\bbearer\s+(?!token\b)[a-z0-9._-]{6,}\b/i,
  /\bsk-[a-z0-9_-]+\b/i,
  /\blnbc[a-z0-9]+\b/i,
  /\blntb[a-z0-9]+\b/i,
  /\blno[a-z0-9]+\b/i,
  /\bspark1[a-z0-9]{20,}\b/i,
  /\bsprt1[a-z0-9]{20,}\b/i,
  /\bspt1[a-z0-9]{20,}\b/i,
  /\bsp1[a-z0-9]{20,}\b/i,
  /\bprivate-repo:\/\/\S*/i,
  /\bprivate_repo\b/i,
  /\braw prompt\b/i,
  /\bxprv[a-z0-9]+\b/i,
  /\/Users\/[^\s]+\/\.cache\b/i,
  /\/home\/[^\s]+\/\.cache\b/i,
]

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
    activeAssignmentRuns: `${paths.home}/active-assignment-runs`,
    assignmentState: `${paths.home}/assignment-state.json`,
    ledger: `${paths.home}/ledger.jsonl`,
  }
}

export async function ensureStateDirectories(paths: PylonPaths) {
  await mkdir(paths.home, { recursive: true })
  await mkdir(paths.cache, { recursive: true })
  await mkdir(paths.releases, { recursive: true })
  await mkdir(paths.activeAssignmentRuns, { recursive: true })
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
  // #6354: NEVER drop capabilities the persisted runtime already holds. The
  // bootstrap/config capability set (e.g. tassadar + nip90 + labor from
  // config.json) is a non-default base, but DYNAMIC capabilities probed by
  // `provider go-online` — notably `capability.pylon.local_codex` and
  // `capability.pylon.local_claude_agent` for accounts linked AFTER the base
  // config was written — live only in the persisted runtime. The old logic
  // overwrote the runtime with the requested base whenever it carried any
  // non-default ref, so every read-ish command (`status`/`heartbeat`/
  // `assignment`) stripped codex/claude and the standing fleet's heartbeats
  // advertised `codex available=0`, 409ing genuinely codex-available Pylons.
  // `loadOrCreate` must preserve, never mutate-away, what the node already
  // declared; `go-online` remains the authority for a full replace via
  // `writeRuntimeState` (so unlinking a capability still removes it there).
  const capabilityRefs =
    existing?.capabilityRefs
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
    sparkPayoutTargetRef: existing?.sparkPayoutTargetRef ?? null,
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

export async function ensurePylonLocalState(
  summary: Pick<BootstrapSummary, "bootstrap" | "paths">,
): Promise<PylonLocalState> {
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
    version: PYLON_VERSION,
    paths,
    identity,
    runtime,
    presence,
  }
}

export function assertPublicProjectionSafe(value: unknown, path = "projection"): asserts value is PublicProjection {
  if (value === null || value === undefined) return
  if (typeof value === "string") {
    if (forbiddenStringPatterns.some(pattern => pattern.test(value))) {
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
    },
  } as const

  assertPublicProjectionSafe(projection)
  return projection
}
