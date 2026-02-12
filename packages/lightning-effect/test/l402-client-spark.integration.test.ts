import { Clock, Effect, Layer } from "effect"
import { describe, expect, it } from "@effect/vitest"

import { CredentialCacheInMemoryLayer } from "../src/layers/inMemoryCredentialCache.js"
import { L402ClientLiveLayer } from "../src/layers/l402ClientLive.js"
import { makeSpendPolicyLayer } from "../src/layers/defaultSpendPolicy.js"
import { makeInvoicePayerSparkLayer } from "../src/adapters/invoicePayerSpark.js"
import { L402ClientService } from "../src/services/l402Client.js"
import { L402TransportService } from "../src/services/l402Transport.js"
import { SparkPaymentService } from "../src/services/sparkPayment.js"
import type { InvoicePaymentRequest } from "../src/contracts/payment.js"

describe("l402 client + spark full flow", () => {
  it.effect("pays 402 challenge through spark and reuses cached credential", () => {
    const calls: Array<{ readonly auth: string | null; readonly status: number }> = []

    const transportLayer = Layer.succeed(
      L402TransportService,
      L402TransportService.of({
        send: (request) =>
          Effect.sync(() => {
            const auth =
              request.headers?.Authorization ??
              request.headers?.authorization ??
              null
            if (!auth || !auth.startsWith("L402 ")) {
              calls.push({ auth: null, status: 402 })
              return {
                status: 402,
                headers: {
                  "www-authenticate":
                    'L402 invoice="lnbcrt1invoice_spark_flow", macaroon="mac_spark_flow", amount_msats=2500',
                },
                body: '{"error":"payment_required"}',
              }
            }

            calls.push({ auth, status: 200 })
            return {
              status: 200,
              headers: { "content-type": "application/json" },
              body: '{"ok":true}',
            }
          }),
      }),
    )

    const sparkLayer = Layer.succeed(
      SparkPaymentService,
      SparkPaymentService.of({
        payBolt11: (request: InvoicePaymentRequest) =>
          Effect.gen(function* () {
            const now = yield* Clock.currentTimeMillis
            return {
              paymentId: `spark_${request.host}`,
              amountMsats: 2500,
              preimageHex: "cd".repeat(32),
              paidAtMs: now,
            }
          }),
      }),
    )

    const baseLayer = Layer.mergeAll(
      CredentialCacheInMemoryLayer,
      makeSpendPolicyLayer({
        defaultMaxSpendMsats: 50_000,
      }),
      transportLayer,
      makeInvoicePayerSparkLayer().pipe(Layer.provide(sparkLayer)),
    )
    const clientLayer = L402ClientLiveLayer.pipe(Layer.provide(baseLayer))
    const layer = Layer.merge(baseLayer, clientLayer)

    return Effect.gen(function* () {
      const client = yield* L402ClientService

      const first = yield* client.fetchWithL402({
        url: "https://api.example.com/spark-flow",
        method: "GET",
        maxSpendMsats: 10_000,
      })

      expect(first.statusCode).toBe(200)
      expect(first.paid).toBe(true)
      expect(first.cacheStatus).toBe("miss")
      expect(first.paymentId).toBe("spark_api.example.com")
      expect(first.proofReference).toBe("preimage:cdcdcdcdcdcdcdcd")
      expect(first.authorizationHeader).toMatch(/^L402 /)
      expect(calls).toHaveLength(2)
      expect(calls[0]?.status).toBe(402)
      expect(calls[1]?.status).toBe(200)

      const second = yield* client.fetchWithL402({
        url: "https://api.example.com/spark-flow",
        method: "GET",
        maxSpendMsats: 10_000,
      })

      expect(second.statusCode).toBe(200)
      expect(second.paid).toBe(false)
      expect(second.cacheStatus).toBe("hit")
      expect(second.authorizationHeader).toBe(first.authorizationHeader)
      expect(calls).toHaveLength(3)
      expect(calls[2]?.auth).toBe(first.authorizationHeader)
    }).pipe(Effect.provide(layer))
  })
})
