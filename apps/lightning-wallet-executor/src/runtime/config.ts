import { Context, Effect, Layer } from "effect"

import type { ExecutorMode, SparkNetwork } from "../contracts.js"
import { WalletExecutorConfigError } from "../errors.js"

export type WalletExecutorConfig = Readonly<{
  host: string
  port: number
  walletId: string
  authToken: string | null
  mode: ExecutorMode
  network: SparkNetwork
  sparkApiKey: string | null
  mnemonicProvider: "env" | "gcp"
  mnemonicEnvVar: string
  mnemonicSecretVersion: string | null
  requestCapMsats: number
  windowCapMsats: number
  windowMs: number
  paymentTimeoutSecs: number
  allowedHosts: ReadonlySet<string>
}>

export class WalletExecutorConfigService extends Context.Tag(
  "@openagents/lightning-wallet-executor/WalletExecutorConfigService",
)<WalletExecutorConfigService, WalletExecutorConfig>() {}

const toConfigError = (field: string, message: string): WalletExecutorConfigError =>
  WalletExecutorConfigError.make({ field, message })

const parseNonEmpty = (field: string, value: string | undefined, fallback?: string): Effect.Effect<string, WalletExecutorConfigError> =>
  Effect.gen(function* () {
    const candidate = value?.trim() ?? fallback?.trim() ?? ""
    if (!candidate) {
      return yield* toConfigError(field, "missing required environment variable")
    }
    return candidate
  })

const parseIntBounded = (
  field: string,
  value: string | undefined,
  options: { readonly fallback: number; readonly min: number; readonly max: number },
): Effect.Effect<number, WalletExecutorConfigError> =>
  Effect.gen(function* () {
    const raw = value?.trim()
    const parsed = raw && raw.length > 0 ? Number(raw) : options.fallback
    if (!Number.isFinite(parsed) || !Number.isInteger(parsed)) {
      return yield* toConfigError(field, "must be an integer")
    }
    if (parsed < options.min || parsed > options.max) {
      return yield* toConfigError(field, `must be between ${options.min} and ${options.max}`)
    }
    return parsed
  })

const parseMode = (value: string | undefined): ExecutorMode =>
  value?.trim().toLowerCase() === "spark" ? "spark" : "mock"

const parseSparkNetwork = (value: string | undefined): SparkNetwork =>
  value?.trim().toLowerCase() === "mainnet" ? "mainnet" : "regtest"

const normalizeHostSet = (input: string | undefined): ReadonlySet<string> => {
  const values = (input ?? "")
    .split(",")
    .map((part) => part.trim().toLowerCase())
    .filter((part) => part.length > 0)
  return new Set(values)
}

