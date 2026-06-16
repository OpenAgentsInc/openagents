// ---------------------------------------------------------------------------
// Spark backup receive helper adapter (slice 2 of #5078).
//
// Backs the slice-1 `SparkBackupHelper` contract with the real Breez SDK Spark
// JavaScript package (`@breeztech/breez-sdk-spark`). RECEIVE MODES ONLY.
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
// addresses/invoices/payment-requests, preimages, mnemonics, or SDK/wallet
// storage paths. Raw material is returned only inside the `SparkBackupHelper`
// JSON stdout that slice-1 code keeps out of public projections (and surfaces
// locally only under `--show-local-target`).
// ---------------------------------------------------------------------------

import { mkdirSync } from "node:fs"
import * as nodePath from "node:path"
import type {
  LegacySparkCommandRunner,
  SparkBackupCommand,
  SparkBackupHelper,
  WalletCommandResult,
} from "./wallet"
import { SparkBunStorage } from "./spark-bun-storage"

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
 * (it is an optional dependency loaded at runtime), so this is a narrow,
 * receive-only shape covering exactly the four helper commands.
 */
type BreezSparkSdk = {
  getInfo(request: { ensureSynced?: boolean }): Promise<{ balanceSats?: number }>
  receivePayment(request: {
    paymentMethod: { type: "sparkAddress" }
  }): Promise<{ paymentRequest?: string }>
  listPayments(request: { limit?: number }): Promise<{ payments?: unknown[] }>
  listUnclaimedDeposits(request: Record<string, never>): Promise<{ deposits?: unknown[] }>
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

function helperError(command: SparkBackupCommand, message: string): WalletCommandResult {
  // The message is helper stderr; slice-1 code keeps it out of public
  // projections. We still avoid echoing any secret-shaped material here.
  return { exitCode: 1, stdout: "", stderr: `spark backup helper ${command}: ${message}` }
}

function helperOk(payload: Record<string, unknown>): WalletCommandResult {
  return { exitCode: 0, stdout: JSON.stringify(payload), stderr: "" }
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
async function loadBreezSparkModule(): Promise<BreezSparkModule> {
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
 * the adapter exposes status/address/history/unclaimed-deposits and never a
 * send/pay path.
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
          const info = await withTimeout(sdk.getInfo({ ensureSynced: true }), timeoutMs, "spark getInfo")
          const deposits = await withTimeout(
            sdk.listUnclaimedDeposits({}),
            timeoutMs,
            "spark listUnclaimedDeposits",
          ).catch(() => ({ deposits: undefined }))
          const balance = typeof info?.balanceSats === "number" ? info.balanceSats : null
          const unclaimed = Array.isArray(deposits?.deposits) ? deposits.deposits.length : null
          return helperOk({
            balance_sats: balance,
            unclaimed_deposit_count: unclaimed,
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
        default:
          return helperError(command, "unsupported command")
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
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
