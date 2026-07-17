import {
  FullAutoRunClientProjection as FullAutoRunClientProjectionSchema,
  FullAutoRunClientRunProjection as FullAutoRunClientRunProjectionSchema,
  type FullAutoRunClientProjection,
} from "@openagentsinc/khala-sync"
import { Schema as S } from "effect"

/**
 * FA-RUN-05 (#8981): fetch/publish ergonomics for the FullAutoRun mobile
 * projection, mirroring `fetchFleetRunClientProjection`'s shape one-for-one
 * (`fleet-run-client-projection.ts` in this package). Two call sites use
 * this module: mobile (#8982) GETs the projection with
 * `fetchFullAutoRunClientProjection`; Desktop POSTs a fresh projection with
 * `publishFullAutoRunClientProjection` on run start, every lifecycle
 * transition, and a periodic heartbeat while Running.
 */
export const FULL_AUTO_RUNS_PATH = "/api/full-auto-runs"

const FetchResponseEnvelope = S.Struct({
  ok: S.Literal(true),
  projection: FullAutoRunClientProjectionSchema,
})
const PublishResponseEnvelope = S.Struct({
  ok: S.Literal(true),
  projection: FullAutoRunClientProjectionSchema,
})

export type FullAutoRunProjectionFetchResult =
  | Readonly<{ state: "available"; projection: FullAutoRunClientProjection }>
  | Readonly<{ state: "unauthorized" }>
  | Readonly<{ state: "unavailable" }>

export type FullAutoRunProjectionPublishResult =
  | Readonly<{ state: "published"; projection: FullAutoRunClientProjection }>
  | Readonly<{ state: "unauthorized" }>
  | Readonly<{ state: "unavailable" }>

export type FullAutoRunProjectionFetch = (
  input: URL | RequestInfo,
  init?: RequestInit,
) => Promise<Response>

export const fetchFullAutoRunClientProjection = async (input: Readonly<{
  baseUrl: string
  accessToken: string
  fetchImpl?: FullAutoRunProjectionFetch
}>): Promise<FullAutoRunProjectionFetchResult> => {
  try {
    const response = await (input.fetchImpl ?? fetch)(
      new URL(FULL_AUTO_RUNS_PATH, input.baseUrl),
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
    const envelope = S.decodeUnknownSync(FetchResponseEnvelope)(await response.json(), {
      onExcessProperty: "preserve",
    })
    return { state: "available", projection: envelope.projection }
  } catch {
    return { state: "unavailable" }
  }
}

/**
 * Publishes the caller-supplied run projection (or `null` to signal "no
 * active run right now") as the owner's current FullAutoRun projection.
 * `run: null` never overwrites live data as a caller mistake because it is
 * the ONLY way to clear the projection -- Desktop calls this exclusively
 * from its own registry transition results, never a partial/derived value.
 */
export const publishFullAutoRunClientProjection = async (input: Readonly<{
  baseUrl: string
  accessToken: string
  run: FullAutoRunClientProjection["run"]
  fetchImpl?: FullAutoRunProjectionFetch
}>): Promise<FullAutoRunProjectionPublishResult> => {
  try {
    const body = input.run === null
      ? { run: null }
      : { run: S.encodeUnknownSync(FullAutoRunClientRunProjectionSchema)(input.run) }
    const response = await (input.fetchImpl ?? fetch)(
      new URL(FULL_AUTO_RUNS_PATH, input.baseUrl),
      {
        method: "POST",
        headers: {
          authorization: `Bearer ${input.accessToken}`,
          "content-type": "application/json",
        },
        body: JSON.stringify(body),
      },
    )
    if (response.status === 401 || response.status === 403) {
      return { state: "unauthorized" }
    }
    if (!response.ok) return { state: "unavailable" }
    const envelope = S.decodeUnknownSync(PublishResponseEnvelope)(await response.json(), {
      onExcessProperty: "preserve",
    })
    return { state: "published", projection: envelope.projection }
  } catch {
    return { state: "unavailable" }
  }
}
