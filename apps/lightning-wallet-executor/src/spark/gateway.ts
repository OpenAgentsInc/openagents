import SparkSdk from "@breeztech/breez-sdk-spark/nodejs"
import { Context, Effect, Layer, Ref } from "effect"

import { SparkGatewayError } from "../errors.js"
import { WalletExecutorConfigService } from "../runtime/config.js"
import { MnemonicSecretProviderService } from "../runtime/secrets.js"

export type SparkWalletInfo = Readonly<{
  identityPubkey: string | null
  balanceSats: number | null
  tokenBalanceCount: number
}>

export type SparkPreparedPayment = Readonly<{
  amountMsats: number
  paymentMethodType: string
  prepareResponse: unknown
}>

export type SparkSentPayment = Readonly<{
  paymentId: string
  status: "completed" | "pending" | "failed"
  amountMsats: number
  preimageHex: string | null
  paidAtMs: number
}>

export type SparkGatewayApi = Readonly<{
  connect: () => Effect.Effect<void, SparkGatewayError>
  disconnect: () => Effect.Effect<void>
  getInfo: () => Effect.Effect<SparkWalletInfo, SparkGatewayError>
  preparePayment: (invoice: string) => Effect.Effect<SparkPreparedPayment, SparkGatewayError>
  sendPayment: (prepared: SparkPreparedPayment) => Effect.Effect<SparkSentPayment, SparkGatewayError>
}>

export class SparkGatewayService extends Context.Tag(
  "@openagents/lightning-wallet-executor/SparkGatewayService",
)<SparkGatewayService, SparkGatewayApi>() {}

type SparkSdkClient = {
  disconnect: () => Promise<void>
  getInfo: (input: { ensureSynced: boolean }) => Promise<{
    identityPubkey: string
    balanceSats: number
    tokenBalances: Map<string, unknown>
  }>
  prepareSendPayment: (input: { paymentRequest: string }) => Promise<{
    amount: bigint
    paymentMethod: {
      type: string
    }
  }>
  sendPayment: (input: {
    prepareResponse: unknown
    options: {
      type: "bolt11Invoice"
      preferSpark: boolean
      completionTimeoutSecs: number
    }
  }) => Promise<{
    payment: {
      id: string
      status: string
      amount: bigint
      timestamp: number
      details?: {
        type: string
        preimage?: string
      }
    }
  }>
}

const parseBigintToNumber = (value: bigint): number => {
  const maxSafe = BigInt(Number.MAX_SAFE_INTEGER)
  if (value > maxSafe) return Number.MAX_SAFE_INTEGER
  if (value < 0) return 0
  return Number(value)
}

const normalizePreimageHex = (value: string | undefined): string | null => {
  if (!value) return null
  const normalized = value.trim().toLowerCase()
  if (!/^[0-9a-f]+$/i.test(normalized)) return null
  if (normalized.length % 2 !== 0) return null
  return normalized
}

const toPaidAtMs = (timestampSecs: number): number =>
  timestampSecs > 1_000_000_000_000 ? Math.floor(timestampSecs) : Math.floor(timestampSecs * 1_000)

const toMessage = (value: unknown): string => {
  if (value instanceof Error) return value.message
  return String(value)
}

const quotedMsatsFromPrepare = (prepared: { amount: bigint }): number =>
  Math.max(0, parseBigintToNumber(prepared.amount) * 1_000)

