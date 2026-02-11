import { Effect, Layer } from "effect"
import { describe, expect, it } from "@effect/vitest"

import type { InvoicePaymentRequest } from "../src/contracts/payment.js"
import { makeInvoicePayerDemoLayer } from "../src/adapters/invoicePayerDemo.js"
import { makeInvoicePayerLndLayer } from "../src/adapters/invoicePayerLnd.js"
import { InvoicePayerService } from "../src/services/invoicePayer.js"

const demoRequest: InvoicePaymentRequest = {
  invoice: "lnbcrt1demo_invoice",
  host: "api.example.com",
  maxAmountMsats: 12_345,
}

const runPay = (layer: Layer.Layer<InvoicePayerService>, request: InvoicePaymentRequest) =>
  Effect.gen(function* () {
    const payer = yield* InvoicePayerService
    return yield* payer.payInvoice(request)
  }).pipe(Effect.provide(layer))

describe("invoice payer contract", () => {
  it.effect("demo adapter produces deterministic successful payments", () =>
    Effect.gen(function* () {
      const layer = makeInvoicePayerDemoLayer()

      const first = yield* runPay(layer, demoRequest)
      const second = yield* runPay(layer, demoRequest)

      expect(first.paymentId).toBe(second.paymentId)
      expect(first.preimageHex).toBe(second.preimageHex)
      expect(first.paidAtMs).toBe(1_700_000_000_000)
      expect(first.preimageHex).toMatch(/^[0-9a-f]+$/)
      expect(first.preimageHex.length).toBe(64)
    }),
  )

  it.effect("demo adapter emits typed failure modes", () =>
    Effect.gen(function* () {
      const failed = yield* Effect.either(
        runPay(
          makeInvoicePayerDemoLayer({ failureMode: "payment_failed" }),
          demoRequest,
        ),
      )
      expect(failed._tag).toBe("Left")
      if (failed._tag === "Left") {
        expect(failed.left._tag).toBe("PaymentFailedError")
      }

      const timedOut = yield* Effect.either(
        runPay(
          makeInvoicePayerDemoLayer({ failureMode: "timeout", timeoutMs: 25 }),
          demoRequest,
        ),
      )
      expect(timedOut._tag).toBe("Left")
      if (timedOut._tag === "Left") {
        expect(timedOut.left._tag).toBe("PaymentTimeoutError")
        if (timedOut.left._tag === "PaymentTimeoutError") {
          expect(timedOut.left.timeoutMs).toBe(25)
        }
      }

      const missingPreimage = yield* Effect.either(
        runPay(
          makeInvoicePayerDemoLayer({ failureMode: "missing_preimage" }),
          demoRequest,
        ),
      )
      expect(missingPreimage._tag).toBe("Left")
      if (missingPreimage._tag === "Left") {
        expect(missingPreimage.left._tag).toBe("PaymentMissingPreimageError")
      }
    }),
  )

  it.effect("lnd adapter parses successful response and returns preimageHex", () =>
    Effect.gen(function* () {
      let capturedHeaders: Record<string, string> = {}
      let capturedBody: Record<string, unknown> = {}

      const layer = makeInvoicePayerLndLayer({
        endpoint: "https://lnd.example/v1/channels/transactions",
        macaroonHex: "deadbeef",
        fetchImplementation: async (_input, init) => {
          capturedHeaders = (init?.headers as Record<string, string>) ?? {}
          capturedBody = JSON.parse(String(init?.body ?? "{}")) as Record<string, unknown>

          return new Response(
            JSON.stringify({
              payment_hash: "hash_123",
              payment_preimage: "ab".repeat(32),
              payment_route: { total_amt_msat: "2500" },
            }),
            { status: 200, headers: { "content-type": "application/json" } },
          )
        },
      })

      const result = yield* runPay(layer, demoRequest)

      expect(result.paymentId).toBe("hash_123")
      expect(result.amountMsats).toBe(2500)
      expect(result.preimageHex).toBe("ab".repeat(32))
      expect(capturedHeaders["grpc-metadata-macaroon"]).toBe("deadbeef")
      expect(capturedBody.payment_request).toBe(demoRequest.invoice)
      expect(capturedBody.fee_limit_msat).toBe(String(demoRequest.maxAmountMsats))
    }),
  )

  it.effect("lnd adapter emits typed payment_failed and missing_preimage errors", () =>
    Effect.gen(function* () {
      const failedLayer = makeInvoicePayerLndLayer({
        endpoint: "https://lnd.example/v1/channels/transactions",
        fetchImplementation: async () =>
          new Response(JSON.stringify({ payment_error: "insufficient_balance" }), {
            status: 200,
            headers: { "content-type": "application/json" },
          }),
      })
      const failed = yield* Effect.either(runPay(failedLayer, demoRequest))
      expect(failed._tag).toBe("Left")
      if (failed._tag === "Left") {
        expect(failed.left._tag).toBe("PaymentFailedError")
      }

      const missingPreimageLayer = makeInvoicePayerLndLayer({
        endpoint: "https://lnd.example/v1/channels/transactions",
        fetchImplementation: async () =>
          new Response(JSON.stringify({ payment_hash: "hash_456" }), {
            status: 200,
            headers: { "content-type": "application/json" },
          }),
      })
      const missingPreimage = yield* Effect.either(runPay(missingPreimageLayer, demoRequest))
      expect(missingPreimage._tag).toBe("Left")
      if (missingPreimage._tag === "Left") {
        expect(missingPreimage.left._tag).toBe("PaymentMissingPreimageError")
        if (missingPreimage.left._tag === "PaymentMissingPreimageError") {
          expect(missingPreimage.left.paymentId).toBe("hash_456")
        }
      }
    }),
  )

  it.effect("lnd adapter emits typed timeout error", () =>
    Effect.gen(function* () {
      const timeoutLayer = makeInvoicePayerLndLayer({
        endpoint: "https://lnd.example/v1/channels/transactions",
        timeoutMs: 10,
        fetchImplementation: async (_input, init) =>
          await new Promise<Response>((_resolve, reject) => {
            const signal = init?.signal
            const onAbort = () => reject(new DOMException("Aborted", "AbortError"))

            if (signal?.aborted) {
              onAbort()
              return
            }

            signal?.addEventListener("abort", onAbort, { once: true })
          }),
      })

      const result = yield* Effect.either(runPay(timeoutLayer, demoRequest))
      expect(result._tag).toBe("Left")
      if (result._tag === "Left") {
        expect(result.left._tag).toBe("PaymentTimeoutError")
        if (result.left._tag === "PaymentTimeoutError") {
          expect(result.left.timeoutMs).toBe(10)
        }
      }
    }),
  )
})
