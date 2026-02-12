import { Effect, Layer } from "effect"

import type { InvoicePaymentRequest } from "../contracts/payment.js"
import { InvoicePayerService } from "../services/invoicePayer.js"
import { SparkPaymentService } from "../services/sparkPayment.js"

export const makeInvoicePayerSparkLayer = (): Layer.Layer<InvoicePayerService, never, SparkPaymentService> =>
  Layer.effect(
    InvoicePayerService,
    Effect.gen(function* () {
      const spark = yield* SparkPaymentService

      return InvoicePayerService.of({
        payInvoice: (request: InvoicePaymentRequest) => spark.payBolt11(request),
      })
    }),
  )

export const InvoicePayerSparkLayer = makeInvoicePayerSparkLayer()
