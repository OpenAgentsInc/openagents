import { Effect } from "effect"
import { describe, expect, it } from "@effect/vitest"

import { runEp212RoutesSmoke } from "../src/programs/smokeEp212Routes.js"

describe("lightning-ops smoke:ep212-routes", () => {
  it.effect("mock mode validates challenge shape, paid success, and over-cap block", () =>
    Effect.gen(function* () {
      const summary = yield* runEp212RoutesSmoke({
        mode: "mock",
        requestId: "smoke:ep212-routes:test",
      })

      expect(summary.ok).toBe(true)
      expect(summary.mode).toBe("mock")
      expect(summary.routeA.challengeStatusCode).toBe(402)
      expect(summary.routeA.paidStatusCode).toBe(200)
      expect(summary.routeA.paidAmountMsats).toBeGreaterThan(0)
      expect(summary.routeA.responseBytes).toBeGreaterThan(0)
      expect(summary.routeB.challengeStatusCode).toBe(402)
      expect(summary.routeB.blocked).toBe(true)
      expect(summary.routeB.denyReasonCode).toBe("amount_over_cap")
      expect(summary.routeB.payerCallsAfter).toBe(summary.routeB.payerCallsBefore)
    }),
  )

  it.effect("live mode fails fast when wallet executor base URL is missing", () =>
    Effect.gen(function* () {
      const prevBase = process.env.OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL

      delete process.env.OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL

      try {
        const attempted = yield* Effect.either(
          runEp212RoutesSmoke({
            mode: "live",
            requestId: "smoke:ep212-routes:missing-wallet",
          }),
        )

        expect(attempted._tag).toBe("Left")
        if (attempted._tag === "Left") {
          expect(String(attempted.left)).toContain("ConfigError")
        }
      } finally {
        process.env.OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL = prevBase
      }
    }),
  )
})
