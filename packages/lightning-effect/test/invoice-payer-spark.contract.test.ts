import { Effect, Layer } from "effect"
import { describe, expect, it } from "@effect/vitest"

import type { InvoicePaymentRequest } from "../src/contracts/payment.js"
import {
  PaymentFailedError,
  PaymentMissingPreimageError,
  PaymentTimeoutError,
} from "../src/errors/lightningErrors.js"
import { makeInvoicePayerSparkLayer } from "../src/adapters/invoicePayerSpark.js"
import type { InvoicePaymentResult } from "../src/contracts/payment.js"
import { InvoicePayerService } from "../src/services/invoicePayer.js"
import { SparkPaymentService, type SparkPaymentError } from "../src/services/sparkPayment.js"

const request: InvoicePaymentRequest = {
  invoice: "lnbcrt1spark_invoice",
  host: "spark.example.com",
  maxAmountMsats: 25_000,
}

const runPay = (layer: Layer.Layer<InvoicePayerService>) =>
  Effect.gen(function* () {
    const payer = yield* InvoicePayerService
    return yield* payer.payInvoice(request)
  }).pipe(Effect.provide(layer))

const makeSparkLayer = (
  handler: (
    request: InvoicePaymentRequest,
  ) => Effect.Effect<InvoicePaymentResult, SparkPaymentError>,
) =>
  Layer.succeed(
    SparkPaymentService,
    SparkPaymentService.of({
      payBolt11: handler,
    }),
  )

describe("invoice payer spark adapter contract", () => {
  it.effect("passes through successful spark bolt11 payment", () =>
    Effect.gen(function* () {
      const layer = makeInvoicePayerSparkLayer().pipe(
        Layer.provide(
          makeSparkLayer(() =>
            Effect.succeed({
              paymentId: "spark-payment-1",
              amountMsats: 2_500,
              preimageHex: "ab".repeat(32),
              paidAtMs: 1_700_000_000_100,
            }),
          ),
        ),
      )

      const result = yield* runPay(layer)
      expect(result.paymentId).toBe("spark-payment-1")
      expect(result.amountMsats).toBe(2_500)
      expect(result.preimageHex).toBe("ab".repeat(32))
      expect(result.paidAtMs).toBe(1_700_000_000_100)
    }),
  )

  it.effect("preserves typed spark payment failures", () =>
    Effect.gen(function* () {
      const failedLayer = makeInvoicePayerSparkLayer().pipe(
        Layer.provide(
          makeSparkLayer(() =>
            PaymentFailedError.make({
              invoice: request.invoice,
              reason: "spark_send_failed",
            }),
          ),
        ),
      )
      const failed = yield* Effect.either(runPay(failedLayer))
      expect(failed._tag).toBe("Left")
      if (failed._tag === "Left") {
        expect(failed.left._tag).toBe("PaymentFailedError")
      }

      const timeoutLayer = makeInvoicePayerSparkLayer().pipe(
        Layer.provide(
          makeSparkLayer(() =>
            PaymentTimeoutError.make({
              invoice: request.invoice,
              timeoutMs: 1234,
            }),
          ),
        ),
      )
      const timeout = yield* Effect.either(runPay(timeoutLayer))
      expect(timeout._tag).toBe("Left")
      if (timeout._tag === "Left") {
        expect(timeout.left._tag).toBe("PaymentTimeoutError")
      }

      const preimageLayer = makeInvoicePayerSparkLayer().pipe(
        Layer.provide(
          makeSparkLayer(() =>
            PaymentMissingPreimageError.make({
              invoice: request.invoice,
              paymentId: "spark-payment-2",
            }),
          ),
        ),
      )
      const missingPreimage = yield* Effect.either(runPay(preimageLayer))
      expect(missingPreimage._tag).toBe("Left")
      if (missingPreimage._tag === "Left") {
        expect(missingPreimage.left._tag).toBe("PaymentMissingPreimageError")
      }
    }),
  )
})
