import { Effect } from "effect"
import {
  KhalaCodeDesktopRpcBridgeFailure,
  KhalaCodeDesktopRpcMethodNames,
  KhalaCodeDesktopRpcMethodSchemas,
  decodeKhalaCodeDesktopRpcParameters,
  decodeKhalaCodeDesktopRpcResult,
  type KhalaCodeDesktopRPCSchema,
  type KhalaCodeDesktopRpcBridgeFailure as DesktopBridgeFailure,
  type KhalaCodeDesktopRpcMethodName,
} from "../../../clients/khala-code-desktop/src/shared/rpc.js"
import { Schema as S } from "effect"

export type KhalaCodeRpcMethodName = KhalaCodeDesktopRpcMethodName
export const KhalaCodeRpcMethodNames = KhalaCodeDesktopRpcMethodNames
export const KhalaCodeRpcMethodSchemas = KhalaCodeDesktopRpcMethodSchemas

type DesktopRequestMap = KhalaCodeDesktopRPCSchema["requests"]
type DesktopRequest<M extends KhalaCodeRpcMethodName> = DesktopRequestMap[M]

export type KhalaCodeRpcArgs<M extends KhalaCodeRpcMethodName> =
  Parameters<DesktopRequest<M>>
export type KhalaCodeRpcResult<M extends KhalaCodeRpcMethodName> =
  Awaited<ReturnType<DesktopRequest<M>>>

export type KhalaCodeRpcUnknownField = {
  readonly path: string
  readonly value: unknown
}

export type KhalaCodeRpcSchemaOracle = {
  readonly decoded: boolean
  readonly method: KhalaCodeRpcMethodName
  readonly unknownFields: readonly KhalaCodeRpcUnknownField[]
}

export type KhalaCodeRpcCallOk<M extends KhalaCodeRpcMethodName> = {
  readonly ok: true
  readonly method: M
  readonly oracle: KhalaCodeRpcSchemaOracle
  readonly value: KhalaCodeRpcResult<M>
}

export type KhalaCodeRpcTransportFailure = {
  readonly _tag: "KhalaCodeRpcTransportFailure"
  readonly method: KhalaCodeRpcMethodName
  readonly message: string
  readonly url: string
  readonly cause?: unknown
}

export type KhalaCodeRpcHttpFailure = {
  readonly _tag: "KhalaCodeRpcHttpFailure"
  readonly method: KhalaCodeRpcMethodName
  readonly status: number
  readonly statusText: string
  readonly detail: string
  readonly bridgeFailure?: DesktopBridgeFailure
  readonly payload?: unknown
  readonly url: string
}

export type KhalaCodeRpcJsonFailure = {
  readonly _tag: "KhalaCodeRpcJsonFailure"
  readonly method: KhalaCodeRpcMethodName
  readonly message: string
  readonly body: string
  readonly url: string
  readonly cause?: unknown
}

export type KhalaCodeRpcSchemaFailure = {
  readonly _tag: "KhalaCodeRpcSchemaFailure"
  readonly method: KhalaCodeRpcMethodName
  readonly message: string
  readonly phase: "request" | "response"
  readonly payload: unknown
  readonly cause?: unknown
}

export type KhalaCodeRpcClientFailure =
  | KhalaCodeRpcTransportFailure
  | KhalaCodeRpcHttpFailure
  | KhalaCodeRpcJsonFailure
  | KhalaCodeRpcSchemaFailure

export type KhalaCodeRpcCallFailure = {
  readonly ok: false
  readonly failure: KhalaCodeRpcClientFailure
  readonly oracle: KhalaCodeRpcSchemaOracle
}

export type KhalaCodeRpcDecodeResult<M extends KhalaCodeRpcMethodName> =
  | KhalaCodeRpcCallOk<M>
  | KhalaCodeRpcCallFailure

export type KhalaCodeRpcConsistencyMismatch = {
  readonly path: string
  readonly left: unknown
  readonly right: unknown
}

export type KhalaCodeRpcConsistencyResult = {
  readonly ok: boolean
  readonly leftLabel: string
  readonly rightLabel: string
  readonly mismatches: readonly KhalaCodeRpcConsistencyMismatch[]
}

export type KhalaCodeRpcClientOptions = {
  readonly baseUrl?: string | URL
  readonly fetch?: KhalaCodeRpcFetch
  readonly headers?: HeadersInit
}

export type KhalaCodeRpcFetch = (
  input: RequestInfo | URL,
  init?: RequestInit,
) => Promise<Response>

