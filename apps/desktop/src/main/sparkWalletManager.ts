import { Clock, Context, Effect, Layer, Ref } from "effect"

import type { InvoicePaymentRequest, InvoicePaymentResult } from "@openagentsinc/lightning-effect/contracts"
import type {
  BreezSdk,
  Config as SparkConfig,
  Network as SparkNetwork,
  PrepareSendPaymentResponse,
  Seed as SparkSeed,
  SendPaymentResponse,
} from "@breeztech/breez-sdk-spark/nodejs"
import { SdkBuilder, defaultConfig } from "@breeztech/breez-sdk-spark/nodejs"
import { generateSeedWords, validateWords } from "nostr-effect"

import {
  DesktopSecureStorageError,
  DesktopSecureStorageService,
} from "./desktopSecureStorage"

export type SparkWalletLifecycle = "disconnected" | "connecting" | "connected" | "error"

export type SparkWalletManagerErrorCode =
  | "api_key_missing"
  | "mnemonic_missing"
  | "mnemonic_invalid"
  | "secure_storage_error"
  | "connect_failed"
  | "not_connected"
  | "payment_prepare_failed"
  | "payment_failed"
  | "payment_missing_preimage"
  | "payment_amount_exceeds_budget"
  | "payment_pending"

export class SparkWalletManagerError extends Error {
  readonly code: SparkWalletManagerErrorCode

  constructor(code: SparkWalletManagerErrorCode, message: string) {
    super(message)
    this.name = "SparkWalletManagerError"
    this.code = code
  }
}

export type SparkWalletStatus = Readonly<{
  readonly lifecycle: SparkWalletLifecycle
  readonly network: SparkNetwork
  readonly apiKeyConfigured: boolean
  readonly mnemonicStored: boolean
  readonly identityPubkey: string | null
  readonly balanceSats: number | null
  readonly tokenBalanceCount: number
  readonly lastSyncedAtMs: number | null
  readonly lastPaymentId: string | null
  readonly lastPaymentAtMs: number | null
  readonly lastErrorCode: SparkWalletManagerErrorCode | null
  readonly lastErrorMessage: string | null
}>

type SparkSdkClient = Pick<
  BreezSdk,
  "disconnect" | "getInfo" | "prepareSendPayment" | "sendPayment"
>

type SparkSdkConnectInput = Readonly<{
  readonly mnemonic: string
  readonly passphrase: string
  readonly network: SparkNetwork
  readonly apiKey: string | null
  readonly storageDir: string
}>

const createRealSparkSdkClient = async (
  input: SparkSdkConnectInput,
): Promise<SparkSdkClient> => {
  const config: SparkConfig = defaultConfig(input.network)
  if (input.apiKey && input.apiKey.trim().length > 0) {
    config.apiKey = input.apiKey.trim()
  }

  const seed: SparkSeed = input.passphrase.trim().length > 0
    ? {
        type: "mnemonic",
        mnemonic: input.mnemonic,
        passphrase: input.passphrase.trim(),
      }
    : {
        type: "mnemonic",
        mnemonic: input.mnemonic,
      }

  let builder = SdkBuilder.new(config, seed)
  builder = await builder.withDefaultStorage(input.storageDir)
  return await builder.build()
}

export type SparkWalletManagerConfig = Readonly<{
  readonly network: SparkNetwork
  readonly apiKey: string | null
  readonly storageDir: string
  readonly mnemonicStorageKey: string
  readonly mnemonicPassphrase: string
  readonly autoGenerateMnemonic: boolean
  readonly paymentCompletionTimeoutSecs: number
  readonly sdkConnect: (input: SparkSdkConnectInput) => Promise<SparkSdkClient>
}>

const asNonEmptyString = (value: string | undefined): string | null => {
  if (typeof value !== "string") return null
  const trimmed = value.trim()
  return trimmed.length > 0 ? trimmed : null
}

const parseNetwork = (value: string | undefined): SparkNetwork =>
  value?.trim().toLowerCase() === "mainnet" ? "mainnet" : "regtest"

