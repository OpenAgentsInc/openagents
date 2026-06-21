import { existsSync, readFileSync, writeFileSync } from "node:fs"
import { join } from "node:path"

// AO-1 / AO-2 (EPIC #5441, issues #5442 + #5443): first-run agent
// self-registration + node child-env injection.
//
// The central break in the auto-onboarding chain (audit
// `docs/launch/2026-06-18-autopilot-desktop-availability-audit.md` §3b): a fresh
// desktop install boots an isolated node. The node auto-generates a Nostr
// identity and auto-provisions a receive-ready Spark wallet, but it never
// creates an OpenAgents *agent account* — nothing calls
// `POST /api/agents/register`, so `OPENAGENTS_AGENT_TOKEN` is never minted, and
// presence / payout-target / Tassadar-join all stay dark because the desktop
// also never sets `PYLON_OPENAGENTS_BASE_URL` or `PYLON_ASSIGNMENT_WORKER=1`.
//
// This module closes both gaps without any new Pylon runtime code:
//   - AO-1: `selfRegisterAgent` calls the public self-serve registration
//     endpoint using the node's generated identity, and persists the returned
//     `oa_agent_...` token securely in the managed PYLON_HOME. It is idempotent
//     (reuses a persisted token, never re-registers) and offline-tolerant (a
//     failed call surfaces an honest result, never throws/crashes; the caller
//     retries and the chain converges when back online).
//   - AO-2: `buildOnboardingChildEnv` produces the env switches the existing
//     Pylon runtime already reads at boot — `PYLON_OPENAGENTS_BASE_URL`,
//     `OPENAGENTS_AGENT_TOKEN`, `PYLON_ASSIGNMENT_WORKER=1` — so presence
//     (bearer path), the Spark payout-target registration (#5305), the
//     assignment worker, and the Tassadar work-claim loop all light up.
//
// Secrets boundary (AGENTS.md): the token lives in the Bun host + the managed
// home file only; it is NEVER logged, printed, sent to the webview, or committed.
// `redactToken` exists so any status/log line that must mention the credential
// shows a prefix-only form.

// The default product base URL the node registers presence/assignment against.
export const DEFAULT_OPENAGENTS_BASE_URL = "https://openagents.com"

// Where the minted agent credential is persisted inside the managed PYLON_HOME.
// 0600 like the node's own `identity.json` — private, never world-readable.
const CREDENTIAL_FILENAME = "agent-credential.json"
// The node writes its identity here (apps/pylon/src/state.ts `loadOrCreateIdentity`).
const IDENTITY_FILENAME = "identity.json"

// AF-1 (#5898): defensive client-side shape check for a Spark receive address
// before we attach it to a registration body. Mirrors the server's accepted
// `sparkAddress` pattern (apps/openagents.com/workers/api/src/agent-registration.ts):
// a Spark HRP (`spark`/`sp`/testnet/regtest/signet/staging variants) + `1` +
// bech32m payload. We only forward addresses that match, so a transient/garbled
// read (e.g. an error string) is dropped rather than posted. This is a syntactic
// guard, not a claim of on-chain validity.
const SPARK_ADDRESS_PATTERN =
  /^(?:spark|sparkt|sparkrt|sparks|sp|spt|sprt|sps)1[qpzry9x8gf2tvdw0s3jn54khce6mua7l]{16,512}$/i

// Normalize a candidate Spark address: trim, validate shape, else null. NEVER
// log the input/output here — it is payment material. Exported so the forum
// tip-recipient claim (AF-2 #5899) reuses the exact same client-side guard.
export const sanitizeSparkAddress = (
  value: string | null | undefined,
): string | null => {
  if (typeof value !== "string") return null
  const trimmed = value.trim()
  if (trimmed.length === 0 || trimmed.length > 600) return null
  return SPARK_ADDRESS_PATTERN.test(trimmed) ? trimmed : null
}

