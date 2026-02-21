import { createServer, type IncomingMessage, type Server, type ServerResponse } from "node:http"
import crypto from "node:crypto"

import { Context, Effect } from "effect"

import { decodePayBolt11HttpRequest, type ErrorResponse } from "../contracts.js"
import {
  HttpRequestDecodeError,
  PolicyDeniedError,
  SecretLoadError,
  SparkGatewayError,
  WalletExecutorConfigError,
} from "../errors.js"
import type { WalletExecutorConfig } from "../runtime/config.js"
import { WalletExecutorConfigService } from "../runtime/config.js"
import { handleCompatWalletRoute, isWalletCompatHttpError } from "./wallets-compat.js"
import type { WalletExecutorApi } from "../wallet/executor.js"
import { WalletExecutorService } from "../wallet/executor.js"

const json = (status: number, body: unknown, response: ServerResponse, requestId: string) => {
  response.statusCode = status
  response.setHeader("content-type", "application/json; charset=utf-8")
  response.setHeader("x-request-id", requestId)
  response.end(JSON.stringify(body))
}

const readRequestBody = (request: IncomingMessage): Effect.Effect<string, HttpRequestDecodeError> =>
  Effect.async((resume) => {
    const chunks: Array<Buffer> = []

    request.on("data", (chunk: unknown) => {
      if (typeof chunk === "string") {
        chunks.push(Buffer.from(chunk))
        return
      }
      if (Buffer.isBuffer(chunk)) {
        chunks.push(chunk)
      }
    })

    request.on("error", (error) => {
      resume(
        Effect.fail(
          HttpRequestDecodeError.make({
            message: `request stream error: ${String(error)}`,
          }),
        ),
      )
    })

    request.on("end", () => {
      resume(Effect.succeed(Buffer.concat(chunks).toString("utf8")))
    })
  })

const decodeJsonBody = (rawBody: string): Effect.Effect<unknown, HttpRequestDecodeError> =>
  Effect.try({
    try: () => JSON.parse(rawBody),
    catch: (error) =>
      HttpRequestDecodeError.make({
        message: `invalid json body: ${String(error)}`,
      }),
  })

const toErrorResponse = (requestId: string, code: string, message: string, details?: Record<string, unknown>): ErrorResponse => ({
  requestId,
  code,
  message,
  ...(details ? { details } : {}),
})

const requestIdFrom = (request: IncomingMessage): string => {
  const raw = request.headers["x-request-id"]
  if (typeof raw === "string" && raw.trim().length > 0) return raw.trim()
  return crypto.randomUUID()
}

const bearerTokenFromRequest = (request: IncomingMessage): string | null => {
  const raw = request.headers.authorization
  if (typeof raw !== "string") return null
  const trimmed = raw.trim()
  if (trimmed.length === 0) return null
  const match = /^bearer\s+(.+)$/i.exec(trimmed)
  if (!match) return null
  const token = match[1]?.trim() ?? ""
  return token.length > 0 ? token : null
}

const bearerTokenMatches = (expected: string, actual: string | null): boolean => {
  if (!actual) return false
  const expectedBytes = Buffer.from(expected, "utf8")
  const actualBytes = Buffer.from(actual, "utf8")
  if (expectedBytes.length !== actualBytes.length) return false
  return crypto.timingSafeEqual(expectedBytes, actualBytes)
}

