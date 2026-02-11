import { Effect, Layer } from "effect"
import { describe, expect, it } from "@effect/vitest"

import { makeInvoicePayerDemoLayer } from "../src/adapters/invoicePayerDemo.js"
import { CredentialCacheInMemoryLayer } from "../src/layers/inMemoryCredentialCache.js"
import { L402ClientLiveLayer } from "../src/layers/l402ClientLive.js"
import { makeSpendPolicyLayer } from "../src/layers/defaultSpendPolicy.js"
import { L402ClientService } from "../src/services/l402Client.js"

const baseLayer = Layer.mergeAll(
  CredentialCacheInMemoryLayer,
  makeSpendPolicyLayer({
    defaultMaxSpendMsats: 50_000,
    allowedHosts: ["api.example.com"],
    blockedHosts: [],
  }),
  makeInvoicePayerDemoLayer({
    fixedAmountMsats: 2500,
    fixedPaidAtMs: 1_700_000_000_000,
  }),
)
const demoLayer = L402ClientLiveLayer.pipe(Layer.provide(baseLayer))

describe("l402 client live", () => {
  it.effect("pays once, then reuses cached credential", () =>
    Effect.gen(function* () {
      const client = yield* L402ClientService

      const first = yield* client.authorizeRequest({
        url: "https://api.example.com/premium-data",
        maxSpendMsats: 10_000,
        challengeHeader:
          'L402 invoice="lnbcrt1invoice", macaroon="AgEDbWFjYXJvb24=", amount_msats=2500',
      })

      expect(first.fromCache).toBe(false)
      expect(first.amountMsats).toBe(2500)
      expect(first.paymentId).toMatch(/^pay_/)
      expect(first.authorizationHeader).toContain("macaroon=")

      const second = yield* client.authorizeRequest({
        url: "https://api.example.com/premium-data",
        maxSpendMsats: 10_000,
      })

      expect(second.fromCache).toBe(true)
      expect(second.amountMsats).toBe(2500)
      expect(second.paymentId).toBe(null)
      expect(second.authorizationHeader).toBe(first.authorizationHeader)
      expect(second.proofReference).toBe(first.proofReference)
    }).pipe(Effect.provide(demoLayer)),
  )

  it.effect("rejects requests for disallowed hosts", () =>
    Effect.gen(function* () {
      const client = yield* L402ClientService

      const result = yield* Effect.either(
        client.authorizeRequest({
          url: "https://other-host.example/premium-data",
          maxSpendMsats: 10_000,
          challengeHeader:
            'L402 invoice="lnbcrt1invoice", macaroon="AgEDbWFjYXJvb24=", amount_msats=2500',
        }),
      )

      expect(result._tag).toBe("Left")
      if (result._tag === "Left") {
        expect(result.left._tag).toBe("DomainNotAllowedError")
      }
    }).pipe(Effect.provide(demoLayer)),
  )
})
