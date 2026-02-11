import type { LndPaymentRecord } from "@openagentsinc/lnd-effect/contracts"
import { makeLndDeterministicLayer } from "@openagentsinc/lnd-effect/adapters"
import { LndServiceUnavailableError } from "@openagentsinc/lnd-effect/errors"
import { LndPaymentService } from "@openagentsinc/lnd-effect/services"
import { Effect, Layer } from "effect"
import { describe, expect, it } from "@effect/vitest"

import type { InvoicePaymentRequest } from "../src/contracts/payment.js"
import { makeInvoicePayerLndEffectLayer } from "../src/lnd-effect/index.js"
import { InvoicePayerService } from "../src/services/invoicePayer.js"

const request: InvoicePaymentRequest = {
  invoice: "lnbcrt1lndeffect_demo_invoice",
  host: "api.example.com",
  maxAmountMsats: 2_500,
}

const runPay = (layer: Layer.Layer<InvoicePayerService>, input: InvoicePaymentRequest) =>
  Effect.gen(function* () {
    const payer = yield* InvoicePayerService
    return yield* payer.payInvoice(input)
  }).pipe(Effect.provide(layer))

const makePaymentLayer = (payment: LndPaymentRecord) =>
  Layer.succeed(
    LndPaymentService,
    LndPaymentService.of({
      sendPayment: () => Effect.succeed(payment),
      trackPayment: () => Effect.succeed(payment),
      listPayments: () => Effect.succeed({ payments: [payment] }),
    }),
  )

describe("lnd-effect invoice payer integration", () => {
  it.effect("executes through lnd-effect deterministic adapter with stable payment proof fields", () =>
    Effect.gen(function* () {
      const layer = makeInvoicePayerLndEffectLayer().pipe(
        Layer.provide(makeLndDeterministicLayer()),
      )

      const first = yield* runPay(layer, request)
      const second = yield* runPay(layer, request)

      expect(first.paymentId).toBe(second.paymentId)
      expect(first.preimageHex).toBe(second.preimageHex)
      expect(first.preimageHex).toMatch(/^[0-9a-f]+$/)
      expect(first.preimageHex.length).toBe(64)
      expect(first.amountMsats).toBe(request.maxAmountMsats)
      expect(first.paidAtMs).toBeGreaterThanOrEqual(0)
    }),
  )

  it.effect("maps lnd service unavailability into PaymentFailedError", () =>
    Effect.gen(function* () {
      const layer = makeInvoicePayerLndEffectLayer().pipe(
        Layer.provide(
          Layer.succeed(
            LndPaymentService,
            LndPaymentService.of({
              sendPayment: () =>
                LndServiceUnavailableError.make({
                  service: "payments",
                  reason: "connection_refused",
                }),
              trackPayment: () =>
                LndServiceUnavailableError.make({
                  service: "payments",
                  reason: "connection_refused",
                }),
              listPayments: () =>
                LndServiceUnavailableError.make({
                  service: "payments",
                  reason: "connection_refused",
                }),
            }),
          ),
        ),
      )

      const result = yield* Effect.either(runPay(layer, request))
      expect(result._tag).toBe("Left")
      if (result._tag === "Left") {
        expect(result.left._tag).toBe("PaymentFailedError")
        if (result.left._tag === "PaymentFailedError") {
          expect(result.left.reason).toContain("lnd_service_unavailable:payments:connection_refused")
        }
      }
    }),
  )

  it.effect("maps failed lnd payment state into PaymentFailedError", () =>
    Effect.gen(function* () {
      const layer = makeInvoicePayerLndEffectLayer().pipe(
        Layer.provide(
          makePaymentLayer({
            paymentHash: "hash_failed_1",
            amountSat: 10,
            feeSat: 1,
            status: "failed",
            failureReason: "route_not_found",
            createdAtMs: 1_700_000_000_000,
            updatedAtMs: 1_700_000_000_001,
          }),
        ),
      )

      const result = yield* Effect.either(runPay(layer, request))
      expect(result._tag).toBe("Left")
      if (result._tag === "Left") {
        expect(result.left._tag).toBe("PaymentFailedError")
        if (result.left._tag === "PaymentFailedError") {
          expect(result.left.reason).toBe("route_not_found")
        }
      }
    }),
  )

  it.effect("maps succeeded payment without preimage into PaymentMissingPreimageError", () =>
    Effect.gen(function* () {
      const layer = makeInvoicePayerLndEffectLayer().pipe(
        Layer.provide(
          makePaymentLayer({
            paymentHash: "hash_missing_preimage_1",
            amountSat: 2,
            feeSat: 1,
            status: "succeeded",
            createdAtMs: 1_700_000_000_000,
            updatedAtMs: 1_700_000_000_001,
          }),
        ),
      )

      const result = yield* Effect.either(runPay(layer, request))
      expect(result._tag).toBe("Left")
      if (result._tag === "Left") {
        expect(result.left._tag).toBe("PaymentMissingPreimageError")
        if (result.left._tag === "PaymentMissingPreimageError") {
          expect(result.left.paymentId).toBe("hash_missing_preimage_1")
        }
      }
    }),
  )

  it.effect("maps in_flight payments that never settle into PaymentTimeoutError", () =>
    Effect.gen(function* () {
      const inFlight: LndPaymentRecord = {
        paymentHash: "hash_in_flight_1",
        amountSat: 0,
        feeSat: 0,
        status: "in_flight",
        createdAtMs: 1_700_000_000_000,
        updatedAtMs: 1_700_000_000_002,
      }

      const layer = makeInvoicePayerLndEffectLayer({
        inFlightTimeoutMs: 1234,
      }).pipe(Layer.provide(makePaymentLayer(inFlight)))

      const result = yield* Effect.either(runPay(layer, request))
      expect(result._tag).toBe("Left")
      if (result._tag === "Left") {
        expect(result.left._tag).toBe("PaymentTimeoutError")
        if (result.left._tag === "PaymentTimeoutError") {
          expect(result.left.timeoutMs).toBe(1234)
        }
      }
    }),
  )
})