export const defaultSparkWalletManagerConfig = (input: {
  readonly userDataPath: string
  readonly env: NodeJS.ProcessEnv
}): SparkWalletManagerConfig => ({
  network: parseNetwork(input.env.OA_DESKTOP_SPARK_NETWORK),
  apiKey: asNonEmptyString(input.env.OA_DESKTOP_SPARK_API_KEY) ?? asNonEmptyString(input.env.OA_DESKTOP_BREEZ_API_KEY),
  storageDir: `${input.userDataPath}/spark`,
  mnemonicStorageKey: "spark.wallet.mnemonic",
  mnemonicPassphrase: asNonEmptyString(input.env.OA_DESKTOP_SPARK_MNEMONIC_PASSPHRASE) ?? "",
  autoGenerateMnemonic: input.env.OA_DESKTOP_SPARK_AUTO_GENERATE_MNEMONIC !== "0",
  paymentCompletionTimeoutSecs: Math.max(
    1,
    Math.min(120, Number(input.env.OA_DESKTOP_SPARK_PAYMENT_TIMEOUT_SECS ?? 45)),
  ),
  sdkConnect: createRealSparkSdkClient,
})

export class SparkWalletManagerConfigService extends Context.Tag(
  "@openagents/desktop/SparkWalletManagerConfigService",
)<SparkWalletManagerConfigService, SparkWalletManagerConfig>() {}

export const SparkWalletManagerConfigLive = (config: SparkWalletManagerConfig) =>
  Layer.succeed(SparkWalletManagerConfigService, config)

const initialSparkWalletStatus = (network: SparkNetwork, apiKeyConfigured: boolean): SparkWalletStatus => ({
  lifecycle: "disconnected",
  network,
  apiKeyConfigured,
  mnemonicStored: false,
  identityPubkey: null,
  balanceSats: null,
  tokenBalanceCount: 0,
  lastSyncedAtMs: null,
  lastPaymentId: null,
  lastPaymentAtMs: null,
  lastErrorCode: null,
  lastErrorMessage: null,
})

const normalizePreimageHex = (value: string | undefined): string | null => {
  if (!value) return null
  const normalized = value.trim().toLowerCase()
  if (!/^[0-9a-f]+$/i.test(normalized)) return null
  if (normalized.length % 2 !== 0) return null
  return normalized
}

const parseBigintToNumber = (value: bigint): number => {
  const maxSafe = BigInt(Number.MAX_SAFE_INTEGER)
  if (value > maxSafe) return Number.MAX_SAFE_INTEGER
  if (value < 0) return 0
  return Number(value)
}

const timestampToMs = (value: number): number =>
  value > 1_000_000_000_000 ? Math.floor(value) : Math.floor(value * 1_000)

const extractPreimageHex = (response: SendPaymentResponse): string | null => {
  const details = response.payment.details
  if (details?.type !== "lightning") return null
  return normalizePreimageHex(details.preimage)
}

const quotedAmountMsats = (prepare: PrepareSendPaymentResponse): number =>
  Math.max(0, parseBigintToNumber(prepare.amount) * 1_000)

export type SparkWalletManagerApi = Readonly<{
  readonly bootstrap: () => Effect.Effect<void, SparkWalletManagerError>
  readonly disconnect: () => Effect.Effect<void, SparkWalletManagerError>
  readonly refresh: () => Effect.Effect<SparkWalletStatus, SparkWalletManagerError>
  readonly snapshot: () => Effect.Effect<SparkWalletStatus>
  readonly payInvoice: (
    request: InvoicePaymentRequest,
  ) => Effect.Effect<InvoicePaymentResult, SparkWalletManagerError>
}>

export class SparkWalletManagerService extends Context.Tag("@openagents/desktop/SparkWalletManagerService")<
  SparkWalletManagerService,
  SparkWalletManagerApi
>() {}

const toMessage = (value: unknown): string => {
  if (value instanceof Error) return value.message
  return String(value)
}

