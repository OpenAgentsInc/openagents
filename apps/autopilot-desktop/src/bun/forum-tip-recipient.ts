import { existsSync, readFileSync, writeFileSync } from "node:fs"
import { join } from "node:path"
import {
  DEFAULT_OPENAGENTS_BASE_URL,
  loadPersistedCredential,
  readNodeIdentity,
  redactToken,
  sanitizeSparkAddress,
  type ReadFile,
  type WriteFile,
} from "./agent-onboarding.js"
import {
  canAttemptForumWrite,
  classifyForumWriteStatus,
  recordForumWriteAttempt,
} from "./forum-loop-bounds.js"

// AF-2 (#5899): automated forum tip-recipient readiness claim.
//
// Once the bundled node's Spark wallet is receive-ready, the user's agent should
// be able to *receive* tips on its forum posts. The forum gates tipping on
// `tipRecipientReadiness.tippingAvailable` (apps/openagents.com/workers/api/src/
// forum-routes.ts); a recipient becomes tippable by claiming readiness via
// `POST /api/forum/tip-recipient-wallets/claims` (the same call as
// `scripts/forum.mjs claim-tip-wallet`). Supplying the node's Spark address makes
// the readiness project `directPayment.kind = "spark_address"`
// (forum/recipient-wallet-readiness.ts), so a Spark sender can pay it directly.
//
// This is RECEIVE-ONLY and requires no spend authority, so it is safe to
// automate. SENDING tips (`tip-post`, `--approve-live-spend`) stays owner-gated
// and is explicitly out of scope.
//
// Discipline (mirrors `selfRegisterAgent`):
//   - Idempotent: a persisted receipt short-circuits; the request also carries a
//     deterministic-per-home `Idempotency-Key`, and a 409 is treated as
//     already-claimed (no duplicate).
//   - Offline-tolerant: any network/error path returns an honest non-throwing
//     outcome for the caller to retry; it never crashes the app.
//   - Secrets boundary (AGENTS.md): the agent token and the raw `spark1…` address
//     are payment/credential material. They ride ONLY the authenticated request
//     (Authorization bearer + body) and are NEVER logged, surfaced in a reason,
//     sent to the webview, or committed. Only public-safe redacted refs and the
//     boolean readiness cross any other boundary.

// Where the tip-ready receipt is persisted inside the managed PYLON_HOME.
const TIP_READY_FILENAME = "forum-tip-ready.json"

// The readiness ref the audit specifies for a Spark offline-receive claim.
const SPARK_OFFLINE_RECEIVE_READINESS_REF =
  "readiness.public.spark_address.offline_receive_ready"

// A descriptive User-Agent for openagents.com requests. AGENTS.md warns that a
// default/empty UA can hit the CDN `error code: 1010` 403; reuse a stable label.
const DEFAULT_USER_AGENT = "autopilot-desktop"

export type PersistedTipReadyReceipt = {
  // Public-safe wallet ref claimed for this home (never wallet material).
  readonly walletRef: string
  // Whether the server reported the recipient as tippable after the claim.
  readonly tippingAvailable: boolean
  readonly claimedAt: string
}

const defaultReadFile: ReadFile = (path: string) => {
  try {
    if (!existsSync(path)) return null
    return readFileSync(path, "utf8")
  } catch {
    return null
  }
}

const defaultWriteFile: WriteFile = (path: string, content: string) => {
  writeFileSync(path, content, { mode: 0o600 })
}

/**
 * Load a previously persisted tip-ready receipt from the managed home. Returns
 * null when none exists or the file is malformed. Never throws.
 */
export const loadTipReadyReceipt = (
  home: string,
  readFile: ReadFile = defaultReadFile,
): PersistedTipReadyReceipt | null => {
  const raw = readFile(join(home, TIP_READY_FILENAME))
  if (raw === null) return null
  let parsed: unknown
  try {
    parsed = JSON.parse(raw)
  } catch {
    return null
  }
  if (typeof parsed !== "object" || parsed === null) return null
  const record = parsed as Record<string, unknown>
  const walletRef =
    typeof record.walletRef === "string" && record.walletRef.trim().length > 0
      ? record.walletRef.trim()
      : null
  if (walletRef === null) return null
  return {
    walletRef,
    tippingAvailable: record.tippingAvailable === true,
    claimedAt:
      typeof record.claimedAt === "string" && record.claimedAt.length > 0
        ? record.claimedAt
        : new Date().toISOString(),
  }
}

