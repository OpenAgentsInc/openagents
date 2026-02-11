import { Effect, Layer } from "effect"

import type { InvoicePaymentRequest, InvoicePaymentResult } from "../contracts/payment.js"
import {
  PaymentFailedError,
  PaymentMissingPreimageError,
  PaymentTimeoutError,
} from "../errors/lightningErrors.js"
import { InvoicePayerService } from "../services/invoicePayer.js"

const toDeterministicHex64 = (seed: string): string => {
  let acc = 0
  for (let i = 0; i < seed.length; i += 1) {
    acc = (acc + seed.charCodeAt(i) * (i + 17)) % 0xffffffff
  }

  const hexChunks: Array<string> = []
  for (let i = 0; i < 8; i += 1) {
    const part = ((acc + i * 2654435761) >>> 0).toString(16).padStart(8, "0")
    hexChunks.push(part)
  }

  return hexChunks.join("").slice(0, 64)
}

const toPaymentId = (seed: string): string => `pay_${toDeterministicHex64(seed).slice(0, 24)}`

const defaultDemoPaidAtMs = 1_700_000_000_000

export type InvoicePayerDemoFailureMode = "payment_failed" | "timeout" | "missing_preimage"

export const makeInvoicePayerDemoLayer = (options?: {
  readonly fixedAmountMsats?: number
  readonly fixedPaidAtMs?: number
  readonly failureMode?: InvoicePayerDemoFailureMode
  readonly failForInvoiceIncludes?: string
  readonly timeoutMs?: number
}) =>
  Layer.succeed(
    InvoicePayerService,
    InvoicePayerService.of({
      payInvoice: (request: InvoicePaymentRequest) =>
        Effect.gen(function* () {
          const shouldFail =
            options?.failureMode !== undefined &&
            (options.failForInvoiceIncludes
              ? request.invoice.includes(options.failForInvoiceIncludes)
              : true)

          if (shouldFail) {
            switch (options.failureMode) {
              case "payment_failed":
                return yield* PaymentFailedError.make({
                  invoice: request.invoice,
                  reason: "demo_payment_failed",
                })
              case "timeout":
                return yield* PaymentTimeoutError.make({
                  invoice: request.invoice,
                  timeoutMs: Math.max(1, Math.floor(options.timeoutMs ?? 5_000)),
                })
              case "missing_preimage":
                return yield* PaymentMissingPreimageError.make({
                  invoice: request.invoice,
                  paymentId: toPaymentId(request.invoice),
                })
            }
          }

          const safeAmount =
            options?.fixedAmountMsats !== undefined
              ? Math.max(0, Math.floor(options.fixedAmountMsats))
              : request.maxAmountMsats

          const result: InvoicePaymentResult = {
            paymentId: toPaymentId(`${request.host}:${request.invoice}:${safeAmount}`),
            amountMsats: safeAmount,
            preimageHex: toDeterministicHex64(`${request.invoice}:${request.host}`),
            paidAtMs:
              options?.fixedPaidAtMs !== undefined
                ? Math.max(0, Math.floor(options.fixedPaidAtMs))
                : defaultDemoPaidAtMs,
          }
          return result
        }),
    }),
  )
