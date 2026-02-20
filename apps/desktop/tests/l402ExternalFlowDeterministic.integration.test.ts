import http from "node:http"
import type { AddressInfo } from "node:net"

import { Effect, Layer, Option } from "effect"
import { describe, expect, it } from "@effect/vitest"

import {
  CredentialCacheInMemoryLayer,
  L402ClientLiveLayer,
  L402ClientService,
  L402TransportError,
  L402TransportService,
  InvoicePayerService,
  makeInvoicePayerDemoLayer,
  makeSpendPolicyLayer,
} from "@openagentsinc/lightning-effect"

import { AuthGatewayService } from "../src/effect/authGateway"
import { ConnectivityProbeService } from "../src/effect/connectivity"
import { DesktopAppService } from "../src/effect/app"
import { makeDesktopLayer } from "../src/effect/layer"
import {
  L402ExecutorService,
  type L402ExecutionResult,
} from "../src/effect/l402Executor"
import { TaskProviderService } from "../src/effect/taskProvider"
import type {
  ExecutorTask,
  ExecutorTaskRequest,
  ExecutorTaskStatus,
} from "../src/effect/model"

import crypto from "node:crypto"

type SellerCall = Readonly<{
  readonly path: string
  readonly auth: string | null
  readonly status: number
}>

type TaskEvent = Readonly<{
  readonly taskId: string
  readonly fromStatus?: ExecutorTaskStatus
  readonly toStatus: ExecutorTaskStatus
  readonly reason?: string
  readonly errorCode?: string
  readonly errorMessage?: string
  readonly metadata?: unknown
}>

const now = () => Date.now()

const json = (status: number, body: unknown, headers?: Record<string, string>) =>
  ({
    status,
    headers: {
      "content-type": "application/json; charset=utf-8",
      ...(headers ?? {}),
    },
    body: JSON.stringify(body),
  }) as const

const startMockL402Seller = async () => {
  const calls: Array<SellerCall> = []

  const server = http.createServer((req, res) => {
    const url = new URL(req.url ?? "/", "http://127.0.0.1")
    const auth = typeof req.headers.authorization === "string" ? req.headers.authorization : null

    const respond = (payload: { readonly status: number; readonly headers: Record<string, string>; readonly body: string }) => {
      res.statusCode = payload.status
      for (const [k, v] of Object.entries(payload.headers)) res.setHeader(k, v)
      res.end(payload.body)
    }

    if (url.pathname === "/premium") {
      if (!auth || !auth.startsWith("L402 ")) {
        calls.push({ path: url.pathname, auth: null, status: 402 })
        return respond(
          json(
            402,
            { error: "payment_required" },
            {
              "www-authenticate":
                'L402 invoice="lnbcrt1invoice_demo_premium", macaroon="mac_demo_premium", amount_msats=2500',
            },
          ),
        )
      }

      calls.push({ path: url.pathname, auth, status: 200 })
      return respond(json(200, { ok: true, resource: "premium" }, { "content-type": "application/json" }))
    }

    if (url.pathname === "/overcap") {
      // Force a new 402 quote even if a cached token exists, so tests can assert
      // spend-cap blocks happen before paying/retrying.
      calls.push({ path: url.pathname, auth, status: 402 })
      return respond(
        json(
          402,
          { error: "payment_required" },
          {
            "www-authenticate":
              'L402 invoice="lnbcrt1invoice_demo_overcap", macaroon="mac_demo_overcap", amount_msats=9000',
          },
        ),
      )
    }

    calls.push({ path: url.pathname, auth, status: 404 })
    return respond(json(404, { ok: false, error: "not_found" }))
  })

  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", () => resolve()))
  const address = server.address() as AddressInfo
  const baseUrl = `http://127.0.0.1:${address.port}`

  return {
    baseUrl,
    calls,
    close: async () => {
      await new Promise<void>((resolve, reject) =>
        server.close((err) => (err ? reject(err) : resolve())),
      )
    },
  } as const
}

const authGatewayTestLayer = Layer.succeed(
  AuthGatewayService,
  AuthGatewayService.of({
    startMagicCode: () => Effect.void,
    verifyMagicCode: ({ email }) =>
      Effect.succeed({
        userId: "user_ep212_test",
        token: "token_ep212_test",
        user: {
          id: "user_ep212_test",
          email,
          firstName: "EP",
          lastName: "212",
        },
      }),
    getSession: () =>
      Effect.succeed({
        userId: null,
        token: null,
        user: null,
      }),
  }),
)

