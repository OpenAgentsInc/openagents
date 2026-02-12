import { Effect } from "effect"
import { describe, expect, it } from "@effect/vitest"

import { WalletExecutorService } from "../src/wallet/executor.js"
import { makeTestConfig, makeWalletTestLayer } from "./fixtures.js"

describe("wallet executor policy", () => {
  it.effect("denies host outside allowlist before payment", () =>
    Effect.gen(function* () {
      const wallet = yield* WalletExecutorService
      const attempted = yield* Effect.either(
        wallet.payBolt11({
          invoice: "lnbc1hostdenied",
          maxAmountMsats: 100_000,
          host: "example.com",
        }),
      )

      expect(attempted._tag).toBe("Left")
      if (attempted._tag === "Left") {
        expect(attempted.left._tag).toBe("PolicyDeniedError")
        if (attempted.left._tag === "PolicyDeniedError") {
          expect(attempted.left.code).toBe("host_not_allowed")
        }
      }
    }).pipe(Effect.provide(makeWalletTestLayer())),
  )

  it.effect("denies request max amount above service cap", () =>
    Effect.gen(function* () {
      const wallet = yield* WalletExecutorService
      const attempted = yield* Effect.either(
        wallet.payBolt11({
          invoice: "lnbc1capdenied",
          maxAmountMsats: 300_000,
          host: "sats4ai.com",
        }),
      )

      expect(attempted._tag).toBe("Left")
      if (attempted._tag === "Left") {
        expect(attempted.left._tag).toBe("PolicyDeniedError")
        if (attempted.left._tag === "PolicyDeniedError") {
          expect(attempted.left.code).toBe("request_cap_exceeded")
        }
      }
    }).pipe(Effect.provide(makeWalletTestLayer())),
  )

  it.effect("denies when rolling window cap is exceeded", () =>
    Effect.gen(function* () {
      const wallet = yield* WalletExecutorService

      const first = yield* wallet.payBolt11({
        invoice: "lnbc1windowfirst",
        maxAmountMsats: 100_000,
        host: "sats4ai.com",
      })
      expect(first.payment.amountMsats).toBeGreaterThan(0)

      const second = yield* Effect.either(
        wallet.payBolt11({
          invoice: "lnbc1windowsecond",
          maxAmountMsats: 100_000,
          host: "sats4ai.com",
        }),
      )

      expect(second._tag).toBe("Left")
      if (second._tag === "Left") {
        expect(second.left._tag).toBe("PolicyDeniedError")
        if (second.left._tag === "PolicyDeniedError") {
          expect(second.left.code).toBe("window_cap_exceeded")
        }
      }
    }).pipe(
      Effect.provide(
        makeWalletTestLayer({
          config: makeTestConfig({
            requestCapMsats: 120_000,
            windowCapMsats: 80_000,
          }),
          mock: {
            quotedAmountMsats: 45_000,
          },
        }),
      ),
    ),
  )

  it.effect("maps spark prepare failure to typed SparkGatewayError", () =>
    Effect.gen(function* () {
      const wallet = yield* WalletExecutorService

      const attempted = yield* Effect.either(
        wallet.payBolt11({
          invoice: "lnbc1preparefail",
          maxAmountMsats: 100_000,
          host: "sats4ai.com",
        }),
      )

      expect(attempted._tag).toBe("Left")
      if (attempted._tag === "Left") {
        expect(attempted.left._tag).toBe("SparkGatewayError")
        if (attempted.left._tag === "SparkGatewayError") {
          expect(attempted.left.code).toBe("prepare_failed")
        }
      }
    }).pipe(
      Effect.provide(
        makeWalletTestLayer({
          mock: {
            failPrepare: true,
          },
        }),
      ),
    ),
  )
})
