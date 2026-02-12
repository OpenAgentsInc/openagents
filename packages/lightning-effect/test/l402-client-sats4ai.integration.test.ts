import { Clock, Effect, Layer } from "effect"
import { describe, expect, it } from "@effect/vitest"

import { CredentialCacheInMemoryLayer } from "../src/layers/inMemoryCredentialCache.js"
import { L402ClientLiveLayer } from "../src/layers/l402ClientLive.js"
import { makeSpendPolicyLayer } from "../src/layers/defaultSpendPolicy.js"
import { InvoicePayerService } from "../src/services/invoicePayer.js"
import { L402ClientService } from "../src/services/l402Client.js"
import { L402TransportService } from "../src/services/l402Transport.js"

describe("l402 client sats4ai compatibility", () => {
  it.effect("uses host-configured colon authorization header strategy", () => {
    const requests: Array<{ authorization: string | null; status: number }> = []

    const transportLayer = Layer.succeed(
      L402TransportService,
      L402TransportService.of({
        send: (request) =>
          Effect.sync(() => {
            const authorization =
              request.headers?.Authorization ?? request.headers?.authorization ?? null

            if (!authorization) {
              requests.push({ authorization: null, status: 402 })
              return {
                status: 402,
                headers: {
                  "www-authenticate":
                    'L402 invoice="lnbc1sats4ai_invoice", macaroon="mac_sats4ai", amount_msats=2500',
                },
                body: '{"error":"payment_required"}',
              }
            }

            const expected = `L402 mac_sats4ai:${"ef".repeat(32)}`
            if (authorization !== expected) {
              requests.push({ authorization, status: 401 })
              return {
                status: 401,
                headers: { "content-type": "application/json" },
                body: '{"error":"unauthorized"}',
              }
            }

            requests.push({ authorization, status: 200 })
            return {
              status: 200,
              headers: { "content-type": "application/json" },
              body: '{"ok":true,"source":"sats4ai"}',
            }
          }),
      }),
    )

    const payerLayer = Layer.succeed(
      InvoicePayerService,
      InvoicePayerService.of({
        payInvoice: (request) =>
          Effect.gen(function* () {
            const now = yield* Clock.currentTimeMillis
            return {
              paymentId: `pay_${request.host}`,
              amountMsats: 2500,
              preimageHex: "ef".repeat(32),
              paidAtMs: now,
            }
          }),
      }),
    )

    const baseLayer = Layer.mergeAll(
      CredentialCacheInMemoryLayer,
      makeSpendPolicyLayer({
        defaultMaxSpendMsats: 50_000,
        allowedHosts: ["sats4ai.com"],
      }),
      transportLayer,
      payerLayer,
    )

    const clientLayer = L402ClientLiveLayer.pipe(Layer.provide(baseLayer))
    const layer = Layer.merge(baseLayer, clientLayer)

    return Effect.gen(function* () {
      const client = yield* L402ClientService

      const result = yield* client.fetchWithL402({
        url: "https://sats4ai.com/api/l402/text-generation",
        method: "POST",
        body: '{"model":"Best"}',
        maxSpendMsats: 10_000,
        authorizationHeaderStrategyByHost: {
          "sats4ai.com": "macaroon_preimage_colon",
        },
      })

      expect(result.statusCode).toBe(200)
      expect(result.authorizationHeaderStrategy).toBe("macaroon_preimage_colon")
      expect(result.authorizationHeader).toBe(`L402 mac_sats4ai:${"ef".repeat(32)}`)
      expect(result.paid).toBe(true)
      expect(result.responseBody).toContain("sats4ai")

      expect(requests).toHaveLength(2)
      expect(requests[0]?.status).toBe(402)
      expect(requests[1]?.status).toBe(200)
    }).pipe(Effect.provide(layer))
  })

  it.effect("keeps key/value authorization format when strategy is configured explicitly", () => {
    const requests: Array<{ authorization: string | null; status: number }> = []

    const transportLayer = Layer.succeed(
      L402TransportService,
      L402TransportService.of({
        send: (request) =>
          Effect.sync(() => {
            const authorization =
              request.headers?.Authorization ?? request.headers?.authorization ?? null

            if (!authorization) {
              requests.push({ authorization: null, status: 402 })
              return {
                status: 402,
                headers: {
                  "www-authenticate":
                    'L402 invoice="lnbc1spec_invoice", macaroon="mac_spec", amount_msats=2500',
                },
              }
            }

            const expected = `L402 macaroon=\"mac_spec\", preimage=\"${"aa".repeat(32)}\"`
            if (authorization !== expected) {
              requests.push({ authorization, status: 401 })
              return { status: 401, headers: {} }
            }

            requests.push({ authorization, status: 200 })
            return { status: 200, headers: {} }
          }),
      }),
    )

    const payerLayer = Layer.succeed(
      InvoicePayerService,
      InvoicePayerService.of({
        payInvoice: (request) =>
          Effect.gen(function* () {
            const now = yield* Clock.currentTimeMillis
            return {
              paymentId: `pay_${request.host}`,
              amountMsats: 2500,
              preimageHex: "aa".repeat(32),
              paidAtMs: now,
            }
          }),
      }),
    )

    const baseLayer = Layer.mergeAll(
      CredentialCacheInMemoryLayer,
      makeSpendPolicyLayer({
        defaultMaxSpendMsats: 50_000,
        allowedHosts: ["api.example.com"],
      }),
      transportLayer,
      payerLayer,
    )
    const clientLayer = L402ClientLiveLayer.pipe(Layer.provide(baseLayer))
    const layer = Layer.merge(baseLayer, clientLayer)

    return Effect.gen(function* () {
      const client = yield* L402ClientService
      const result = yield* client.fetchWithL402({
        url: "https://api.example.com/premium",
        maxSpendMsats: 10_000,
        authorizationHeaderStrategy: "macaroon_preimage_params",
      })

      expect(result.statusCode).toBe(200)
      expect(result.authorizationHeaderStrategy).toBe("macaroon_preimage_params")
      expect(result.authorizationHeader).toBe(
        `L402 macaroon=\"mac_spec\", preimage=\"${"aa".repeat(32)}\"`,
      )
      expect(requests).toHaveLength(2)
      expect(requests[1]?.status).toBe(200)
    }).pipe(Effect.provide(layer))
  })
})