const createLiveSparkGatewayLayer = Layer.effect(
  SparkGatewayService,
  Effect.gen(function* () {
    const config = yield* WalletExecutorConfigService
    const secrets = yield* MnemonicSecretProviderService

    const sdkRef = yield* Ref.make<SparkSdkClient | null>(null)

    const connect = () =>
      Effect.gen(function* () {
        const existing = yield* Ref.get(sdkRef)
        if (existing) return

        const apiKey = config.sparkApiKey?.trim() ?? ""
        if (!apiKey) {
          return yield* SparkGatewayError.make({
            code: "api_key_missing",
            message: "spark api key is missing",
          })
        }

        const mnemonic = yield* secrets.loadMnemonic().pipe(
          Effect.mapError((error) =>
            SparkGatewayError.make({
              code: "mnemonic_missing",
              message: error.message,
            }),
          ),
        )

        const sparkConfig = SparkSdk.defaultConfig(config.network)
        sparkConfig.apiKey = apiKey

        const seed = { type: "mnemonic" as const, mnemonic }

        const sdk = yield* Effect.tryPromise({
          try: async () => {
            let builder = SparkSdk.SdkBuilder.new(sparkConfig, seed)
            builder = await builder.withDefaultStorage(`./output/spark-wallet-executor/${config.walletId}`)
            return (await builder.build()) as SparkSdkClient
          },
          catch: (error) =>
            SparkGatewayError.make({
              code: "connect_failed",
              message: `failed to build spark sdk: ${toMessage(error)}`,
            }),
        })

        yield* Ref.set(sdkRef, sdk)
      })

    const disconnect = () =>
      Effect.gen(function* () {
        const sdk = yield* Ref.get(sdkRef)
        if (!sdk) return
        yield* Effect.tryPromise({
          try: async () => await sdk.disconnect(),
          catch: (error) => new Error(String(error)),
        }).pipe(Effect.catchAll(() => Effect.void))
        yield* Ref.set(sdkRef, null)
      })

    const requireClient = () =>
      Effect.gen(function* () {
        yield* connect()
        const sdk = yield* Ref.get(sdkRef)
        if (!sdk) {
          return yield* SparkGatewayError.make({
            code: "connect_failed",
            message: "spark sdk client unavailable after connect",
          })
        }
        return sdk
      })

    const getInfo = () =>
      Effect.gen(function* () {
        const sdk = yield* requireClient()
        const info = yield* Effect.tryPromise({
          try: async () => await sdk.getInfo({ ensureSynced: false }),
          catch: (error) =>
            SparkGatewayError.make({
              code: "connect_failed",
              message: `spark getInfo failed: ${toMessage(error)}`,
            }),
        })

        return {
          identityPubkey: info.identityPubkey,
          balanceSats: Math.max(0, Math.floor(info.balanceSats)),
          tokenBalanceCount: info.tokenBalances.size,
        } satisfies SparkWalletInfo
      })

    const preparePayment = (invoice: string) =>
      Effect.gen(function* () {
        const sdk = yield* requireClient()

        const prepared = yield* Effect.tryPromise({
          try: async () =>
            await sdk.prepareSendPayment({
              paymentRequest: invoice,
            }),
          catch: (error) =>
            SparkGatewayError.make({
              code: "prepare_failed",
              message: `spark prepare payment failed: ${toMessage(error)}`,
            }),
        })

        return {
          amountMsats: quotedMsatsFromPrepare(prepared),
          paymentMethodType: prepared.paymentMethod.type,
          prepareResponse: prepared,
        } satisfies SparkPreparedPayment
      })

    const sendPayment = (prepared: SparkPreparedPayment) =>
      Effect.gen(function* () {
        const sdk = yield* requireClient()

        const send = yield* Effect.tryPromise({
          try: async () =>
            await sdk.sendPayment({
              prepareResponse: prepared.prepareResponse,
              options: {
                type: "bolt11Invoice",
                preferSpark: true,
                completionTimeoutSecs: config.paymentTimeoutSecs,
              },
            }),
          catch: (error) =>
            SparkGatewayError.make({
              code: "send_failed",
              message: `spark send payment failed: ${toMessage(error)}`,
            }),
        })

        const statusRaw = send.payment.status
        const status: "completed" | "pending" | "failed" =
          statusRaw === "failed" ? "failed" : statusRaw === "pending" ? "pending" : "completed"
        const paymentId = send.payment.id
        const preimageHex = normalizePreimageHex(
          send.payment.details?.type === "lightning" ? send.payment.details.preimage : undefined,
        )
        const amountMsats = Math.max(0, parseBigintToNumber(send.payment.amount) * 1_000)

        return {
          paymentId,
          status,
          amountMsats,
          preimageHex,
          paidAtMs: toPaidAtMs(send.payment.timestamp),
        } satisfies SparkSentPayment
      })

    return SparkGatewayService.of({
      connect,
      disconnect,
      getInfo,
      preparePayment,
      sendPayment,
    })
  }),
)

export const SparkGatewayLive = createLiveSparkGatewayLayer

export type MockSparkGatewayConfig = Readonly<{
  initialBalanceSats?: number
  quotedAmountMsats?: number
  failPrepare?: boolean
  failSend?: boolean
  pendingOnSend?: boolean
  missingPreimage?: boolean
}>

export const makeSparkGatewayMockLayer = (input?: MockSparkGatewayConfig) =>
  Layer.effect(
    SparkGatewayService,
    Effect.gen(function* () {
      const stateRef = yield* Ref.make({
        identityPubkey: "spark-mock-identity",
        balanceSats: input?.initialBalanceSats ?? 50_000,
        nextPaymentId: 1,
      })

      const quotedAmountMsats = Math.max(1_000, input?.quotedAmountMsats ?? 50_000)

      return SparkGatewayService.of({
        connect: () => Effect.void,
        disconnect: () => Effect.sync(() => undefined),
        getInfo: () =>
          Effect.gen(function* () {
            const state = yield* Ref.get(stateRef)
            return {
              identityPubkey: state.identityPubkey,
              balanceSats: state.balanceSats,
              tokenBalanceCount: 0,
            } satisfies SparkWalletInfo
          }),
        preparePayment: () =>
          input?.failPrepare
            ? SparkGatewayError.make({
                code: "prepare_failed",
                message: "mock prepare failure",
              })
            : Effect.succeed({
                amountMsats: quotedAmountMsats,
                paymentMethodType: "bolt11Invoice",
                prepareResponse: {
                  amount: BigInt(Math.floor(quotedAmountMsats / 1_000)),
                  paymentMethod: { type: "bolt11Invoice" },
                },
              } satisfies SparkPreparedPayment),
        sendPayment: () =>
          input?.failSend
            ? SparkGatewayError.make({
                code: "send_failed",
                message: "mock send failure",
              })
            : Effect.gen(function* () {
                const state = yield* Ref.get(stateRef)
                const paymentId = `mock-pay-${state.nextPaymentId}`
                const nextBalance = Math.max(0, state.balanceSats - Math.floor(quotedAmountMsats / 1_000))

                yield* Ref.set(stateRef, {
                  ...state,
                  nextPaymentId: state.nextPaymentId + 1,
                  balanceSats: nextBalance,
                })

                if (input?.pendingOnSend) {
                  return {
                    paymentId,
                    status: "pending" as const,
                    amountMsats: quotedAmountMsats,
                    preimageHex: null,
                    paidAtMs: Date.now(),
                  } satisfies SparkSentPayment
                }

                return {
                  paymentId,
                  status: "completed" as const,
                  amountMsats: quotedAmountMsats,
                  preimageHex: input?.missingPreimage ? null : "ab".repeat(32),
                  paidAtMs: Date.now(),
                } satisfies SparkSentPayment
              }),
      })
    }),
  )
