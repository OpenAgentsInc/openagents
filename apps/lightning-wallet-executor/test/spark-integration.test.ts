import { Effect } from "effect"
import { describe, expect, it } from "@effect/vitest"

import { WalletExecutorService } from "../src/wallet/executor.js"
import { makeWalletTestLayer } from "./fixtures.js"

describe("spark integration (mocked gateway)", () => {
  it.effect("returns deterministic payment fields on success", () =>
    Effect.gen(function* () {
      const wallet = yield* WalletExecutorService
      const result = yield* wallet.payBolt11({
        invoice: "lnbc1success",
        maxAmountMsats: 100_000,
        host: "sats4ai.com",
      })

      expect(result.payment.paymentId).toMatch(/^mock-pay-/)
      expect(result.payment.amountMsats).toBe(50_000)
      expect(result.payment.preimageHex).toHaveLength(64)
      expect(result.quotedAmountMsats).toBe(50_000)
    }).pipe(
      Effect.provide(
        makeWalletTestLayer({
          mock: {
            quotedAmountMsats: 50_000,
          },
        }),
      ),
    ),
  )

  it.effect("maps send failure to typed SparkGatewayError", () =>
    Effect.gen(function* () {
      const wallet = yield* WalletExecutorService
      const attempted = yield* Effect.either(
        wallet.payBolt11({
          invoice: "lnbc1sendfail",
          maxAmountMsats: 100_000,
          host: "sats4ai.com",
        }),
      )

      expect(attempted._tag).toBe("Left")
      if (attempted._tag === "Left") {
        expect(attempted.left._tag).toBe("SparkGatewayError")
        if (attempted.left._tag === "SparkGatewayError") {
          expect(attempted.left.code).toBe("send_failed")
        }
      }
    }).pipe(
      Effect.provide(
        makeWalletTestLayer({
          mock: {
            failSend: true,
          },
        }),
      ),
    ),
  )

  it.effect("maps pending payment to typed SparkGatewayError", () =>
    Effect.gen(function* () {
      const wallet = yield* WalletExecutorService
      const attempted = yield* Effect.either(
        wallet.payBolt11({
          invoice: "lnbc1pending",
          maxAmountMsats: 100_000,
          host: "sats4ai.com",
        }),
      )

      expect(attempted._tag).toBe("Left")
      if (attempted._tag === "Left") {
        expect(attempted.left._tag).toBe("SparkGatewayError")
        if (attempted.left._tag === "SparkGatewayError") {
          expect(attempted.left.code).toBe("payment_pending")
        }
      }
    }).pipe(
      Effect.provide(
        makeWalletTestLayer({
          mock: {
            pendingOnSend: true,
          },
        }),
      ),
    ),
  )
})