/**
 * Observable boolean for the onboarding wizard: has this home claimed forum
 * tip-recipient readiness yet? Public-safe (reads only the persisted receipt's
 * boolean/ref). Never throws.
 */
export const isForumTipReady = (
  home: string,
  readFile: ReadFile = defaultReadFile,
): boolean => loadTipReadyReceipt(home, readFile) !== null

// Derive deterministic public-safe refs from the node npub suffix. These are
// redacted, non-secret handles only — never wallet material.
const npubSuffix = (npub: string): string =>
  npub.replace(/^npub1?/i, "").slice(0, 12)

const walletRefFor = (npub: string): string =>
  `wallet.public.autopilot_${npubSuffix(npub)}.redacted`

const receiveCapabilityRefFor = (npub: string): string =>
  `receive_capability.public.autopilot_${npubSuffix(npub)}.spark_offline_receive`

export type ClaimFetch = (
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

export type ClaimForumTipReadyOptions = {
  // Managed PYLON_HOME for the chosen identity.
  readonly home: string
  // Product base URL (default openagents.com).
  readonly baseUrl?: string
  // Gate: the forum tip-recipient claim only runs once the wallet can receive.
  readonly walletReceiveReady: boolean
  // The node's OWN raw Spark receive address. Payment material — forwarded only
  // into the authenticated claim body, never logged. Null/blank/malformed defers
  // (the wallet may not have produced one yet).
  readonly sparkAddress?: string | null
  // Descriptive User-Agent (default `autopilot-desktop`).
  readonly userAgent?: string
  // Injectables (default to real implementations).
  readonly fetchImpl?: ClaimFetch
  readonly readFile?: ReadFile
  readonly writeFile?: WriteFile
  // Honest, non-secret status callback. The message never contains the token or
  // the raw Spark address.
  readonly log?: (message: string) => void
}

export type ClaimForumTipReadyResult =
  // Readiness is now claimed (freshly, or reused from a persisted receipt / a
  // 409 already-claimed conflict).
  | {
      readonly outcome: "reused" | "claimed"
      readonly receipt: PersistedTipReadyReceipt
    }
  // The wallet is not receive-ready yet; the caller retries once it is.
  | { readonly outcome: "wallet_not_ready" }
  // No agent credential persisted yet (registration has not completed).
  | { readonly outcome: "not_registered" }
  // The node has not produced a usable Spark address yet; retry later.
  | { readonly outcome: "spark_pending" }
  // The node identity is not readable yet; retry later.
  | { readonly outcome: "identity_pending" }
  // AF-5 (#5902): the daily forum-write cap is exhausted; back off.
  | { readonly outcome: "rate_capped" }
  // The claim could not complete (offline / server error). Retry converges.
  | { readonly outcome: "deferred"; readonly reason: string }

const endpoint = (baseUrl: string, path: string): string =>
  new URL(path, baseUrl.endsWith("/") ? baseUrl : `${baseUrl}/`).toString()

/**
 * AF-2: ensure forum tip-recipient readiness is claimed for this home once the
 * Spark wallet is receive-ready. Idempotent, offline-tolerant, receive-only.
 * NEVER logs/persists/exposes the agent token or the raw Spark address.
 */
export const claimForumTipRecipientReadiness = async (
  options: ClaimForumTipReadyOptions,
): Promise<ClaimForumTipReadyResult> => {
  const baseUrl = options.baseUrl ?? DEFAULT_OPENAGENTS_BASE_URL
  const fetchImpl =
    options.fetchImpl ?? (globalThis.fetch as unknown as ClaimFetch)
  const readFile = options.readFile ?? defaultReadFile
  const writeFile = options.writeFile ?? defaultWriteFile
  const userAgent = options.userAgent ?? DEFAULT_USER_AGENT
  const log = options.log ?? (() => {})

  // 1. Reuse a persisted receipt — never re-claim.
  const existing = loadTipReadyReceipt(options.home, readFile)
  if (existing !== null) {
    return { outcome: "reused", receipt: existing }
  }

  // 2. The claim is receive-side: gate on wallet receive-readiness.
  if (!options.walletReceiveReady) {
    return { outcome: "wallet_not_ready" }
  }

  // 3. Need the persisted agent credential to authenticate the claim. The token
  //    is read here and used only as the bearer; it is never logged.
  const credential = loadPersistedCredential(options.home, readFile)
  if (credential === null) {
    return { outcome: "not_registered" }
  }

  // 4. Need the node identity (npub) to derive public-safe refs.
  const identity = readNodeIdentity(options.home, readFile)
  if (identity === null) {
    return { outcome: "identity_pending" }
  }

  // 5. Need the node's Spark address (payment material). Defer if not ready.
  const sparkAddress = sanitizeSparkAddress(options.sparkAddress)
  if (sparkAddress === null) {
    return { outcome: "spark_pending" }
  }

  // 6. AF-5 (#5902): honor the daily forum-write cap before the claim write.
  if (!canAttemptForumWrite(options.home, readFile)) {
    log("[forum-tip] daily forum-write cap reached; backing off")
    return { outcome: "rate_capped" }
  }

  const walletRef = walletRefFor(identity.npub)
  const body = {
    sparkAddress,
    providerClass: "mdk_agent_wallet",
    readinessRefs: [SPARK_OFFLINE_RECEIVE_READINESS_REF],
    receiveCapabilityRef: receiveCapabilityRefFor(identity.npub),
    walletRef,
  }
  const idempotencyKey = `autopilot-tip-ready-${npubSuffix(identity.npub)}`

  // Record the write attempt against today's budget right before sending.
  recordForumWriteAttempt(options.home, readFile, writeFile)
  let response: { readonly status: number; json(): Promise<unknown> }
  try {
    response = await fetchImpl(
      endpoint(baseUrl, "/api/forum/tip-recipient-wallets/claims"),
      {
        method: "POST",
        headers: {
          authorization: `Bearer ${credential.token}`,
          "content-type": "application/json",
          "idempotency-key": idempotencyKey,
          "user-agent": userAgent,
        },
        body: JSON.stringify(body),
      },
    )
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error)
    log(`[forum-tip] tip-readiness claim deferred (offline): ${reason}`)
    return { outcome: "deferred", reason }
  }

  const disposition = classifyForumWriteStatus(response.status)
  // A 409 means this home already claimed readiness (idempotency replay or a
  // prior claim). That IS ready — persist a receipt and treat as reused, never a
  // duplicate.
  if (disposition === "conflict") {
    const receipt: PersistedTipReadyReceipt = {
      walletRef,
      tippingAvailable: true,
      claimedAt: new Date().toISOString(),
    }
    writeFile(
      join(options.home, TIP_READY_FILENAME),
      `${JSON.stringify(receipt, null, 2)}\n`,
    )
    log("[forum-tip] tip-readiness already claimed for this home (reused)")
    return { outcome: "reused", receipt }
  }

  if (disposition === "rate_limited") {
    log("[forum-tip] tip-readiness claim deferred: rate limited (429)")
    return { outcome: "deferred", reason: "rate_limited" }
  }

  if (disposition === "payment_required") {
    // Receive-side claim should not be payable; if the server ever demands
    // payment, defer rather than spend (sending stays owner-gated).
    log("[forum-tip] tip-readiness claim deferred: payment required (402)")
    return { outcome: "deferred", reason: "payment_required" }
  }

  // The claim endpoint returns 201 on success; any other non-2xx defers.
  if (disposition !== "ok" || response.status !== 201) {
    log(
      `[forum-tip] tip-readiness claim deferred: unexpected status ${response.status}`,
    )
    return { outcome: "deferred", reason: `status_${response.status}` }
  }

  let tippingAvailable = false
  try {
    const parsed = (await response.json()) as {
      readonly tipRecipientReadiness?: { readonly tippingAvailable?: unknown }
    }
    tippingAvailable = parsed.tipRecipientReadiness?.tippingAvailable === true
  } catch {
    // A malformed body still means the 201 claim landed; record it as ready and
    // let the next poll reconcile the precise flag.
    tippingAvailable = true
  }

  const receipt: PersistedTipReadyReceipt = {
    walletRef,
    tippingAvailable,
    claimedAt: new Date().toISOString(),
  }
  writeFile(
    join(options.home, TIP_READY_FILENAME),
    `${JSON.stringify(receipt, null, 2)}\n`,
  )
  log(
    `[forum-tip] forum tip-recipient readiness claimed (${redactToken(credential.token)})`,
  )
  return { outcome: "claimed", receipt }
}
