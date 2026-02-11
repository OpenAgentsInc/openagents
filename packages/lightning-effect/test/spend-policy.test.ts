import { Effect, Layer } from "effect"
import { describe, expect, it } from "@effect/vitest"

import { CredentialCacheInMemoryLayer } from "../src/layers/inMemoryCredentialCache.js"
import { L402ClientLiveLayer } from "../src/layers/l402ClientLive.js"
import { makeSpendPolicyLayer } from "../src/layers/defaultSpendPolicy.js"
import { InvoicePayerService } from "../src/services/invoicePayer.js"
import { L402ClientService } from "../src/services/l402Client.js"
import { L402TransportService } from "../src/services/l402Transport.js"
import { SpendPolicyService } from "../src/services/spendPolicy.js"

describe("spend policy guardrails", () => {
  it.effect("returns stable typed denial reasons for allow/deny decisions", () =>
    Effect.gen(function* () {
      const policy = yield* SpendPolicyService

      const allowed = yield* Effect.either(
        policy.ensureRequestAllowed({
          host: "api.example.com",
          quotedAmountMsats: 1_500,
          maxSpendMsats: 10_000,
        }),
      )
      expect(allowed._tag).toBe("Right")

      const blocked = yield* Effect.either(
        policy.ensureRequestAllowed({
          host: "blocked.example.com",
          quotedAmountMsats: 1_000,
          maxSpendMsats: 10_000,
        }),
      )
      expect(blocked._tag).toBe("Left")
      if (blocked._tag === "Left") {
        expect(blocked.left._tag).toBe("DomainNotAllowedError")
        if (blocked.left._tag === "DomainNotAllowedError") {
          expect(blocked.left.reasonCode).toBe("host_blocked")
        }
      }

      const notAllowlisted = yield* Effect.either(
        policy.ensureRequestAllowed({
          host: "not-allowlisted.example.com",
          quotedAmountMsats: 1_000,
          maxSpendMsats: 10_000,
        }),
      )
      expect(notAllowlisted._tag).toBe("Left")
      if (notAllowlisted._tag === "Left") {
        expect(notAllowlisted.left._tag).toBe("DomainNotAllowedError")
        if (notAllowlisted.left._tag === "DomainNotAllowedError") {
          expect(notAllowlisted.left.reasonCode).toBe("host_not_allowlisted")
        }
      }

      const overCap = yield* Effect.either(
        policy.ensureRequestAllowed({
          host: "api.example.com",
          quotedAmountMsats: 5_000,
          maxSpendMsats: 10_000,
        }),
      )
      expect(overCap._tag).toBe("Left")
      if (overCap._tag === "Left") {
        expect(overCap.left._tag).toBe("BudgetExceededError")
        if (overCap.left._tag === "BudgetExceededError") {
          expect(overCap.left.reasonCode).toBe("amount_over_cap")
        }
      }
    }).pipe(
      Effect.provide(
        makeSpendPolicyLayer({
          defaultMaxSpendMsats: 2_000,
          allowedHosts: ["api.example.com"],
          blockedHosts: ["blocked.example.com"],
        }),
      ),
    ),
  )

  it.effect("denied over-cap request never calls InvoicePayer", () => {
    const payCount = { current: 0 }

    const transportLayer = Layer.succeed(
      L402TransportService,
      L402TransportService.of({
        send: () =>
          Effect.succeed({
            status: 402,
            headers: {
              "www-authenticate":
                'L402 invoice="lnbcrt1budget", macaroon="mac_budget", amount_msats=5000',
            },
            body: '{"error":"payment_required"}',
          }),
      }),
    )

    const payerLayer = Layer.succeed(
      InvoicePayerService,
      InvoicePayerService.of({
        payInvoice: () =>
          Effect.sync(() => {
            payCount.current += 1
            return {
              paymentId: "pay_should_not_happen",
              amountMsats: 5_000,
              preimageHex: "ab".repeat(32),
              paidAtMs: 1_700_000_000_000,
            }
          }),
      }),
    )

    const baseLayer = Layer.mergeAll(
      CredentialCacheInMemoryLayer,
      makeSpendPolicyLayer({
        defaultMaxSpendMsats: 1_000,
        allowedHosts: ["api.example.com"],
        blockedHosts: [],
      }),
      transportLayer,
      payerLayer,
    )
    const clientLayer = L402ClientLiveLayer.pipe(Layer.provide(baseLayer))
    const runtimeLayer = Layer.merge(baseLayer, clientLayer)

    return Effect.gen(function* () {
      const client = yield* L402ClientService
      const denied = yield* Effect.either(
        client.fetchWithL402({
          url: "https://api.example.com/premium",
          maxSpendMsats: 10_000,
        }),
      )

      expect(denied._tag).toBe("Left")
      if (denied._tag === "Left") {
        expect(denied.left._tag).toBe("BudgetExceededError")
        if (denied.left._tag === "BudgetExceededError") {
          expect(denied.left.reasonCode).toBe("amount_over_cap")
        }
      }

      expect(payCount.current).toBe(0)
    }).pipe(Effect.provide(runtimeLayer))
  })

  it.effect("disallowed domain returns typed policy error and never calls InvoicePayer", () => {
    const payCount = { current: 0 }

    const transportLayer = Layer.succeed(
      L402TransportService,
      L402TransportService.of({
        send: () =>
          Effect.succeed({
            status: 402,
            headers: {
              "www-authenticate":
                'L402 invoice="lnbcrt1domain", macaroon="mac_domain", amount_msats=500',
            },
            body: '{"error":"payment_required"}',
          }),
      }),
    )

    const payerLayer = Layer.succeed(
      InvoicePayerService,
      InvoicePayerService.of({
        payInvoice: () =>
          Effect.sync(() => {
            payCount.current += 1
            return {
              paymentId: "pay_should_not_happen",
              amountMsats: 500,
              preimageHex: "cd".repeat(32),
              paidAtMs: 1_700_000_000_000,
            }
          }),
      }),
    )

    const baseLayer = Layer.mergeAll(
      CredentialCacheInMemoryLayer,
      makeSpendPolicyLayer({
        defaultMaxSpendMsats: 10_000,
        allowedHosts: ["other.example.com"],
        blockedHosts: [],
      }),
      transportLayer,
      payerLayer,
    )
    const clientLayer = L402ClientLiveLayer.pipe(Layer.provide(baseLayer))
    const runtimeLayer = Layer.merge(baseLayer, clientLayer)

    return Effect.gen(function* () {
      const client = yield* L402ClientService
      const denied = yield* Effect.either(
        client.fetchWithL402({
          url: "https://api.example.com/premium",
          maxSpendMsats: 10_000,
        }),
      )

      expect(denied._tag).toBe("Left")
      if (denied._tag === "Left") {
        expect(denied.left._tag).toBe("DomainNotAllowedError")
        if (denied.left._tag === "DomainNotAllowedError") {
          expect(denied.left.reasonCode).toBe("host_not_allowlisted")
        }
      }

      expect(payCount.current).toBe(0)
    }).pipe(Effect.provide(runtimeLayer))
  })
})
