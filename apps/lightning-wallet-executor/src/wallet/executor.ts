import { Clock, Context, Effect, Layer, Ref } from "effect"
import crypto from "node:crypto"

import type {
  InvoicePaymentRequest,
  InvoicePaymentResult,
  WalletExecutionReceipt,
  WalletStatus,
} from "../contracts.js"
import { PolicyDeniedError, SparkGatewayError } from "../errors.js"
import { buildWalletExecutionReceipt } from "../receipt/canonical.js"
import { WalletExecutorConfigService } from "../runtime/config.js"
import { SparkGatewayService } from "../spark/gateway.js"

export type PayBolt11Result = Readonly<{
  payment: InvoicePaymentResult
  quotedAmountMsats: number
  windowSpendMsatsAfterPayment: number
  receipt: WalletExecutionReceipt
}>

export type WalletExecutorApi = Readonly<{
  bootstrap: () => Effect.Effect<void, SparkGatewayError>
  status: () => Effect.Effect<WalletStatus>
  payBolt11: (
    request: InvoicePaymentRequest,
    options?: { readonly requestId?: string },
  ) => Effect.Effect<PayBolt11Result, PolicyDeniedError | SparkGatewayError>
}>

export class WalletExecutorService extends Context.Tag(
  "@openagents/lightning-wallet-executor/WalletExecutorService",
)<WalletExecutorService, WalletExecutorApi>() {}

type PaymentHistoryItem = Readonly<{
  paymentId: string
  amountMsats: number
  paidAtMs: number
  host: string
}>

type StatusState = Readonly<{
  lifecycle: WalletStatus["lifecycle"]
  identityPubkey: string | null
  balanceSats: number | null
  recentPaymentsCount: number
  lastPaymentId: string | null
  lastPaymentAtMs: number | null
  lastErrorCode: string | null
  lastErrorMessage: string | null
  updatedAtMs: number
}>

const sanitizeHost = (host: string): string => host.trim().toLowerCase()
const resolveRequestId = (requestId?: string): string => requestId?.trim() || crypto.randomUUID()
const hashInvoice = (invoice: string): string => crypto.createHash("sha256").update(invoice).digest("hex")

const pruneWindow = (
  rows: ReadonlyArray<PaymentHistoryItem>,
  nowMs: number,
  windowMs: number,
): Array<PaymentHistoryItem> => rows.filter((row) => nowMs - row.paidAtMs <= windowMs)

const sumMsats = (rows: ReadonlyArray<PaymentHistoryItem>): number =>
  rows.reduce((total, row) => total + row.amountMsats, 0)

const structuredLog = (event: string, payload: Record<string, unknown>) => {
  console.log(
    JSON.stringify({
      ts: new Date().toISOString(),
      service: "lightning-wallet-executor",
      event,
      ...payload,
    }),
  )
}

