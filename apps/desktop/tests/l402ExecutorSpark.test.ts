import { Effect, Layer } from "effect"
import { describe, expect, it } from "@effect/vitest"

import { L402ExecutorLive, L402ExecutorService } from "../src/effect/l402Executor"
import type { ExecutorTask } from "../src/effect/model"
import { SparkWalletGatewayService } from "../src/effect/sparkWalletGateway"

const makeTask = (url: string): ExecutorTask => {
  const now = Date.now()
  return {
    id: "task-spark-1",
    ownerId: "user-spark",
    status: "queued",
    request: {
      url,
      method: "GET",
      maxSpendMsats: 10_000,
    },
    attemptCount: 0,
    createdAtMs: now,
    updatedAtMs: now,
  }
}

const withFetchHarness = <A, E, R>(
  program: Effect.Effect<A, E, R>,
): Effect.Effect<A, E, R> =>
  Effect.acquireUseRelease(
    Effect.sync(() => {
      const originalFetch = globalThis.fetch
      let challengeIssued = false
      globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
        const request = input instanceof Request ? input : new Request(input, init)
        const auth = request.headers.get("authorization") ?? request.headers.get("Authorization")
        if (!auth || !challengeIssued) {
          challengeIssued = true
          return new Response('{"error":"payment_required"}', {
            status: 402,
            headers: {
              "content-type": "application/json; charset=utf-8",
              "www-authenticate":
                'L402 invoice="lnbcrt1sparkflow", macaroon="spark_macaroon", amount_msats=2500',
            },
          })
        }
        return new Response('{"ok":true}', {
          status: 200,
          headers: {
            "content-type": "application/json; charset=utf-8",
          },
        })
      }) as typeof fetch
      return originalFetch
    }),
    () => program,
    (originalFetch) =>
      Effect.sync(() => {
        globalThis.fetch = originalFetch
      }),
  )

describe("desktop l402 executor spark flow", () => {
  it.effect("prefers spark backend and caches credential on repeat requests", () => {
    const payCalls = { current: 0 }

    const sparkGatewayLayer = Layer.succeed(
      SparkWalletGatewayService,
      SparkWalletGatewayService.of({
        snapshot: () =>
          Effect.succeed({
            lifecycle: "connected",
            network: "regtest",
            apiKeyConfigured: true,
            mnemonicStored: true,
            identityPubkey: "spark-pub",
            balanceSats: 5000,
            tokenBalanceCount: 0,
            lastSyncedAtMs: Date.now(),
            lastPaymentId: null,
            lastPaymentAtMs: null,
            lastErrorCode: null,
            lastErrorMessage: null,
          }),
        bootstrap: () => Effect.void,
        refresh: () =>
          Effect.succeed({
            lifecycle: "connected",
            network: "regtest",
            apiKeyConfigured: true,
            mnemonicStored: true,
            identityPubkey: "spark-pub",
            balanceSats: 5000,
            tokenBalanceCount: 0,
            lastSyncedAtMs: Date.now(),
            lastPaymentId: null,
            lastPaymentAtMs: null,
            lastErrorCode: null,
            lastErrorMessage: null,
          }),
        payInvoice: (request) =>
          Effect.sync(() => {
            payCalls.current += 1
            return {
              paymentId: `spark-pay-${payCalls.current}`,
              amountMsats: Math.min(request.maxAmountMsats, 2500),
              preimageHex: "ab".repeat(32),
              paidAtMs: Date.now(),
            }
          }),
      }),
    )

    const layer = Layer.provideMerge(L402ExecutorLive, sparkGatewayLayer)
    const task = makeTask("https://seller.example.com/premium")

    return withFetchHarness(
      Effect.gen(function* () {
        const executor = yield* L402ExecutorService

        const first = yield* executor.execute(task)
        expect(first.status).toBe("paid")
        if (first.status === "paid" || first.status === "cached") {
          expect(first.paymentBackend).toBe("spark")
          expect(first.proofReference).toMatch(/^preimage:/)
        }
        expect(payCalls.current).toBe(1)

        const second = yield* executor.execute(task)
        expect(second.status).toBe("cached")
        if (second.status === "paid" || second.status === "cached") {
          expect(second.paymentBackend).toBe("spark")
        }
        expect(payCalls.current).toBe(1)
      }).pipe(Effect.provide(layer)),
    )
  })
})
