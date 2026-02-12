import crypto from "node:crypto"

import {
  BudgetExceededError,
  buildAuthorizationHeader,
  DomainNotAllowedError,
  fetchWithL402,
  L402TransportError,
  parseChallengeHeader,
  PaymentFailedError,
  PaymentMissingPreimageError,
  type L402Credential,
  type FetchWithL402Deps,
  type InvoicePayerApi,
} from "@openagentsinc/lightning-effect"
import { Effect } from "effect"

import { ConfigError } from "../errors.js"

export type Ep212RoutesSmokeMode = "mock" | "live"

export type Ep212RoutesSmokeSummary = Readonly<{
  ok: true
  requestId: string
  mode: Ep212RoutesSmokeMode
  walletBackend: "mock" | "wallet_executor"
  routeA: Readonly<{
    url: string
    challengeStatusCode: number
    quotedAmountMsats: number | null
    paidStatusCode: number
    paidAmountMsats: number
    paymentId: string | null
    proofReference: string
    responseBytes: number
    responseSha256: string
  }>
  routeB: Readonly<{
    url: string
    challengeStatusCode: number
    quotedAmountMsats: number | null
    maxSpendMsats: number
    blocked: true
    denyReasonCode: string
    payerCallsBefore: number
    payerCallsAfter: number
  }>
}>

type FetchLike = (input: string, init?: RequestInit) => Promise<Response>

const MOCK_ROUTE_A_URL = "https://l402.openagents.com/ep212/premium-signal"
const MOCK_ROUTE_B_URL = "https://l402.openagents.com/ep212/expensive-signal"
const MOCK_CAP_MSATS = 100_000
const MOCK_ROUTE_A_AMOUNT_MSATS = 70_000
const MOCK_ROUTE_B_AMOUNT_MSATS = 250_000
const ROUTE_A_SCOPE = "ep212-route-a"
const ROUTE_B_SCOPE = "ep212-route-b"

const sha256Hex = (value: string): string => crypto.createHash("sha256").update(value, "utf8").digest("hex")

const deterministicPreimage = (invoice: string): string => sha256Hex(`ep212:${invoice}`).slice(0, 64)

const envString = (key: string, fallback?: string): Effect.Effect<string, ConfigError> =>
  Effect.sync(() => process.env[key]?.trim() || fallback || "").pipe(
    Effect.flatMap((value) =>
      value.length > 0
        ? Effect.succeed(value)
        : ConfigError.make({
            field: key,
            message: "missing required environment variable",
          }),
    ),
  )

const envInt = (key: string, fallback: number): Effect.Effect<number, ConfigError> =>
  Effect.sync(() => process.env[key]?.trim() || String(fallback)).pipe(
    Effect.flatMap((raw) => {
      const parsed = Number(raw)
      if (Number.isFinite(parsed) && parsed > 0) return Effect.succeed(Math.floor(parsed))
      return ConfigError.make({
        field: key,
        message: "must be a positive integer",
      })
    }),
  )

const authorizationFromHeaders = (headers: Headers | Record<string, string> | undefined): string | null => {
  if (!headers) return null
  if (headers instanceof Headers) {
    const value = headers.get("authorization")
    return value && value.trim().length > 0 ? value.trim() : null
  }
  for (const [key, value] of Object.entries(headers)) {
    if (key.toLowerCase() !== "authorization") continue
    const next = String(value ?? "").trim()
    return next.length > 0 ? next : null
  }
  return null
}

const toHeadersRecord = (headers: Headers): Record<string, string> => {
  const out: Record<string, string> = {}
  headers.forEach((value, key) => {
    out[key] = value
  })
  return out
}

const parseChallengeFromResponse = (response: Response) =>
  Effect.gen(function* () {
    if (response.status !== 402) {
      return yield* Effect.fail(new Error(`expected 402 challenge, got status ${response.status}`))
    }
    const header = response.headers.get("www-authenticate")
    if (!header || header.trim().length === 0) {
      return yield* Effect.fail(new Error("expected www-authenticate header on 402 challenge"))
    }
    const challenge = yield* parseChallengeHeader(header)
    return {
      statusCode: response.status,
      amountMsats: typeof challenge.amountMsats === "number" ? challenge.amountMsats : null,
    }
  })