export const WalletExecutorLive = Layer.effect(
  WalletExecutorService,
  Effect.gen(function* () {
    const config = yield* WalletExecutorConfigService
    const spark = yield* SparkGatewayService

    const statusRef = yield* Ref.make<StatusState>({
      lifecycle: "disconnected",
      identityPubkey: null,
      balanceSats: null,
      recentPaymentsCount: 0,
      lastPaymentId: null,
      lastPaymentAtMs: null,
      lastErrorCode: null,
      lastErrorMessage: null,
      updatedAtMs: 0,
    })

    const historyRef = yield* Ref.make<Array<PaymentHistoryItem>>([])

    const setStatus = (
      update: (current: StatusState, nowMs: number) => StatusState,
    ): Effect.Effect<void> =>
      Effect.gen(function* () {
        const nowMs = yield* Clock.currentTimeMillis
        yield* Ref.update(statusRef, (current) => update(current, nowMs))
      })

    const refreshStatusFromSparkBestEffort = (): Effect.Effect<void> =>
      refreshFromSpark().pipe(
        Effect.catchTag("SparkGatewayError", (error) =>
          setStatus((current, nowMs) => ({
            ...current,
            lifecycle: current.lifecycle === "connected" ? "connected" : "error",
            lastErrorCode: error.code,
            lastErrorMessage: error.message,
            updatedAtMs: nowMs,
          })),
        ),
      )

    const status = (): Effect.Effect<WalletStatus> =>
      Effect.gen(function* () {
        // Keep balance/status live for UI by refreshing from Spark on each status read.
        yield* refreshStatusFromSparkBestEffort()

        const current = yield* Ref.get(statusRef)
        return {
          walletId: config.walletId,
          mode: config.mode,
          authMode: config.authToken ? "bearer_static" : "disabled",
          authEnforced: Boolean(config.authToken),
          authTokenVersion: config.authTokenVersion,
          lifecycle: current.lifecycle,
          network: config.network,
          identityPubkey: current.identityPubkey,
          balanceSats: current.balanceSats,
          apiKeyConfigured: Boolean(config.sparkApiKey),
          ready: current.lifecycle === "connected",
          allowedHostCount: config.allowedHosts.size,
          requestCapMsats: config.requestCapMsats,
          windowCapMsats: config.windowCapMsats,
          windowMs: config.windowMs,
          recentPaymentsCount: current.recentPaymentsCount,
          lastPaymentId: current.lastPaymentId,
          lastPaymentAtMs: current.lastPaymentAtMs,
          lastErrorCode: current.lastErrorCode,
          lastErrorMessage: current.lastErrorMessage,
          updatedAtMs: current.updatedAtMs,
        } satisfies WalletStatus
      })

    const refreshFromSpark = (): Effect.Effect<void, SparkGatewayError> =>
      Effect.gen(function* () {
        const info = yield* spark.getInfo()

        yield* setStatus((current, nowMs) => ({
          ...current,
          lifecycle: "connected",
          identityPubkey: info.identityPubkey,
          balanceSats: info.balanceSats,
          lastErrorCode: null,
          lastErrorMessage: null,
          updatedAtMs: nowMs,
        }))
      })

    const bootstrap = (): Effect.Effect<void, SparkGatewayError> =>
      Effect.gen(function* () {
        yield* setStatus((current, nowMs) => ({
          ...current,
          lifecycle: "connecting",
          updatedAtMs: nowMs,
        }))

        const connected = yield* Effect.either(spark.connect())
        if (connected._tag === "Left") {
          const error = connected.left
          yield* setStatus((current, nowMs) => ({
            ...current,
            lifecycle: "error",
            lastErrorCode: error.code,
            lastErrorMessage: error.message,
            updatedAtMs: nowMs,
          }))
          return yield* Effect.fail(error)
        }

        return yield* refreshFromSpark().pipe(
          Effect.catchTag("SparkGatewayError", (error) =>
            setStatus((current, nowMs) => ({
              ...current,
              lifecycle: "error",
              lastErrorCode: error.code,
              lastErrorMessage: error.message,
              updatedAtMs: nowMs,
            })).pipe(Effect.zipRight(Effect.fail(error))),
          ),
        )
      })

    const ensureHostAllowed = (host: string): Effect.Effect<void, PolicyDeniedError> => {
      const normalized = sanitizeHost(host)
      if (config.allowedHosts.size === 0 || config.allowedHosts.has(normalized)) {
        return Effect.void
      }
      return PolicyDeniedError.make({
        code: "host_not_allowed",
        message: `host ${normalized} is not in the wallet executor allowlist`,
        host: normalized,
      })
    }

    const ensureRequestCapAllowed = (request: InvoicePaymentRequest): Effect.Effect<void, PolicyDeniedError> => {
      if (request.maxAmountMsats <= config.requestCapMsats) {
        return Effect.void
      }
      return PolicyDeniedError.make({
        code: "request_cap_exceeded",
        message: `request max amount ${request.maxAmountMsats} msats exceeds service cap ${config.requestCapMsats} msats`,
        host: sanitizeHost(request.host),
        maxAllowedMsats: config.requestCapMsats,
        quotedAmountMsats: request.maxAmountMsats,
      })
    }

    const enforceWindowCap = (host: string, quotedAmountMsats: number): Effect.Effect<number, PolicyDeniedError> =>
      Effect.gen(function* () {
        const nowMs = yield* Clock.currentTimeMillis
        const pruned = yield* Ref.modify(historyRef, (rows) => {
          const next = pruneWindow(rows, nowMs, config.windowMs)
          return [next, next] as const
        })
        const currentWindowSpend = sumMsats(pruned)

        if (currentWindowSpend + quotedAmountMsats > config.windowCapMsats) {
          return yield* PolicyDeniedError.make({
            code: "window_cap_exceeded",
            message: `window cap exceeded: ${currentWindowSpend + quotedAmountMsats} > ${config.windowCapMsats} msats`,
            host: sanitizeHost(host),
            quotedAmountMsats,
            windowSpendMsats: currentWindowSpend,
            windowCapMsats: config.windowCapMsats,
          })
        }

        return currentWindowSpend
      })

    const payBolt11 = (
      request: InvoicePaymentRequest,
      options?: { readonly requestId?: string },
    ): Effect.Effect<PayBolt11Result, PolicyDeniedError | SparkGatewayError> =>
      Effect.gen(function* () {
        const requestId = resolveRequestId(options?.requestId)
        const host = sanitizeHost(request.host)
        const invoiceHash = hashInvoice(request.invoice)

        yield* bootstrap()
        yield* ensureHostAllowed(host)
        yield* ensureRequestCapAllowed(request)

        structuredLog("payment.prepare_start", {
          requestId,
          walletId: config.walletId,
          host,
          invoiceHash,
          requestMaxMsats: request.maxAmountMsats,
        })

        const prepared = yield* spark.preparePayment(request.invoice)

        if (prepared.paymentMethodType !== "bolt11Invoice") {
          return yield* SparkGatewayError.make({
            code: "unsupported_payment_method",
            message: `unsupported payment method: ${prepared.paymentMethodType}`,
          })
        }

        if (prepared.amountMsats > request.maxAmountMsats || prepared.amountMsats > config.requestCapMsats) {
          return yield* PolicyDeniedError.make({
            code: "quoted_amount_exceeds_cap",
            message: `quoted amount ${prepared.amountMsats} msats exceeds cap`,
            host,
            maxAllowedMsats: Math.min(request.maxAmountMsats, config.requestCapMsats),
            quotedAmountMsats: prepared.amountMsats,
          })
        }

        const currentWindowSpend = yield* enforceWindowCap(host, prepared.amountMsats)

        const sent = yield* spark.sendPayment(prepared)

        if (sent.status === "pending") {
          return yield* SparkGatewayError.make({
            code: "payment_pending",
            message: "payment did not complete before timeout",
          })
        }

        if (sent.status === "failed") {
          return yield* SparkGatewayError.make({
            code: "payment_failed",
            message: "payment failed",
          })
        }

        if (!sent.preimageHex) {
          return yield* SparkGatewayError.make({
            code: "payment_missing_preimage",
            message: "payment completed without a preimage",
          })
        }

        const paidAmountMsats = sent.amountMsats > 0 ? sent.amountMsats : prepared.amountMsats

        yield* Ref.update(historyRef, (rows) => {
          const nextRows = pruneWindow(rows, sent.paidAtMs, config.windowMs)
          nextRows.push({
            paymentId: sent.paymentId,
            amountMsats: paidAmountMsats,
            paidAtMs: sent.paidAtMs,
            host,
          })
          return nextRows
        })

        const history = yield* Ref.get(historyRef)
        const windowSpendAfter = sumMsats(history)
        const receipt = buildWalletExecutionReceipt({
          requestId,
          walletId: config.walletId,
          host,
          paymentId: sent.paymentId,
          invoiceHash,
          quotedAmountMsats: prepared.amountMsats,
          settledAmountMsats: paidAmountMsats,
          preimageHex: sent.preimageHex,
          paidAtMs: sent.paidAtMs,
        })

        yield* refreshFromSpark().pipe(Effect.catchTag("SparkGatewayError", () => Effect.void))

        yield* setStatus((current, nowMs) => ({
          ...current,
          lifecycle: "connected",
          recentPaymentsCount: history.length,
          lastPaymentId: sent.paymentId,
          lastPaymentAtMs: sent.paidAtMs,
          lastErrorCode: null,
          lastErrorMessage: null,
          updatedAtMs: nowMs,
        }))

        structuredLog("payment.sent", {
          requestId,
          walletId: config.walletId,
          host,
          invoiceHash,
          paymentId: sent.paymentId,
          amountMsats: paidAmountMsats,
          quotedAmountMsats: prepared.amountMsats,
          windowSpendMsats: currentWindowSpend,
          windowSpendMsatsAfterPayment: windowSpendAfter,
          receiptId: receipt.receiptId,
          receiptHash: receipt.canonicalJsonSha256,
          outcome: "paid",
        })

        return {
          payment: {
            paymentId: sent.paymentId,
            amountMsats: paidAmountMsats,
            preimageHex: sent.preimageHex,
            paidAtMs: sent.paidAtMs,
          },
          quotedAmountMsats: prepared.amountMsats,
          windowSpendMsatsAfterPayment: windowSpendAfter,
          receipt,
        } satisfies PayBolt11Result
      }).pipe(
        Effect.tapError((error) =>
          Effect.sync(() => {
            if (error._tag === "PolicyDeniedError") {
              structuredLog("payment.policy_denied", {
                walletId: config.walletId,
                host: sanitizeHost(request.host),
                requestMaxMsats: request.maxAmountMsats,
                code: error.code,
                message: error.message,
              })
            } else {
              structuredLog("payment.spark_failed", {
                walletId: config.walletId,
                host: sanitizeHost(request.host),
                requestMaxMsats: request.maxAmountMsats,
                code: error.code,
                message: error.message,
              })
            }
          }),
        ),
      )

    return WalletExecutorService.of({
      bootstrap,
      status,
      payBolt11,
    })
  }),
)
