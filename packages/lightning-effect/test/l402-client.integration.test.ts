import { Clock, Effect, Layer, TestClock } from "effect"
import { describe, expect, it } from "@effect/vitest"

import { buildAuthorizationHeader } from "../src/l402/buildAuthorizationHeader.js"
import { CredentialCacheInMemoryLayer } from "../src/layers/inMemoryCredentialCache.js"
import { L402ClientLiveLayer } from "../src/layers/l402ClientLive.js"
import { makeSpendPolicyLayer } from "../src/layers/defaultSpendPolicy.js"
import { CredentialCacheService } from "../src/services/credentialCache.js"
import { InvoicePayerService } from "../src/services/invoicePayer.js"
import { L402ClientService } from "../src/services/l402Client.js"
import { L402TransportService } from "../src/services/l402Transport.js"

type IntegrationState = {
  readonly requests: Array<{
    readonly url: string
    readonly authorization: string | null
  }>
  readonly payCount: { current: number }
  readonly issuedChallenges: { current: number }
  readonly validAuthHeaders: Set<string>
}

const authorizationFromHeaders = (headers?: Record<string, string>): string | null => {
  if (!headers) return null
  for (const [key, value] of Object.entries(headers)) {
    if (key.toLowerCase() === "authorization") return value
  }
  return null
}

const createIntegrationLayer = () => {
  const state: IntegrationState = {
    requests: [],
    payCount: { current: 0 },
    issuedChallenges: { current: 0 },
    validAuthHeaders: new Set<string>(),
  }

  const transportLayer = Layer.succeed(
    L402TransportService,
    L402TransportService.of({
      send: (request) =>
        Effect.sync(() => {
          const authorization = authorizationFromHeaders(request.headers)
          state.requests.push({ url: request.url, authorization })

          if (authorization && state.validAuthHeaders.has(authorization)) {
            return {
              status: 200,
              headers: { "content-type": "application/json" },
              body: '{"ok":true}',
            }
          }

          state.issuedChallenges.current += 1
          const challengeId = state.issuedChallenges.current

          return {
            status: 402,
            headers: {
              "www-authenticate": `L402 invoice="lnbcrt1invoice_${challengeId}", macaroon="mac_${challengeId}", amount_msats=2500`,
            },
            body: '{"error":"payment_required"}',
          }
        }),
    }),
  )

  const payerLayer = Layer.succeed(
    InvoicePayerService,
    InvoicePayerService.of({
      payInvoice: (request) =>
        Effect.gen(function* () {
          state.payCount.current += 1

          const suffix = request.invoice.split("_").at(-1) ?? "0"
          const preimage = `preimage_${suffix}`
          const nowMs = yield* Clock.currentTimeMillis

          const auth = buildAuthorizationHeader({
            host: request.host,
            scope: "default",
            macaroon: `mac_${suffix}`,
            preimageHex: preimage,
            amountMsats: 2500,
            issuedAtMs: nowMs,
          })
          state.validAuthHeaders.add(auth)

          return {
            paymentId: `pay_${suffix}`,
            amountMsats: 2500,
            preimageHex: preimage,
            paidAtMs: nowMs,
          }
        }),
    }),
  )

  const baseLayer = Layer.mergeAll(
    CredentialCacheInMemoryLayer,
    makeSpendPolicyLayer({
      defaultMaxSpendMsats: 50_000,
      allowedHosts: ["api.example.com"],
      blockedHosts: [],
    }),
    transportLayer,
    payerLayer,
  )
  const clientLayer = L402ClientLiveLayer.pipe(Layer.provide(baseLayer))

  return {
    state,
    layer: Layer.merge(baseLayer, clientLayer),
  }
}

describe("l402 client integration", () => {
  it.effect("first request pays and succeeds, second request reuses cache", () => {
    const { layer, state } = createIntegrationLayer()

    return Effect.gen(function* () {
      const client = yield* L402ClientService

      const first = yield* client.fetchWithL402({
        url: "https://api.example.com/premium",
        maxSpendMsats: 10_000,
      })

      expect(first.statusCode).toBe(200)
      expect(first.paid).toBe(true)
      expect(first.cacheStatus).toBe("miss")
      expect(first.fromCache).toBe(false)
      expect(first.paymentId).toBe("pay_1")
      expect(first.authorizationHeader).not.toBeNull()
      expect(state.payCount.current).toBe(1)

      const second = yield* client.fetchWithL402({
        url: "https://api.example.com/premium",
        maxSpendMsats: 10_000,
      })

      expect(second.statusCode).toBe(200)
      expect(second.paid).toBe(false)
      expect(second.cacheStatus).toBe("hit")
      expect(second.fromCache).toBe(true)
      expect(second.paymentId).toBeNull()
      expect(state.payCount.current).toBe(1)

      expect(state.requests.length).toBe(3)
      expect(state.requests[0]?.authorization).toBeNull()
      expect(state.requests[1]?.authorization).not.toBeNull()
      expect(state.requests[2]?.authorization).toBe(second.authorizationHeader)
    }).pipe(Effect.provide(layer))
  })

  it.effect("invalid cached credential recovers via fresh challenge and payment", () => {
    const { layer, state } = createIntegrationLayer()

    return Effect.gen(function* () {
      const cache = yield* CredentialCacheService
      const client = yield* L402ClientService
      const nowMs = yield* Clock.currentTimeMillis

      const invalidCredentialAuth = buildAuthorizationHeader({
        host: "api.example.com",
        scope: "default",
        macaroon: "old_macaroon",
        preimageHex: "old_preimage",
        amountMsats: 2500,
        issuedAtMs: nowMs,
      })

      yield* cache.putByHost(
        "api.example.com",
        "default",
        {
          host: "api.example.com",
          scope: "default",
          macaroon: "old_macaroon",
          preimageHex: "old_preimage",
          amountMsats: 2500,
          issuedAtMs: nowMs,
        },
        { ttlMs: 60_000 },
      )

      const result = yield* client.fetchWithL402({
        url: "https://api.example.com/premium",
        maxSpendMsats: 10_000,
      })

      expect(result.statusCode).toBe(200)
      expect(result.cacheStatus).toBe("invalid")
      expect(result.paid).toBe(true)
      expect(result.authorizationHeader).not.toBe(invalidCredentialAuth)
      expect(state.payCount.current).toBe(1)
      expect(state.requests.length).toBe(2)
      expect(state.requests[0]?.authorization).toBe(invalidCredentialAuth)
      expect(state.requests[1]?.authorization).toBe(result.authorizationHeader)
    }).pipe(Effect.provide(layer))
  })

  it.effect("stale cache entries trigger refresh payment path", () => {
    const { layer, state } = createIntegrationLayer()

    return Effect.gen(function* () {
      const client = yield* L402ClientService

      const first = yield* client.fetchWithL402({
        url: "https://api.example.com/premium",
        maxSpendMsats: 10_000,
        cacheTtlMs: 1_000,
      })

      expect(first.cacheStatus).toBe("miss")
      expect(first.paid).toBe(true)
      expect(state.payCount.current).toBe(1)

      yield* TestClock.adjust("2 seconds")

      const second = yield* client.fetchWithL402({
        url: "https://api.example.com/premium",
        maxSpendMsats: 10_000,
        cacheTtlMs: 1_000,
      })

      expect(second.statusCode).toBe(200)
      expect(second.cacheStatus).toBe("stale")
      expect(second.paid).toBe(true)
      expect(state.payCount.current).toBe(2)
    }).pipe(Effect.provide(layer))
  })
})
