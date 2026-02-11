import { Effect, Layer } from "effect"

import type { LndRpcRequest, LndRpcResponse } from "../contracts/rpc.js"
import { decodeLndRpcResponse } from "../contracts/rpc.js"
import {
  LndAuthenticationError,
  LndResponseDecodeError,
  LndTransportError,
} from "../errors/lndErrors.js"
import { LndTransportService } from "../services/lndTransportService.js"

export type LndRestTransportLayerOptions = Readonly<{
  readonly endpoint: string
  readonly macaroonHex?: string
  readonly timeoutMs?: number
  readonly fetchImplementation?: (
    input: RequestInfo | URL,
    init?: RequestInit,
  ) => Promise<Response>
}>

const operation = "LndRestTransport.send"

const normalizePath = (path: string): string => (path.startsWith("/") ? path : `/${path}`)

const appendQuery = (base: URL, query: Readonly<Record<string, string>> | undefined): URL => {
  if (!query) return base
  for (const [key, value] of Object.entries(query)) {
    base.searchParams.set(key, value)
  }
  return base
}

export const makeLndRestTransportLayer = (options: LndRestTransportLayerOptions) => {
  const fetchImplementation = options.fetchImplementation ?? fetch
  const endpoint = options.endpoint
  const timeoutMs = Math.max(1, Math.floor(options.timeoutMs ?? 10_000))
  const macaroonHex = options.macaroonHex?.trim()

  return Layer.succeed(
    LndTransportService,
    LndTransportService.of({
      send: (request: LndRpcRequest) =>
        Effect.gen(function* () {
          const url = appendQuery(new URL(normalizePath(request.path), endpoint), request.query)
          const headers = new Headers({
            accept: "application/json",
          })
          if (request.body !== undefined) headers.set("content-type", "application/json")
          if (macaroonHex) headers.set("Grpc-Metadata-macaroon", macaroonHex)

          const abort = new AbortController()
          const timer = setTimeout(() => abort.abort(), timeoutMs)

          const response = yield* Effect.tryPromise({
            try: async () =>
              {
                const init: RequestInit = {
                  method: request.method,
                  headers,
                  signal: abort.signal,
                }
                if (request.body !== undefined) init.body = JSON.stringify(request.body)

                return await fetchImplementation(url, init)
              },
            catch: (error) =>
              LndTransportError.make({
                operation,
                reason: String(error),
              }),
          }).pipe(Effect.ensuring(Effect.sync(() => clearTimeout(timer))))

          if (response.status === 401 || response.status === 403) {
            return yield* LndAuthenticationError.make({
              operation,
              status: response.status,
              reason: "Authentication failed for LND REST transport",
            })
          }

          const bodyText = yield* Effect.tryPromise({
            try: async () => await response.text(),
            catch: (error) =>
              LndTransportError.make({
                operation,
                reason: String(error),
                status: response.status,
              }),
          })

          const parsedBody = (() => {
            if (!bodyText.trim()) return undefined
            try {
              return JSON.parse(bodyText) as unknown
            } catch {
              return null
            }
          })()

          if (parsedBody === null) {
            return yield* LndResponseDecodeError.make({
              operation,
              status: response.status,
              reason: "Failed to parse JSON body from LND REST response",
              body: bodyText,
            })
          }

          const rawResponse = {
            status: response.status,
            ...(parsedBody !== undefined ? { body: parsedBody } : {}),
            headers: (() => {
              const entries: Record<string, string> = {}
              response.headers.forEach((value, key) => {
                entries[key] = value
              })
              return entries
            })(),
          }

          const decoded: LndRpcResponse = yield* decodeLndRpcResponse(rawResponse).pipe(
            Effect.mapError((error) =>
              LndResponseDecodeError.make({
                operation,
                status: response.status,
                reason: String(error),
                ...(bodyText.trim() ? { body: bodyText } : {}),
              }),
            ),
          )

          return decoded
        }),
    }),
  )
}
