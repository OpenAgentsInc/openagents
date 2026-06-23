// Secret-backed HTTP transport for the OpenAgents/Pylon serving-fabric lane
// (#6089). This module is the Worker-side client for a real Pylon gateway
// route/proxy; it does NOT expose endpoint URLs or tokens in public policy
// surfaces. The upstream must speak the Psionic serve response contract, not raw
// OpenAI/vLLM chat completions, so parity/canary/replay evidence travels with
// the completion before the paid network adapter can clear.

import { Effect } from 'effect'

import {
  DEFAULT_HEARTBEAT_TTL_MS,
  type PylonAdmissionInput,
} from './khala-pylon-admission'
import { OPENAGENTS_NETWORK_ADAPTER_ID } from './openagents-network-adapter'
import { InferenceAdapterError } from './provider-adapter'
import {
  type PsionicServeRequest,
  type PsionicServeTransport,
} from './psionic-fabric-serve'

type HttpResponse = Response

export type PylonFabricFetchLike = (
  input: string,
  init: Readonly<{
    method: string
    headers: Record<string, string>
    body: string
  }>,
) => Promise<HttpResponse>

export type PylonFabricHttpTransportEnv = Readonly<{
  OPENAGENTS_NETWORK_ADMITTED_PYLON_REF?: string | undefined
  OPENAGENTS_NETWORK_FABRIC_SERVE_URL?: string | undefined
  OPENAGENTS_NETWORK_FABRIC_SERVE_BEARER_TOKEN?: string | undefined
  OPENAGENTS_NETWORK_PYLON_HEARTBEAT_AT?: string | undefined
  OPENAGENTS_NETWORK_PYLON_HEARTBEAT_STATUS?: string | undefined
  OPENAGENTS_NETWORK_PYLON_SERVING_CAPABILITY_REF?: string | undefined
  OPENAGENTS_NETWORK_PYLON_SERVING_LANE_REF?: string | undefined
  OPENAGENTS_NETWORK_SPARK_PAYOUT_TARGET_REF?: string | undefined
}>

export type PylonFabricHttpTransportConfig = Readonly<{
  endpoint: string
  bearerToken: string
  fetchImpl?: PylonFabricFetchLike | undefined
}>

export const DEFAULT_OPENAGENTS_NETWORK_SERVING_CAPABILITY_REF =
  'pylon.capability.serving.whole_small_model.v0.6'
export const DEFAULT_OPENAGENTS_NETWORK_SERVING_LANE_REF =
  'lane.openagents.pylon.vllm.whole_small_model.v1'

const trimmed = (value: string | undefined): string | undefined => {
  if (typeof value !== 'string') return undefined
  const out = value.trim()
  return out === '' ? undefined : out
}

export const pylonFabricHttpTransportConfigFromEnv = (
  env: PylonFabricHttpTransportEnv,
): PylonFabricHttpTransportConfig | undefined => {
  const endpoint = trimmed(env.OPENAGENTS_NETWORK_FABRIC_SERVE_URL)
  const bearerToken = trimmed(env.OPENAGENTS_NETWORK_FABRIC_SERVE_BEARER_TOKEN)
  if (endpoint === undefined || bearerToken === undefined) {
    return undefined
  }
  return { bearerToken, endpoint }
}

const classifyStatus = (
  status: number,
): Readonly<{ kind: string; retryable: boolean }> => {
  if (status === 429) return { kind: 'rate_limited', retryable: true }
  if (status === 503) return { kind: 'service_overloaded', retryable: true }
  if (status >= 500) return { kind: 'upstream_error', retryable: true }
  return { kind: 'request_rejected', retryable: false }
}

const transportError = (
  input: Readonly<{
    reason: string
    kind: string
    retryable?: boolean | undefined
    httpStatus?: number | undefined
  }>,
): InferenceAdapterError =>
  new InferenceAdapterError({
    adapterId: OPENAGENTS_NETWORK_ADAPTER_ID,
    httpStatus: input.httpStatus,
    kind: input.kind,
    reason: input.reason,
    retryable: input.retryable,
  })

export const makePylonFabricHttpTransport = (
  config: PylonFabricHttpTransportConfig,
): PsionicServeTransport => {
  const fetchImpl = config.fetchImpl ?? fetch
  return (request: PsionicServeRequest) =>
    Effect.tryPromise({
      try: async () => {
        const response = await fetchImpl(config.endpoint, {
          body: JSON.stringify(request),
          headers: {
            accept: 'application/json',
            authorization: `Bearer ${config.bearerToken}`,
            'content-type': 'application/json',
          },
          method: 'POST',
        })
        const body = await response.text()
        if (!response.ok) {
          const classification = classifyStatus(response.status)
          throw transportError({
            httpStatus: response.status,
            kind: classification.kind,
            reason: `Pylon fabric HTTP route rejected serve request with status ${response.status}`,
            retryable: classification.retryable,
          })
        }
        return body
      },
      catch: error =>
        error instanceof InferenceAdapterError
          ? error
          : transportError({
              kind: 'transport_error',
              reason: 'Pylon fabric HTTP route transport failed',
              retryable: true,
            }),
    })
}

export const pylonGatewayAdmissionFromEnv = (
  env: PylonFabricHttpTransportEnv,
  nowMs: number,
): PylonAdmissionInput => {
  const pylonRef =
    trimmed(env.OPENAGENTS_NETWORK_ADMITTED_PYLON_REF) ??
    'pylon.openagents-network.unconfigured'
  const capabilityRef =
    trimmed(env.OPENAGENTS_NETWORK_PYLON_SERVING_CAPABILITY_REF) ??
    DEFAULT_OPENAGENTS_NETWORK_SERVING_CAPABILITY_REF
  const servingLaneRef =
    trimmed(env.OPENAGENTS_NETWORK_PYLON_SERVING_LANE_REF) ??
    DEFAULT_OPENAGENTS_NETWORK_SERVING_LANE_REF
  const payoutTargetRef =
    trimmed(env.OPENAGENTS_NETWORK_SPARK_PAYOUT_TARGET_REF) ?? null

  return {
    heartbeatTtlMs: DEFAULT_HEARTBEAT_TTL_MS,
    nowMs,
    requiredCapabilityRef: capabilityRef,
    snapshot: {
      capabilityRefs: [capabilityRef],
      latestHeartbeatAt:
        trimmed(env.OPENAGENTS_NETWORK_PYLON_HEARTBEAT_AT) ?? null,
      latestHeartbeatStatus:
        trimmed(env.OPENAGENTS_NETWORK_PYLON_HEARTBEAT_STATUS) ?? null,
      pylonRef,
      servingLaneRefs: [servingLaneRef],
      sparkPayoutTargetRef: payoutTargetRef,
      status: 'active',
      walletReady: payoutTargetRef !== null,
    },
  }
}