export type NodeIdentity = {
  // bech32 Nostr public key (npub1...) — stable per managed home; used as the
  // deterministic externalId so re-registration from the same home is a no-op
  // conflict rather than a duplicate agent.
  readonly npub: string
  // Human-ish node label (host-derived or operator-set); seeds the auto display
  // name this phase. AO-3 will let the user choose a name explicitly.
  readonly nodeLabel: string | null
  // Stable pylon ref; seeds a slug candidate.
  readonly pylonRef: string | null
}

export type PersistedAgentCredential = {
  // The full `oa_agent_...` bearer token. NEVER log/print/commit this value.
  readonly token: string
  // Public prefix of the token (safe to surface) for honest status lines.
  readonly tokenPrefix: string
  // The registered agent user id (safe to surface).
  readonly userId: string
  // The externalId used at registration (the node npub) — lets us prove a
  // persisted credential belongs to the current home's identity.
  readonly externalId: string
  readonly registeredAt: string
}

// Minimal projection of the `POST /api/agents/register` 201 response we depend
// on (full contract: apps/openagents.com/workers/api/src/agent-registration.ts
// `AgentRegistrationRecord`). We read only public-safe identity fields + the
// credential token.
type AgentRegistrationResponse = {
  readonly user?: { readonly id?: unknown; readonly status?: unknown }
  readonly credential?: {
    readonly token?: unknown
    readonly tokenPrefix?: unknown
  }
}

// --- token redaction --------------------------------------------------------

// Render a credential as a prefix-only, non-secret string for status/log use.
// `oa_agent_abcd...` -> `oa_agent_…` (we keep only the public scheme prefix).
export const redactToken = (token: string | null | undefined): string => {
  if (typeof token !== "string" || token.length === 0) return "<none>"
  const scheme = "oa_agent_"
  return token.startsWith(scheme) ? `${scheme}…` : "oa_…"
}

// --- identity read ----------------------------------------------------------

export type ReadFile = (path: string) => string | null

const defaultReadFile: ReadFile = path => {
  try {
    if (!existsSync(path)) return null
    return readFileSync(path, "utf8")
  } catch {
    return null
  }
}

const asTrimmedString = (value: unknown): string | null =>
  typeof value === "string" && value.trim().length > 0 ? value.trim() : null

/**
 * Read the node's generated identity from `<home>/identity.json`. Returns null
 * until the node has booted far enough to write it (the launcher polls). Never
 * throws — a malformed/absent file is just "not ready yet".
 */
export const readNodeIdentity = (
  home: string,
  readFile: ReadFile = defaultReadFile,
): NodeIdentity | null => {
  const raw = readFile(join(home, IDENTITY_FILENAME))
  if (raw === null) return null
  let parsed: unknown
  try {
    parsed = JSON.parse(raw)
  } catch {
    return null
  }
  if (typeof parsed !== "object" || parsed === null) return null
  const record = parsed as Record<string, unknown>
  const npub = asTrimmedString(record.npub)
  if (npub === null) return null
  return {
    npub,
    nodeLabel: asTrimmedString(record.nodeLabel),
    pylonRef: asTrimmedString(record.pylonRef),
  }
}

// --- credential persistence -------------------------------------------------

export type WriteFile = (path: string, content: string) => void

const defaultWriteFile: WriteFile = (path, content) => {
  writeFileSync(path, content, { mode: 0o600 })
}

/**
 * Load a previously persisted agent credential from the managed home. Returns
 * null when none exists or the file is malformed/missing a token. Never throws.
 */
export const loadPersistedCredential = (
  home: string,
  readFile: ReadFile = defaultReadFile,
): PersistedAgentCredential | null => {
  const raw = readFile(join(home, CREDENTIAL_FILENAME))
  if (raw === null) return null
  let parsed: unknown
  try {
    parsed = JSON.parse(raw)
  } catch {
    return null
  }
  if (typeof parsed !== "object" || parsed === null) return null
  const record = parsed as Record<string, unknown>
  const token = asTrimmedString(record.token)
  if (token === null || !token.startsWith("oa_agent_")) return null
  const userId = asTrimmedString(record.userId)
  const externalId = asTrimmedString(record.externalId)
  return {
    token,
    tokenPrefix: asTrimmedString(record.tokenPrefix) ?? "oa_agent_",
    userId: userId ?? "",
    externalId: externalId ?? "",
    registeredAt:
      asTrimmedString(record.registeredAt) ?? new Date().toISOString(),
  }
}

