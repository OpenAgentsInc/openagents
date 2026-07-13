import {
  FleetRunClientProjection as FleetRunClientProjectionSchema,
  type FleetRunClientProjection,
} from "@openagentsinc/khala-sync"
import { Schema as S } from "effect"

const ResponseEnvelope = S.Struct({
  ok: S.Literal(true),
  fleet: FleetRunClientProjectionSchema,
})

export type FleetRunProjectionFetchResult =
  | Readonly<{ state: "available"; projection: FleetRunClientProjection }>
  | Readonly<{ state: "unauthorized" }>
  | Readonly<{ state: "unavailable" }>

export type FleetRunProjectionFetch = (
  input: URL | RequestInfo,
  init?: RequestInit,
) => Promise<Response>

export const fetchFleetRunClientProjection = async (input: Readonly<{
  baseUrl: string
  accessToken: string
  fetchImpl?: FleetRunProjectionFetch
}>): Promise<FleetRunProjectionFetchResult> => {
  try {
    const response = await (input.fetchImpl ?? fetch)(
      new URL("/api/sarah/fleet-runs", input.baseUrl),
      {
        method: "GET",
        headers: { authorization: `Bearer ${input.accessToken}` },
        cache: "no-store",
      },
    )
    if (response.status === 401 || response.status === 403) {
      return { state: "unauthorized" }
    }
    if (!response.ok) return { state: "unavailable" }
    const envelope = S.decodeUnknownSync(ResponseEnvelope)(await response.json(), {
      onExcessProperty: "preserve",
    })
    return { state: "available", projection: envelope.fleet }
  } catch {
    return { state: "unavailable" }
  }
}