const checkChallenge = (fetchFn: FetchLike, url: string) =>
  Effect.tryPromise({
    try: () => fetchFn(url, { method: "GET" }),
    catch: (error) => new Error(String(error)),
  }).pipe(Effect.flatMap(parseChallengeFromResponse))

const createL402Deps = (input: {
  readonly fetchFn: FetchLike
  readonly payer: InvoicePayerApi
  readonly defaultMaxSpendMsats: number
  readonly allowedHosts: ReadonlyArray<string>
}): FetchWithL402Deps => {
  const cacheStore = new Map<string, { credential: L402Credential; expiresAtMs: number | null }>()
  const normalizeHost = (value: string): string => value.trim().toLowerCase()
  const allowedHosts = [...new Set(input.allowedHosts.map(normalizeHost))]
  const cacheKey = (host: string, scope: string): string => `${normalizeHost(host)}::${scope}`

  return {
    credentialCache: {
      getByHost: (host, scope, nowMs) =>
        Effect.sync(() => {
          const key = cacheKey(host, scope)
          const hit = cacheStore.get(key)
          if (!hit) return { _tag: "miss" } as const
          if (hit.expiresAtMs !== null && nowMs > hit.expiresAtMs) {
            return { _tag: "stale", credential: hit.credential } as const
          }
          return { _tag: "hit", credential: hit.credential } as const
        }),
      putByHost: (host, scope, credential, options) =>
        Effect.sync(() => {
          const ttlMs = typeof options?.ttlMs === "number" && Number.isFinite(options.ttlMs) ? Math.max(0, Math.floor(options.ttlMs)) : null
          cacheStore.set(cacheKey(host, scope), {
            credential,
            expiresAtMs: ttlMs === null ? null : Date.now() + ttlMs,
          })
        }),
      markInvalid: (host, scope) => Effect.sync(() => void cacheStore.delete(cacheKey(host, scope))),
      clearHost: (host, scope) => Effect.sync(() => void cacheStore.delete(cacheKey(host, scope))),
    },
    payer: input.payer,
    policy: {
      policy: {
        defaultMaxSpendMsats: input.defaultMaxSpendMsats,
        allowedHosts,
        blockedHosts: [],
      },
      ensureRequestAllowed: ({ host, quotedAmountMsats, maxSpendMsats }) =>
        Effect.gen(function* () {
          const normalizedHost = normalizeHost(host)
          if (!allowedHosts.includes(normalizedHost)) {
            return yield* DomainNotAllowedError.make({
              host: normalizedHost,
              reasonCode: "host_not_allowlisted",
              reason: "Host is not present in allowlist",
            })
          }
          const effectiveCap = Math.min(input.defaultMaxSpendMsats, Math.max(0, Math.floor(maxSpendMsats)))
          if (quotedAmountMsats > effectiveCap) {
            return yield* BudgetExceededError.make({
              maxSpendMsats: effectiveCap,
              quotedAmountMsats,
              reasonCode: "amount_over_cap",
              reason: "Quoted invoice amount exceeds configured spend cap",
            })
          }
        }),
    },
    transport: {
      send: (request) =>
        Effect.tryPromise({
          try: async () => {
            const body =
              request.body === undefined
                ? undefined
                : typeof request.body === "string"
                  ? request.body
                  : JSON.stringify(request.body)
            const requestInit: RequestInit = {
              method: request.method ?? "GET",
            }
            if (request.headers) requestInit.headers = request.headers
            if (body !== undefined) requestInit.body = body

            const response = await input.fetchFn(request.url, requestInit)
            const responseBody = await response.text()
            return {
              status: response.status,
              headers: toHeadersRecord(response.headers),
              body: responseBody,
            }
          },
          catch: (error) =>
            L402TransportError.make({
              reason: String(error),
            }),
        }),
    },
  }
}

