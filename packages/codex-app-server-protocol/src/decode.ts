import Ajv2020, { type ValidateFunction } from "ajv/dist/2020.js"

import {
  CLIENT_RESPONSE_DOCUMENTS as bundledClientResponses,
  SERVER_NOTIFICATION_DOCUMENTS as bundledServerNotifications,
  SERVER_REQUEST_DOCUMENTS as bundledServerRequests,
  SERVER_REQUEST_RESPONSE_DOCUMENTS as bundledServerRequestResponses,
} from "./_generated/bundled-0.144.1/wire.gen.ts"
import {
  CLIENT_RESPONSE_DOCUMENTS as currentClientResponses,
  SERVER_NOTIFICATION_DOCUMENTS as currentServerNotifications,
  SERVER_REQUEST_DOCUMENTS as currentServerRequests,
  SERVER_REQUEST_RESPONSE_DOCUMENTS as currentServerRequestResponses,
} from "./_generated/current-source/wire.gen.ts"

export type CodexProtocolLane = "bundled-0.144.1" | "current-source"
export type CodexProtocolDirection = "client-response" | "server-request" | "server-request-response" | "server-notification"

export type CodexDecodedPayload = Readonly<{
  _tag: "Decoded"
  lane: CodexProtocolLane
  direction: CodexProtocolDirection
  method: string
  payload: unknown
}>

export type CodexProtocolDecodeFailure = Readonly<{
  _tag: "DecodeFailure"
  lane: CodexProtocolLane
  direction: CodexProtocolDirection
  method: string
  reason: "unknown_method" | "invalid_payload"
  detail: string
}>

export type CodexProtocolDecodeResult = CodexDecodedPayload | CodexProtocolDecodeFailure

type WireLane = Readonly<{
  methods: Readonly<Record<CodexProtocolDirection, Readonly<Record<string, unknown>>>>
  validators: Map<string, ValidateFunction>
  ajv: { compile: (schema: object) => ValidateFunction }
}>

const Ajv2020Constructor = Ajv2020 as unknown as new (options: object) => WireLane["ajv"]

const makeLane = (
  clientResponses: Readonly<Record<string, unknown>>,
  serverRequests: Readonly<Record<string, unknown>>,
  serverRequestResponses: Readonly<Record<string, unknown>>,
  serverNotifications: Readonly<Record<string, unknown>>,
): WireLane => ({
  methods: {
    "client-response": clientResponses,
    "server-request": serverRequests,
    "server-request-response": serverRequestResponses,
    "server-notification": serverNotifications,
  },
  validators: new Map(),
  ajv: new Ajv2020Constructor({ strict: false, allErrors: true, validateFormats: false }),
})

const lanes: Readonly<Record<CodexProtocolLane, WireLane>> = {
  "bundled-0.144.1": makeLane(bundledClientResponses, bundledServerRequests, bundledServerRequestResponses, bundledServerNotifications),
  "current-source": makeLane(currentClientResponses, currentServerRequests, currentServerRequestResponses, currentServerNotifications),
}

const decode = (
  laneName: CodexProtocolLane,
  direction: CodexProtocolDirection,
  method: string,
  payload: unknown,
): CodexProtocolDecodeResult => {
  const lane = lanes[laneName]
  const document = lane.methods[direction][method]
  if (document === undefined) {
    return { _tag: "DecodeFailure", lane: laneName, direction, method, reason: "unknown_method", detail: "method is absent from the generated protocol inventory" }
  }
  if (document === null) {
    if (payload === undefined || payload === null ||
      (typeof payload === "object" && payload !== null && !Array.isArray(payload) && Object.keys(payload).length === 0)) {
      return { _tag: "Decoded", lane: laneName, direction, method, payload: undefined }
    }
    return { _tag: "DecodeFailure", lane: laneName, direction, method, reason: "invalid_payload", detail: "method does not accept a payload" }
  }
  const cacheKey = `${direction}\u0000${method}`
  let validate = lane.validators.get(cacheKey)
  if (validate === undefined) {
    if (typeof document !== "object" || Array.isArray(document) ||
      typeof (document as { schema?: unknown }).schema !== "object" || (document as { schema?: unknown }).schema === null) {
      return { _tag: "DecodeFailure", lane: laneName, direction, method, reason: "invalid_payload", detail: "generated wire schema is unavailable" }
    }
    const compiled = lane.ajv.compile((document as { schema: object }).schema)
    lane.validators.set(cacheKey, compiled)
    validate = compiled
  }
  const activeValidate = validate
  if (activeValidate(payload)) return { _tag: "Decoded", lane: laneName, direction, method, payload }
  const detail = (activeValidate.errors ?? [])
    .slice(0, 8)
    .map(error => `${error.instancePath || "/"} ${error.message ?? "is invalid"}`)
    .join("; ")
  return {
    _tag: "DecodeFailure",
    lane: laneName,
    direction,
    method,
    reason: "invalid_payload",
    detail: (detail || "generated schema rejected payload").slice(0, 1_000),
  }
}

export const decodeBundledClientResponse = (method: string, payload: unknown): CodexProtocolDecodeResult =>
  decode("bundled-0.144.1", "client-response", method, payload)

export const decodeBundledServerRequest = (method: string, payload: unknown): CodexProtocolDecodeResult =>
  decode("bundled-0.144.1", "server-request", method, payload)

export const decodeBundledServerRequestResponse = (method: string, payload: unknown): CodexProtocolDecodeResult =>
  decode("bundled-0.144.1", "server-request-response", method, payload)

export const decodeBundledServerNotification = (method: string, payload: unknown): CodexProtocolDecodeResult =>
  decode("bundled-0.144.1", "server-notification", method, payload)

export const decodeCurrentServerNotification = (method: string, payload: unknown): CodexProtocolDecodeResult =>
  decode("current-source", "server-notification", method, payload)