/**
 * Persist a minted agent credential to the managed home with 0600 perms. The
 * file holds the secret token; it never leaves the Bun host or this directory.
 */
export const persistCredential = (
  home: string,
  credential: PersistedAgentCredential,
  writeFile: WriteFile = defaultWriteFile,
): void => {
  writeFile(
    join(home, CREDENTIAL_FILENAME),
    `${JSON.stringify(credential, null, 2)}\n`,
  )
}

// --- auto display name + slug -----------------------------------------------

// A sensible default identity for this phase (AO-3 will let the user choose).
// Neutral, deterministic per home: derived from the node label + a short npub
// suffix so two fresh installs do not collide on the display name.
export const autoDisplayName = (identity: NodeIdentity): string => {
  const suffix = identity.npub.replace(/^npub1?/i, "").slice(0, 6)
  const base = identity.nodeLabel ?? "Autopilot Desktop"
  // displayName max length is 120 (agent-registration schema); stay well under.
  return `${base} (${suffix})`.slice(0, 120)
}

// AO-3 (#5444): normalize a user-chosen display name to the agent-registration
// schema (non-empty, max 120 chars; collapse whitespace). Returns null when the
// user supplied nothing usable, so the caller falls back to `autoDisplayName`.
// Never throws — a blank/over-long name is just clamped/rejected, never a crash.
export const normalizeDisplayName = (
  raw: string | null | undefined,
): string | null => {
  if (typeof raw !== "string") return null
  const collapsed = raw.replace(/\s+/g, " ").trim()
  if (collapsed.length === 0) return null
  return collapsed.slice(0, 120)
}

// Derive a registration slug candidate (lowercase, [a-z0-9-], 3..80) from the
// pylon ref or npub. The server treats slug as optional + best-effort-unique; a
// collision just means the server assigns its own — registration still succeeds.
export const autoSlug = (identity: NodeIdentity): string | undefined => {
  const source =
    identity.pylonRef ?? `autopilot-${identity.npub.replace(/^npub1?/i, "")}`
  const slug = source
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 80)
  return slug.length >= 3 ? slug : undefined
}

// --- self-registration (AO-1) ----------------------------------------------

export type RegisterFetch = (
  url: string,
  init: {
    readonly method: string
    readonly headers: Record<string, string>
    readonly body: string
  },
) => Promise<{
  readonly status: number
  json(): Promise<unknown>
}>

export type SelfRegisterOptions = {
  // Managed PYLON_HOME for the chosen identity.
  readonly home: string
  // Product base URL to register against (default openagents.com).
  readonly baseUrl?: string
  // AO-3 (#5444): the display name the user chose when creating a new Autopilot
  // identity ("Create a new Autopilot identity" → user names it). When provided
  // and non-empty it REPLACES the auto-derived display name, so the agent
  // registers under the user's chosen name. Omitted/empty falls back to the
  // neutral `autoDisplayName` default (Phase 1 behavior).
  readonly displayName?: string | null
  // Optional BOLT12 offer to attach for tip-readiness (audit notes this is
  // optional). Omitted when the wallet has not produced one yet.
  readonly bolt12Offer?: string | null
  // AF-1 (#5898): the node's OWN raw Spark receive address (`spark1…`). When the
  // wallet is receive-ready the launcher reads it from the node and passes it in
  // here so agent registration lands tip readiness as `directPayment.kind =
  // "spark_address"` (AGENTS.md Step 3 prefers Spark over BOLT12). PAYMENT
  // MATERIAL: it rides ONLY the authenticated registration request body (the
  // same trust model as the #5305 payout-target path). It is NEVER logged,
  // printed, surfaced in a `deferred` reason, sent to the webview, or committed.
  // Omitted/blank/malformed => registration proceeds without it (unchanged
  // behavior); tip readiness then lands later via the payout-target + forum
  // tip-recipient claim paths.
  readonly sparkAddress?: string | null
  // Injectables (default to real implementations).
  readonly fetchImpl?: RegisterFetch
  readonly readFile?: ReadFile
  readonly writeFile?: WriteFile
  // Honest, non-secret status callback. The message never contains the token.
  readonly log?: (message: string) => void
}

