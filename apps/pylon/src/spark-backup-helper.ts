// ---------------------------------------------------------------------------
// Spark backup receive helper adapter (slice 2 of #5078) plus the private
// consented own-wallet sweep adapter (#5169).
//
// Backs the slice-1 `SparkBackupHelper` contract with the real Breez SDK Spark
// JavaScript package (`@breeztech/breez-sdk-spark`). Public helper commands are
// receive/status/claim only; the sweep adapter is private to `migrate-spark`.
//
// Feasibility (verified 2026-06-15): the package is WASM-based, not native
// bindings. Its nodejs entry imports and initializes cleanly under the Pylon
// Bun runtime (Bun 1.3.x). We use the SDK directly in-process rather than an
// isolated helper binary.
//
// Storage under Bun (#5080): the SDK's DEFAULT `connect()` path uses a
// better-sqlite3-backed storage backend, and better-sqlite3 is NOT supported by
// Bun (oven-sh/bun#4290). The only other shipped backends are MySQL/Postgres.
// So `connect()` fails at "initialize database" under Pylon's Bun runtime. We
// fix this by building the SDK via `SdkBuilder.new(config, seed)` and injecting
// a Bun-native `bun:sqlite` storage backend through `.withStorage()` (see
// `spark-bun-storage.ts`, a faithful port of the SDK's reference storage). The
// legacy `connect()` path is kept only as a fallback for injected test modules
// that do not expose `SdkBuilder`.
//
// Inert by default: this module performs NO work and imports NO SDK code until
// the backup is opt-in enabled AND a Breez/Spark credential + a wallet seed are
// configured. With anything missing the adapter resolves to the slice-1
// `unavailableSparkBackupHelper` so `classifySparkBackupReceive` reports the
// existing `helper-unavailable` / `credential-missing` blockers. The SDK is an
// OPTIONAL dependency; if it is not installed the import fails softly and the
// helper stays unavailable.
//
// SAFETY: this module NEVER logs or emits Breez API keys, raw Spark
// addresses/invoices/payment-requests, preimages, mnemonics, raw MDK receive
// targets, or SDK/wallet storage paths. Raw receive material is returned only
// inside local helper stdout (for explicit local display) or kept inside the
// private sweep closure; public projections get refs only.
// ---------------------------------------------------------------------------

import { createHash } from "node:crypto"
import { mkdirSync } from "node:fs"
import * as nodePath from "node:path"
import type {
  LegacySparkCommandRunner,
  SparkBackupCommand,
  SparkBackupHelper,
  SparkBackupSendTransfer,
  SparkBackupSendTransferResult,
  SparkBackupSweepTransfer,
  SparkBackupSweepTransferResult,
  WalletCommandResult,
} from "./wallet.js"
// #5304: the single canonical "is the Spark backup ON?" resolver (default-ON
// unless an explicit OFF override). Used so the helper resolver enables by
// default instead of requiring PYLON_SPARK_BACKUP_ENABLED.
import { isSparkBackupDefaultEnabled } from "./wallet.js"
import { SparkBunStorage } from "./spark-bun-storage.js"
import { toSatNumber } from "./sat-number.js"
import { ensureSparkWasmAvailable } from "./spark-wasm-runtime.js"
// Re-export so existing importers (and tests) can keep importing it from here.
export { toSatNumber } from "./sat-number.js"

// Embedded default OpenAgents Breez/Spark API key. This key was committed to the
// repo historically (commit 783f33d5f, as the Rust EmbeddedDefault) and is
// owner-authorized for hardcoding so the receive-only Spark backup works
// out-of-box once enabled. It is a service API key, not a wallet seed, and grants
// no spend authority. Any of the env vars below override it. Inert-by-default is
// enforced by the PYLON_SPARK_BACKUP_ENABLED flag, NOT by key presence.
export const DEFAULT_OPENAGENTS_SPARK_API_KEY =
  "MIIBfjCCATCgAwIBAgIHPYzgGw0A+zAFBgMrZXAwEDEOMAwGA1UEAxMFQnJlZXowHhcNMjQxMTI0MjIxOTMzWhcNMzQxMTIyMjIxOTMzWjA3MRkwFwYDVQQKExBPcGVuQWdlbnRzLCBJbmMuMRowGAYDVQQDExFDaHJpc3RvcGhlciBEYXZpZDAqMAUGAytlcAMhANCD9cvfIDwcoiDKKYdT9BunHLS2/OuKzV8NS0SzqV13o4GBMH8wDgYDVR0PAQH/BAQDAgWgMAwGA1UdEwEB/wQCMAAwHQYDVR0OBBYEFNo5o+5ea0sNMlW/75VgGJCv2AcJMB8GA1UdIwQYMBaAFN6q1pJW843ndJIW/Ey2ILJrKJhrMB8GA1UdEQQYMBaBFGNocmlzQG9wZW5hZ2VudHMuY29tMAUGAytlcANBABvQIfNsop0kGIk0bgO/2kPum5B5lv6pYaSBXz73G1RV+eZj/wuW88lNQoGwVER+rA9+kWWTaR/dpdi8AFwjxw0="

/**
 * Minimal structural view of the Breez SDK Spark surface this adapter uses.
 * We intentionally do NOT depend on the SDK's type package at compile time
 * (it is an optional dependency loaded at runtime), so this is a narrow shape
 * covering the receive helper commands plus the consented send/sweep calls.
 */
type BreezSparkSdk = {
  getInfo(request: { ensureSynced?: boolean }): Promise<{ balanceSats?: number }>
  receivePayment(request: {
    paymentMethod: { type: "sparkAddress" }
  }): Promise<{ paymentRequest?: string }>
  listPayments(request: { limit?: number; statusFilter?: string[] }): Promise<{ payments?: unknown[] }>
  listUnclaimedDeposits(request: Record<string, never>): Promise<{ deposits?: unknown[] }>
  // #5166: claim Lightning HTLC receives (offline-receive funds arrive as HTLCs
  // that must be claimed before they credit the balance). syncWallet pulls latest.
  syncWallet?(request: Record<string, never>): Promise<unknown>
  claimHtlcPayment(request: { preimage: string }): Promise<{ payment?: { amount?: unknown } }>
  // Static Lightning Address (LNURL-pay) hosted by this Spark wallet's LSP.
  // Optional-tolerant: not all SDK builds / injected test fakes expose these.
  getLightningAddress?(): Promise<{ lightningAddress?: string } | undefined>
  registerLightningAddress?(request: {
    username: string
    description?: string
  }): Promise<{ lightningAddress?: string }>
  checkLightningAddressAvailable?(request: { username: string }): Promise<boolean>
  parse?(input: string): Promise<
    | ({ type: "lightningAddress"; payRequest: unknown } & Record<string, unknown>)
    | ({ type: "lnurlPay" } & Record<string, unknown>)
    | ({ type: string } & Record<string, unknown>)
  >
  prepareLnurlPay?(request: {
    amount: bigint
    comment?: string
    payRequest: unknown
  }): Promise<unknown>
  lnurlPay?(request: {
    prepareResponse: unknown
    idempotencyKey?: string
  }): Promise<{
    payment?: {
      id?: unknown
      amount?: unknown
      fees?: unknown
      status?: unknown
    }
  }>
  prepareSendPayment?(request: {
    paymentRequest: string
    amount?: bigint
  }): Promise<unknown>
  sendPayment?(request: {
    prepareResponse: unknown
    options?: {
      type: "bolt11Invoice"
      preferSpark: boolean
      completionTimeoutSecs?: number
    }
    idempotencyKey?: string
  }): Promise<{
    payment?: {
      id?: unknown
      amount?: unknown
      fees?: unknown
      status?: unknown
    }
    id?: unknown
    amount?: unknown
    fees?: unknown
    status?: unknown
  }>
  disconnect?(): Promise<void> | void
}

/**
 * Minimal structural view of the SDK's `Storage` interface — only what
 * `withStorage()` needs to accept our `bun:sqlite` backend. The real SDK type
 * has 28 methods; `SparkBunStorage` implements all of them faithfully.
 */
type BreezSparkStorage = unknown

/**
 * Minimal structural view of `SdkBuilder` (d.ts line ~1658). We inject a
 * `bun:sqlite`-backed storage here to bypass the better-sqlite3 default that
 * fails under Bun (#5080).
 */
type BreezSparkSdkBuilder = {
  withStorage(storage: BreezSparkStorage): BreezSparkSdkBuilder
  build(): Promise<BreezSparkSdk>
}

type BreezSparkModule = {
  defaultConfig(network: string): { apiKey?: string } & Record<string, unknown>
  // Builder path (real SDK + Bun storage). Preferred under Pylon's Bun runtime.
  // `SdkBuilder.new` is a STATIC factory method on the class (not a constructor).
  SdkBuilder?: {
    new: (
      config: Record<string, unknown>,
      seed: { type: "mnemonic"; mnemonic: string; passphrase?: string },
    ) => BreezSparkSdkBuilder
  }
  // Legacy default path (better-sqlite3 storage). Fails under Bun; kept only for
  // injected test modules that do not expose SdkBuilder.
  connect?(request: {
    config: Record<string, unknown>
    seed: { type: "mnemonic"; mnemonic: string; passphrase?: string }
    storageDir?: string
  }): Promise<BreezSparkSdk>
}

export type SparkBackupAdapterConfig = {
  apiKey: string
  mnemonic: string
  network?: "mainnet" | "regtest"
  storageDir?: string
  // Test seam: inject a fake module instead of importing the real SDK.
  loadModule?: () => Promise<BreezSparkModule>
  // Test seam / safety: cap how long an SDK call may run before we treat the
  // helper as unavailable (short-lived sidecar discipline from the audit).
  timeoutMs?: number
  // #5207 warm session: when true, this adapter reuses a process-level singleton
  // Spark SDK session (built once, KEPT ALIVE, NOT disconnected per op) keyed by
  // (mnemonic+network+storageDir) instead of cold-building and disconnecting a
  // fresh SDK per invocation. DEFAULTS OFF — the one-shot CLI keeps the cold,
  // self-contained path (each command builds → syncs → sends → disconnects);
  // only the long-lived daemon turns it on (so the warm SDK lives in the right
  // process). When `undefined`, the adapter consults the PYLON_SPARK_WARM_SESSION
  // env flag (also default-off). When off, the cold path is byte-for-byte
  // unchanged.
  warmSession?: boolean
  // #5254: explicit operator override that RAISES the allowed-fee ceiling for
  // the pre-send fee guard. Undefined keeps the default bound (the helper also
  // consults PYLON_SPARK_MAX_FEE_SATS). A per-call `maxFeeSats` on the transfer
  // input takes precedence over this adapter-level default.
  maxFeeSats?: number
}

const DEFAULT_SPARK_TIMEOUT_MS = 15_000
// #5196: the SDK is given this long to confirm a Lightning send actually settles.
// The send withTimeout (below) MUST outlast it, or a slow/large send is aborted
// while it completes server-side — reporting send-failed AFTER the funds already
// moved (a dangerous false-negative that invites a double-spend retry).
const SEND_COMPLETION_TIMEOUT_SECS = 60
const SEND_TIMEOUT_BUFFER_MS = 15_000
// #5197: a fresh post-restart Spark sync can take longer than the short read
// timeout. Give status reads room to FORCE-sync before reporting a balance, so
// the first read after restart doesn't fall back to a stale (pre-sync) balance
// and present it as confirmed-spendable. Warm reads (already synced) stay fast.
const READ_SYNC_TIMEOUT_MS = 45_000
// #5195 follow-up: resolving an EXTERNAL Lightning Address is two sequential
// HTTPS round-trips to an arbitrary third-party LNURL server, which can be much
// slower than the short read timeout. Reusing the 15s read timeout made external
// sends to slow hosts blow the wrap and surface as a generic "timed out" — which
// the outer catch then classified as INDETERMINATE (send-pending) even though
// NOTHING was sent. Give LNURL-pay resolution its OWN generous timeout, and bound
// each individual fetch (below) so one hung host fails with a CLEAR reason.
const LNURL_RESOLVE_TIMEOUT_MS = 30_000
const LNURL_FETCH_TIMEOUT_MS = 12_000

