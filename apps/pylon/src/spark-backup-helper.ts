// ---------------------------------------------------------------------------
// Spark backup receive helper adapter (slice 2 of #5078).
//
// Backs the slice-1 `SparkBackupHelper` contract with the real Breez SDK Spark
// JavaScript package (`@breeztech/breez-sdk-spark`). RECEIVE MODES ONLY.
//
// Feasibility (verified 2026-06-15): the package is WASM-based, not native
// bindings. Its nodejs entry imports and initializes cleanly under the Pylon
// Bun runtime (Bun 1.3.x). The only native pieces (`better-sqlite3`, `pg`,
// `mysql2`) are OPTIONAL storage backends; the SDK falls back to filesystem
// storage via `storageDir`, so no native module is required for our
// receive-only fallback. We therefore use the SDK directly in-process rather
// than an isolated helper binary.
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

import type { SparkBackupCommand, SparkBackupHelper, WalletCommandResult } from "./wallet"

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

type BreezSparkModule = {
  defaultConfig(network: string): { apiKey?: string } & Record<string, unknown>
  connect(request: {
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
  if (typeof mod?.connect !== "function" || typeof mod?.defaultConfig !== "function") {
    throw new Error("breez sdk spark module missing connect/defaultConfig")
  }
  return mod
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
      sdk = await withTimeout(
        mod.connect({
          config: sdkConfig,
          seed: { type: "mnemonic", mnemonic: config.mnemonic, passphrase: undefined },
          storageDir: config.storageDir,
        }),
        timeoutMs,
        "spark sdk connect",
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

  const apiKey = [env.OPENAGENTS_SPARK_API_KEY, env.BREEZ_API_KEY, env.PYLON_SPARK_BACKUP_API_KEY].find(
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