export type KhalaCodeRpcRequestSurface = {
  readonly [M in KhalaCodeRpcMethodName]: (
    ...args: KhalaCodeRpcArgs<M>
  ) => Effect.Effect<KhalaCodeRpcResult<M>, KhalaCodeRpcClientFailure>
}

type JsonObject = { readonly [key: string]: unknown }

const decodeBridgeFailure = (payload: unknown): DesktopBridgeFailure | undefined => {
  try {
    return S.decodeUnknownSync(KhalaCodeDesktopRpcBridgeFailure)(payload)
  } catch {
    return undefined
  }
}

const errorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

const isObject = (value: unknown): value is JsonObject =>
  value !== null && typeof value === "object" && !Array.isArray(value)

const objectKeys = (value: unknown): readonly string[] =>
  isObject(value) ? Object.keys(value) : []

const pathJoin = (base: string, key: string): string =>
  base.length === 0 ? key : `${base}.${key}`

const collectUnknownFields = (
  raw: unknown,
  decoded: unknown,
  path = "",
): readonly KhalaCodeRpcUnknownField[] => {
  if (Array.isArray(raw) && Array.isArray(decoded)) {
    return raw.flatMap((value, index) =>
      collectUnknownFields(value, decoded[index], `${path}[${index}]`),
    )
  }
  if (!isObject(raw) || !isObject(decoded)) return []

  const decodedKeys = new Set(objectKeys(decoded))
  const direct = Object.entries(raw)
    .filter(([key]) => !decodedKeys.has(key))
    .map(([key, value]) => ({ path: pathJoin(path, key), value }))
  const nested = Object.keys(decoded).flatMap((key) =>
    collectUnknownFields(raw[key], decoded[key], pathJoin(path, key)),
  )
  return [...direct, ...nested]
}

const parsePayloadDetail = (payload: unknown): string => {
  if (isObject(payload)) {
    const detail = payload.detail
    if (typeof detail === "string" && detail.length > 0) return detail
    const error = payload.error
    if (typeof error === "string" && error.length > 0) return error
    const message = payload.message
    if (typeof message === "string" && message.length > 0) return message
  }
  if (typeof payload === "string" && payload.length > 0) return payload
  return "unknown error"
}

const parseJsonBody = (
  method: KhalaCodeRpcMethodName,
  body: string,
  url: string,
): Effect.Effect<unknown, KhalaCodeRpcJsonFailure> =>
  Effect.try({
    try: () => body.length === 0 ? null : JSON.parse(body) as unknown,
    catch: (cause) => ({
      _tag: "KhalaCodeRpcJsonFailure",
      method,
      message: errorMessage(cause),
      body,
      url,
      cause,
    }),
  })

const methodUrl = (baseUrl: string | URL, method: KhalaCodeRpcMethodName): string =>
  new URL(`/rpc/${encodeURIComponent(method)}`, baseUrl).toString()

export const decodeKhalaCodeRpcResultOrFailure = <M extends KhalaCodeRpcMethodName>(
  method: M,
  payload: unknown,
): KhalaCodeRpcDecodeResult<M> => {
  try {
    const value = decodeKhalaCodeDesktopRpcResult(method, payload) as KhalaCodeRpcResult<M>
    return {
      ok: true,
      method,
      oracle: {
        decoded: true,
        method,
        unknownFields: collectUnknownFields(payload, value),
      },
      value,
    }
  } catch (cause) {
    return {
      ok: false,
      failure: {
        _tag: "KhalaCodeRpcSchemaFailure",
        method,
        message: errorMessage(cause),
        phase: "response",
        payload,
        cause,
      },
      oracle: {
        decoded: false,
        method,
        unknownFields: [],
      },
    }
  }
}

export const decodeKhalaCodeRpcParametersOrFailure = <M extends KhalaCodeRpcMethodName>(
  method: M,
  args: readonly unknown[],
): { readonly ok: true; readonly args: readonly unknown[] } | KhalaCodeRpcCallFailure => {
  try {
    return {
      ok: true,
      args: decodeKhalaCodeDesktopRpcParameters(method, args),
    }
  } catch (cause) {
    return {
      ok: false,
      failure: {
        _tag: "KhalaCodeRpcSchemaFailure",
        method,
        message: errorMessage(cause),
        phase: "request",
        payload: args,
        cause,
      },
      oracle: {
        decoded: false,
        method,
        unknownFields: [],
      },
    }
  }
}