// #5254 — PRE-SEND FEE GUARD.
//
// A 44-sat external Lightning-Address send paid a REAL ~4,096-sat LSP routing
// fee (#5250 confirmed the fee is real, on `prepareResponse.paymentMethod`, not
// change). #5250 reconciles that fee onto the receipt AFTER the fact; #5254
// REFUSES an insane send BEFORE it dispatches so zero sats move when the
// computed cost is grossly disproportionate to the amount.
//
// The bound is `preparedFee > max(FEE_GUARD_FLOOR_SATS, FEE_GUARD_MAX_PCT *
// amountSats)`. The floor lets tiny absolute fees through unconditionally
// (e.g. a normal 3-sat Spark transfer fee on a 1-sat send is fine); above the
// floor the fee may not exceed FEE_GUARD_MAX_PCT of the amount.
//
// Defaults are chosen to PASS normal sends and REJECT the 44/4096 case
// decisively:
//   - FEE_GUARD_FLOOR_SATS = 50: any send whose prepared fee is <= 50 sats
//     always passes, regardless of amount. Real native Spark fees and small
//     Lightning fees sit well under this.
//   - FEE_GUARD_MAX_PCT = 0.5 (50%): above the floor, the fee may be at most
//     half the amount. A healthy external Lightning send pays a routing fee
//     that is a small fraction of the amount; paying >= 50% in fees is a
//     red-flag tiny-amount / expensive-route case.
//   Worked examples:
//     amount 44,    fee 4096 -> ceiling max(50, 22)=50    -> 4096 > 50    -> REJECT.
//     amount 1000,  fee 16   -> ceiling max(50, 500)=500  -> 16 <= 500    -> PASS.
//     amount 100000, fee 200 -> ceiling max(50, 50000)    -> 200 <= 50000 -> PASS.
//     amount X,     native fee 0 -> 0 <= any ceiling      -> PASS (always).
// An operator can RAISE the ceiling explicitly (never silently) via the
// `PYLON_SPARK_MAX_FEE_SATS` env var and/or a `--max-fee <sats>` input threaded
// through to `sendSparkPaymentFromSdk` (see `resolveFeeCeilingSats`).
export const FEE_GUARD_FLOOR_SATS = 50
export const FEE_GUARD_MAX_PCT = 0.5

/**
 * Resolve the effective allowed-fee ceiling for a send (#5254).
 *
 * The default bound is `max(FEE_GUARD_FLOOR_SATS, FEE_GUARD_MAX_PCT *
 * amountSats)`. An explicit operator override RAISES the ceiling: it is the
 * MAX of the default bound and any provided override (a per-call `maxFeeSats`
 * input and/or the `PYLON_SPARK_MAX_FEE_SATS` env var). Overrides can only
 * loosen the guard, never tighten it below the default, and a malformed
 * override is ignored (falls back to the default bound).
 */
export function resolveFeeCeilingSats(
  amountSats: number,
  override?: { maxFeeSats?: number; env?: NodeJS.ProcessEnv },
): number {
  const defaultBound = Math.max(FEE_GUARD_FLOOR_SATS, Math.floor(FEE_GUARD_MAX_PCT * amountSats))
  const candidates = [defaultBound]
  const inputOverride = override?.maxFeeSats
  if (typeof inputOverride === "number" && Number.isFinite(inputOverride) && inputOverride >= 0) {
    candidates.push(Math.floor(inputOverride))
  }
  const envRaw = (override?.env ?? process.env).PYLON_SPARK_MAX_FEE_SATS
  if (typeof envRaw === "string" && envRaw.trim() !== "") {
    const envOverride = Number(envRaw)
    if (Number.isFinite(envOverride) && envOverride >= 0) {
      candidates.push(Math.floor(envOverride))
    }
  }
  return Math.max(...candidates)
}

// #5257 — PER-DESTINATION-DOMAIN FEE POLICY (the "blacklist").
//
// The ~4,096-sat fee on a 44-sat send (#5250) was the SDK's TOTAL prepared
// Lightning fee with NO per-hop/per-LSP breakdown, so we cannot attribute it to
// an intermediate routing node. The ONE thing we CAN attribute is the
// destination Lightning-Address DOMAIN (the LNURL-pay endpoint), which the
// sending node resolves anyway. #5257 attributes the prepared fee to that bare
// domain (public-safe — e.g. `bitnob.io`; the full `name@domain` is payment
// material and stays redacted) and lets an operator deny or fee-cap a domain.
//
// This is keyed by DOMAIN and composes with — does not replace — the #5254
// amount-relative magnitude guard. Both run PRE-dispatch (before sendPayment),
// so a refusal moves zero sats. Knobs (all env, read like PYLON_SPARK_MAX_FEE_SATS):
//   - PYLON_SPARK_DENY_DOMAINS    comma-separated deny list (e.g. "bitnob.io,foo.bar").
//   - PYLON_SPARK_ALLOW_DOMAINS   comma-separated allowlist OVERRIDE. A domain
//                                 present here is forced through regardless of the
//                                 deny list or per-domain fee bound (operator
//                                 knowingly-accepts; never silent — it is explicit).
//   - PYLON_SPARK_DOMAIN_FEE_MAX_PCT  optional per-domain ceiling as a percentage
//                                 of the amount (e.g. "10" = fee may be at most 10%
//                                 of amountSats for ANY LA domain).
//   - PYLON_SPARK_DOMAIN_FEE_MAX_SATS optional small absolute floor below which a
//                                 fee always passes the per-domain bound (defaults
//                                 to FEE_GUARD_FLOOR_SATS). Above it, the pct bound
//                                 applies.
// The existing per-call `maxFeeSats` / PYLON_SPARK_MAX_FEE_SATS operator override
// (the #5254 path) ALSO forces a flagged domain through: an explicit raised
// ceiling is a deliberate operator acceptance, so we honor it here too.

// Extract the bare domain (part after `@`, lowercased) from a Lightning Address.
// Returns null when the input is not a syntactic lud16 `name@domain`. The domain
// is PUBLIC-SAFE attribution material; callers must NOT surface the full address.
export function domainFromLightningAddress(value: string): string | null {
  const trimmed = value.trim().toLowerCase()
  if (!isLightningAddress(trimmed)) return null
  const at = trimmed.indexOf("@")
  if (at <= 0 || at === trimmed.length - 1) return null
  return trimmed.slice(at + 1)
}

function parseDomainList(raw: string | undefined): Set<string> {
  const set = new Set<string>()
  if (typeof raw !== "string") return set
  for (const part of raw.split(",")) {
    const domain = part.trim().toLowerCase()
    if (domain !== "") set.add(domain)
  }
  return set
}

function parseNonNegativeNumber(raw: string | undefined): number | null {
  if (typeof raw !== "string" || raw.trim() === "") return null
  const value = Number(raw)
  return Number.isFinite(value) && value >= 0 ? value : null
}

export type DomainFeePolicyDecision =
  | { refuse: false }
  // `reason` is a fixed enum (no payment material); `domain` is the bare,
  // public-safe destination domain that triggered the refusal.
  | { refuse: true; domain: string; reason: "deny_list" | "fee_bound" }

/**
 * Evaluate the #5257 per-destination-domain policy for a Lightning-Address send.
 *
 * PRE-dispatch and DOMAIN-keyed (complements the #5254 amount-relative guard):
 *   - An allowlist (PYLON_SPARK_ALLOW_DOMAINS) OR an explicit operator
 *     fee-ceiling override (`maxFeeSats` / PYLON_SPARK_MAX_FEE_SATS) forces the
 *     send through (deliberate operator acceptance; never silent).
 *   - Otherwise a denied domain refuses with `deny_list`.
 *   - Otherwise a domain whose prepared fee exceeds the per-domain bound
 *     (PYLON_SPARK_DOMAIN_FEE_MAX_PCT, above a small absolute floor) refuses
 *     with `fee_bound`.
 *   - Native Spark and non-LA destinations never reach here (domain === null).
 */
export function evaluateDomainFeePolicy(input: {
  domain: string | null
  amountSats: number
  preparedFeeSats: number | null
  // An explicit operator fee-ceiling override (#5254 path) is a deliberate
  // acceptance, so it also overrides the domain policy.
  hasExplicitFeeOverride?: boolean
  env?: NodeJS.ProcessEnv
}): DomainFeePolicyDecision {
  const { domain } = input
  // Only Lightning-Address (domain-bearing) sends are subject to the policy.
  if (domain === null || domain === "") return { refuse: false }
  const env = input.env ?? process.env

  // Operator allowlist OR an explicit raised fee ceiling = knowingly-accepted.
  const allow = parseDomainList(env.PYLON_SPARK_ALLOW_DOMAINS)
  if (allow.has(domain)) return { refuse: false }
  if (input.hasExplicitFeeOverride === true) return { refuse: false }

  // Deny list.
  const deny = parseDomainList(env.PYLON_SPARK_DENY_DOMAINS)
  if (deny.has(domain)) return { refuse: true, domain, reason: "deny_list" }

  // Per-domain fee bound: above a small absolute floor, the prepared fee may not
  // exceed PYLON_SPARK_DOMAIN_FEE_MAX_PCT of the amount. Absent the pct knob,
  // there is no per-domain fee bound (the #5254 magnitude guard still applies).
  const maxPct = parseNonNegativeNumber(env.PYLON_SPARK_DOMAIN_FEE_MAX_PCT)
  if (maxPct !== null && input.preparedFeeSats !== null && input.preparedFeeSats > 0) {
    const floor = parseNonNegativeNumber(env.PYLON_SPARK_DOMAIN_FEE_MAX_SATS) ?? FEE_GUARD_FLOOR_SATS
    const bound = Math.max(floor, Math.floor((maxPct / 100) * input.amountSats))
    if (input.preparedFeeSats > bound) return { refuse: true, domain, reason: "fee_bound" }
  }

  return { refuse: false }
}

// Send-latency audit: opt-in per-step timing (PYLON_SPARK_DEBUG=1) so the cold
// per-command send pipeline cost is MEASURABLE on real infra. Monotonic clock,
// no payment material — just labelled millisecond deltas on stderr.
const sparkTiming = (label: string, ms: number) => {
  if (process.env.PYLON_SPARK_DEBUG === "1") {
    console.error(`[spark-timing] ${label}=${Math.round(ms)}ms`)
  }
}