const toManagerError = (
  code: SparkWalletManagerErrorCode,
  message: string,
): SparkWalletManagerError =>
  new SparkWalletManagerError(code, message)

export const SparkWalletManagerLive = Layer.effect(
  SparkWalletManagerService,
  Effect.gen(function* () {
    const config = yield* SparkWalletManagerConfigService
    const secureStorage = yield* DesktopSecureStorageService

    const statusRef = yield* Ref.make<SparkWalletStatus>(
      initialSparkWalletStatus(config.network, Boolean(config.apiKey)),
    )
    const sdkRef = yield* Ref.make<SparkSdkClient | null>(null)
    const mnemonicRef = yield* Ref.make<string | null>(null)

    const setStatus = Effect.fn("SparkWalletManager.setStatus")(function* (
      update: (current: SparkWalletStatus, nowMs: number) => SparkWalletStatus,
    ) {
      const nowMs = yield* Clock.currentTimeMillis
      yield* Ref.update(statusRef, (current) => update(current, nowMs))
    })

    const updateError = (input: {
      readonly code: SparkWalletManagerErrorCode
      readonly message: string
    }) =>
      setStatus((current) => ({
        ...current,
        lifecycle: "error",
        lastErrorCode: input.code,
        lastErrorMessage: input.message,
      })).pipe(
        Effect.zipRight(Effect.fail(toManagerError(input.code, input.message))),
      )

    const clearError = setStatus((current) => ({
      ...current,
      lastErrorCode: null,
      lastErrorMessage: null,
    }))

    const readStoredMnemonic = () =>
      secureStorage.getSecret(config.mnemonicStorageKey).pipe(
        Effect.mapError((error) =>
          toManagerError(
            "secure_storage_error",
            `failed to read spark mnemonic (${error.code}): ${error.message}`,
          )),
      )

    const writeStoredMnemonic = (mnemonic: string) =>
      secureStorage.setSecret(config.mnemonicStorageKey, mnemonic).pipe(
        Effect.mapError((error) =>
          toManagerError(
            "secure_storage_error",
            `failed to persist spark mnemonic (${error.code}): ${error.message}`,
          )),
      )

    const ensureMnemonic = Effect.fn("SparkWalletManager.ensureMnemonic")(function* () {
      const inMemory = yield* Ref.get(mnemonicRef)
      if (inMemory && validateWords(inMemory)) {
        yield* setStatus((current) => ({
          ...current,
          mnemonicStored: true,
        }))
        return inMemory
      }

      const stored = yield* readStoredMnemonic()
      if (stored && validateWords(stored)) {
        yield* Ref.set(mnemonicRef, stored)
        yield* setStatus((current) => ({
          ...current,
          mnemonicStored: true,
        }))
        return stored
      }

      if (stored && !validateWords(stored)) {
        if (!config.autoGenerateMnemonic) {
          return yield* updateError({
            code: "mnemonic_invalid",
            message: "stored spark mnemonic is invalid and auto-generation is disabled",
          })
        }
      }

      if (!config.autoGenerateMnemonic) {
        return yield* updateError({
          code: "mnemonic_missing",
          message: "spark mnemonic is missing and auto-generation is disabled",
        })
      }

      const mnemonic = generateSeedWords()
      if (!validateWords(mnemonic)) {
        return yield* updateError({
          code: "mnemonic_invalid",
          message: "generated spark mnemonic did not pass validation",
        })
      }

      yield* writeStoredMnemonic(mnemonic)
      yield* Ref.set(mnemonicRef, mnemonic)
      yield* setStatus((current) => ({
        ...current,
        mnemonicStored: true,
      }))
      return mnemonic
    })

    const connect = Effect.fn("SparkWalletManager.connect")(function* () {
      const existing = yield* Ref.get(sdkRef)
      if (existing) return existing

      const mnemonic = yield* ensureMnemonic()

      if (!config.apiKey) {
        return yield* updateError({
          code: "api_key_missing",
          message: "spark api key is missing (set OA_DESKTOP_SPARK_API_KEY or OA_DESKTOP_BREEZ_API_KEY)",
        })
      }

      yield* setStatus((current) => ({
        ...current,
        lifecycle: "connecting",
      }))

      const sdk = yield* Effect.tryPromise({
        try: async () =>
          await config.sdkConnect({
            mnemonic,
            passphrase: config.mnemonicPassphrase,
            network: config.network,
            apiKey: config.apiKey,
            storageDir: config.storageDir,
          }),
        catch: (error) =>
          toManagerError(
            "connect_failed",
            `failed to initialize spark sdk: ${toMessage(error)}`,
          ),
      }).pipe(
        Effect.catchAll((error) =>
          updateError({
            code: error.code,
            message: error.message,
          }),
        ),
      )

      yield* Ref.set(sdkRef, sdk)
      yield* clearError
      return sdk
    })

    const refresh = Effect.fn("SparkWalletManager.refresh")(function* () {
      const sdk = yield* Ref.get(sdkRef)
      if (!sdk) {
        const snapshot = yield* Ref.get(statusRef)
        return snapshot
      }

      const info = yield* Effect.tryPromise({
        try: async () => await sdk.getInfo({ ensureSynced: false }),
        catch: (error) =>
          toManagerError(
            "connect_failed",
            `failed to fetch spark wallet info: ${toMessage(error)}`,
          ),
      }).pipe(
        Effect.catchAll((error) =>
          updateError({
            code: error.code,
            message: error.message,
          }),
        ),
      )

      yield* setStatus((current, nowMs) => ({
        ...current,
        lifecycle: "connected",
        mnemonicStored: true,
        identityPubkey: info.identityPubkey,
        balanceSats: Math.max(0, Math.floor(info.balanceSats)),
        tokenBalanceCount: info.tokenBalances.size,
        lastSyncedAtMs: nowMs,
        lastErrorCode: null,
        lastErrorMessage: null,
      }))

      return yield* Ref.get(statusRef)
    })

    const bootstrap = Effect.fn("SparkWalletManager.bootstrap")(function* () {
      yield* ensureMnemonic()
      yield* connect()
      yield* refresh()
    })

    const disconnect = Effect.fn("SparkWalletManager.disconnect")(function* () {
      const sdk = yield* Ref.get(sdkRef)
      if (!sdk) {
        yield* setStatus((current) => ({
          ...current,
          lifecycle: "disconnected",
        }))
        return
      }

      yield* Effect.tryPromise({
        try: async () => await sdk.disconnect(),
        catch: (error) =>
          toManagerError(
            "connect_failed",
            `failed to disconnect spark sdk: ${toMessage(error)}`,
          ),
      }).pipe(
        Effect.catchAll((error) =>
          updateError({
            code: error.code,
            message: error.message,
          }),
        ),
      )

      yield* Ref.set(sdkRef, null)
      yield* setStatus((current) => ({
        ...current,
        lifecycle: "disconnected",
      }))
    })

    const payInvoice = Effect.fn("SparkWalletManager.payInvoice")(function* (
      request: InvoicePaymentRequest,
    ) {
      const sdk = yield* connect()

      const prepare = yield* Effect.tryPromise({
        try: async () =>
          await sdk.prepareSendPayment({
            paymentRequest: request.invoice,
          }),
        catch: (error) =>
          toManagerError(
            "payment_prepare_failed",
            `spark prepare payment failed: ${toMessage(error)}`,
          ),
      }).pipe(
        Effect.catchAll((error) =>
          updateError({
            code: error.code,
            message: error.message,
          }),
        ),
      )

      if (prepare.paymentMethod.type !== "bolt11Invoice") {
        return yield* updateError({
          code: "payment_prepare_failed",
          message: `spark prepare returned unsupported payment method: ${prepare.paymentMethod.type}`,
        })
      }

      const amountMsats = quotedAmountMsats(prepare)
      if (amountMsats > request.maxAmountMsats) {
        return yield* updateError({
          code: "payment_amount_exceeds_budget",
          message: `spark quoted amount ${amountMsats} msats exceeds cap ${request.maxAmountMsats} msats`,
        })
      }

      const send = yield* Effect.tryPromise({
        try: async () =>
          await sdk.sendPayment({
            prepareResponse: prepare,
            options: {
              type: "bolt11Invoice",
              preferSpark: true,
              completionTimeoutSecs: config.paymentCompletionTimeoutSecs,
            },
          }),
        catch: (error) =>
          toManagerError(
            "payment_failed",
            `spark send payment failed: ${toMessage(error)}`,
          ),
      }).pipe(
        Effect.catchAll((error) =>
          updateError({
            code: error.code,
            message: error.message,
          }),
        ),
      )

      if (send.payment.status === "failed") {
        return yield* updateError({
          code: "payment_failed",
          message: `spark payment failed for ${request.host}`,
        })
      }

      if (send.payment.status === "pending") {
        return yield* updateError({
          code: "payment_pending",
          message: "spark payment remained pending after configured completion timeout",
        })
      }

      const preimageHex = extractPreimageHex(send)
      if (!preimageHex) {
        return yield* updateError({
          code: "payment_missing_preimage",
          message: "spark payment completed without lightning preimage",
        })
      }

      const paidAtMs = timestampToMs(send.payment.timestamp)
      const settledAmountMsats = amountMsats > 0
        ? amountMsats
        : Math.max(0, parseBigintToNumber(send.payment.amount) * 1_000)

      const result: InvoicePaymentResult = {
        paymentId: send.payment.id,
        amountMsats: settledAmountMsats,
        preimageHex,
        paidAtMs,
      }

      yield* setStatus((current) => ({
        ...current,
        lifecycle: "connected",
        lastPaymentId: result.paymentId,
        lastPaymentAtMs: result.paidAtMs,
        lastErrorCode: null,
        lastErrorMessage: null,
      }))

      yield* refresh()
      return result
    })

    return SparkWalletManagerService.of({
      bootstrap,
      disconnect,
      refresh,
      snapshot: () => Ref.get(statusRef),
      payInvoice,
    })
  }),
)

