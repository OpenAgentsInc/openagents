import type { ShareProjectionV1 } from '@openagentsinc/sync-schema'

// Live data for the `/share/{shareId}` route. Ported from the fetch logic in
// `apps/web/src/page/loggedOut/update.ts` (`LoadShareProjection`) — same
// endpoint, same request shape, same fail-soft posture. This app cannot
// import from `apps/web` (separate package), so only the small amount of
// fetch/decode logic is reproduced here; the wire type itself is the shared
// canonical `ShareProjectionV1` from `@openagentsinc/sync-schema` (a
// type-only import, so it adds no runtime bundle weight).

export const shareProjectionUrl = (shareId: string): string =>
  `/api/share/${encodeURIComponent(shareId)}/v1/data`

export type ShareProjectionResult =
  | Readonly<{ tag: 'loaded'; projection: ShareProjectionV1 }>
  | Readonly<{ tag: 'failed'; status: number; error: string }>

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null

export const fetchShareProjection = async (
  shareId: string,
  fetchFn: typeof fetch = fetch,
): Promise<ShareProjectionResult> => {
  try {
    const response = await fetchFn(shareProjectionUrl(shareId), {
      cache: 'no-store',
      credentials: 'include',
      headers: { accept: 'application/json' },
    })

    if (!response.ok) {
      const errorPayload = await response.json().catch(() => null)
      const error =
        isRecord(errorPayload) && typeof errorPayload.error === 'string'
          ? errorPayload.error
          : `Share returned HTTP ${response.status}.`

      return { tag: 'failed', status: response.status, error }
    }

    const payload = await response.json().catch(() => null)

    if (!isRecord(payload) || !isRecord(payload.projection)) {
      return {
        tag: 'failed',
        status: 0,
        error: 'Share response was malformed.',
      }
    }

    return {
      tag: 'loaded',
      projection: payload.projection as ShareProjectionV1,
    }
  } catch (cause) {
    return {
      tag: 'failed',
      status: 0,
      error: cause instanceof Error ? cause.message : String(cause),
    }
  }
}

// Ported from `apps/web/src/display-copy.ts`. Rewrites the internal
// "Adjutant" codename (and its `@adjutant` mention command) to the
// user-facing "Autopilot" name before any share content renders.
const internalAdjutantCommand = /(^|\s)@adjutant(?=$|\s)/g

export const userFacingCopy = (value: string): string =>
  value
    .replaceAll('Adjutant', 'Autopilot')
    .replace(internalAdjutantCommand, match =>
      match.replace('@adjutant', '@autopilot'),
    )
