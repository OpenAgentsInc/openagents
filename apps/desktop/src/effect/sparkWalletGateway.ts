import { Context, Effect, Layer } from "effect"

import {
  PaymentFailedError,
  PaymentMissingPreimageError,
} from "@openagentsinc/lightning-effect"
import type { InvoicePaymentRequest, InvoicePaymentResult } from "@openagentsinc/lightning-effect/contracts"

export type DesktopSparkWalletLifecycle = "disconnected" | "connecting" | "connected" | "error"
export type DesktopSparkWalletNetwork = "mainnet" | "regtest"

export type DesktopSparkWalletStatus = Readonly<{
  readonly lifecycle: DesktopSparkWalletLifecycle
  readonly network: DesktopSparkWalletNetwork
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

const fallbackStatus = (): DesktopSparkWalletStatus => ({
  lifecycle: "disconnected",
  network: "regtest",
  apiKeyConfigured: false,
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

const normalizeLifecycle = (value: unknown): DesktopSparkWalletLifecycle => {
  if (
    value === "disconnected" ||
    value === "connecting" ||
    value === "connected" ||
    value === "error"
  ) {
    return value
  }
  return "disconnected"
}

const normalizeNetwork = (value: unknown): DesktopSparkWalletNetwork =>
  value === "mainnet" ? "mainnet" : "regtest"

const normalizeSnapshot = (value: unknown): DesktopSparkWalletStatus => {
  if (!value || typeof value !== "object") return fallbackStatus()
  const record = value as Record<string, unknown>

  return {
    lifecycle: normalizeLifecycle(record.lifecycle),
    network: normalizeNetwork(record.network),
    apiKeyConfigured: record.apiKeyConfigured === true,
    mnemonicStored: record.mnemonicStored === true,
    identityPubkey: typeof record.identityPubkey === "string" ? record.identityPubkey : null,
    balanceSats: typeof record.balanceSats === "number" && Number.isFinite(record.balanceSats)
      ? record.balanceSats
      : null,
    tokenBalanceCount: typeof record.tokenBalanceCount === "number" && Number.isFinite(record.tokenBalanceCount)
      ? Math.max(0, Math.floor(record.tokenBalanceCount))
      : 0,
    lastSyncedAtMs: typeof record.lastSyncedAtMs === "number" && Number.isFinite(record.lastSyncedAtMs)
      ? record.lastSyncedAtMs
      : null,
    lastPaymentId: typeof record.lastPaymentId === "string" ? record.lastPaymentId : null,
    lastPaymentAtMs: typeof record.lastPaymentAtMs === "number" && Number.isFinite(record.lastPaymentAtMs)
      ? record.lastPaymentAtMs
      : null,
    lastErrorCode: typeof record.lastErrorCode === "string" ? record.lastErrorCode : null,
    lastErrorMessage: typeof record.lastErrorMessage === "string" ? record.lastErrorMessage : null,
  }
}

const normalizePaymentResult = (
  input: InvoicePaymentRequest,
  value: unknown,
): Effect.Effect<InvoicePaymentResult, PaymentFailedError | PaymentMissingPreimageError> => {
  if (!value || typeof value !== "object") {
    return Effect.fail(
      PaymentFailedError.make({
        invoice: input.invoice,
        reason: "spark_gateway_invalid_payment_result",
      }),
    )
  }
  const record = value as Record<string, unknown>
  const paymentId = typeof record.paymentId === "string" && record.paymentId.trim().length > 0
    ? record.paymentId.trim()
    : null
  const amountMsats = typeof record.amountMsats === "number" && Number.isFinite(record.amountMsats)
    ? Math.max(0, Math.floor(record.amountMsats))
    : null
  const preimageHex = typeof record.preimageHex === "string" && /^[0-9a-f]+$/i.test(record.preimageHex)
    ? record.preimageHex.trim().toLowerCase()
    : null
  const paidAtMs = typeof record.paidAtMs === "number" && Number.isFinite(record.paidAtMs)
    ? Math.max(0, Math.floor(record.paidAtMs))
    : Date.now()

  if (!paymentId || amountMsats === null) {
    return Effect.fail(
      PaymentFailedError.make({
        invoice: input.invoice,
        reason: "spark_gateway_missing_payment_fields",
      }),
    )
  }
  if (!preimageHex) {
    return Effect.fail(
      PaymentMissingPreimageError.make({
        invoice: input.invoice,
        paymentId,
      }),
    )
  }

  return Effect.succeed({
    paymentId,
    amountMsats,
    preimageHex,
    paidAtMs,
  })
}

const sparkBridge = () => {
  if (typeof window === "undefined") return undefined
  return window.openAgentsDesktop?.sparkWallet
}

export type SparkWalletGatewayApi = Readonly<{
  readonly snapshot: () => Effect.Effect<DesktopSparkWalletStatus>
  readonly bootstrap: () => Effect.Effect<void>
  readonly refresh: () => Effect.Effect<DesktopSparkWalletStatus>
  readonly payInvoice: (
    request: InvoicePaymentRequest,
  ) => Effect.Effect<InvoicePaymentResult, PaymentFailedError | PaymentMissingPreimageError>
}>

export class SparkWalletGatewayService extends Context.Tag("@openagents/desktop/SparkWalletGatewayService")<
  SparkWalletGatewayService,
  SparkWalletGatewayApi
>() {}

export const SparkWalletGatewayLive = Layer.succeed(
  SparkWalletGatewayService,
  SparkWalletGatewayService.of({
    snapshot: () =>
      Effect.promise(async () => {
        const bridge = sparkBridge()
        if (!bridge) return fallbackStatus()
        return normalizeSnapshot(await bridge.snapshot())
      }).pipe(Effect.catchAll(() => Effect.succeed(fallbackStatus()))),

    bootstrap: () =>
      Effect.promise(async () => {
        await sparkBridge()?.bootstrap()
      }).pipe(Effect.catchAll(() => Effect.void)),

    refresh: () =>
      Effect.promise(async () => {
        const bridge = sparkBridge()
        if (!bridge) return fallbackStatus()
        return normalizeSnapshot(await bridge.refresh())
      }).pipe(Effect.catchAll(() => Effect.succeed(fallbackStatus()))),

    payInvoice: (request) =>
      Effect.gen(function* () {
        const bridge = sparkBridge()
        if (!bridge) {
          return yield* PaymentFailedError.make({
            invoice: request.invoice,
            reason: "spark_bridge_unavailable",
          })
        }
        const raw = yield* Effect.tryPromise({
          try: async () => await bridge.payInvoice(request),
          catch: (error) =>
            PaymentFailedError.make({
              invoice: request.invoice,
              reason: `spark_gateway_call_failed:${String(error)}`,
            }),
        })
        return yield* normalizePaymentResult(request, raw)
      }),
  }),
)