const connectivityTestLayer = Layer.succeed(
  ConnectivityProbeService,
  ConnectivityProbeService.of({
    probe: () =>
      Effect.succeed({
        openAgentsReachable: true,
        syncReachable: true,
        syncProvider: "khala",
        checkedAtMs: now(),
      }),
  }),
)

type InMemoryTaskProviderStore = {
  readonly tasks: Array<ExecutorTask>
  readonly events: Array<TaskEvent>
}

const makeInMemoryTaskProviderLayer = (store: InMemoryTaskProviderStore) =>
  Layer.succeed(
    TaskProviderService,
    TaskProviderService.of({
      heartbeatExecutorPresence: () => Effect.void,

      enqueueDemoTask: ({ payload, token }) =>
        Effect.sync(() => {
          void token
          const task: ExecutorTask = {
            id: crypto.randomUUID(),
            ownerId: "user_ep212_test",
            status: "queued",
            request: {
              url: payload.trim(),
              method: "GET",
              maxSpendMsats: 2_500,
              scope: "episode-212",
            } satisfies ExecutorTaskRequest,
            attemptCount: 0,
            createdAtMs: now(),
            updatedAtMs: now(),
          }
          store.tasks.push(task)
          return task
        }),

      listTasks: ({ token, status, limit }) =>
        Effect.sync(() => {
          void token
          void status
          void limit
          return store.tasks.slice()
        }),

      pollPendingTask: ({ userId, token }) =>
        Effect.sync(() => {
          void userId
          void token
          const next = store.tasks
            .filter((row) => row.status === "approved")
            .sort((a, b) => a.createdAtMs - b.createdAtMs)[0]
          return next ? Option.some(next) : Option.none<ExecutorTask>()
        }),

      transitionTask: ({ taskId, token, toStatus, reason, errorCode, errorMessage, metadata }) =>
        Effect.sync(() => {
          void token
          const index = store.tasks.findIndex((row) => row.id === taskId)
          if (index < 0) throw new Error("task_not_found")
          const current = store.tasks[index]!
          const next: ExecutorTask = {
            ...current,
            status: toStatus,
            updatedAtMs: now(),
            ...(errorCode ? { lastErrorCode: errorCode } : {}),
            ...(errorMessage ? { lastErrorMessage: errorMessage, failureReason: errorMessage } : {}),
            ...(metadata !== undefined ? { metadata } : {}),
          }
          store.tasks[index] = next
          store.events.push({
            taskId,
            fromStatus: current.status,
            toStatus,
            ...(reason ? { reason } : {}),
            ...(errorCode ? { errorCode } : {}),
            ...(errorMessage ? { errorMessage } : {}),
            ...(metadata !== undefined ? { metadata } : {}),
          })
          return next
        }),
    }),
  )