export const loadWalletExecutorConfig = (
  env: NodeJS.ProcessEnv,
): Effect.Effect<WalletExecutorConfig, WalletExecutorConfigError> =>
  Effect.gen(function* () {
    const host = env.OA_LIGHTNING_WALLET_EXECUTOR_HOST?.trim() || "127.0.0.1"
    const port = yield* parseIntBounded("OA_LIGHTNING_WALLET_EXECUTOR_PORT", env.OA_LIGHTNING_WALLET_EXECUTOR_PORT, {
      fallback: 8788,
      min: 1,
      max: 65535,
    })
    const walletId = yield* parseNonEmpty(
      "OA_LIGHTNING_WALLET_ID",
      env.OA_LIGHTNING_WALLET_ID,
      "openagents-ep212",
    )
    const authToken = env.OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN?.trim() || null

    const mode = parseMode(env.OA_LIGHTNING_WALLET_EXECUTOR_MODE)
    const network = parseSparkNetwork(env.OA_LIGHTNING_SPARK_NETWORK)

    const requestCapMsats = yield* parseIntBounded(
      "OA_LIGHTNING_WALLET_REQUEST_CAP_MSATS",
      env.OA_LIGHTNING_WALLET_REQUEST_CAP_MSATS,
      { fallback: 200_000, min: 1_000, max: 100_000_000 },
    )
    const windowCapMsats = yield* parseIntBounded(
      "OA_LIGHTNING_WALLET_WINDOW_CAP_MSATS",
      env.OA_LIGHTNING_WALLET_WINDOW_CAP_MSATS,
      { fallback: 1_000_000, min: 10_000, max: 5_000_000_000 },
    )
    const windowMs = yield* parseIntBounded("OA_LIGHTNING_WALLET_WINDOW_MS", env.OA_LIGHTNING_WALLET_WINDOW_MS, {
      fallback: 300_000,
      min: 1_000,
      max: 86_400_000,
    })
    const paymentTimeoutSecs = yield* parseIntBounded(
      "OA_LIGHTNING_SPARK_PAYMENT_TIMEOUT_SECS",
      env.OA_LIGHTNING_SPARK_PAYMENT_TIMEOUT_SECS,
      { fallback: 45, min: 1, max: 300 },
    )

    const mnemonicProvider = env.OA_LIGHTNING_WALLET_MNEMONIC_PROVIDER?.trim().toLowerCase() === "gcp" ? "gcp" : "env"
    const mnemonicEnvVar = env.OA_LIGHTNING_WALLET_MNEMONIC_ENV_VAR?.trim() || "OA_LIGHTNING_WALLET_MNEMONIC"
    const mnemonicSecretVersion = env.OA_LIGHTNING_WALLET_MNEMONIC_SECRET_VERSION?.trim() || null

    const sparkApiKey = env.OA_LIGHTNING_SPARK_API_KEY?.trim() || env.OA_LIGHTNING_BREEZ_API_KEY?.trim() || null

    const allowedHosts = normalizeHostSet(env.OA_LIGHTNING_WALLET_ALLOWED_HOSTS)

    if (mode === "spark") {
      if (!sparkApiKey) {
        return yield* toConfigError(
          "OA_LIGHTNING_SPARK_API_KEY",
          "required when OA_LIGHTNING_WALLET_EXECUTOR_MODE=spark",
        )
      }
      if (mnemonicProvider === "gcp" && !mnemonicSecretVersion) {
        return yield* toConfigError(
          "OA_LIGHTNING_WALLET_MNEMONIC_SECRET_VERSION",
          "required when OA_LIGHTNING_WALLET_MNEMONIC_PROVIDER=gcp",
        )
      }
      if (allowedHosts.size === 0) {
        return yield* toConfigError(
          "OA_LIGHTNING_WALLET_ALLOWED_HOSTS",
          "must include at least one host when mode=spark",
        )
      }
    }

    return {
      host,
      port,
      walletId,
      authToken,
      mode,
      network,
      sparkApiKey,
      mnemonicProvider,
      mnemonicEnvVar,
      mnemonicSecretVersion,
      requestCapMsats,
      windowCapMsats,
      windowMs,
      paymentTimeoutSecs,
      allowedHosts,
    }
  })

export const WalletExecutorConfigLive = Layer.effect(
  WalletExecutorConfigService,
  loadWalletExecutorConfig(process.env),
)

export const makeWalletExecutorConfigTestLayer = (config: WalletExecutorConfig) =>
  Layer.succeed(WalletExecutorConfigService, config)

export const defaultWalletExecutorConfig = (): WalletExecutorConfig => ({
  host: "127.0.0.1",
  port: 8788,
  walletId: "openagents-ep212",
  authToken: null,
  mode: "mock",
  network: "regtest",
  sparkApiKey: null,
  mnemonicProvider: "env",
  mnemonicEnvVar: "OA_LIGHTNING_WALLET_MNEMONIC",
  mnemonicSecretVersion: null,
  requestCapMsats: 200_000,
  windowCapMsats: 1_000_000,
  windowMs: 300_000,
  paymentTimeoutSecs: 45,
  allowedHosts: new Set(["sats4ai.com", "l402.openagents.com"]),
})