// #5194: strip absolute filesystem paths (which can carry a username) from a
// debug string while keeping URLs/hosts/error types/messages visible. Same
// sanitizer the rc.27 `[spark-helper:build_error]` line used; shared so every
// new diagnostic line redacts identically.
export function sanitizeSparkDebug(value: string): string {
  return value.replace(/\/(?:Users|home|private|var|opt|tmp|Library)\/[^\s'"):]+/g, "<path>")
}

// #5194: emit a pre-build step marker so the LAST marker that fires on a
// failing host pinpoints exactly where the in-process SDK build path bails
// BEFORE `SdkBuilder.build()` (the point the rc.27 build_error line covered).
// stderr-only, PYLON_SPARK_DEBUG-gated, never any seed/secret.
const sparkPreBuild = (step: string, detail?: string) => {
  if (process.env.PYLON_SPARK_DEBUG === "1") {
    console.error(`[spark-helper:pre-build] ${step}${detail ? `: ${sanitizeSparkDebug(detail)}` : ""}`)
  }
}

function helperError(command: SparkBackupCommand, message: string): WalletCommandResult {
  // The message is helper stderr; slice-1 code keeps it out of public
  // projections. We still avoid echoing any secret-shaped material here.
  return { exitCode: 1, stdout: "", stderr: `spark backup helper ${command}: ${message}` }
}

// #5194: a fixed, public-safe enum that classifies WHY the Spark helper failed.
// The raw helper stderr is private (it can contain a storage path or an SDK
// internal message), so it is NEVER surfaced in a projection. This enum is the
// ONLY failure detail any node — a one-shot CLI or a daemon-routed read — gets
// to see, and it is a bounded set of constant strings with no payment material,
// no paths, and no secrets.
//
// The root cause of #5194 was invisible: a helper read that degraded to
// `helper-unavailable` carried NO reason at all, so a node operator could not
// tell a corrupt local wallet DB from a blocked network or a missing dependency.
// Worse, when the read is served by the long-lived daemon's warm session, the
// daemon process owns the SDK and its `[spark-helper:*]` / `[spark-getinfo]`
// debug lines (gated on PYLON_SPARK_DEBUG=1 in the DAEMON's env, not the CLI's)
// never reach the CLI terminal — so the operator sees `helperReady:false` with
// no explanation. This enum closes that gap.
export type SparkHelperUnavailableReason =
  // The wallet storage DB failed to initialize/migrate (e.g. a corrupt or
  // incompatible `storage.sql` in a long-lived home). UPSTREAM of getInfo.
  | "db_init_failed"
  // An SDK call exceeded its timeout (a stuck build/connect/sync, often a
  // blocked or very slow network egress to the Spark sync/LSP servers).
  | "timeout"
  // The Spark/Breez network could not be reached during build/connect/sync.
  | "network_unreachable"
  // The optional Breez SDK Spark package / WASM did not load in this runtime.
  | "module_load_failed"
  // The helper ran but produced no usable target/data (non-error empty result).
  | "no_result"
  // Any other helper failure whose shape we do not specifically recognize.
  | "unknown"

/**
 * #5194: classify a (private) Spark helper stderr message into a bounded,
 * public-safe failure reason. Pattern-matching here is deliberately deterministic
 * and confined to a CLOSED enum output — it never echoes the raw message — so it
 * is safe to place in a public projection. This is NOT user-intent routing; it
 * is fixed diagnostic classification of our own SDK's error strings.
 */
export function classifySparkHelperFailureReason(stderr: string): SparkHelperUnavailableReason {
  const message = (stderr ?? "").toLowerCase()
  if (message === "") return "unknown"
  // Storage init/migration failure — the #5194 corrupt-DB case (upstream of getInfo).
  if (
    /initialize database|migration failed|file is not a database|database disk image is malformed|storageerror/.test(
      message,
    )
  ) {
    return "db_init_failed"
  }
  // The module/WASM did not load (selftest-style failure or a missing optional dep).
  if (/missing defaultconfig|missing sdkbuilder|cannot find module|sdk load|wasm/.test(message)) {
    return "module_load_failed"
  }
  // Timeout from withTimeout(...) ("<label> timed out").
  if (/timed out/.test(message)) return "timeout"
  // Network/connect failure to the Spark/Breez sync or LSP servers.
  if (
    /unable to connect|connection refused|dns|getaddrinfo|econnrefused|enotfound|etimedout|certificate|unreachable|fetch failed|\bnetwork\b/.test(
      message,
    )
  ) {
    return "network_unreachable"
  }
  return "unknown"
}

function helperOk(payload: Record<string, unknown>): WalletCommandResult {
  return { exitCode: 0, stdout: JSON.stringify(payload), stderr: "" }
}

function publicRef(prefix: string, value: string) {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`
}

function paymentValue(payload: unknown) {
  const value = payload as {
    payment?: { id?: unknown; amount?: unknown; fees?: unknown; status?: unknown }
    id?: unknown
    amount?: unknown
    fees?: unknown
    status?: unknown
  }
  return value.payment ?? value
}

function paymentRef(prefix: string, payment: { id?: unknown; status?: unknown }, idempotencyKey: string) {
  return publicRef(
    prefix,
    [
      typeof payment.id === "string" ? payment.id : "id-redacted",
      typeof payment.status === "string" ? payment.status : "status-redacted",
      idempotencyKey,
    ].join(":"),
  )
}

function publicStatus(value: unknown) {
  return typeof value === "string" && value.trim() !== "" ? value.trim() : null
}

// The Breez Spark SDK uses the idempotencyKey as the on-wire TransferId, which
// MUST be a valid UUID — a plain string key is rejected with "Invalid TransferId
// format" (the original #5185 cause). Derive a stable UUID from the caller's
// idempotency key, exactly as the treasury Spark sender does.
function uuidFromStableSeed(seed: string): string {
  const hex = createHash("sha256").update(seed).digest("hex")
  const variant = ((Number.parseInt(hex.slice(16, 17), 16) & 0x3) | 0x8).toString(16)
  return [
    hex.slice(0, 8),
    hex.slice(8, 12),
    `5${hex.slice(13, 16)}`,
    `${variant}${hex.slice(17, 20)}`,
    hex.slice(20, 32),
  ].join("-")
}

// lud16 Lightning Address: name@domain.tld.
const lightningAddressPattern =
  /^[a-z0-9._+-]{1,64}@[a-z0-9.-]{1,190}\.[a-z]{2,63}$/u
const isLightningAddress = (value: string): boolean =>
  lightningAddressPattern.test(value.trim().toLowerCase())

// #5225: a native Spark address. The Spark address uses a bech32m-style encoding
// with the human-readable prefix `spark1` and a long lowercase bech32 data part
// (observed ~68 chars on real infra). We mirror the conservative style of
// `lightningAddressPattern`: a strict `spark1` HRP, the bech32 charset (the data
// part excludes `1`, `b`, `i`, `o`; `1` is the HRP separator), and a bounded
// length. A native Spark→Spark send settles WITHOUT a Lightning routing fee, so
// classifying it up front lets us route it natively (NO bolt11Invoice/preferSpark
// options, NO LNURL resolve, NO Lightning fallback) and label it `spark_native`
// instead of mislabeling it `payment_request`.
const sparkAddressPattern = /^spark1[023456789acdefghjklmnpqrstuvwxyz]{20,180}$/u
export const isSparkAddress = (value: string): boolean =>
  sparkAddressPattern.test(value.trim().toLowerCase())

const asRecord = (value: unknown): Record<string, unknown> | null =>
  typeof value === "object" && value !== null ? (value as Record<string, unknown>) : null

// AbortSignal.timeout(...) rejects fetch with a DOMException whose name is
// "TimeoutError" (some runtimes use "AbortError"). Treat either as a per-fetch
// timeout so we can report a distinct, non-indeterminate lnurl_resolve reason.
const isAbortError = (error: unknown): boolean => {
  const name = (error as { name?: unknown } | null)?.name
  return name === "TimeoutError" || name === "AbortError"
}

// Resolve a Lightning Address to a BOLT11 via the standard LNURL-pay flow
// (GET /.well-known/lnurlp/<name> -> callback?amount=<msat>). Result-typed; the
// returned bolt11 is payment material. #5195: we pay the resolved BOLT11 through
// the proven sendPayment path because the SDK's lnurlPay throws "Tree service
// error: insufficient funds" from the Spark leaf structure even when funded.
async function resolveLightningAddressInvoice(
  address: string,
  amountSats: number,
): Promise<{ ok: true; bolt11: string } | { ok: false; reason: string }> {
  const trimmed = address.trim()
  const at = trimmed.indexOf("@")
  if (at <= 0 || at === trimmed.length - 1) return { ok: false, reason: "not_a_lightning_address" }
  const name = trimmed.slice(0, at)
  const domain = trimmed.slice(at + 1)
  const metaUrl = `https://${domain}/.well-known/lnurlp/${encodeURIComponent(name)}`
  let meta: Record<string, unknown> | null
  try {
    const response = await fetch(metaUrl, {
      headers: { accept: "application/json" },
      signal: AbortSignal.timeout(LNURL_FETCH_TIMEOUT_MS),
    })
    if (!response.ok) return { ok: false, reason: `lnurlp_meta_http_${response.status}` }
    meta = asRecord(await response.json())
  } catch (error) {
    // A per-fetch abort is a CLEAR, distinct failure (a slow external host),
    // not the generic "timed out" the outer catch maps to INDETERMINATE.
    return {
      ok: false,
      reason: isAbortError(error) ? "lnurlp_meta_timeout" : "lnurlp_meta_fetch_failed",
    }
  }
  if (meta === null || meta.tag !== "payRequest") return { ok: false, reason: "lnurlp_meta_not_pay_request" }
  const callback = typeof meta.callback === "string" ? meta.callback : ""
  if (callback === "" || !/^https:\/\//u.test(callback)) return { ok: false, reason: "lnurlp_meta_callback_invalid" }
  const amountMsat = amountSats * 1000
  const minSendable = Number(meta.minSendable ?? 0)
  const maxSendable = Number(meta.maxSendable ?? Number.MAX_SAFE_INTEGER)
  if (
    Number.isFinite(minSendable) &&
    Number.isFinite(maxSendable) &&
    (amountMsat < minSendable || amountMsat > maxSendable)
  ) {
    return { ok: false, reason: `amount_out_of_range_${minSendable}_${maxSendable}_msat` }
  }
  let callbackUrl: URL
  try {
    callbackUrl = new URL(callback)
  } catch {
    return { ok: false, reason: "lnurlp_callback_unparseable" }
  }
  callbackUrl.searchParams.set("amount", String(amountMsat))
  let invoice: Record<string, unknown> | null
  try {
    const response = await fetch(callbackUrl.toString(), {
      headers: { accept: "application/json" },
      signal: AbortSignal.timeout(LNURL_FETCH_TIMEOUT_MS),
    })
    if (!response.ok) return { ok: false, reason: `lnurlp_callback_http_${response.status}` }
    invoice = asRecord(await response.json())
  } catch (error) {
    return {
      ok: false,
      reason: isAbortError(error) ? "lnurlp_callback_timeout" : "lnurlp_callback_fetch_failed",
    }
  }
  if (invoice === null) return { ok: false, reason: "lnurlp_callback_not_json" }
  if (invoice.status === "ERROR") return { ok: false, reason: "lnurlp_callback_error" }
  const pr = typeof invoice.pr === "string" ? invoice.pr.trim() : ""
  if (pr === "" || !/^ln[a-z0-9]/iu.test(pr)) return { ok: false, reason: "lnurlp_callback_no_invoice" }
  return { ok: true, bolt11: pr }
}

async function sendSparkPaymentFromSdk(input: {
  amountSats: number
  destination: string
  idempotencyKey: string
  prefix: string
  sdk: BreezSparkSdk
  timeoutMs: number
  allowLnurlPay: boolean
  // #5254: explicit operator override that RAISES the allowed-fee ceiling so a
  // knowingly-expensive send can proceed. Undefined keeps the default bound
  // (PYLON_SPARK_MAX_FEE_SATS is also consulted inside resolveFeeCeilingSats).
  maxFeeSats?: number
}): Promise<SparkBackupSendTransferResult> {
  let destination = input.destination.trim()
  if (destination === "") {
    return { ok: false, failureRef: publicRef(`${input.prefix}_failure`, "empty destination") }
  }
  if (!Number.isInteger(input.amountSats) || input.amountSats <= 0) {
    return { ok: false, failureRef: publicRef(`${input.prefix}_failure`, "invalid amount") }
  }
  // Valid-UUID TransferId for every SDK send call (see uuidFromStableSeed).
  const sdkIdempotencyKey = uuidFromStableSeed(input.idempotencyKey)

  // #5225: a native Spark address routes Spark→Spark with NO Lightning routing
  // (verified on real infra: feeSats 0, settles in seconds). Detect it up front
  // so we (a) NEVER enter the Lightning-Address LNURL-resolve branch for it and
  // (b) NEVER attempt the BOLT11/preferSpark:false Lightning fallback for it.
  // The native send is just prepare → sendPayment with `options: undefined`
  // (already the non-bolt11 path below); this flag makes the intent explicit and
  // drives the `spark_native` method label. A `spark1…` HRP cannot also match the
  // lud16 `name@domain` Lightning-Address shape, but guarding explicitly keeps the
  // money-path classification unambiguous and future-proof.
  const isSparkNative = isSparkAddress(destination)

  // #5195: pay a Lightning Address by resolving it to a BOLT11 (LNURL-pay) and
  // sending that through the proven sendPayment path below — NOT via the SDK's
  // lnurlPay, which throws "Tree service error: insufficient funds" from the
  // Spark leaf structure even when the balance covers the amount. This mirrors
  // how the treasury Spark sender pays Lightning Addresses.
  let method: "payment_request" | "lnurl_pay" | "spark_native" =
    isSparkNative ? "spark_native" : "payment_request"

  // #5257: attribute the send to the destination Lightning-Address DOMAIN
  // (public-safe; the full `name@domain` stays redacted) and apply the operator
  // per-domain policy. Captured from the ORIGINAL destination before LNURL
  // resolution rewrites it to a BOLT11. Null for non-LA sends (bolt11/bolt12,
  // native Spark) — those are never subject to the domain policy.
  const destinationDomain =
    !isSparkNative && input.allowLnurlPay ? domainFromLightningAddress(destination) : null
  // An explicit operator fee-ceiling override (#5254 path) is a deliberate
  // acceptance that also overrides the domain policy.
  const hasExplicitFeeOverride =
    (typeof input.maxFeeSats === "number" && Number.isFinite(input.maxFeeSats)) ||
    (typeof process.env.PYLON_SPARK_MAX_FEE_SATS === "string" &&
      process.env.PYLON_SPARK_MAX_FEE_SATS.trim() !== "")

  // #5257: enforce the DENY-LIST pre-resolve — a denied domain must not even
  // trigger an LNURL round-trip. The per-domain FEE BOUND needs the prepared fee,
  // so it is enforced after prepare (still pre-dispatch) below. The failureRef
  // carries the bare domain (public-safe) so `sendWithSparkBackup` surfaces the
  // `destination_fee_policy` blocker; zero sats move.
  if (destinationDomain !== null) {
    const denyDecision = evaluateDomainFeePolicy({
      domain: destinationDomain,
      amountSats: input.amountSats,
      preparedFeeSats: null,
      hasExplicitFeeOverride,
    })
    if (denyDecision.refuse && denyDecision.reason === "deny_list") {
      if (process.env.PYLON_SPARK_DEBUG === "1") {
        console.error(`[spark-send] fee_domain:${destinationDomain}:denied reason=deny_list`)
      }
      return {
        ok: false,
        failureRef: `${input.prefix}.destination_fee_policy:${destinationDomain}`,
      }
    }
  }

  if (!isSparkNative && input.allowLnurlPay && isLightningAddress(destination)) {
    // #5195 follow-up: resolve LNURL-pay under its OWN generous timeout, NOT the
    // short read timeout — an external LNURL server is slower than a local SDK
    // read, and each inner fetch is independently abort-bounded so a hung host
    // yields a CLEAR lnurl_resolve failure (not a scary send-pending).
    const resolved = await withTimeout(
      resolveLightningAddressInvoice(destination, input.amountSats),
      LNURL_RESOLVE_TIMEOUT_MS,
      "lnurl-pay resolve",
    )
    if (!resolved.ok) {
      // Surface the resolve reason under PYLON_SPARK_DEBUG. This is a RETURNED
      // failure (not a thrown error), so the [spark-send] catch log never sees it;
      // without this an operator only gets a hashed failureRef and can't tell a
      // dead recipient endpoint (e.g. lnurlp_meta_timeout — bitnob unreachable)
      // from a real bug. The reason is a fixed enum string, no payment material.
      if (process.env.PYLON_SPARK_DEBUG === "1") {
        console.error(`[spark-send] lnurl_resolve:${resolved.reason}`)
      }
      return { ok: false, failureRef: publicRef(`${input.prefix}_failure`, `lnurl_resolve:${resolved.reason}`) }
    }
    destination = resolved.bolt11
    method = "lnurl_pay"
  }

  if (typeof input.sdk.prepareSendPayment !== "function" || typeof input.sdk.sendPayment !== "function") {
    return { ok: false, failureRef: `${input.prefix}.send_unsupported` }
  }

  // SDK methods stay bound to the instance: they are wasm-bindgen methods that
  // read `this.__wbg_ptr`, so a detached reference throws "undefined is not an
  // object". Always call via `input.sdk.<method>(...)`.
  //
  // #5185: prepare WITH the amount (matches the treasury Spark sender — the SDK
  // accepts it for amount-encoded and amountless invoices alike). Send preferring
  // the native Spark rail; on the SDK's "Invalid TransferId format" validation
  // failure, fall back to settling the BOLT11 over Lightning (preferSpark:false)
  // under a FRESH idempotency UUID — the exact fix the treasury Spark sender uses.
  const tPrepare = performance.now()
  const prepareResponse = await withTimeout(
    input.sdk.prepareSendPayment({
      paymentRequest: destination,
      amount: BigInt(input.amountSats),
    }),
    input.timeoutMs,
    "spark prepareSendPayment",
  )
  sparkTiming("prepare_send_payment", performance.now() - tPrepare)
  const paymentMethod = (
    prepareResponse as {
      paymentMethod?: {
        type?: string
        // #5250: the REAL computed cost of the send is surfaced HERE on the
        // prepared payment method, not (reliably) on the send-result `fees`.
        // Per the Breez Spark SDK `SendPaymentMethod` union (d.ts line ~1435):
        //   bolt11Invoice → { lightningFeeSats: number; sparkTransferFeeSats?: number }
        //   sparkAddress  → { fee: string }
        //   sparkInvoice  → { fee: string }
        // These flow through `toSatNumber` (number | bigint | decimal string).
        fee?: unknown
        lightningFeeSats?: unknown
        sparkTransferFeeSats?: unknown
      }
    }
  )?.paymentMethod
  const isBolt11 = paymentMethod?.type === "bolt11Invoice"
  // #5225: confirm the native label off the SDK's resolved payment method, not
  // only the destination string. A non-bolt11 resolved method that we did NOT
  // already classify as an LNURL-pay (Lightning Address → BOLT11) is a native
  // Spark send, so report it as `spark_native`. A bolt11 method keeps the
  // existing `payment_request`/`lnurl_pay` label; we never downgrade an
  // already-resolved `lnurl_pay`.
  if (!isBolt11 && method !== "lnurl_pay") {
    method = "spark_native"
  }

  // #5254 + #5250: extract the REAL prepared cost of this send from the prepared
  // payment method (the SAME fields #5250 reconciles onto the receipt). This is
  // computed HERE, BEFORE dispatch, so the fee guard below can refuse an insane
  // send without moving any sats. The settled-fee reconciliation further down
  // reuses `preparedFee` rather than recomputing it.
  const preparedFeeFromMethod = toSatNumber(paymentMethod?.fee)
  const preparedLightningFeeSats = toSatNumber(paymentMethod?.lightningFeeSats)
  const preparedSparkTransferFeeSats = toSatNumber(paymentMethod?.sparkTransferFeeSats)
  // Sum the bolt11 components (lightning + optional spark-transfer); the spark
  // address/invoice case uses the flat `fee`. Any present component wins over a
  // missing one; null only when nothing is reported.
  const preparedFeeComponents = [
    preparedFeeFromMethod,
    preparedLightningFeeSats,
    preparedSparkTransferFeeSats,
  ].filter((value): value is number => value !== null)
  const preparedFee =
    preparedFeeComponents.length > 0
      ? preparedFeeComponents.reduce((sum, value) => sum + value, 0)
      : null

  // #5257 — DOMAIN ATTRIBUTION + PER-DOMAIN FEE BOUND (pre-dispatch).
  // For a Lightning-Address send, record the bare destination domain alongside
  // the prepared fee (public-safe telemetry under PYLON_SPARK_DEBUG so we build a
  // picture of which domains quote extortionate fees), then enforce the operator
  // per-domain fee bound BEFORE dispatch. The deny-list check already ran above;
  // here we only need the fee-bound (which requires the prepared fee). Composes
  // with the #5254 magnitude guard below — both pre-dispatch, both zero-movement.
  if (destinationDomain !== null) {
    if (process.env.PYLON_SPARK_DEBUG === "1") {
      console.error(
        `[spark-send] fee_domain:${destinationDomain}:${preparedFee === null ? "unknown" : preparedFee}`,
      )
    }
    const domainDecision = evaluateDomainFeePolicy({
      domain: destinationDomain,
      amountSats: input.amountSats,
      preparedFeeSats: preparedFee,
      hasExplicitFeeOverride,
    })
    if (domainDecision.refuse) {
      if (process.env.PYLON_SPARK_DEBUG === "1") {
        console.error(
          `[spark-send] fee_domain:${destinationDomain}:denied reason=${domainDecision.reason}`,
        )
      }
      return {
        ok: false,
        failureRef: `${input.prefix}.destination_fee_policy:${destinationDomain}`,
      }
    }
  }

  // #5254 — PRE-SEND FEE GUARD. Refuse a send whose prepared fee is grossly
  // disproportionate to the amount BEFORE calling sendPayment, so zero sats
  // move. A native Spark send (fee 0) and any send with a tiny absolute fee
  // pass trivially. The ceiling can be RAISED (never silently) by the operator
  // via PYLON_SPARK_MAX_FEE_SATS and/or the per-call `maxFeeSats` input. The
  // failureRef is operator-legible and PUBLIC-SAFE — integers only, no payment
  // material — so `sendWithSparkBackup` can surface a clear fee-too-high blocker.
  if (preparedFee !== null && preparedFee > 0) {
    const ceiling = resolveFeeCeilingSats(input.amountSats, { maxFeeSats: input.maxFeeSats })
    if (preparedFee > ceiling) {
      if (process.env.PYLON_SPARK_DEBUG === "1") {
        console.error(
          `[spark-send] fee_too_high prepared=${preparedFee} amount=${input.amountSats} ceiling=${ceiling}`,
        )
      }
      return {
        ok: false,
        failureRef: `${input.prefix}.fee_too_high:prepared=${preparedFee}:amount=${input.amountSats}`,
      }
    }
  }

  // #5196: wait at least the SDK's completion window + a buffer for the send to
  // settle — never the short read-timeout — so a slow/large send is not aborted
  // while it completes server-side (false-negative). prepare/resolve above stay
  // on the short timeout (they are fast).
  const sendTimeoutMs = Math.max(
    input.timeoutMs,
    SEND_COMPLETION_TIMEOUT_SECS * 1000 + SEND_TIMEOUT_BUFFER_MS,
  )
  const sendPrepared = (idempotency: string, preferSpark: boolean) => {
    // Re-establish the narrowing inside the closure (TS drops the outer
    // typeof-guard narrowing for the optional method here). The method must be
    // invoked as `input.sdk.sendPayment(...)` so wasm-bindgen keeps `this`.
    if (typeof input.sdk.sendPayment !== "function") {
      throw new Error("spark sdk sendPayment is unavailable")
    }
    return withTimeout(
      input.sdk.sendPayment({
        prepareResponse,
        options: isBolt11
          ? { type: "bolt11Invoice", preferSpark, completionTimeoutSecs: SEND_COMPLETION_TIMEOUT_SECS }
          : undefined,
        idempotencyKey: idempotency,
      }),
      sendTimeoutMs,
      "spark sendPayment",
    )
  }
  const tSettle = performance.now()
  const sent = isBolt11
    ? await sendPrepared(sdkIdempotencyKey, true).catch(error => {
        const message = (
          error instanceof Error ? error.message : String(error)
        ).toLowerCase()
        if (message.includes("invalid transferid format")) {
          return sendPrepared(
            uuidFromStableSeed(`${input.idempotencyKey}:bolt11-lightning-fallback`),
            false,
          )
        }
        throw error
      })
    : await sendPrepared(sdkIdempotencyKey, true)
  sparkTiming("send_payment_settle", performance.now() - tSettle)
  const payment = paymentValue(sent)
  const amount = toSatNumber(payment.amount) ?? input.amountSats
  // #5250 — RECONCILE THE FEE.
  //
  // BUG (rc.22): a 44-sat Lightning-Address send reported feeSats:0 while the
  // wallet balance dropped 4,140 sats (= 44 + 4096). The 4,096 was the REAL
  // Lightning/LSP routing fee for the tiny external send, computed by the SDK
  // at PREPARE time and surfaced on `prepareResponse.paymentMethod`
  // (`lightningFeeSats`/`sparkTransferFeeSats` for bolt11Invoice; `fee` for a
  // spark address/invoice). The send-RESULT `payment.fees` came back 0/absent,
  // so reading only that hid the spent sats and produced a non-reconciling
  // receipt (amountSats + feeSats != balance delta).
  //
  // Fix: when the send-result fee is present AND non-zero, trust it (it is the
  // authoritative settled fee). Otherwise fall back to the prepared fee — the
  // same fields the treasury Spark sender extracts (`spark-treasury.mjs`:
  // preparedFeeSats / preparedLightningFeeSats / preparedSparkTransferFeeSats).
  // For a bolt11 method the total prepared cost is lightning + spark-transfer;
  // for a spark address/invoice it is the single `fee`. There is NO separate
  // "change"/leaf field in the SDK result — balance delta = amount + fees — so
  // a correctly-reported fee fully reconciles the delta with no residual.
  const sendResultFee = toSatNumber(payment.fees)
  // #5254 hoisted `preparedFee` (and its components) above the dispatch so the
  // pre-send fee guard could use it; reuse it here for the settled-fee fallback.
  // Trust a real, non-zero settled fee; otherwise use the prepared fee so the
  // receipt no longer claims feeSats:0 when sats were actually spent.
  const fee =
    sendResultFee !== null && sendResultFee > 0 ? sendResultFee : preparedFee ?? sendResultFee
  // True when the reported fee came from the prepared method rather than the
  // settled send result — public-safe provenance for the money-path receipt.
  const feeFromPrepared =
    (sendResultFee === null || sendResultFee === 0) && preparedFee !== null && preparedFee > 0
  return {
    ok: true,
    transferRef: publicRef(
      input.prefix,
      [
        typeof payment.id === "string" ? payment.id : "id-redacted",
        publicStatus(payment.status) ?? "status-redacted",
        method,
        String(amount),
        fee === null ? "fee-unknown" : String(fee),
        input.idempotencyKey,
      ].join(":"),
    ),
    sparkPaymentRef: paymentRef(`${input.prefix}_payment`, payment, input.idempotencyKey),
    amountSats: amount,
    feeSats: fee,
    feeFromPrepared,
    // #5257: public-safe destination-domain attribution (null for non-LA sends).
    destinationDomain,
    method,
    status: publicStatus(payment.status),
  }
}

async function withTimeout<T>(promise: Promise<T>, ms: number, label: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined
  try {
    return await Promise.race([
      promise,
      new Promise<never>((_, reject) => {
        timer = setTimeout(() => reject(new Error(`${label} timed out`)), ms)
      }),
    ])
  } finally {
    if (timer !== undefined) clearTimeout(timer)
  }
}

/**
 * Default loader for the real Breez SDK Spark package. Uses the nodejs subpath
 * (CJS/ESM) which auto-loads WASM. Kept lazy + optional: a missing package
 * rejects, and the adapter degrades to `helper-unavailable`.
 */
export async function loadBreezSparkModule(): Promise<BreezSparkModule> {
  // #5166: in a compiled standalone binary, extract the embedded WASM to disk and
  // point the SDK at it (via PYLON_SPARK_WASM_PATH) BEFORE importing the SDK,
  // whose nodejs entry loads the WASM eagerly at import time. No-op in source/npm.
  await ensureSparkWasmAvailable()
  // Dynamic import so packaging stays clean and the dependency is optional.
  const mod = (await import(/* @vite-ignore */ "@breeztech/breez-sdk-spark")) as unknown as BreezSparkModule
  if (typeof mod?.defaultConfig !== "function") {
    throw new Error("breez sdk spark module missing defaultConfig")
  }
  // We need at least one way to construct an SDK: the Bun-friendly SdkBuilder
  // (preferred) or the legacy connect() default.
  if (typeof mod?.SdkBuilder?.new !== "function" && typeof mod?.connect !== "function") {
    throw new Error("breez sdk spark module missing SdkBuilder.new/connect")
  }
  return mod
}

/**
 * Diagnostic: attempt to load the Breez Spark SDK module with NO network and NO
 * wallet seed, and report whether it actually loaded (#5166). The whole point is
 * to distinguish "the SDK module does not even load in this runtime" (e.g. a
 * compiled standalone binary that failed to bundle/instantiate the WASM) from
 * the other reasons the backup helper can be unavailable (missing seed, network
 * failure). Used by `pylon wallet spark-selftest` and the RC binary build guard.
 *
 * Never touches the network, never needs a seed, never returns secrets — only a
 * boolean and a redacted error message.
 */
export async function sparkModuleSelftest(
  loadModule: () => Promise<BreezSparkModule> = loadBreezSparkModule,
): Promise<{ moduleLoaded: boolean; reason: string | null }> {
  try {
    const mod = await loadModule()
    const ok =
      typeof mod?.defaultConfig === "function" &&
      (typeof mod?.SdkBuilder?.new === "function" || typeof mod?.connect === "function")
    return { moduleLoaded: ok, reason: ok ? null : "module loaded but missing defaultConfig/SdkBuilder" }
  } catch (error) {
    return { moduleLoaded: false, reason: error instanceof Error ? error.message : String(error) }
  }
}

/**
 * Build a short-lived SDK session. Prefers `SdkBuilder.new(...).withStorage(...)`
 * with a `bun:sqlite` backend so storage initializes under Pylon's Bun runtime
 * (#5080). Falls back to the legacy `connect()` default only when the module
 * does not expose `SdkBuilder` (e.g. injected test fakes).
 */
async function buildSparkSdk(
  mod: BreezSparkModule,
  sdkConfig: Record<string, unknown>,
  seed: { type: "mnemonic"; mnemonic: string; passphrase?: string },
  storageDir: string | undefined,
  timeoutMs: number,
): Promise<BreezSparkSdk> {
  if (typeof mod.SdkBuilder?.new === "function") {
    sparkPreBuild("build:sdkbuilder-available")
    // Match the SDK's default storage location: a `storage.sql` file inside the
    // configured storage directory. With no storageDir we use an in-process,
    // ephemeral DB (":memory:") so a session still works without on-disk state.
    let dbPath = ":memory:"
    if (storageDir) {
      // Match createDefaultStorage: ensure the data dir exists, db file is
      // `storage.sql` inside it. Private local wallet state only.
      mkdirSync(storageDir, { recursive: true })
      dbPath = nodePath.join(storageDir, "storage.sql")
    }
    sparkPreBuild("build:storage-dir-ready", `db=${dbPath === ":memory:" ? ":memory:" : "storage.sql"}`)
    const storage = new SparkBunStorage(dbPath)
    sparkPreBuild("build:storage-constructed")
    const builder = mod.SdkBuilder.new(sdkConfig, seed).withStorage(storage)
    sparkPreBuild("build:about-to-call-build")
    try {
      const built = await withTimeout(builder.build(), timeoutMs, "spark sdk build")
      sparkPreBuild("build:build-returned")
      return built
    } catch (buildError) {
      // #5194: when SdkBuilder.build() throws for a reason outside
      // classifySparkHelperFailureReason()'s buckets (helperUnavailableReason
      // "unknown"), surface the RAW exception name + message so a
      // deterministic-repro host can report the actual cause. Gated on
      // PYLON_SPARK_DEBUG and filesystem-path-sanitized (no $HOME/temp paths,
      // which could carry a username) — URLs/hosts/error types stay visible.
      if (process.env.PYLON_SPARK_DEBUG === "1") {
        const e = buildError as { name?: string; message?: string }
        const name = typeof e?.name === "string" && e.name ? e.name : "Error"
        const msg = sanitizeSparkDebug(typeof e?.message === "string" ? e.message : String(buildError)).slice(0, 400)
        console.error(`[spark-helper:build_error] ${name}: ${msg}`)
      }
      throw buildError
    }
  }
  if (typeof mod.connect === "function") {
    sparkPreBuild("build:connect-fallback")
    return await withTimeout(
      mod.connect({ config: sdkConfig, seed, storageDir }),
      timeoutMs,
      "spark sdk connect",
    )
  }
  sparkPreBuild("build:no-constructor")
  throw new Error("breez sdk spark module exposes neither SdkBuilder nor connect")
}

// ---------------------------------------------------------------------------
// #5207 warm, persistent Spark session.
//
// The audit (docs/2026-06-17-spark-send-latency-audit.md) measured ~3.5-4.4s of
// every `wallet send --rail spark` as pure per-command overhead: a cold process
// loads the WASM, builds a fresh SDK, runs a full `syncWallet`, sends, then
// disconnects — redone from scratch on every command. The fix is a warm SDK
// session kept alive for the life of the long-lived Pylon daemon, with sync
// moved to a background timer, so the send path skips build + sync entirely.
//
// SAFETY (money path):
// - The warm session is a process-level singleton keyed by
//   (mnemonic-derived id + network + storageDir). Concurrent get-or-build
//   callers dedupe to ONE in-flight build promise so we NEVER hold two SDKs for
//   the same wallet (which could race sends).
// - The SDK is assumed NOT safe for concurrent sends: every operation routed
//   through the warm session is SERIALIZED on a per-session promise chain.
// - The keying hashes the mnemonic (never stores it raw in the map key) and the
//   session record never exposes payment material.
// - When the warm flag is OFF (the default), none of this is touched: the cold
//   build → sync → send → disconnect path is byte-for-byte unchanged.
// ---------------------------------------------------------------------------

type WarmSparkSession = {
  // The built, connected SDK kept alive for the life of the process. `null`
  // while the first build is still in flight.
  sdk: BreezSparkSdk | null
  // In-flight build promise: concurrent acquirers await the SAME build so only
  // one SDK is ever constructed per (wallet+network+storageDir).
  building: Promise<BreezSparkSdk> | null
  // Serializes every op on this session (the SDK is treated as NOT concurrency
  // safe for sends). Each op chains onto this; a failing op does not poison the
  // chain (we swallow into a settled tail).
  opChain: Promise<unknown>
  // monotonic performance.now() of the last successful syncWallet, or null if
  // never synced. Used by the send path's "synced within last N seconds" guard.
  lastSyncMs: number | null
}

// Process-level registry. Keyed by a non-secret digest of network+storageDir+
// a digest of the mnemonic (the raw seed is never used as a map key).
const warmSparkSessions = new Map<string, WarmSparkSession>()

function warmSessionKey(config: {
  mnemonic: string
  network: "mainnet" | "regtest"
  storageDir?: string
}): string {
  const mnemonicDigest = createHash("sha256").update(config.mnemonic).digest("hex")
  return createHash("sha256")
    .update([config.network, config.storageDir ?? ":memory:", mnemonicDigest].join(" "))
    .digest("hex")
}

/**
 * Decide whether this adapter should use the warm singleton session. Explicit
 * `config.warmSession` wins; otherwise consult the PYLON_SPARK_WARM_SESSION env
 * flag. DEFAULTS OFF so the one-shot CLI keeps the cold, self-contained path.
 */
function resolveWarmSession(config: SparkBackupAdapterConfig, env: NodeJS.ProcessEnv = process.env): boolean {
  if (config.warmSession !== undefined) return config.warmSession
  return env.PYLON_SPARK_WARM_SESSION === "1" || env.PYLON_SPARK_WARM_SESSION === "true"
}

/**
 * Get-or-build the warm SDK session for this wallet. Concurrent callers await a
 * single in-flight build so exactly one SDK is constructed. The returned SDK is
 * KEPT ALIVE — callers must NOT disconnect it.
 */
async function getOrBuildWarmSparkSdk(
  config: SparkBackupAdapterConfig,
  network: "mainnet" | "regtest",
  timeoutMs: number,
): Promise<{ session: WarmSparkSession; sdk: BreezSparkSdk }> {
  const key = warmSessionKey({ mnemonic: config.mnemonic, network, storageDir: config.storageDir })
  let session = warmSparkSessions.get(key)
  if (session === undefined) {
    session = { sdk: null, building: null, opChain: Promise.resolve(), lastSyncMs: null }
    warmSparkSessions.set(key, session)
  }
  if (session.sdk !== null) return { session, sdk: session.sdk }
  if (session.building === null) {
    const loadModule = config.loadModule ?? loadBreezSparkModule
    session.building = (async () => {
      sparkPreBuild("warm:start", `network=${network}`)
      const mod = await withTimeout(loadModule(), timeoutMs, "spark sdk load")
      sparkPreBuild("warm:module-loaded")
      const sdkConfig = mod.defaultConfig(network)
      sdkConfig.apiKey = config.apiKey
      sparkPreBuild("warm:config-built")
      return buildSparkSdk(
        mod,
        sdkConfig,
        { type: "mnemonic", mnemonic: config.mnemonic, passphrase: undefined },
        config.storageDir,
        timeoutMs,
      )
    })()
    // If the build fails, clear the in-flight promise so a later acquirer can
    // retry rather than awaiting a permanently-rejected promise.
    session.building.catch(() => {
      if (session && session.sdk === null) session!.building = null
    })
  }
  const sdk = await session.building
  session.sdk = sdk
  session.building = null
  return { session, sdk }
}

/**
 * Serialize an operation on the warm session's op chain. The SDK is treated as
 * NOT safe for concurrent sends, so every warm op runs one-at-a-time. A failing
 * op rejects to its own caller but does NOT poison the chain for the next op.
 */
function runSerializedOnWarmSession<T>(session: WarmSparkSession, op: () => Promise<T>): Promise<T> {
  const result = session.opChain.then(op, op)
  // Keep the chain alive on a settled tail regardless of this op's outcome.
  session.opChain = result.then(
    () => undefined,
    () => undefined,
  )
  return result
}

/**
 * Acquire an SDK for one logical operation.
 *
 * - Warm: returns the process-level singleton (kept alive), a `release` that is
 *   a NO-OP (we never disconnect a warm session per op), the `session` for sync
 *   bookkeeping/serialization, and `warm: true`.
 * - Cold (default): builds a fresh short-lived SDK exactly as before; `release`
 *   disconnects it. This path is byte-for-byte unchanged from the legacy code.
 */
async function acquireSparkSession(
  config: SparkBackupAdapterConfig,
  network: "mainnet" | "regtest",
  timeoutMs: number,
): Promise<{
  sdk: BreezSparkSdk
  warm: boolean
  session: WarmSparkSession | null
  release: () => Promise<void>
}> {
  if (resolveWarmSession(config)) {
    const { session, sdk } = await getOrBuildWarmSparkSdk(config, network, timeoutMs)
    return { sdk, warm: true, session, release: async () => undefined }
  }
  // Cold path (unchanged behavior): load → build → caller op → disconnect in
  // release(). #5194: instrumented with pre-build step markers so a failing
  // host shows exactly how far it got before `build()`.
  sparkPreBuild("cold:start", `network=${network}`)
  const loadModule = config.loadModule ?? loadBreezSparkModule
  const mod = await withTimeout(loadModule(), timeoutMs, "spark sdk load")
  sparkPreBuild("cold:module-loaded")
  const sdkConfig = mod.defaultConfig(network)
  sdkConfig.apiKey = config.apiKey
  sparkPreBuild("cold:config-built")
  const sdk = await buildSparkSdk(
    mod,
    sdkConfig,
    { type: "mnemonic", mnemonic: config.mnemonic, passphrase: undefined },
    config.storageDir,
    timeoutMs,
  )
  sparkPreBuild("cold:sdk-built")
  return {
    sdk,
    warm: false,
    session: null,
    release: async () => {
      if (sdk.disconnect) {
        try {
          await sdk.disconnect()
        } catch {
          // best-effort cleanup; never throw from disconnect.
        }
      }
    },
  }
}

/**
 * #5207: run a background `syncWallet` against the warm session so the wallet is
 * already current when a send arrives. Builds the session if needed. Serialized
 * on the session op chain (so it never races an in-flight send), records the
 * sync time on success, and is best-effort: a failed/timed-out sync leaves the
 * previous `lastSyncMs` untouched and resolves `{ synced: false }`. Returns
 * quickly when the SDK build itself fails (e.g. no network), without throwing.
 */
export async function syncWarmSparkSession(
  config: SparkBackupAdapterConfig,
): Promise<{ synced: boolean; reason?: string }> {
  const network = config.network ?? "mainnet"
  const timeoutMs = config.timeoutMs ?? DEFAULT_SPARK_TIMEOUT_MS
  let acquired: Awaited<ReturnType<typeof getOrBuildWarmSparkSdk>>
  try {
    acquired = await getOrBuildWarmSparkSdk(config, network, timeoutMs)
  } catch (error) {
    return { synced: false, reason: error instanceof Error ? error.message : String(error) }
  }
  const { session, sdk } = acquired
  if (typeof sdk.syncWallet !== "function") return { synced: false, reason: "syncWallet unsupported" }
  return runSerializedOnWarmSession(session, async () => {
    const readSyncTimeoutMs = Math.max(timeoutMs, READ_SYNC_TIMEOUT_MS)
    try {
      await withTimeout(sdk.syncWallet!({}), readSyncTimeoutMs, "spark syncWallet (background)")
      session.lastSyncMs = performance.now()
      return { synced: true }
    } catch (error) {
      return { synced: false, reason: error instanceof Error ? error.message : String(error) }
    }
  })
}

/**
 * #5207: how recently (ms) the warm session must have synced for a send to skip
 * its own pre-send `syncWallet`. The daemon syncs on a ~20-30s timer, so a guard
 * of ~60s means a steady-state send never re-syncs on the critical path while a
 * stale/never-synced session still force-syncs once before sending.
 */
const WARM_SYNC_FRESH_WINDOW_MS = 60_000

/**
 * #5207: shut down all warm Spark sessions (daemon shutdown). Best-effort: each
 * disconnect is guarded and never throws. After this the registry is empty so a
 * later acquire rebuilds cleanly.
 */
export async function closeWarmSparkSession(): Promise<void> {
  const sessions = Array.from(warmSparkSessions.values())
  warmSparkSessions.clear()
  for (const session of sessions) {
    const sdk = session.sdk
    if (sdk?.disconnect) {
      try {
        await sdk.disconnect()
      } catch {
        // best-effort cleanup; never throw from disconnect.
      }
    }
  }
}

/**
 * Test-only: reset the warm-session registry WITHOUT disconnecting (used to keep
 * unit tests isolated from each other). Not part of the production shutdown path
 * (use `closeWarmSparkSession` for that, which disconnects).
 */
export function __resetWarmSparkSessionsForTest(): void {
  warmSparkSessions.clear()
}

/**
 * Build a real Spark backup helper backed by the Breez SDK Spark package.
 *
 * Each invocation connects a short-lived SDK session (audit "lifecycle
 * cleanup" lesson: initialize -> answer -> disconnect), maps the SDK result
 * into the slice-1 helper JSON contract, and disconnects. RECEIVE MODES ONLY:
 * the adapter exposes status/address/history/unclaimed-deposits/claim. Spend is
 * deliberately outside this helper command contract; use the explicit
 * `createSparkBackupSendTransfer` / `createSparkBackupSweepTransfer` closures.
 */
export function createSparkBackupHelper(config: SparkBackupAdapterConfig): SparkBackupHelper {
  // #5207: SDK acquisition (cold build vs warm reuse) and module loading are now
  // owned by `acquireSparkSession`, which reads `config.loadModule`.
  const network = config.network ?? "mainnet"
  const timeoutMs = config.timeoutMs ?? DEFAULT_SPARK_TIMEOUT_MS

  return async (command: SparkBackupCommand): Promise<WalletCommandResult> => {
    if (!config.apiKey || config.apiKey.trim() === "") {
      return helperError(command, "missing breez api key")
    }
    if (!config.mnemonic || config.mnemonic.trim() === "") {
      return helperError(command, "missing wallet seed")
    }

    // #5207: acquire a session — warm (reused singleton, never disconnected per
    // op) when enabled, else a cold short-lived SDK (disconnected in `release`).
    // The command body is identical for both; only acquisition + teardown differ.
    let acquired: Awaited<ReturnType<typeof acquireSparkSession>> | null = null
    try {
      acquired = await acquireSparkSession(config, network, timeoutMs)
      const sdk = acquired.sdk
      // Run the command body. On a warm session, serialize it on the session op
      // chain so it never races an in-flight send (the SDK is treated as NOT
      // concurrency-safe). On the cold path it runs directly (its own SDK).
      const runBody = async (): Promise<WalletCommandResult> => {
      switch (command) {
        case "address": {
          // Static Spark address (receive-only). The audit confirms Spark
          // addresses are static and generated via receivePayment with a
          // sparkAddress payment method.
          const res = await withTimeout(
            sdk.receivePayment({ paymentMethod: { type: "sparkAddress" } }),
            timeoutMs,
            "spark receivePayment",
          )
          const raw = typeof res?.paymentRequest === "string" ? res.paymentRequest : null
          if (!raw) return helperError(command, "no spark address returned")
          // Raw target rides in helper stdout only; slice-1 redacts it.
          return helperOk({ spark_address: raw })
        }
        case "status": {
          // Sync first: getInfo({ensureSynced}) alone returned a stale null in
          // real-infra testing even right after a claim; an explicit syncWallet
          // makes the balance + pending list current (#5166). Guarded so older
          // SDK builds / test fakes without syncWallet degrade gracefully.
          // #5197: give the sync room to complete after a restart (it can exceed
          // the short read timeout) so we don't report a stale pre-sync balance.
          const readSyncTimeoutMs = Math.max(timeoutMs, READ_SYNC_TIMEOUT_MS)
          if (typeof sdk.syncWallet === "function") {
            await withTimeout(sdk.syncWallet({}), readSyncTimeoutMs, "spark syncWallet").catch(() => undefined)
          }
          // #5194: some hosts fail getInfo({ensureSynced:true}) fast even after a
          // successful wallet open + syncWallet — the read path degrades to
          // helper-unavailable while send works. Fall back to a non-forced read so
          // it stays readable, but #5197: TRACK that the balance is then a
          // possibly-stale, non-authoritative read so the projection flags it as
          // refreshing instead of presenting it as a confirmed-spendable balance.
          let balanceSynced = true
          const info = await withTimeout(
            sdk.getInfo({ ensureSynced: true }),
            readSyncTimeoutMs,
            "spark getInfo",
          ).catch(error => {
            balanceSynced = false
            if (process.env.PYLON_SPARK_DEBUG === "1") {
              console.error(
                `[spark-getinfo] ensureSynced failed: ${error instanceof Error ? error.message : String(error)}`,
              )
            }
            return withTimeout(
              sdk.getInfo({ ensureSynced: false }),
              timeoutMs,
              "spark getInfo (no ensureSync)",
            )
          })
          const deposits = await withTimeout(
            sdk.listUnclaimedDeposits({}),
            timeoutMs,
            "spark listUnclaimedDeposits",
          ).catch(() => ({ deposits: undefined }))
          // The SDK's wasm-bindgen layer can return u64 balances as a bigint (or
          // a decimal string), not a JS number. A strict `typeof === "number"`
          // check then reports a real balance as `null`, making received funds
          // invisible (#5166 / offline-receive reconcile). Coerce defensively.
          const balance = toSatNumber(
            (info as { balanceSats?: unknown; balance_sats?: unknown; balanceSat?: unknown })?.balanceSats ??
              (info as { balance_sats?: unknown })?.balance_sats ??
              (info as { balanceSat?: unknown })?.balanceSat,
          )
          const unclaimed = Array.isArray(deposits?.deposits) ? deposits.deposits.length : null
          // READ-ONLY (#5166): surface pending Lightning HTLCs — offline-received
          // funds awaiting `backup-claim` — so an operator (or a cautious owner
          // who only permits read commands) can SEE the funds before claiming.
          let claimableCount = 0
          let claimableSats = 0
          const list =
            typeof sdk.listPayments === "function"
              ? await withTimeout(
                  sdk.listPayments({ statusFilter: ["pending"], limit: 100 }),
                  timeoutMs,
                  "spark listPayments",
                ).catch(() => ({ payments: undefined }))
              : { payments: undefined }
          const payments = Array.isArray(list?.payments) ? list.payments : []
          for (const p of payments) {
            const htlc = (p as { details?: { htlcDetails?: { status?: string } } }).details?.htlcDetails
            if (htlc && htlc.status === "waitingForPreimage") {
              claimableCount += 1
              const amt = toSatNumber((p as { amount?: unknown }).amount)
              if (amt !== null) claimableSats += amt
            }
          }
          return helperOk({
            balance_sats: balance,
            // #5197: false => balance came from a non-forced fallback read and may
            // be stale (e.g. mid-sync right after restart); the projection flags it
            // refreshing rather than confirmed-spendable.
            balance_synced: balanceSynced,
            unclaimed_deposit_count: unclaimed,
            claimable_htlc_count: claimableCount,
            claimable_htlc_sats: claimableSats,
          })
        }
        case "history": {
          const res = await withTimeout(sdk.listPayments({ limit: 50 }), timeoutMs, "spark listPayments")
          const count = Array.isArray(res?.payments) ? res.payments.length : 0
          // Public-safe summary only: count, never raw payment material.
          return helperOk({ payment_count: count })
        }
        case "unclaimed-deposits": {
          const res = await withTimeout(
            sdk.listUnclaimedDeposits({}),
            timeoutMs,
            "spark listUnclaimedDeposits",
          )
          const count = Array.isArray(res?.deposits) ? res.deposits.length : 0
          return helperOk({ unclaimed_deposit_count: count })
        }
        case "lightning-address": {
          // Static Lightning Address (LNURL-pay) hosted by this wallet's Spark
          // LSP. We pay OFFLINE recipients by paying this address from the MDK
          // treasury; the Spark LSP holds funds if the recipient is offline.
          // RECEIVE-SIDE registration only — no send/pay path here.
          if (typeof sdk.getLightningAddress !== "function" || typeof sdk.registerLightningAddress !== "function") {
            return helperError(command, "lightning address unsupported")
          }
          // 1) If one is already registered, return it.
          const existing = await withTimeout(
            sdk.getLightningAddress(),
            timeoutMs,
            "spark getLightningAddress",
          )
          if (existing && typeof existing.lightningAddress === "string" && existing.lightningAddress !== "") {
            return helperOk({ lightning_address: existing.lightningAddress })
          }
          // 2) Otherwise derive a deterministic username from the wallet's
          //    static spark address and register one. The username is a stable
          //    function of the wallet so re-runs reuse the same identity.
          const addr = await withTimeout(
            sdk.receivePayment({ paymentMethod: { type: "sparkAddress" } }),
            timeoutMs,
            "spark receivePayment",
          )
          const sparkAddress = typeof addr?.paymentRequest === "string" ? addr.paymentRequest : null
          if (!sparkAddress) return helperError(command, "no spark address to derive username")
          const username = "oa" + createHash("sha256").update(sparkAddress).digest("hex").slice(0, 16)
          if (typeof sdk.checkLightningAddressAvailable === "function") {
            // Best-effort availability check; ignore failures and proceed to
            // register (registration is the authority).
            await withTimeout(
              sdk.checkLightningAddressAvailable({ username }),
              timeoutMs,
              "spark checkLightningAddressAvailable",
            ).catch(() => undefined)
          }
          const registered = await withTimeout(
            sdk.registerLightningAddress({
              username,
              description: "OpenAgents Pylon Spark backup",
            }),
            timeoutMs,
            "spark registerLightningAddress",
          )
          const raw = typeof registered?.lightningAddress === "string" ? registered.lightningAddress : null
          if (!raw) return helperError(command, "no lightning address returned")
          // Raw address rides in helper stdout only; slice-1 redacts it.
          return helperOk({ lightning_address: raw })
        }
        case "claim": {
          // #5166 (the real receive bug): a Lightning payment to a Spark
          // Lightning Address arrives as an HTLC the recipient must CLAIM by
          // revealing its preimage. Until claimed it is invisible — getInfo
          // balance stays 0 and it is NOT an on-chain "unclaimed deposit". The
          // one-shot helper previously never claimed, so offline-received funds
          // never credited. Sync, then claim every pending Lightning HTLC whose
          // preimage we already hold, then report the post-claim balance.
          if (typeof sdk.syncWallet === "function") {
            await withTimeout(sdk.syncWallet({}), timeoutMs, "spark syncWallet").catch(() => undefined)
          }
          const list = await withTimeout(
            sdk.listPayments({ statusFilter: ["pending"], limit: 100 }),
            timeoutMs,
            "spark listPayments",
          )
          const payments = Array.isArray(list?.payments) ? list.payments : []
          let claimedCount = 0
          let claimedSats = 0
          let claimableSeen = 0
          for (const p of payments) {
            const details = (p as { details?: { htlcDetails?: { status?: string; preimage?: string } } }).details
            const htlc = details?.htlcDetails
            if (!htlc || htlc.status !== "waitingForPreimage") continue
            claimableSeen += 1
            const preimage = typeof htlc.preimage === "string" ? htlc.preimage : ""
            if (preimage === "") continue
            try {
              const res = await withTimeout(
                sdk.claimHtlcPayment({ preimage }),
                timeoutMs,
                "spark claimHtlcPayment",
              )
              claimedCount += 1
              const amt = toSatNumber((res as { payment?: { amount?: unknown } })?.payment?.amount)
              if (amt !== null) claimedSats += amt
            } catch {
              // Skip individual claim failures; report the rest.
            }
          }
          const info = await withTimeout(sdk.getInfo({ ensureSynced: true }), timeoutMs, "spark getInfo")
          const balance = toSatNumber(
            (info as { balanceSats?: unknown })?.balanceSats,
          )
          return helperOk({
            claimed_count: claimedCount,
            claimed_sats: claimedSats,
            claimable_seen: claimableSeen,
            pending_seen: payments.length,
            balance_sats: balance,
          })
        }
        default:
          return helperError(command, "unsupported command")
      }
      }
      return acquired.warm && acquired.session
        ? await runSerializedOnWarmSession(acquired.session, runBody)
        : await runBody()
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      // #5194: surface the raw helper failure (status/getInfo/sync/etc.) under
      // PYLON_SPARK_DEBUG=1 so the underlying reason is visible on hosts where a
      // command degrades to helper-unavailable; off by default (no payment material).
      if (process.env.PYLON_SPARK_DEBUG === "1") {
        console.error(`[spark-helper:${command}] ${message}`)
      }
      // Surface missing-credential shape so slice-1 classification can map it.
      return helperError(command, message)
    } finally {
      // #5207: warm sessions are KEPT ALIVE (release is a no-op); cold sessions
      // disconnect here exactly as before.
      if (acquired) await acquired.release()
    }
  }
}

/**
 * Private transfer adapter for the consented Spark-backup -> MDK sweep.
 *
 * This is deliberately NOT exposed as a generic Spark send command. The caller
 * must provide a fresh local MDK receive target and an idempotency key; this
 * adapter pays exactly that target from the node's own Spark backup wallet and
 * returns only public-safe refs/amounts. Raw BOLT11 targets stay inside this
 * closure and are never emitted in helper JSON or projections.
 */
function narrowSweepResult(result: SparkBackupSendTransferResult): SparkBackupSweepTransferResult {
  if (!result.ok) return result
  return {
    ok: true,
    transferRef: result.transferRef,
    amountSats: result.amountSats,
    feeSats: result.feeSats,
  }
}

export function createSparkBackupSweepTransfer(config: SparkBackupAdapterConfig): SparkBackupSweepTransfer {
  // #5207: SDK acquisition is owned by `acquireSparkSession` (cold vs warm).
  const network = config.network ?? "mainnet"
  const timeoutMs = config.timeoutMs ?? DEFAULT_SPARK_TIMEOUT_MS

  return async ({
    amountSats,
    destination,
    idempotencyKey,
  }: {
    amountSats: number
    destination: string
    idempotencyKey: string
  }) => {
    if (!config.apiKey || config.apiKey.trim() === "") {
      return { ok: false, failureRef: "wallet.spark_backup_transfer.missing_credential" }
    }
    if (!config.mnemonic || config.mnemonic.trim() === "") {
      return { ok: false, failureRef: "wallet.spark_backup_transfer.missing_seed" }
    }

    let acquired: Awaited<ReturnType<typeof acquireSparkSession>> | null = null
    try {
      acquired = await acquireSparkSession(config, network, timeoutMs)
      const sdk = acquired.sdk
      // The send itself is the only mutating op here; serialize it on the warm
      // session so it never races a concurrent send/sync (the SDK is treated as
      // NOT concurrency-safe). Cold path runs directly on its own SDK.
      const doSend = () =>
        sendSparkPaymentFromSdk({
          amountSats,
          destination,
          idempotencyKey,
          prefix: "wallet.spark_backup_transfer",
          sdk,
          timeoutMs,
          allowLnurlPay: false,
          // #5254: adapter-level operator override (env PYLON_SPARK_MAX_FEE_SATS
          // is also consulted inside the guard). The sweep input shape has no
          // per-call knob, so only the config-level override applies here.
          ...(config.maxFeeSats === undefined ? {} : { maxFeeSats: config.maxFeeSats }),
        })
      const result =
        acquired.warm && acquired.session
          ? await runSerializedOnWarmSession(acquired.session, doSend)
          : await doSend()
      return narrowSweepResult(result)
    } catch (error) {
      return {
        ok: false,
        failureRef: publicRef(
          "wallet.spark_backup_transfer_failure",
          error instanceof Error ? error.message : String(error),
        ),
      }
    } finally {
      // #5207: warm sessions are KEPT ALIVE (no-op release); cold sessions
      // disconnect here exactly as before.
      if (acquired) await acquired.release()
    }
  }
}

/**
 * Explicit Spark spend adapter for `wallet send --rail spark --confirm-send`.
 *
 * Pays either a BOLT11/Spark payment request (`prepareSendPayment` →
 * `sendPayment`) or a Lightning Address/LNURL-pay destination (`parse` →
 * `prepareLnurlPay` → `lnurlPay`). Raw destination material stays inside this
 * closure. The returned refs are digests only.
 */
export function createSparkBackupSendTransfer(config: SparkBackupAdapterConfig): SparkBackupSendTransfer {
  // #5207: SDK acquisition is owned by `acquireSparkSession` (cold vs warm).
  const network = config.network ?? "mainnet"
  const timeoutMs = config.timeoutMs ?? DEFAULT_SPARK_TIMEOUT_MS

  return async ({
    amountSats,
    destination,
    idempotencyKey,
    maxFeeSats,
  }: {
    amountSats: number
    destination: string
    idempotencyKey: string
    // #5254: per-call operator override that RAISES the allowed-fee ceiling.
    // Falls back to the adapter-level `config.maxFeeSats`; the env override
    // (PYLON_SPARK_MAX_FEE_SATS) is consulted inside the guard regardless.
    maxFeeSats?: number
  }) => {
    if (!config.apiKey || config.apiKey.trim() === "") {
      return { ok: false, failureRef: "wallet.spark_backup_send.missing_credential" }
    }
    if (!config.mnemonic || config.mnemonic.trim() === "") {
      return { ok: false, failureRef: "wallet.spark_backup_send.missing_seed" }
    }
    // Per-call override wins over the adapter-level default.
    const effectiveMaxFeeSats = maxFeeSats ?? config.maxFeeSats

    let acquired: Awaited<ReturnType<typeof acquireSparkSession>> | null = null
    // Latency audit: process start -> reaching this send closure (cold binary
    // start + CLI parse + option resolution). The rest are per-step deltas.
    sparkTiming("process_to_send_closure", process.uptime() * 1000)
    const tStart = performance.now()
    try {
      // #5207: warm → reuse the singleton (no module load / build / disconnect);
      // cold → load + build a fresh SDK exactly as before. The build/connect
      // timing labels stay so a regression is still measurable on either path.
      const tBuild = performance.now()
      acquired = await acquireSparkSession(config, network, timeoutMs)
      const sdk = acquired.sdk
      if (!acquired.warm) {
        sparkTiming("module_load", performance.now() - tStart)
        sparkTiming("sdk_build_connect", performance.now() - tBuild)
      }

      // The pre-send sync + the send are serialized together on the warm session
      // so the background-sync timer can never interleave between them, and so
      // two concurrent sends can never run against the same SDK.
      const runSendWithSync = async (): Promise<SparkBackupSendTransferResult> => {
        if (typeof sdk.syncWallet === "function") {
          // #5207: on a warm session, SKIP the pre-send sync when the background
          // timer synced within the freshness window — the wallet is already
          // current, so the ~3s sync is off the critical path. A stale/never-synced
          // warm session (or any cold session) still force-syncs once before
          // sending, preserving the original safety posture.
          const session = acquired!.session
          const recentlySynced =
            acquired!.warm &&
            session?.lastSyncMs !== null &&
            session?.lastSyncMs !== undefined &&
            performance.now() - session.lastSyncMs < WARM_SYNC_FRESH_WINDOW_MS
          if (recentlySynced) {
            sparkTiming("sync_wallet_skipped_warm", 0)
          } else {
            const tSync = performance.now()
            await withTimeout(sdk.syncWallet({}), timeoutMs, "spark syncWallet").catch(() => undefined)
            sparkTiming("sync_wallet", performance.now() - tSync)
            if (acquired!.warm && session) session.lastSyncMs = performance.now()
          }
        }
        const tSend = performance.now()
        const sendResult = await sendSparkPaymentFromSdk({
          amountSats,
          destination,
          idempotencyKey,
          prefix: "wallet.spark_backup_send",
          sdk,
          timeoutMs,
          allowLnurlPay: true,
          // #5254: thread the operator fee-ceiling override into the guard.
          ...(effectiveMaxFeeSats === undefined ? {} : { maxFeeSats: effectiveMaxFeeSats }),
        })
        sparkTiming("send_payment_step", performance.now() - tSend)
        return sendResult
      }
      const result =
        acquired.warm && acquired.session
          ? await runSerializedOnWarmSession(acquired.session, runSendWithSync)
          : await runSendWithSync()
      sparkTiming("transfer_total_in_closure", performance.now() - tStart)
      return result
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      // #5185: opt-in raw diagnostics (PYLON_SPARK_DEBUG=1) so an operator can
      // see the exact SDK failure when a send is rejected; off by default so no
      // payment material reaches normal output.
      if (process.env.PYLON_SPARK_DEBUG === "1") {
        console.error(`[spark-send] ${message}`)
      }
      // #5196: a TIMEOUT is an INDETERMINATE outcome, not a clean failure — the
      // payment may have settled server-side (a slow send completing after the
      // wait window), so the caller must verify the balance before any retry to
      // avoid a double-spend. Mark it with a distinct ref the projection maps to
      // a "verify before retry" next action.
      const timedOut = /timed out/i.test(message)
      return {
        ok: false,
        failureRef: publicRef(
          timedOut ? "wallet.spark_backup_send_indeterminate" : "wallet.spark_backup_send_failure",
          message,
        ),
      }
    } finally {
      // #5207: warm sessions are KEPT ALIVE (no-op release); cold sessions
      // disconnect here exactly as before.
      if (acquired) await acquired.release()
    }
  }
}

/**
 * Resolve the Spark backup helper for the running node from env + local seed.
 * Returns `null` when the backup is not opt-in enabled, no credential is
 * configured, or no wallet seed is available — callers then fall through to the
 * slice-1 `unavailableSparkBackupHelper`, keeping default behavior inert.
 *
 * This NEVER returns the credential or seed to the caller; it only wires them
 * into the closure that talks to the SDK.
 */
export function resolveSparkBackupHelper(input: {
  env?: NodeJS.ProcessEnv
  mnemonic?: string | null
  storageDir?: string
  loadModule?: () => Promise<BreezSparkModule>
  // #5207: when the long-lived daemon resolves the helper it passes
  // `warmSession: true` so reads reuse the warm session. Undefined keeps the
  // adapter's default (PYLON_SPARK_WARM_SESSION env, default-off).
  warmSession?: boolean
  // #5194: explicit opt-in intent from the caller. The receive/status/payout
  // CLI commands are ALREADY gated on opt-in upstream and always intend the
  // helper to run, so they pass `enabled: true`. Previously this resolver only
  // consulted `env.PYLON_SPARK_BACKUP_ENABLED`; if that var was not exported in
  // the operator's shell it returned null, the gate silently substituted the
  // inert `unavailableSparkBackupHelper` (whose stderr classifies to the
  // useless `unknown` reason), and the in-process SDK build was NEVER attempted
  // — exactly the silent `helper-unavailable`/`unknown` dead-end seen on
  // deterministic-repro hosts. An explicit `enabled: true` now wires the
  // in-process helper regardless of the env flag.
  enabled?: boolean
}): SparkBackupHelper | null {
  const env = input.env ?? process.env
  // #5304: enabled BY DEFAULT. An explicit caller `enabled: true` always wires
  // the helper (the receive/status/payout CLI paths pass this). Otherwise we
  // consult the canonical default-ON resolver, which is ON unless the operator
  // set an explicit OFF override (PYLON_SPARK_BACKUP_DISABLED=1 or
  // PYLON_SPARK_BACKUP_ENABLED=0/false).
  const enabled = input.enabled === true || isSparkBackupDefaultEnabled(env)
  if (!enabled) {
    if (process.env.PYLON_SPARK_DEBUG === "1") {
      console.error("[spark-helper:resolve] helper not wired: opt-out override set (PYLON_SPARK_BACKUP_DISABLED / PYLON_SPARK_BACKUP_ENABLED=0)")
    }
    return null
  }

  const apiKey = [env.OPENAGENTS_SPARK_API_KEY, env.BREEZ_API_KEY, env.PYLON_SPARK_BACKUP_API_KEY, DEFAULT_OPENAGENTS_SPARK_API_KEY].find(
    (value) => value !== undefined && value.trim() !== "",
  )
  if (!apiKey) {
    if (process.env.PYLON_SPARK_DEBUG === "1") {
      console.error("[spark-helper:resolve] helper not wired: no Breez/Spark api key (and embedded default missing)")
    }
    return null
  }

  const mnemonic = input.mnemonic ?? null
  if (!mnemonic || mnemonic.trim() === "") {
    if (process.env.PYLON_SPARK_DEBUG === "1") {
      console.error("[spark-helper:resolve] helper not wired: no wallet seed (identity mnemonic absent)")
    }
    return null
  }

  const network = env.PYLON_SPARK_BACKUP_NETWORK === "regtest" ? "regtest" : "mainnet"

  return createSparkBackupHelper({
    apiKey,
    mnemonic,
    network,
    storageDir: input.storageDir,
    loadModule: input.loadModule,
    ...(input.warmSession === undefined ? {} : { warmSession: input.warmSession }),
  })
}

/**
 * Resolve the Breez/Spark API key for legacy migration: first any configured
 * env override, else the embedded owner-authorized default key. ALWAYS returns
 * a usable key (the embedded default), so legacy migration never dead-ends on a
 * missing manual Breez key (#5085). The key is a service API key, not a wallet
 * seed, and grants no spend authority. NEVER logged or returned to projections.
 */
export function resolveLegacySparkApiKey(env: NodeJS.ProcessEnv = process.env): string {
  return (
    [env.OPENAGENTS_SPARK_API_KEY, env.BREEZ_API_KEY, env.PYLON_SPARK_BACKUP_API_KEY].find(
      (value) => value !== undefined && value.trim() !== "",
    ) ?? DEFAULT_OPENAGENTS_SPARK_API_KEY
  )
}

/**
 * Build a `LegacySparkCommandRunner` backed by the Bun-native Breez SDK Spark
 * helper (`createSparkBackupHelper`) instead of the external `spark-wallet-cli`
 * binary (#5085). Spark wallets are deterministic from the seed, so the user's
 * 12-word identity mnemonic re-derives their old Spark wallet and balance with
 * the embedded service key — no manual Breez key, no external binary.
 *
 * The legacy migration preflight only ever invokes `["status"]`; this maps that
 * to the helper's `status` command (which returns `{balance_sats,
 * unclaimed_deposit_count}`). RECEIVE-SIDE ONLY: there is no send/pay path.
 *
 * SAFETY: the mnemonic and API key ride only inside the helper closure; they
 * are never returned to the caller, logged, or placed in any projection. A
 * missing/blank mnemonic resolves to a runner that reports the helper as
 * unavailable rather than throwing.
 */
export function legacySparkHelperRunner(input: {
  env?: NodeJS.ProcessEnv
  mnemonic: string | null | undefined
  storageDir?: string
  network?: "mainnet" | "regtest"
  // First-sync against the live network can take longer than the receive-only
  // default; callers may widen this for the legacy migration probe.
  timeoutMs?: number
  loadModule?: () => Promise<BreezSparkModule>
}): LegacySparkCommandRunner {
  const env = input.env ?? process.env
  const mnemonic = input.mnemonic ?? null
  const network =
    input.network ?? (env.PYLON_SPARK_BACKUP_NETWORK === "regtest" ? "regtest" : "mainnet")
  // The legacy first-sync re-derives the user's old Spark wallet from the seed
  // and may sync more state than the steady-state receive helper. Allow an env
  // override, default to a generous 90s ceiling.
  const timeoutMs =
    input.timeoutMs ??
    (() => {
      const raw = env.PYLON_LEGACY_SPARK_TIMEOUT_MS
      const parsed = raw === undefined ? NaN : Number(raw)
      return Number.isFinite(parsed) && parsed > 0 ? parsed : 90_000
    })()

  return async (args: string[]): Promise<WalletCommandResult> => {
    if (!mnemonic || mnemonic.trim() === "") {
      return {
        exitCode: 1,
        stdout: "",
        stderr: "legacy spark helper: missing identity or recovery mnemonic",
      }
    }
    const helper = createSparkBackupHelper({
      apiKey: resolveLegacySparkApiKey(env),
      mnemonic,
      network,
      storageDir: input.storageDir,
      timeoutMs,
      loadModule: input.loadModule,
    })
    const command = args[0]
    switch (command) {
      case "status":
        return helper("status")
      case "address":
        return helper("address")
      case "history":
        return helper("history")
      case "unclaimed-deposits":
        return helper("unclaimed-deposits")
      default:
        return {
          exitCode: 1,
          stdout: "",
          stderr: `legacy spark helper: unsupported command`,
        }
    }
  }
}