const routeRequest = (
  request: IncomingMessage,
  response: ServerResponse,
  wallet: WalletExecutorApi,
  config: WalletExecutorConfig,
): Effect.Effect<void, HttpRequestDecodeError | PolicyDeniedError | SparkGatewayError | WalletExecutorConfigError | SecretLoadError> =>
  Effect.gen(function* () {
    const requestId = requestIdFrom(request)
    const method = (request.method ?? "GET").toUpperCase()
    const path = new URL(request.url ?? "/", "http://127.0.0.1").pathname

    if (method === "GET" && path === "/healthz") {
      json(200, { ok: true, requestId }, response, requestId)
      return
    }

    if (config.authToken) {
      const bearerToken = bearerTokenFromRequest(request)
      if (!bearerTokenMatches(config.authToken, bearerToken)) {
        response.setHeader("www-authenticate", 'Bearer realm="wallet-executor"')
        json(401, { ok: false, error: toErrorResponse(requestId, "unauthorized", "missing or invalid bearer token") }, response, requestId)
        return
      }
    }

    if (path.startsWith("/wallets/")) {
      const parsed = yield* readRequestBody(request).pipe(
        Effect.flatMap(decodeJsonBody),
      )

      const compatResult = yield* Effect.either(
        Effect.tryPromise({
          try: async () => await handleCompatWalletRoute(method, path, parsed, config),
          catch: (error) => error,
        }),
      )

      if (compatResult._tag === "Left") {
        const error = compatResult.left
        if (isWalletCompatHttpError(error)) {
          json(
            error.status,
            {
              ok: false,
              error: toErrorResponse(requestId, error.code, error.message, error.details),
            },
            response,
            requestId,
          )
          return
        }

        json(
          500,
          {
            ok: false,
            error: toErrorResponse(
              requestId,
              "internal_error",
              error instanceof Error ? error.message : String(error),
            ),
          },
          response,
          requestId,
        )
        return
      }

      json(compatResult.right.status, compatResult.right.body, response, requestId)
      return
    }

    if (method === "GET" && path === "/status") {
      const status = yield* wallet.status()
      json(200, { ok: true, requestId, status }, response, requestId)
      return
    }

    if (method === "POST" && path === "/pay-bolt11") {
      const parsed = yield* readRequestBody(request).pipe(
        Effect.flatMap(decodeJsonBody),
        Effect.flatMap((body) =>
          Effect.try({
            try: () => decodePayBolt11HttpRequest(body),
            catch: (error) =>
              HttpRequestDecodeError.make({
                message: error instanceof Error ? error.message : String(error),
              }),
          }),
        ),
      )

      const outcome = yield* Effect.either(wallet.payBolt11(parsed.payment, { requestId }))

      if (outcome._tag === "Left") {
        const error = outcome.left
        if (error._tag === "PolicyDeniedError") {
          json(
            403,
            {
              ok: false,
              error: toErrorResponse(requestId, error.code, error.message, {
                ...(error.host ? { host: error.host } : {}),
                ...(error.maxAllowedMsats !== undefined ? { maxAllowedMsats: error.maxAllowedMsats } : {}),
                ...(error.quotedAmountMsats !== undefined ? { quotedAmountMsats: error.quotedAmountMsats } : {}),
                ...(error.windowSpendMsats !== undefined ? { windowSpendMsats: error.windowSpendMsats } : {}),
                ...(error.windowCapMsats !== undefined ? { windowCapMsats: error.windowCapMsats } : {}),
              }),
            },
            response,
            requestId,
          )
          return
        }

        json(
          error.code === "payment_pending" ? 504 : 502,
          {
            ok: false,
            error: toErrorResponse(requestId, error.code, error.message),
          },
          response,
          requestId,
        )
        return
      }

      json(
        200,
        {
          ok: true,
          requestId,
          result: {
            requestId,
            walletId: config.walletId,
            payment: outcome.right.payment,
            quotedAmountMsats: outcome.right.quotedAmountMsats,
            windowSpendMsatsAfterPayment: outcome.right.windowSpendMsatsAfterPayment,
            receipt: outcome.right.receipt,
          },
        },
        response,
        requestId,
      )
      return
    }

    json(404, { ok: false, error: toErrorResponse(requestId, "not_found", "route not found") }, response, requestId)
  })

const respondFromTopLevelError = (
  request: IncomingMessage,
  response: ServerResponse,
  error: unknown,
): Effect.Effect<void> =>
  Effect.sync(() => {
    const requestId = requestIdFrom(request)

    if (error && typeof error === "object" && "_tag" in error) {
      const tagged = error as { readonly _tag: string; readonly message?: string; readonly code?: string }
      if (tagged._tag === "HttpRequestDecodeError") {
        json(400, { ok: false, error: toErrorResponse(requestId, "invalid_request", tagged.message ?? "invalid request") }, response, requestId)
        return
      }
      if (tagged._tag === "WalletExecutorConfigError") {
        json(500, { ok: false, error: toErrorResponse(requestId, "config_error", tagged.message ?? "config error") }, response, requestId)
        return
      }
      if (tagged._tag === "SecretLoadError") {
        json(500, { ok: false, error: toErrorResponse(requestId, "secret_error", tagged.message ?? "secret load error") }, response, requestId)
        return
      }
      if (tagged._tag === "SparkGatewayError") {
        const code = tagged.code ?? "spark_error"
        json(code === "payment_pending" ? 504 : 502, { ok: false, error: toErrorResponse(requestId, code, tagged.message ?? "spark error") }, response, requestId)
        return
      }
      if (tagged._tag === "PolicyDeniedError") {
        const code = tagged.code ?? "policy_denied"
        json(403, { ok: false, error: toErrorResponse(requestId, code, tagged.message ?? "policy denied") }, response, requestId)
        return
      }
    }

    json(500, { ok: false, error: toErrorResponse(requestId, "internal_error", String(error)) }, response, requestId)
  })

export type WalletExecutorHttpServer = Readonly<{
  server: Server
  close: Effect.Effect<void, Error>
  address: string
}>

export class WalletExecutorHttpServerService extends Context.Tag(
  "@openagents/lightning-wallet-executor/WalletExecutorHttpServerService",
)<WalletExecutorHttpServerService, WalletExecutorHttpServer>() {}

export const makeWalletExecutorHttpServer = Effect.gen(function* () {
  const config = yield* WalletExecutorConfigService
  const wallet = yield* WalletExecutorService

  const handler = (req: IncomingMessage, res: ServerResponse) => {
    void Effect.runPromise(
      routeRequest(req, res, wallet, config).pipe(
        Effect.catchAll((error) => respondFromTopLevelError(req, res, error)),
        Effect.catchAllDefect((defect) => respondFromTopLevelError(req, res, defect)),
      ),
    )
  }

  const server = createServer(handler)

  yield* Effect.tryPromise({
    try: async () =>
      await new Promise<void>((resolve, reject) => {
        server.once("error", reject)
        server.listen(config.port, config.host, () => {
          server.off("error", reject)
          resolve()
        })
      }),
    catch: (error) => new Error(String(error)),
  })

  const address = `http://${config.host}:${config.port}`

  return {
    server,
    address,
    close: Effect.tryPromise({
      try: async () =>
        await new Promise<void>((resolve, reject) => {
          server.close((error) => {
            if (error) {
              reject(error)
              return
            }
            resolve()
          })
        }),
      catch: (error) => new Error(String(error)),
    }),
  } satisfies WalletExecutorHttpServer
})
