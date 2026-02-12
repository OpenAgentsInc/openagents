import { Effect, Layer } from "effect"
import { describe, expect, it } from "@effect/vitest"
import crypto from "node:crypto"

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
  opts?: {
    readonly successBody?: string
  },
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
              "www-authenticate": 'L402 invoice="lnbcrt1sparkflow", macaroon="spark_macaroon", amount_msats=2500',
            },
          })
        }
        const body = opts?.successBody ?? '{"ok":true}'
        return new Response(body, {
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
          expect(first.responseContentType).toBe("application/json; charset=utf-8")
          expect(first.responseBytes).toBe(11)
          expect(first.responseBodyTextPreview).toBe('{"ok":true}')
          expect(first.responseBodySha256).toBe(
            "4062edaf750fb8074e7e83e0c9028c94e32468a8b6f1614774328ef045150f93",
          )
          expect(first.cacheHit).toBe(false)
          expect(first.paid).toBe(true)
          expect(first.cacheStatus).toBe("miss")
        }
        expect(payCalls.current).toBe(1)

        const second = yield* executor.execute(task)
        expect(second.status).toBe("cached")
        if (second.status === "paid" || second.status === "cached") {
          expect(second.paymentBackend).toBe("spark")
          expect(second.cacheHit).toBe(true)
          expect(second.paid).toBe(false)
          expect(second.cacheStatus).toBe("hit")
        }
        expect(payCalls.current).toBe(1)
      }).pipe(Effect.provide(layer)),
    )
  })

  it.effect("stores bounded payload preview + sha256 for large bodies", () => {
    const bigBody = "x".repeat(9_000)
    const expectedPreview = bigBody.slice(0, 8_192)
    const expectedSha = crypto.createHash("sha256").update(bigBody, "utf8").digest("hex")

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
    const task = makeTask("https://seller.example.com/premium-big")

    return withFetchHarness(
      Effect.gen(function* () {
        const executor = yield* L402ExecutorService

        const res = yield* executor.execute(task)
        expect(res.status).toBe("paid")
        if (res.status === "paid" || res.status === "cached") {
          expect(res.responseBytes).toBe(9_000)
          expect(res.responseBodyTextPreview).toBe(expectedPreview)
          expect(res.responseBodyTextPreview?.length).toBe(8_192)
          expect(res.responseBodySha256).toBe(expectedSha)
        }
        expect(payCalls.current).toBe(1)
      }).pipe(Effect.provide(layer)),
      { successBody: bigBody },
    )
  })
})