const createMockFetch = (): FetchLike => {
  const routeAHost = new URL(MOCK_ROUTE_A_URL).host
  const routeAAuth = buildAuthorizationHeader({
    host: routeAHost,
    scope: ROUTE_A_SCOPE,
    macaroon: "mac_ep212_a",
    preimageHex: deterministicPreimage("lnmock_ep212_route_a"),
    amountMsats: MOCK_ROUTE_A_AMOUNT_MSATS,
    issuedAtMs: 0,
  })

  return async (input, init) => {
    const url = new URL(input)
    const method = (init?.method ?? "GET").toUpperCase()
    const auth = authorizationFromHeaders(init?.headers as Record<string, string> | undefined)
    if (method !== "GET") return new Response(JSON.stringify({ ok: false, error: "method_not_allowed" }), { status: 405 })

    if (url.pathname === "/ep212/premium-signal") {
      if (!auth) {
        return new Response(JSON.stringify({ ok: false, error: "payment_required" }), {
          status: 402,
          headers: {
            "content-type": "application/json; charset=utf-8",
            "www-authenticate":
              'L402 invoice="lnmock_ep212_route_a", macaroon="mac_ep212_a", amount_msats=70000',
          },
        })
      }
      if (auth !== routeAAuth) {
        return new Response(JSON.stringify({ ok: false, error: "credential_rejected" }), { status: 401 })
      }
      return new Response(
        JSON.stringify({
          ok: true,
          source: "mock.ep212",
          route: "/ep212/premium-signal",
          signal: { symbol: "BTC", confidence: 0.8123, horizon: "4h" },
        }),
        {
          status: 200,
          headers: { "content-type": "application/json; charset=utf-8" },
        },
      )
    }

    if (url.pathname === "/ep212/expensive-signal") {
      return new Response(JSON.stringify({ ok: false, error: "payment_required" }), {
        status: 402,
        headers: {
          "content-type": "application/json; charset=utf-8",
          "www-authenticate":
            'L402 invoice="lnmock_ep212_route_b", macaroon="mac_ep212_b", amount_msats=250000',
        },
      })
    }

    return new Response(JSON.stringify({ ok: false, error: "not_found" }), { status: 404 })
  }
}

const createMockPayer = (calls: { count: number }): InvoicePayerApi => ({
  payInvoice: (request) =>
    Effect.sync(() => {
      calls.count += 1
      const amountMsats =
        request.invoice === "lnmock_ep212_route_b"
          ? MOCK_ROUTE_B_AMOUNT_MSATS
          : MOCK_ROUTE_A_AMOUNT_MSATS
      return {
        paymentId: `mock_pay_${calls.count}`,
        amountMsats,
        preimageHex: deterministicPreimage(request.invoice),
        paidAtMs: 1_736_000_000_000 + calls.count,
      }
    }),
})

const createWalletExecutorPayer = (input: {
  readonly fetchFn: FetchLike
  readonly baseUrl: string
  readonly authToken: string | null
  readonly timeoutMs: number
  readonly requestId: string
  readonly calls: { count: number }
}): InvoicePayerApi => ({
  payInvoice: (request) =>
    Effect.gen(function* () {
      input.calls.count += 1
      const controller = new AbortController()
      const timeout = setTimeout(() => controller.abort(), input.timeoutMs)

      const response = yield* Effect.tryPromise({
        try: () =>
          input.fetchFn(`${input.baseUrl}/pay-bolt11`, {
            method: "POST",
            headers: {
              "content-type": "application/json",
              "x-request-id": input.requestId,
              ...(input.authToken ? { authorization: `Bearer ${input.authToken}` } : {}),
            },
            body: JSON.stringify({
              requestId: input.requestId,
              payment: request,
            }),
            signal: controller.signal,
          }),
        catch: (error) =>
          PaymentFailedError.make({
            invoice: request.invoice,
            reason: String(error),
          }),
      }).pipe(Effect.ensuring(Effect.sync(() => clearTimeout(timeout))))

      const payloadRaw = yield* Effect.tryPromise({
        try: () => response.text(),
        catch: () =>
          PaymentFailedError.make({
            invoice: request.invoice,
            reason: "wallet_executor_response_read_failed",
          }),
      })

      const parsed = (() => {
        if (!payloadRaw || payloadRaw.trim().length === 0) return null
        try {
          return JSON.parse(payloadRaw) as Record<string, unknown>
        } catch {
          return null
        }
      })()

      const errorRec =
        parsed && parsed.error && typeof parsed.error === "object"
          ? (parsed.error as Record<string, unknown>)
          : null

      if (!response.ok) {
        const code = typeof errorRec?.code === "string" ? errorRec.code : `wallet_executor_http_${response.status}`
        const message = typeof errorRec?.message === "string" ? errorRec.message : null
        return yield* PaymentFailedError.make({
          invoice: request.invoice,
          reason: [code, message].filter(Boolean).join(": "),
        })
      }

      const payment =
        parsed &&
        parsed.result &&
        typeof parsed.result === "object" &&
        (parsed.result as Record<string, unknown>).payment &&
        typeof (parsed.result as Record<string, unknown>).payment === "object"
          ? ((parsed.result as Record<string, unknown>).payment as Record<string, unknown>)
          : null

      const paymentId = typeof payment?.paymentId === "string" ? payment.paymentId.trim() : ""
      const preimageHex = typeof payment?.preimageHex === "string" ? payment.preimageHex.trim() : ""
      const amountMsats = typeof payment?.amountMsats === "number" && Number.isFinite(payment.amountMsats)
        ? Math.max(0, Math.floor(payment.amountMsats))
        : NaN
      const paidAtMs = typeof payment?.paidAtMs === "number" && Number.isFinite(payment.paidAtMs)
        ? Math.max(0, Math.floor(payment.paidAtMs))
        : Date.now()

      if (!paymentId || !Number.isFinite(amountMsats)) {
        return yield* PaymentFailedError.make({
          invoice: request.invoice,
          reason: "wallet_executor_invalid_payment_shape",
        })
      }
      if (!preimageHex) {
        return yield* PaymentMissingPreimageError.make({
          invoice: request.invoice,
          paymentId,
        })
      }

      return {
        paymentId,
        amountMsats,
        preimageHex,
        paidAtMs,
      }
    }),
})

