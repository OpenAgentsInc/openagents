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

  it.effect("blocks pre-payment when quoted invoice amount exceeds cap (no payer call)", () => {
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
        payInvoice: (_request) =>
          Effect.sync(() => {
            payCalls.current += 1
            return {
              paymentId: `spark-pay-${payCalls.current}`,
              amountMsats: 0,
              preimageHex: "ab".repeat(32),
              paidAtMs: Date.now(),
            }
          }),
      }),
    )

    const layer = Layer.provideMerge(L402ExecutorLive, sparkGatewayLayer)
    const baseTask = makeTask("https://seller.example.com/premium-overcap")
    const task: ExecutorTask = {
      ...baseTask,
      request: {
        ...baseTask.request,
        maxSpendMsats: 1_000,
      },
    }

    return withFetchHarness(
      Effect.gen(function* () {
        const executor = yield* L402ExecutorService
        const res = yield* executor.execute(task)
        expect(res.status).toBe("blocked")
        if (res.status === "blocked") {
          expect(res.errorCode).toBe("BudgetExceededError")
          expect(res.denyReasonCode).toBe("amount_over_cap")
          expect(res.host).toBe("seller.example.com")
          expect(res.maxSpendMsats).toBe(1_000)
          expect(res.quotedAmountMsats).toBe(2_500)
        }
        expect(payCalls.current).toBe(0)
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

  it.effect("uses persistent credential cache bridge when available (survives executor re-init)", () => {
    const payCalls = { current: 0 }
    const putCalls = { current: 0 }

    const cache = new Map<string, { readonly credential: unknown; readonly expiresAtMs: number }>()
    const toKey = (host: string, scope: string) => `${host.trim().toLowerCase()}::${scope.trim().toLowerCase()}`
    const bridge = {
      getByHost: async (input: { readonly host: string; readonly scope: string; readonly nowMs: number }) => {
        const entry = cache.get(toKey(input.host, input.scope))
        if (!entry) return { _tag: "miss" as const }
        if (input.nowMs >= entry.expiresAtMs) return { _tag: "stale" as const, credential: entry.credential }
        return { _tag: "hit" as const, credential: entry.credential }
      },
      putByHost: async (input: {
        readonly host: string
        readonly scope: string
        readonly credential: unknown
        readonly options?: { readonly ttlMs?: number }
      }) => {
        putCalls.current += 1
        const ttlMs = Math.max(0, Math.floor(input.options?.ttlMs ?? 10 * 60 * 1000))
        const issuedAtMs =
          typeof (input.credential as { readonly issuedAtMs?: unknown } | null)?.issuedAtMs === "number"
            ? Math.max(0, Math.floor((input.credential as { readonly issuedAtMs: number }).issuedAtMs))
            : Date.now()
        cache.set(toKey(input.host, input.scope), {
          credential: input.credential,
          expiresAtMs: issuedAtMs + ttlMs,
        })
      },
      markInvalid: async (input: { readonly host: string; readonly scope: string }) => {
        cache.delete(toKey(input.host, input.scope))
      },
      clearHost: async (input: { readonly host: string; readonly scope: string }) => {
        cache.delete(toKey(input.host, input.scope))
      },
    }

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

    const task = makeTask("https://seller.example.com/premium-persisted")

    const withWindowHarness = <A, E, R>(program: Effect.Effect<A, E, R>) =>
      Effect.acquireUseRelease(
        Effect.sync(() => {
          const g = globalThis as unknown as { window?: unknown }
          const current = g.window
          g.window = {
            openAgentsDesktop: {
              l402CredentialCache: bridge,
            },
          }
          return current
        }),
        () => program,
        (priorWindow) =>
          Effect.sync(() => {
            const g = globalThis as unknown as { window?: unknown }
            if (priorWindow === undefined) {
              delete g.window
            } else {
              g.window = priorWindow
            }
          }),
      )

    return withWindowHarness(
      withFetchHarness(
        Effect.gen(function* () {
          const runOnce = () =>
            Effect.gen(function* () {
              const executor = yield* L402ExecutorService
              return yield* executor.execute(task)
            }).pipe(Effect.provide(Layer.provideMerge(L402ExecutorLive, sparkGatewayLayer)))

          const first = yield* runOnce()
          expect(first.status).toBe("paid")
          expect(payCalls.current).toBe(1)
          expect(putCalls.current).toBe(1)

          const second = yield* runOnce()
          expect(second.status).toBe("cached")
          expect(payCalls.current).toBe(1)
          expect(putCalls.current).toBe(1)
        }),
      ),
    )
  })
})
