import { Effect } from "effect"
import { describe, expect, it } from "@effect/vitest"

import * as Root from "../src/index.js"
import * as Contracts from "../src/contracts/index.js"
import * as Services from "../src/services/index.js"

describe("buyer-side compatibility", () => {
  it.effect("retains legacy buyer contracts with stable decode/encode behavior", () =>
    Effect.gen(function* () {
      const paymentRequest = yield* Root.decodeInvoicePaymentRequest({
        invoice: "lnbcrt1legacy_invoice",
        host: "api.example.com",
        maxAmountMsats: 10_000,
      })
      expect(paymentRequest.invoice).toBe("lnbcrt1legacy_invoice")

      const spendPolicy = yield* Root.decodeSpendPolicy({
        defaultMaxSpendMsats: 100_000,
        allowedHosts: ["api.example.com"],
        blockedHosts: [],
      })
      expect(spendPolicy.defaultMaxSpendMsats).toBe(100_000)

      const challenge = yield* Root.parseChallengeHeader(
        'L402 invoice="lnbcrt1legacy_invoice", macaroon="mac_legacy", amount_msats=2500',
      )
      const header = Root.buildAuthorizationHeader({
        host: "api.example.com",
        scope: "default",
        macaroon: challenge.macaroon,
        preimageHex: "00".repeat(32),
        amountMsats: challenge.amountMsats ?? 2_500,
        issuedAtMs: 1_700_000_000_000,
      })
      expect(header.startsWith("L402 macaroon=")).toBe(true)

      const encoded = yield* Root.encodeInvoicePaymentRequest(paymentRequest)
      expect(encoded.maxAmountMsats).toBe(10_000)

      const synced = Root.decodeInvoicePaymentResultSync({
        paymentId: "pay_1",
        amountMsats: 2_500,
        preimageHex: "ab".repeat(32),
        paidAtMs: 1_700_000_000_001,
      })
      expect(synced.paymentId).toBe("pay_1")
    }),
  )

  it.effect("retains legacy buyer services/adapters and documented subpath exports", () =>
    Effect.gen(function* () {
      expect(Root.decodeInvoicePaymentRequest).toBe(Contracts.decodeInvoicePaymentRequest)
      expect(Root.InvoicePayerService).toBe(Services.InvoicePayerService)

      const payment = yield* Effect.gen(function* () {
        const payer = yield* Root.InvoicePayerService
        return yield* payer.payInvoice({
          invoice: "lnbcrt1legacy_invoice",
          host: "api.example.com",
          maxAmountMsats: 2_500,
        })
      }).pipe(
        Effect.provide(
          Root.makeInvoicePayerDemoLayer({
            fixedAmountMsats: 2_500,
            fixedPaidAtMs: 1_700_000_000_000,
          }),
        ),
      )

      expect(payment.amountMsats).toBe(2_500)
      expect(payment.preimageHex.length).toBe(64)
    }),
  )
})