const runEp212Smoke = (input: {
  readonly mode: Ep212RoutesSmokeMode
  readonly requestId: string
  readonly fetchFn: FetchLike
  readonly routeAUrl: string
  readonly routeBUrl: string
  readonly maxSpendMsats: number
  readonly payer: InvoicePayerApi
  readonly payerCalls: { count: number }
  readonly walletBackend: "mock" | "wallet_executor"
}) =>
  Effect.gen(function* () {
    const routeAHost = new URL(input.routeAUrl).host.toLowerCase()
    const routeBHost = new URL(input.routeBUrl).host.toLowerCase()
    const deps = createL402Deps({
      fetchFn: input.fetchFn,
      payer: input.payer,
      defaultMaxSpendMsats: input.maxSpendMsats,
      allowedHosts: [routeAHost, routeBHost],
    })

    const routeAChallenge = yield* checkChallenge(input.fetchFn, input.routeAUrl)
    const routeAResult = yield* fetchWithL402(
      {
        url: input.routeAUrl,
        method: "GET",
        scope: ROUTE_A_SCOPE,
        maxSpendMsats: input.maxSpendMsats,
        forceRefresh: true,
        cacheTtlMs: 120_000,
      },
      deps,
    )

    if (routeAResult.statusCode !== 200 || routeAResult.paid !== true) {
      return yield* Effect.fail(
        new Error(`route A expected paid status 200, got status=${routeAResult.statusCode} paid=${routeAResult.paid}`),
      )
    }

    const routeABody = routeAResult.responseBody ?? ""
    const routeAResponseBytes = Buffer.byteLength(routeABody, "utf8")
    const routeAResponseSha256 = sha256Hex(routeABody)

    const routeBChallenge = yield* checkChallenge(input.fetchFn, input.routeBUrl)
    if (typeof routeBChallenge.amountMsats !== "number") {
      return yield* Effect.fail(new Error("route B challenge missing amount_msats"))
    }
    if (routeBChallenge.amountMsats <= input.maxSpendMsats) {
      return yield* Effect.fail(
        new Error(
          `route B expected quoted amount above cap: quoted=${routeBChallenge.amountMsats} cap=${input.maxSpendMsats}`,
        ),
      )
    }

    const payerCallsBefore = input.payerCalls.count
    const blockedAttempt = yield* Effect.either(
      fetchWithL402(
        {
          url: input.routeBUrl,
          method: "GET",
          scope: ROUTE_B_SCOPE,
          maxSpendMsats: input.maxSpendMsats,
          forceRefresh: true,
          cacheTtlMs: 120_000,
        },
        deps,
      ),
    )
    const payerCallsAfter = input.payerCalls.count

    if (blockedAttempt._tag !== "Left") {
      return yield* Effect.fail(
        new Error(`route B expected policy block but fetch succeeded with status=${blockedAttempt.right.statusCode}`),
      )
    }
    if (blockedAttempt.left._tag !== "BudgetExceededError") {
      return yield* Effect.fail(
        new Error(`route B expected BudgetExceededError but saw ${blockedAttempt.left._tag}`),
      )
    }
    if (payerCallsAfter !== payerCallsBefore) {
      return yield* Effect.fail(
        new Error(
          `route B expected no payment call on policy block: before=${payerCallsBefore} after=${payerCallsAfter}`,
        ),
      )
    }

    return {
      ok: true as const,
      requestId: input.requestId,
      mode: input.mode,
      walletBackend: input.walletBackend,
      routeA: {
        url: input.routeAUrl,
        challengeStatusCode: routeAChallenge.statusCode,
        quotedAmountMsats: routeAChallenge.amountMsats,
        paidStatusCode: routeAResult.statusCode,
        paidAmountMsats: routeAResult.amountMsats,
        paymentId: routeAResult.paymentId,
        proofReference: routeAResult.proofReference,
        responseBytes: routeAResponseBytes,
        responseSha256: routeAResponseSha256,
      },
      routeB: {
        url: input.routeBUrl,
        challengeStatusCode: routeBChallenge.statusCode,
        quotedAmountMsats: routeBChallenge.amountMsats,
        maxSpendMsats: input.maxSpendMsats,
        blocked: true as const,
        denyReasonCode: blockedAttempt.left.reasonCode,
        payerCallsBefore,
        payerCallsAfter,
      },
    } satisfies Ep212RoutesSmokeSummary
  })

