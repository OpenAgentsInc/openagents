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
} from "./wallet"
import { SparkBunStorage } from "./spark-bun-storage"
import { toSatNumber } from "./sat-number"
import { ensureSparkWasmAvailable } from "./spark-wasm-runtime"
// Re-export so existing importers (and tests) can keep importing it from here.
export { toSatNumber } from "./sat-number"

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

// Send-latency audit: opt-in per-step timing (PYLON_SPARK_DEBUG=1) so the cold
// per-command send pipeline cost is MEASURABLE on real infra. Monotonic clock,
// no payment material — just labelled millisecond deltas on stderr.
const sparkTiming = (label: string, ms: number) => {
  if (process.env.PYLON_SPARK_DEBUG === "1") {
    console.error(`[spark-timing] ${label}=${Math.round(ms)}ms`)
  }
}

function helperError(command: SparkBackupCommand, message: string): WalletCommandResult {
  // The message is helper stderr; slice-1 code keeps it out of public
  // projections. We still avoid echoing any secret-shaped material here.
  return { exitCode: 1, stdout: "", stderr: `spark backup helper ${command}: ${message}` }
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

  // #5195: pay a Lightning Address by resolving it to a BOLT11 (LNURL-pay) and
  // sending that through the proven sendPayment path below — NOT via the SDK's
  // lnurlPay, which throws "Tree service error: insufficient funds" from the
  // Spark leaf structure even when the balance covers the amount. This mirrors
  // how the treasury Spark sender pays Lightning Addresses.
  let method: "payment_request" | "lnurl_pay" = "payment_request"
  if (input.allowLnurlPay && isLightningAddress(destination)) {
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
  const paymentMethod = (prepareResponse as { paymentMethod?: { type?: string } })?.paymentMethod
  const isBolt11 = paymentMethod?.type === "bolt11Invoice"
  // #5196: wait at least the SDK's completion window + a buffer for the send to
  // settle — never the short read-timeout — so a slow/large send is not aborted
  // while it completes server-side (false-negative). prepare/resolve above stay
  // on the short timeout (they are fast).
  const sendTimeoutMs = Math.max(
    input.timeoutMs,
    SEND_COMPLETION_TIMEOUT_SECS * 1000 + SEND_TIMEOUT_BUFFER_MS,
  )
  const sendPrepared = (idempotency: string, preferSpark: boolean) =>
    withTimeout(
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
  const fee = toSatNumber(payment.fees)
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
    const storage = new SparkBunStorage(dbPath)
    const builder = mod.SdkBuilder.new(sdkConfig, seed).withStorage(storage)
    return await withTimeout(builder.build(), timeoutMs, "spark sdk build")
  }
  if (typeof mod.connect === "function") {
    return await withTimeout(
      mod.connect({ config: sdkConfig, seed, storageDir }),
      timeoutMs,
      "spark sdk connect",
    )
  }
  throw new Error("breez sdk spark module exposes neither SdkBuilder nor connect")
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
  const loadModule = config.loadModule ?? loadBreezSparkModule
  const network = config.network ?? "mainnet"
  const timeoutMs = config.timeoutMs ?? DEFAULT_SPARK_TIMEOUT_MS

  return async (command: SparkBackupCommand): Promise<WalletCommandResult> => {
    if (!config.apiKey || config.apiKey.trim() === "") {
      return helperError(command, "missing breez api key")
    }
    if (!config.mnemonic || config.mnemonic.trim() === "") {
      return helperError(command, "missing wallet seed")
    }

    let sdk: BreezSparkSdk | null = null
    try {
      const mod = await withTimeout(loadModule(), timeoutMs, "spark sdk load")
      const sdkConfig = mod.defaultConfig(network)
      sdkConfig.apiKey = config.apiKey
      sdk = await buildSparkSdk(
        mod,
        sdkConfig,
        { type: "mnemonic", mnemonic: config.mnemonic, passphrase: undefined },
        config.storageDir,
        timeoutMs,
      )

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
      if (sdk?.disconnect) {
        try {
          await sdk.disconnect()
        } catch {
          // best-effort cleanup; never throw from disconnect.
        }
      }
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
  const loadModule = config.loadModule ?? loadBreezSparkModule
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

    let sdk: BreezSparkSdk | null = null
    try {
      const mod = await withTimeout(loadModule(), timeoutMs, "spark sdk load")
      const sdkConfig = mod.defaultConfig(network)
      sdkConfig.apiKey = config.apiKey
      sdk = await buildSparkSdk(
        mod,
        sdkConfig,
        { type: "mnemonic", mnemonic: config.mnemonic, passphrase: undefined },
        config.storageDir,
        timeoutMs,
      )
      return narrowSweepResult(await sendSparkPaymentFromSdk({
        amountSats,
        destination,
        idempotencyKey,
        prefix: "wallet.spark_backup_transfer",
        sdk,
        timeoutMs,
        allowLnurlPay: false,
      }))
    } catch (error) {
      return {
        ok: false,
        failureRef: publicRef(
          "wallet.spark_backup_transfer_failure",
          error instanceof Error ? error.message : String(error),
        ),
      }
    } finally {
      if (sdk?.disconnect) {
        try {
          await sdk.disconnect()
        } catch {
          // best-effort cleanup; never throw from disconnect.
        }
      }
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
  const loadModule = config.loadModule ?? loadBreezSparkModule
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
      return { ok: false, failureRef: "wallet.spark_backup_send.missing_credential" }
    }
    if (!config.mnemonic || config.mnemonic.trim() === "") {
      return { ok: false, failureRef: "wallet.spark_backup_send.missing_seed" }
    }

    let sdk: BreezSparkSdk | null = null
    // Latency audit: process start -> reaching this send closure (cold binary
    // start + CLI parse + option resolution). The rest are per-step deltas.
    sparkTiming("process_to_send_closure", process.uptime() * 1000)
    const tStart = performance.now()
    try {
      const mod = await withTimeout(loadModule(), timeoutMs, "spark sdk load")
      sparkTiming("module_load", performance.now() - tStart)
      const sdkConfig = mod.defaultConfig(network)
      sdkConfig.apiKey = config.apiKey
      const tBuild = performance.now()
      sdk = await buildSparkSdk(
        mod,
        sdkConfig,
        { type: "mnemonic", mnemonic: config.mnemonic, passphrase: undefined },
        config.storageDir,
        timeoutMs,
      )
      sparkTiming("sdk_build_connect", performance.now() - tBuild)
      if (typeof sdk.syncWallet === "function") {
        const tSync = performance.now()
        await withTimeout(sdk.syncWallet({}), timeoutMs, "spark syncWallet").catch(() => undefined)
        sparkTiming("sync_wallet", performance.now() - tSync)
      }
      const tSend = performance.now()
      const result = await sendSparkPaymentFromSdk({
        amountSats,
        destination,
        idempotencyKey,
        prefix: "wallet.spark_backup_send",
        sdk,
        timeoutMs,
        allowLnurlPay: true,
      })
      sparkTiming("send_payment_step", performance.now() - tSend)
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
      if (sdk?.disconnect) {
        try {
          await sdk.disconnect()
        } catch {
          // best-effort cleanup; never throw from disconnect.
        }
      }
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
}): SparkBackupHelper | null {
  const env = input.env ?? process.env
  const enabled = env.PYLON_SPARK_BACKUP_ENABLED === "1" || env.PYLON_SPARK_BACKUP_ENABLED === "true"
  if (!enabled) return null

  const apiKey = [env.OPENAGENTS_SPARK_API_KEY, env.BREEZ_API_KEY, env.PYLON_SPARK_BACKUP_API_KEY, DEFAULT_OPENAGENTS_SPARK_API_KEY].find(
    (value) => value !== undefined && value.trim() !== "",
  )
  if (!apiKey) return null

  const mnemonic = input.mnemonic ?? null
  if (!mnemonic || mnemonic.trim() === "") return null

  const network = env.PYLON_SPARK_BACKUP_NETWORK === "regtest" ? "regtest" : "mainnet"

  return createSparkBackupHelper({
    apiKey,
    mnemonic,
    network,
    storageDir: input.storageDir,
    loadModule: input.loadModule,
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