export type SelfRegisterResult =
  // A valid credential is now persisted (either reused or freshly minted).
  | {
      readonly outcome: "reused" | "registered"
      readonly credential: PersistedAgentCredential
    }
  // The node has not written its identity yet; the caller should retry.
  | { readonly outcome: "identity_pending" }
  // Registration could not complete (offline / server error). Offline-tolerant:
  // the caller retries; the chain converges when connectivity returns.
  | { readonly outcome: "deferred"; readonly reason: string }

const endpoint = (baseUrl: string, path: string): string =>
  new URL(path, baseUrl.endsWith("/") ? baseUrl : `${baseUrl}/`).toString()

/**
 * AO-1: ensure the agent for this managed home is self-registered and its token
 * is persisted. Idempotent (reuses a persisted token), offline-tolerant (never
 * throws; returns an honest `deferred`/`identity_pending` for the caller to
 * retry). NEVER logs/prints/commits the token.
 */
export const selfRegisterAgent = async (
  options: SelfRegisterOptions,
): Promise<SelfRegisterResult> => {
  const baseUrl = options.baseUrl ?? DEFAULT_OPENAGENTS_BASE_URL
  const fetchImpl =
    options.fetchImpl ?? (globalThis.fetch as unknown as RegisterFetch)
  const readFile = options.readFile ?? defaultReadFile
  const writeFile = options.writeFile ?? defaultWriteFile
  const log = options.log ?? (() => {})

  // 1. Reuse a valid persisted credential — never re-register.
  const existing = loadPersistedCredential(options.home, readFile)
  if (existing !== null) {
    log(
      `[onboarding] reusing persisted agent credential (${redactToken(existing.token)})`,
    )
    return { outcome: "reused", credential: existing }
  }

  // 2. Need the node's identity to register. Until the node writes it, defer.
  const identity = readNodeIdentity(options.home, readFile)
  if (identity === null) {
    return { outcome: "identity_pending" }
  }

  // 3. Self-register against the public self-serve endpoint. The npub is the
  //    deterministic externalId, so a retry after a partial success (token lost
  //    before persist) returns a 409 conflict instead of a duplicate agent.
  const slug = autoSlug(identity)
  // AO-3 (#5444): a user-chosen name (from "Create a new Autopilot identity")
  // replaces the auto-derived default. Empty/blank falls back to the neutral
  // per-home default so a fresh from-scratch run still registers a sane name.
  const chosenName = normalizeDisplayName(options.displayName)
  const body: Record<string, unknown> = {
    displayName: chosenName ?? autoDisplayName(identity),
    externalId: identity.npub,
    metadata: {
      source: "autopilot-desktop",
      purpose: "contributor_node_auto_onboarding",
      ...(identity.pylonRef ? { pylonRef: identity.pylonRef } : {}),
    },
  }
  if (slug !== undefined) body.slug = slug
  const offer = options.bolt12Offer?.trim()
  if (offer && offer.length > 0) body.bolt12Offer = offer
  // AF-1 (#5898): attach the node's Spark receive address when available so tip
  // readiness lands as `spark_address` (AGENTS.md Step 3). Validated client-side
  // first; payment material, so it only ever rides this authenticated body and
  // is never echoed into a log/reason.
  const sparkAddress = sanitizeSparkAddress(options.sparkAddress)
  if (sparkAddress !== null) body.sparkAddress = sparkAddress

  let response: { readonly status: number; json(): Promise<unknown> }
  try {
    response = await fetchImpl(endpoint(baseUrl, "/api/agents/register"), {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    })
  } catch (error) {
    // Offline / network error: defer, do not crash. Convergence happens on a
    // later retry. The error message cannot contain the token (we never sent a
    // bearer; the body has no secret beyond an optional public BOLT12 offer,
    // which we keep out of the surfaced reason).
    const reason = error instanceof Error ? error.message : String(error)
    log(`[onboarding] registration deferred (offline): ${reason}`)
    return { outcome: "deferred", reason }
  }

  if (response.status === 409) {
    // The agent already exists for this externalId but we have no persisted
    // token (e.g. token lost before persist, or registered on a prior install
    // of the same home). We cannot recover the original token from a conflict;
    // defer so the operator/AO-3 flow can resolve, rather than minting a
    // duplicate. This is rare and only on a damaged home.
    log(
      "[onboarding] registration conflict: agent already exists for this identity; deferring",
    )
    return { outcome: "deferred", reason: "agent_registration_conflict" }
  }

  if (response.status !== 201) {
    log(
      `[onboarding] registration deferred: unexpected status ${response.status}`,
    )
    return { outcome: "deferred", reason: `status_${response.status}` }
  }

  let parsed: AgentRegistrationResponse
  try {
    parsed = (await response.json()) as AgentRegistrationResponse
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error)
    log(`[onboarding] registration deferred: malformed response (${reason})`)
    return { outcome: "deferred", reason: "malformed_response" }
  }

  const token = asTrimmedString(parsed.credential?.token)
  if (token === null || !token.startsWith("oa_agent_")) {
    log("[onboarding] registration deferred: response missing agent token")
    return { outcome: "deferred", reason: "missing_token" }
  }

  const credential: PersistedAgentCredential = {
    token,
    tokenPrefix: asTrimmedString(parsed.credential?.tokenPrefix) ?? "oa_agent_",
    userId: asTrimmedString(parsed.user?.id) ?? "",
    externalId: identity.npub,
    registeredAt: new Date().toISOString(),
  }
  persistCredential(options.home, credential, writeFile)
  log(`[onboarding] agent registered + token persisted (${redactToken(token)})`)
  return { outcome: "registered", credential }
}

