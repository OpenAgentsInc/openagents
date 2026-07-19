import { AtifTrajectory } from '@openagentsinc/atif/trace'
import { Exit, Schema as S } from 'effect'

export class TraceBlobRef extends S.Class<TraceBlobRef>('TraceBlobRef')({
  kind: S.Literals(['video', 'screenshot', 'image']),
  r2Key: S.String,
  contentType: S.optionalKey(S.String),
  caption: S.optionalKey(S.String),
}) {}

const TraceAuthority = S.Struct({
  acceptedWorkAuthority: S.Boolean,
  payoutAuthority: S.Boolean,
  publicClaimAuthority: S.Boolean,
})

const TraceDataMarket = S.Struct({
  trainingConsent: S.Boolean,
  license: S.optionalKey(S.String),
  uploadSource: S.Literals(['agent', 'user_session']),
  reward: S.Struct({
    eligible: S.Boolean,
    amountSats: S.NullOr(S.Number),
    status: S.Literal('tbd'),
  }),
})

export class TraceProjection extends S.Class<TraceProjection>('TraceProjection')({
  uuid: S.String,
  schemaVersion: S.String,
  trajectoryId: S.String,
  sessionId: S.optionalKey(S.String),
  visibility: S.Literals(['public', 'unlisted', 'owner_only']),
  agentRef: S.String,
  stepCount: S.Number,
  trajectory: AtifTrajectory,
  blobRefs: S.Array(TraceBlobRef),
  createdAt: S.String,
  dataMarket: TraceDataMarket,
  authority: TraceAuthority,
}) {}

const TraceReadResponse = S.Struct({ trace: TraceProjection })
const decodeTraceReadResponse = S.decodeUnknownExit(TraceReadResponse)

const isRecord = (value: unknown): value is Readonly<Record<string, unknown>> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

/** Keep only the read-scope token used by mobile owner-only deep links. */
export const traceReadToken = (search: string): string | undefined => {
  const token = new URLSearchParams(search).get('token')?.trim()
  return token === undefined || token === '' ? undefined : token
}

export const traceProjectionUrl = (
  traceUuid: string,
  token?: string,
): string => {
  const path = `/api/traces/${encodeURIComponent(traceUuid)}`
  return token === undefined
    ? path
    : `${path}?token=${encodeURIComponent(token)}`
}

export const traceBlobUrl = (
  traceUuid: string,
  r2Key: string,
  token?: string,
): string => {
  const key = r2Key.split('/').map(encodeURIComponent).join('/')
  const path = `/api/traces/${encodeURIComponent(traceUuid)}/blob/${key}`
  return token === undefined
    ? path
    : `${path}?token=${encodeURIComponent(token)}`
}

export type TraceProjectionResult =
  | Readonly<{ tag: 'loaded'; projection: TraceProjection }>
  | Readonly<{ tag: 'failed'; status: number; error: string }>

export const fetchTraceProjection = async (
  traceUuid: string,
  token?: string,
  fetchFn: typeof fetch = fetch,
): Promise<TraceProjectionResult> => {
  try {
    const response = await fetchFn(traceProjectionUrl(traceUuid, token), {
      cache: 'no-store',
      credentials: 'include',
      headers: { accept: 'application/json' },
    })
    const payload = await response.json().catch(() => null)

    if (!response.ok) {
      return {
        tag: 'failed',
        status: response.status,
        error:
          isRecord(payload) && typeof payload.error === 'string'
            ? payload.error
            : `Trace returned HTTP ${response.status}.`,
      }
    }

    const decoded = decodeTraceReadResponse(payload)
    if (Exit.isFailure(decoded)) {
      return {
        tag: 'failed',
        status: 0,
        error: 'Trace response was malformed.',
      }
    }

    return { tag: 'loaded', projection: decoded.value.trace }
  } catch (cause) {
    return {
      tag: 'failed',
      status: 0,
      error: cause instanceof Error ? cause.message : String(cause),
    }
  }
}
