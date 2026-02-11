import { Effect, Layer } from "effect"

import type { InvoicePaymentRequest, InvoicePaymentResult } from "../contracts/payment.js"
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

export const makeInvoicePayerDemoLayer = (options?: {
  readonly fixedAmountMsats?: number
  readonly fixedPaidAtMs?: number
}) =>
  Layer.succeed(
    InvoicePayerService,
    InvoicePayerService.of({
      payInvoice: (request: InvoicePaymentRequest) => {
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
              : Date.now(),
        }
        return Effect.succeed(result)
      },
    }),
  )