const makeDemoL402ExecutorLayer = (payCalls: { count: number }) =>
  Layer.effect(
    L402ExecutorService,
    Effect.gen(function* () {
      const transportLayer = Layer.succeed(
        L402TransportService,
        L402TransportService.of({
          send: (request) =>
            Effect.tryPromise({
              try: async () => {
                const response = await fetch(request.url, {
                  method: request.method ?? "GET",
                  cache: "no-store",
                  ...(request.headers ? { headers: request.headers } : {}),
                  ...(request.body !== undefined ? { body: request.body } : {}),
                })
                const bodyText = await response.text().catch(() => "")
                const headers: Record<string, string> = {}
                response.headers.forEach((value, key) => {
                  headers[key] = value
                })
                return {
                  status: response.status,
                  headers,
                  ...(bodyText.length > 0 ? { body: bodyText } : {}),
                }
              },
              catch: (error) =>
                L402TransportError.make({
                  reason: String(error),
                }),
            }),
        }),
      )

      const countingPayerLayer = Layer.effect(
        InvoicePayerService,
        Effect.gen(function* () {
          const base = yield* InvoicePayerService
          return InvoicePayerService.of({
            payInvoice: (request) =>
              Effect.sync(() => {
                payCalls.count += 1
                return request
              }).pipe(Effect.zipRight(base.payInvoice(request))),
          })
        }),
      ).pipe(Layer.provide(makeInvoicePayerDemoLayer()))

      const baseLayer = Layer.mergeAll(
        CredentialCacheInMemoryLayer,
        makeSpendPolicyLayer({
          defaultMaxSpendMsats: 50_000,
          allowedHosts: [],
          blockedHosts: [],
        }),
        transportLayer,
        countingPayerLayer,
      )
      const clientLayer = L402ClientLiveLayer.pipe(Layer.provide(baseLayer))
      const client = yield* Effect.gen(function* () {
        return yield* L402ClientService
      }).pipe(Effect.provide(clientLayer))

      const truncateUtf8 = (value: string, maxBytes: number): string => {
        const buf = Buffer.from(value, "utf8")
        if (buf.length <= maxBytes) return value
        return buf.subarray(0, Math.max(0, maxBytes)).toString("utf8")
      }

      const execute = (task: ExecutorTask) =>
        Effect.gen(function* () {
          const exit = yield* Effect.either(client.fetchWithL402(task.request))
          if (exit._tag === "Left") {
            const err = exit.left as any
            const tag = typeof err?._tag === "string" ? err._tag : "unknown_error"
            const denyReason =
              typeof err?.reason === "string" && err.reason.trim().length > 0
                ? err.reason
                : typeof err?.message === "string" && err.message.trim().length > 0
                  ? err.message
                  : tag

            if (tag === "BudgetExceededError") {
              return {
                status: "blocked",
                errorCode: tag,
                denyReason,
                denyReasonCode: typeof err?.reasonCode === "string" ? err.reasonCode : null,
                host: typeof err?.host === "string" ? err.host : null,
                maxSpendMsats:
                  typeof err?.maxSpendMsats === "number" && Number.isFinite(err.maxSpendMsats)
                    ? Math.max(0, Math.floor(err.maxSpendMsats))
                    : Math.max(0, Math.floor(task.request.maxSpendMsats)),
                quotedAmountMsats:
                  typeof err?.quotedAmountMsats === "number" && Number.isFinite(err.quotedAmountMsats)
                    ? Math.max(0, Math.floor(err.quotedAmountMsats))
                    : null,
                paymentBackend: "lnd_deterministic",
              } satisfies L402ExecutionResult
            }

            return {
              status: "failed",
              errorCode: tag,
              denyReason,
              paymentBackend: "lnd_deterministic",
            } satisfies L402ExecutionResult
          }

          const result = exit.right
          const responseBody = typeof result.responseBody === "string" ? result.responseBody : null
          const responseBytes = responseBody ? Buffer.byteLength(responseBody, "utf8") : null
          const responseBodySha256 = responseBody
            ? crypto.createHash("sha256").update(responseBody, "utf8").digest("hex")
            : null
          const responseBodyTextPreview = responseBody ? truncateUtf8(responseBody, 8_192) : null
          const responseContentType =
            typeof result.responseContentType === "string" && result.responseContentType.trim().length > 0
              ? result.responseContentType.trim()
              : null
          const cacheHit = result.fromCache === true || result.cacheStatus === "hit"
          const paymentBackend = "lnd_deterministic"

          return result.paid
            ? ({
                status: "paid",
                amountMsats: result.amountMsats,
                paymentId: result.paymentId,
                proofReference: result.proofReference,
                responseStatusCode: result.statusCode,
                responseContentType,
                responseBytes,
                responseBodyTextPreview,
                responseBodySha256,
                cacheHit,
                paid: true,
                cacheStatus: result.cacheStatus,
                paymentBackend,
              } satisfies L402ExecutionResult)
            : ({
                status: "cached",
                amountMsats: result.amountMsats,
                paymentId: result.paymentId,
                proofReference: result.proofReference,
                responseStatusCode: result.statusCode,
                responseContentType,
                responseBytes,
                responseBodyTextPreview,
                responseBodySha256,
                cacheHit,
                paid: false,
                cacheStatus: result.cacheStatus,
                paymentBackend,
              } satisfies L402ExecutionResult)
        })

      return L402ExecutorService.of({ execute })
    }),
  )