export const compareKhalaCodeRpcConsistency = (input: {
  readonly leftLabel?: string
  readonly rightLabel?: string
  readonly left: unknown
  readonly right: unknown
}): KhalaCodeRpcConsistencyResult => {
  const mismatches: KhalaCodeRpcConsistencyMismatch[] = []
  const visit = (left: unknown, right: unknown, path: string): void => {
    if (Object.is(left, right)) return
    if (Array.isArray(left) && Array.isArray(right)) {
      if (left.length !== right.length) {
        mismatches.push({ path: pathJoin(path, "length"), left: left.length, right: right.length })
      }
      for (let index = 0; index < Math.max(left.length, right.length); index += 1) {
        visit(left[index], right[index], `${path}[${index}]`)
      }
      return
    }
    if (isObject(left) && isObject(right)) {
      const keys = new Set([...Object.keys(left), ...Object.keys(right)])
      for (const key of keys) {
        visit(left[key], right[key], pathJoin(path, key))
      }
      return
    }
    mismatches.push({ path: path || "$", left, right })
  }

  visit(input.left, input.right, "")
  return {
    ok: mismatches.length === 0,
    leftLabel: input.leftLabel ?? "left",
    rightLabel: input.rightLabel ?? "right",
    mismatches,
  }
}

export class KhalaCodeRpcClient {
  readonly baseUrl: string | URL
  readonly fetch: KhalaCodeRpcFetch
  readonly headers: HeadersInit | undefined
  readonly request: KhalaCodeRpcRequestSurface

  constructor(options: KhalaCodeRpcClientOptions = {}) {
    this.baseUrl = options.baseUrl ?? "http://127.0.0.1:50021"
    this.fetch = options.fetch ?? globalThis.fetch.bind(globalThis)
    this.headers = options.headers
    const callUnknown = this.call as (
      method: KhalaCodeRpcMethodName,
      ...args: readonly unknown[]
    ) => Effect.Effect<unknown, KhalaCodeRpcClientFailure>
    this.request = Object.fromEntries(
      KhalaCodeRpcMethodNames.map((method) => [
        method,
        (...args: readonly unknown[]) => callUnknown.call(this, method, ...args),
      ]),
    ) as unknown as KhalaCodeRpcRequestSurface
  }

  call<M extends KhalaCodeRpcMethodName>(
    method: M,
    ...args: KhalaCodeRpcArgs<M>
  ): Effect.Effect<KhalaCodeRpcResult<M>, KhalaCodeRpcClientFailure> {
    return Effect.map(this.callWithOracle(method, ...args), (result) => result.value)
  }

  callWithOracle<M extends KhalaCodeRpcMethodName>(
    method: M,
    ...args: KhalaCodeRpcArgs<M>
  ): Effect.Effect<KhalaCodeRpcCallOk<M>, KhalaCodeRpcClientFailure> {
    const decodedArgs = decodeKhalaCodeRpcParametersOrFailure(method, args)
    if (!decodedArgs.ok) return Effect.fail(decodedArgs.failure)

    const url = methodUrl(this.baseUrl, method)
    return Effect.flatMap(
      Effect.tryPromise({
        try: () =>
          this.fetch(url, {
            method: "POST",
            headers: {
              "content-type": "application/json",
              ...this.headers,
            },
            body: JSON.stringify({ args: decodedArgs.args }),
          }),
        catch: (cause) => ({
          _tag: "KhalaCodeRpcTransportFailure",
          method,
          message: errorMessage(cause),
          url,
          cause,
        }),
      }),
      (response) =>
        Effect.flatMap(
          Effect.tryPromise({
            try: () => response.text(),
            catch: (cause) => ({
              _tag: "KhalaCodeRpcTransportFailure",
              method,
              message: errorMessage(cause),
              url,
              cause,
            }),
          }),
          (body) =>
            Effect.flatMap(parseJsonBody(method, body, url), (payload) => {
              if (!response.ok) {
                const bridgeFailure = decodeBridgeFailure(payload)
                return Effect.fail({
                  _tag: "KhalaCodeRpcHttpFailure",
                  method,
                  status: response.status,
                  statusText: response.statusText,
                  detail: bridgeFailure?.error ?? parsePayloadDetail(payload),
                  ...(bridgeFailure === undefined ? {} : { bridgeFailure }),
                  payload,
                  url,
                })
              }

              const decoded = decodeKhalaCodeRpcResultOrFailure(method, payload)
              return decoded.ok ? Effect.succeed(decoded) : Effect.fail(decoded.failure)
            }),
        ),
    )
  }
}

export const makeKhalaCodeRpcClient = (
  options?: KhalaCodeRpcClientOptions,
): KhalaCodeRpcClient => new KhalaCodeRpcClient(options)