export type SparkWalletRendererSnapshot = Readonly<{
  readonly lifecycle: SparkWalletLifecycle
  readonly network: SparkNetwork
  readonly apiKeyConfigured: boolean
  readonly mnemonicStored: boolean
  readonly identityPubkey: string | null
  readonly balanceSats: number | null
  readonly tokenBalanceCount: number
  readonly lastSyncedAtMs: number | null
  readonly lastPaymentId: string | null
  readonly lastPaymentAtMs: number | null
  readonly lastErrorCode: string | null
  readonly lastErrorMessage: string | null
}>

export const projectSparkWalletSnapshotForRenderer = (
  status: SparkWalletStatus,
): SparkWalletRendererSnapshot => ({
  lifecycle: status.lifecycle,
  network: status.network,
  apiKeyConfigured: status.apiKeyConfigured,
  mnemonicStored: status.mnemonicStored,
  identityPubkey: status.identityPubkey,
  balanceSats: status.balanceSats,
  tokenBalanceCount: status.tokenBalanceCount,
  lastSyncedAtMs: status.lastSyncedAtMs,
  lastPaymentId: status.lastPaymentId,
  lastPaymentAtMs: status.lastPaymentAtMs,
  lastErrorCode: status.lastErrorCode,
  lastErrorMessage: status.lastErrorMessage,
})

export const toSparkWalletManagerError = (
  error: unknown,
): SparkWalletManagerError => {
  if (error instanceof SparkWalletManagerError) return error
  if (error instanceof DesktopSecureStorageError) {
    return toManagerError(
      "secure_storage_error",
      `spark secure storage failed (${error.code}): ${error.message}`,
    )
  }
  return toManagerError("connect_failed", toMessage(error))
}
