import type { LndPaymentRecord } from "@openagentsinc/lnd-effect/contracts"
import type { LndPaymentApi } from "@openagentsinc/lnd-effect/services"
import { LndPaymentService } from "@openagentsinc/lnd-effect/services"
import type { LndServiceUnavailableError } from "@openagentsinc/lnd-effect/errors"
import { Effect, Layer } from "effect"

import type { InvoicePaymentRequest, InvoicePaymentResult } from "../contracts/payment.js"
import {
  PaymentFailedError,
  PaymentMissingPreimageError,
  PaymentTimeoutError,
} from "../errors/lightningErrors.js"
import { InvoicePayerService } from "../services/invoicePayer.js"

const defaultInFlightTimeoutMs = 30_000

export type InvoicePayerLndEffectLayerOptions = Readonly<{
  readonly feeLimitSat?: number
  readonly timeoutSeconds?: number
  readonly inFlightTimeoutMs?: number
  readonly fallbackAmountMsats?: "request_max" | "zero"
}>

const normalizePreimageHex = (value: string | undefined): string | undefined => {
  if (!value) return undefined
  const normalized = value.trim().toLowerCase()
  if (!/^[0-9a-f]+$/.test(normalized)) return undefined
  if (normalized.length % 2 !== 0) return undefined
  return normalized
}

const toTimeoutMs = (options?: InvoicePayerLndEffectLayerOptions): number =>
  Math.max(1, Math.floor(options?.inFlightTimeoutMs ?? defaultInFlightTimeoutMs))

const toFeeLimitSat = (
  request: InvoicePaymentRequest,
  options?: InvoicePayerLndEffectLayerOptions,
): number => {
  if (options?.feeLimitSat !== undefined) {
    return Math.max(0, Math.floor(options.feeLimitSat))
  }
  return Math.max(0, Math.ceil(request.maxAmountMsats / 1_000))
}

const toAmountMsats = (
  request: InvoicePaymentRequest,
  payment: LndPaymentRecord,
  options?: InvoicePayerLndEffectLayerOptions,
): number => {
  if (payment.amountSat > 0) return Math.max(0, Math.floor(payment.amountSat * 1_000))
  if (options?.fallbackAmountMsats === "zero") return 0
  return request.maxAmountMsats
}

const toUnavailablePaymentFailed = (
  request: InvoicePaymentRequest,
  error: LndServiceUnavailableError,
) =>
  PaymentFailedError.make({
    invoice: request.invoice,
    reason: `lnd_service_unavailable:${error.service}:${error.reason}`,
  })

const resolveFinalPayment = Effect.fn("InvoicePayerLndEffect.resolveFinalPayment")(function* (
  request: InvoicePaymentRequest,
  payment: LndPaymentRecord,
  lndPayment: LndPaymentApi,
  options?: InvoicePayerLndEffectLayerOptions,
) {
  if (payment.status !== "in_flight") {
    return payment
  }

  const tracked = yield* lndPayment.trackPayment({ paymentHash: payment.paymentHash }).pipe(
    Effect.mapError((error) => toUnavailablePaymentFailed(request, error)),
  )

  if (tracked.status === "in_flight") {
    return yield* PaymentTimeoutError.make({
      invoice: request.invoice,
      timeoutMs: toTimeoutMs(options),
    })
  }

  return tracked
})

const toPaymentResult = Effect.fn("InvoicePayerLndEffect.toPaymentResult")(function* (
  request: InvoicePaymentRequest,
  payment: LndPaymentRecord,
  options?: InvoicePayerLndEffectLayerOptions,
) {
  if (payment.status === "failed") {
    return yield* PaymentFailedError.make({
      invoice: request.invoice,
      reason: payment.failureReason ?? "lnd_payment_failed",
    })
  }

  const preimageHex = normalizePreimageHex(payment.paymentPreimageHex)
  if (!preimageHex) {
    return yield* PaymentMissingPreimageError.make({
      invoice: request.invoice,
      paymentId: payment.paymentHash,
    })
  }

  const result: InvoicePaymentResult = {
    paymentId: payment.paymentHash,
    amountMsats: toAmountMsats(request, payment, options),
    preimageHex,
    paidAtMs: Math.max(0, Math.floor(payment.updatedAtMs)),
  }

  return result
})

export const makeInvoicePayerLndEffectLayer = (
  options?: InvoicePayerLndEffectLayerOptions,
): Layer.Layer<InvoicePayerService, never, LndPaymentService> =>
  Layer.effect(
    InvoicePayerService,
    Effect.gen(function* () {
      const lndPayment = yield* LndPaymentService

      return InvoicePayerService.of({
        payInvoice: (request: InvoicePaymentRequest) =>
          Effect.gen(function* () {
            const payment = yield* lndPayment
              .sendPayment({
                paymentRequest: request.invoice,
                feeLimitSat: toFeeLimitSat(request, options),
                ...(options?.timeoutSeconds !== undefined
                  ? { timeoutSeconds: Math.max(1, Math.floor(options.timeoutSeconds)) }
                  : {}),
              })
              .pipe(Effect.mapError((error) => toUnavailablePaymentFailed(request, error)))

            const finalPayment = yield* resolveFinalPayment(
              request,
              payment,
              lndPayment,
              options,
            )

            return yield* toPaymentResult(request, finalPayment, options)
          }),
      })
    }),
  )

export const InvoicePayerLndEffectLayer = makeInvoicePayerLndEffectLayer()
