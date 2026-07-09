import { Schema as S } from "effect"

import {
  PylonFleetRunRemotePortError,
  type PylonFleetRunRemoteIntakePort,
} from "./fleet-run-remote-intake.js"

export const PYLON_FLEET_RUN_HTTP_TRANSPORT_SCHEMA =
  "openagents.pylon.fleet_run_transport.v1" as const

const PYLON_REF = /^[a-z0-9][a-z0-9._:-]{2,119}$/u
const RUN_REF = /^fleet_run\.sarah\.[0-9a-f]{20}$/u
const CLAIM_REF = /^claim\.sarah_fleet_run\.[0-9a-f]{24}$/u
const MAX_RESPONSE_BYTES = 256 * 1_024

const SuccessEnvelope = S.Struct({
  schema: S.Literal(PYLON_FLEET_RUN_HTTP_TRANSPORT_SCHEMA),
  operation: S.Literals(["claim", "accept"]),
  result: S.Unknown,
})

const ErrorEnvelope = S.Struct({
  schema: S.Literal(PYLON_FLEET_RUN_HTTP_TRANSPORT_SCHEMA),
  error: S.Struct({
    code: S.Literals([
      "invalid_request",
      "not_authorized",
      "claim_conflict",
      "claim_expired",
      "unavailable",
    ]),
    retryable: S.Boolean,
  }),
})

type TransportOperation = "accept" | "claim"
type TransportErrorCode =
  | "invalid_request"
  | "not_authorized"
  | "claim_conflict"
  | "claim_expired"
  | "unavailable"

export type MakePylonFleetRunHttpIntakeOptions = {
  readonly agentToken: string
  readonly baseUrl: string
  readonly fetchImpl?: typeof globalThis.fetch | undefined
  readonly makeId?: (() => string) | undefined
  readonly requestTimeoutMs?: number | undefined
}

const unavailable = (): PylonFleetRunRemotePortError =>
  new PylonFleetRunRemotePortError({ kind: "unavailable" })

const validateBaseUrl = (value: string): URL => {
  try {
    const parsed = new URL(value)
    const loopbackHttp =
      parsed.protocol === "http:" &&
      (parsed.hostname === "localhost" ||
        parsed.hostname === "127.0.0.1" ||
        parsed.hostname === "[::1]")
    if (
      (parsed.protocol !== "https:" && !loopbackHttp) ||
      parsed.username !== "" ||
      parsed.password !== ""
    ) {
      throw unavailable()
    }
    return parsed
  } catch (error) {
    if (error instanceof PylonFleetRunRemotePortError) throw error
    throw unavailable()
  }
}

const mapCode = (
  code: TransportErrorCode,
): PylonFleetRunRemotePortError =>
  new PylonFleetRunRemotePortError({
    kind:
      code === "not_authorized"
        ? "not_authorized"
        : code === "claim_expired"
          ? "claim_expired"
          : code === "claim_conflict" || code === "invalid_request"
            ? "claim_conflict"
            : "unavailable",
  })

const readBoundedJson = async (response: Response): Promise<unknown> => {
  const declaredLength = Number(response.headers.get("content-length"))
  if (Number.isFinite(declaredLength) && declaredLength > MAX_RESPONSE_BYTES) {
    throw unavailable()
  }
  const text = await response.text()
  if (new TextEncoder().encode(text).byteLength > MAX_RESPONSE_BYTES) {
    throw unavailable()
  }
  try {
    return JSON.parse(text) as unknown
  } catch {
    throw unavailable()
  }
}

/**
 * Private bearer adapter for the Pylon standing node. The token is closed over
 * and never accepted by, or returned from, the orchestration service.
 */