describe("desktop EP212 deterministic L402 flow (mock seller)", () => {
  it.effect("approval gating, paid -> cached, and overcap blocks pre-payment", () => {
    const payCalls = { count: 0 }
    const start = Effect.tryPromise({
      try: () => startMockL402Seller(),
      catch: (error) => error as any,
    })

    return Effect.scoped(
      Effect.acquireRelease(
        start,
        (seller) =>
          Effect.tryPromise({
            try: () => seller.close(),
            catch: () => undefined,
          }).pipe(Effect.orDie),
      ).pipe(
        Effect.flatMap((seller) => {
        const store: InMemoryTaskProviderStore = {
          tasks: [],
          events: [],
        }

        const desktopLayer = makeDesktopLayer(
          {
            openAgentsBaseUrl: "https://openagents.local",
            khalaSyncEnabled: true,
            khalaSyncUrl: "wss://khala.local/sync/socket/websocket",
            executorTickMs: 25,
          },
          {
            authGateway: authGatewayTestLayer,
            connectivity: connectivityTestLayer,
            taskProvider: makeInMemoryTaskProviderLayer(store),
            l402Executor: makeDemoL402ExecutorLayer(payCalls),
          },
        ) as unknown as Layer.Layer<DesktopAppService | TaskProviderService, never, never>

        return Effect.gen(function* () {
          const app = yield* DesktopAppService
          const tasks = yield* TaskProviderService

          yield* app.bootstrap()
          yield* app.verifyMagicCode({ email: "ep212@openagents.com", code: "123456" })

          const premiumUrl = `${seller.baseUrl}/premium`
          const overcapUrl = `${seller.baseUrl}/overcap`

          const queuedA = yield* app.enqueueDemoTask(premiumUrl)
          expect(queuedA.status).toBe("queued")

          // Approval gating: should not execute queued tasks.
          yield* app.tickExecutor()
          expect(seller.calls).toHaveLength(0)
          expect(store.events).toHaveLength(0)

          yield* tasks.transitionTask({
            taskId: queuedA.id,
            token: "token_ep212_test",
            toStatus: "approved",
            reason: "user_approved",
          })
          yield* app.tickExecutor()

          const eventsAfterA = store.events.slice()
          expect(eventsAfterA.map((e) => e.toStatus)).toEqual(["approved", "running", "paid", "completed"])
          expect(seller.calls.map((c) => c.status)).toEqual([402, 200])
          expect(payCalls.count).toBe(1)

          const paidEvent = eventsAfterA.find((e) => e.toStatus === "paid")
          const paidMeta = (paidEvent?.metadata ?? null) as any
          expect(paidMeta?.paid).toBe(true)
          expect(paidMeta?.cacheHit).toBe(false)
          expect(typeof paidMeta?.responseBodyTextPreview).toBe("string")
          expect(typeof paidMeta?.responseBodySha256).toBe("string")

          // Second task to the same host/scope should use credential cache (no new payment).
          const queuedB = yield* app.enqueueDemoTask(premiumUrl)
          yield* tasks.transitionTask({
            taskId: queuedB.id,
            token: "token_ep212_test",
            toStatus: "approved",
            reason: "user_approved",
          })
          yield* app.tickExecutor()

          const eventsAfterB = store.events.slice()
          const lastFour = eventsAfterB.slice(-4).map((e) => e.toStatus)
          expect(lastFour).toEqual(["approved", "running", "cached", "completed"])
          expect(payCalls.count).toBe(1)
          expect(seller.calls).toHaveLength(3)
          expect(seller.calls[2]?.status).toBe(200)
          expect(seller.calls[2]?.auth).toMatch(/^L402 /)

          const cachedEvent = eventsAfterB.slice(-4).find((e) => e.toStatus === "cached")
          const cachedMeta = (cachedEvent?.metadata ?? null) as any
          expect(cachedMeta?.paid).toBe(false)
          expect(cachedMeta?.cacheHit).toBe(true)
          expect(cachedMeta?.cacheStatus).toBe("hit")

          // Overcap: should block after seeing the 402 quote, before paying/retrying.
          const queuedC = yield* app.enqueueDemoTask(overcapUrl)
          yield* tasks.transitionTask({
            taskId: queuedC.id,
            token: "token_ep212_test",
            toStatus: "approved",
            reason: "user_approved",
          })
          yield* app.tickExecutor()

          const eventsAfterC = store.events.slice()
          const lastThree = eventsAfterC.slice(-3).map((e) => e.toStatus)
          expect(lastThree).toEqual(["approved", "running", "blocked"])
          expect(payCalls.count).toBe(1)
          expect(seller.calls).toHaveLength(4)
          expect(seller.calls[3]?.status).toBe(402)
          expect(seller.calls[3]?.path).toBe("/overcap")
        }).pipe(Effect.provide(desktopLayer))
      }),
      ),
    )
  })
})