// --- node child env injection (AO-2) ---------------------------------------

export type OnboardingChildEnvInput = {
  // Base env (already includes the forced PYLON_HOME from the launcher).
  readonly base: Record<string, string>
  // The persisted agent token, or null when not yet registered.
  readonly agentToken: string | null
  // Product base URL (default openagents.com).
  readonly baseUrl?: string
}

/**
 * AO-2: layer the onboarding env switches onto the node child env so the
 * existing Pylon runtime lights up presence + payout-target + the Tassadar
 * assignment worker. Pure: returns a new record; does not mutate `base`.
 *
 * Only sets the agent-token-gated switches when a token is present — a node
 * without a token boots in its prior isolated (but honest) mode rather than
 * announcing presence it cannot authenticate or looking partially configured.
 * Explicit operator-provided product env is preserved, but first-run auto setup
 * waits until registration has produced a persisted token.
 */
export const buildOnboardingChildEnv = (
  input: OnboardingChildEnvInput,
): Record<string, string> => {
  const baseUrl = input.baseUrl ?? DEFAULT_OPENAGENTS_BASE_URL
  const env: Record<string, string> = { ...input.base }
  if (input.agentToken !== null && input.agentToken.length > 0) {
    // Respect an explicit operator override if one is already present.
    if (!env.PYLON_OPENAGENTS_BASE_URL) {
      env.PYLON_OPENAGENTS_BASE_URL = baseUrl
    }
    env.OPENAGENTS_AGENT_TOKEN = input.agentToken
    // Turn on the assignment/Tassadar work-claim loop. Only meaningful with a
    // token + base URL, both of which are now set.
    if (!env.PYLON_ASSIGNMENT_WORKER) {
      env.PYLON_ASSIGNMENT_WORKER = "1"
    }
  }
  return env
}