export function makePylonFleetRunHttpIntake(
  options: MakePylonFleetRunHttpIntakeOptions,
): PylonFleetRunRemoteIntakePort {
  const baseUrl = validateBaseUrl(options.baseUrl)
  if (options.agentToken.trim() === "") throw unavailable()
  const fetchImpl = options.fetchImpl ?? globalThis.fetch
  const makeId = options.makeId ?? (() => crypto.randomUUID())
  const requestTimeoutMs = options.requestTimeoutMs ?? 15_000
  if (!Number.isInteger(requestTimeoutMs) || requestTimeoutMs < 1_000 || requestTimeoutMs > 60_000) {
    throw unavailable()
  }
  // A server may commit a lease before its response is lost, or local import
  // may fail after a decoded response. Retain the request key until matching
  // accept succeeds so either retry receives the authority's duplicate replay
  // instead of hiding the run until lease expiry. There is intentionally no
  // second local durable store before canonical import; process loss in this
  // narrow window still recovers by the bounded server lease.
  const pendingClaimIds = new Map<string, string>()

  const post = async (
    operation: TransportOperation,
    pylonRef: string,
    body: Readonly<Record<string, unknown>>,
    idempotencyRef: string,
  ): Promise<unknown | null> => {
    if (!PYLON_REF.test(pylonRef)) throw unavailable()
    const url = new URL(
      `/api/pylons/${encodeURIComponent(pylonRef)}/fleet-runs/${operation}`,
      baseUrl,
    )
    let response: Response
    try {
      response = await fetchImpl(url, {
        method: "POST",
        headers: {
          Accept: "application/json",
          Authorization: `Bearer ${options.agentToken}`,
          "Content-Type": "application/json",
          "Idempotency-Key": `pylon.fleet-run.${operation}.${idempotencyRef}`,
        },
        body: JSON.stringify(body),
        signal: AbortSignal.timeout(requestTimeoutMs),
      })
    } catch {
      throw unavailable()
    }

    if (response.status === 204 && operation === "claim") return null
    const raw = await readBoundedJson(response)
    if (!response.ok) {
      try {
        const decoded = S.decodeUnknownSync(ErrorEnvelope)(raw, {
          onExcessProperty: "error",
        })
        throw mapCode(decoded.error.code)
      } catch (error) {
        if (error instanceof PylonFleetRunRemotePortError) throw error
        throw unavailable()
      }
    }
    try {
      const decoded = S.decodeUnknownSync(SuccessEnvelope)(raw, {
        onExcessProperty: "error",
      })
      if (decoded.operation !== operation) throw unavailable()
      return decoded.result
    } catch (error) {
      if (error instanceof PylonFleetRunRemotePortError) throw error
      throw unavailable()
    }
  }

  return {
    claimNext: async ({ pylonRef, runRef }) => {
      if (runRef !== undefined && !RUN_REF.test(runRef)) {
        throw unavailable()
      }
      const scope = runRef ?? "next"
      const requestId = pendingClaimIds.get(scope) ?? makeId()
      pendingClaimIds.set(scope, requestId)
      try {
        const result = await post(
          "claim",
          pylonRef,
          {
            schema: "openagents.pylon.fleet_run_claim.request.v1",
            ...(runRef === undefined ? {} : { runRef }),
          },
          requestId,
        )
        if (result === null) pendingClaimIds.delete(scope)
        return result
      } catch (error) {
        if (
          error instanceof PylonFleetRunRemotePortError &&
          error.kind !== "unavailable"
        ) {
          pendingClaimIds.delete(scope)
        }
        throw error
      }
    },
    acceptClaim: async ({ claimRef, pylonRef, runRef }) => {
      if (!RUN_REF.test(runRef) || !CLAIM_REF.test(claimRef)) {
        throw unavailable()
      }
      const result = await post(
        "accept",
        pylonRef,
        {
          schema: "openagents.pylon.fleet_run_accept.request.v1",
          runRef,
          claimRef,
        },
        claimRef,
      )
      pendingClaimIds.delete("next")
      pendingClaimIds.delete(runRef)
      return result
    },
  }
}