const runMockEp212RoutesSmoke = (requestId: string) => {
  const calls = { count: 0 }
  return runEp212Smoke({
    mode: "mock",
    requestId,
    fetchFn: createMockFetch(),
    routeAUrl: MOCK_ROUTE_A_URL,
    routeBUrl: MOCK_ROUTE_B_URL,
    maxSpendMsats: MOCK_CAP_MSATS,
    payer: createMockPayer(calls),
    payerCalls: calls,
    walletBackend: "mock",
  })
}

const runLiveEp212RoutesSmoke = (requestId: string) =>
  Effect.gen(function* () {
    const routeAUrl = yield* envString("OA_LIGHTNING_OPS_EP212_ROUTE_A_URL", MOCK_ROUTE_A_URL)
    const routeBUrl = yield* envString("OA_LIGHTNING_OPS_EP212_ROUTE_B_URL", MOCK_ROUTE_B_URL)
    const maxSpendMsats = yield* envInt("OA_LIGHTNING_OPS_EP212_MAX_SPEND_MSATS", MOCK_CAP_MSATS)
    const walletExecutorBaseUrl = yield* envString("OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL")
    const walletExecutorAuthTokenRaw = process.env.OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN?.trim()
    const walletExecutorAuthToken =
      walletExecutorAuthTokenRaw && walletExecutorAuthTokenRaw.length > 0 ? walletExecutorAuthTokenRaw : null
    const walletTimeoutMs = yield* envInt("OA_LIGHTNING_WALLET_EXECUTOR_TIMEOUT_MS", 12_000)

    const calls = { count: 0 }
    const fetchFn: FetchLike = async (input, init) => fetch(input, init)

    return yield* runEp212Smoke({
      mode: "live",
      requestId,
      fetchFn,
      routeAUrl,
      routeBUrl,
      maxSpendMsats,
      payer: createWalletExecutorPayer({
        fetchFn,
        baseUrl: walletExecutorBaseUrl.replace(/\/+$/, ""),
        authToken: walletExecutorAuthToken,
        timeoutMs: walletTimeoutMs,
        requestId,
        calls,
      }),
      payerCalls: calls,
      walletBackend: "wallet_executor",
    })
  })

export const runEp212RoutesSmoke = (input?: {
  readonly mode?: Ep212RoutesSmokeMode
  readonly requestId?: string
}) => {
  const mode = input?.mode ?? "mock"
  const requestId = input?.requestId ?? "smoke:ep212-routes"
  return mode === "live" ? runLiveEp212RoutesSmoke(requestId) : runMockEp212RoutesSmoke(requestId)
}
