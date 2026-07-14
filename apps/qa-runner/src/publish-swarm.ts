import type { QaSwarmRunProjection } from '@openagentsinc/qa-swarm-contract'
import { Effect } from 'effect'

export type QaSwarmProjectionPublishConfig = Readonly<{
  baseUrl: string
  token: string
}>

export type QaSwarmProjectionPublishResult =
  | Readonly<{ published: true; publicReadUrl: string; shareUrl: string }>
  | Readonly<{ published: false; reason: string }>

export type QaSwarmPublishFetch = (
  input: string | URL | Request,
  init?: RequestInit,
) => Promise<Response>

export const qaSwarmProjectionPublicReadUrl = (
  baseUrl: string,
  runRef: string,
): string =>
  `${baseUrl.replace(/\/$/, '')}/api/public/qa-swarm/runs/${encodeURIComponent(runRef)}`

export const publishQaSwarmProjection = (input: Readonly<{
  config?: QaSwarmProjectionPublishConfig | undefined
  fetch?: QaSwarmPublishFetch | undefined
  projection: QaSwarmRunProjection
}>): Effect.Effect<QaSwarmProjectionPublishResult> =>
  Effect.gen(function* () {
    const config = input.config
    if (config === undefined || config.token.trim() === '') {
      return { published: false as const, reason: 'QA Swarm publication is not configured' }
    }
    const baseUrl = config.baseUrl.replace(/\/$/, '')
    const response = yield* Effect.tryPromise({
      try: () =>
        (input.fetch ?? globalThis.fetch)(
          `${baseUrl}/api/operator/qa-swarm/runs/${encodeURIComponent(input.projection.runRef)}`,
          {
            method: 'PUT',
            headers: {
              accept: 'application/json',
              authorization: `Bearer ${config.token}`,
              'content-type': 'application/json',
            },
            body: JSON.stringify(input.projection),
          },
        ),
      catch: error => String(error),
    }).pipe(Effect.catch(reason => Effect.succeed(null as Response | null)))
    if (response === null || !response.ok) {
      return {
        published: false as const,
        reason: response === null
          ? 'QA Swarm publication request failed'
          : `QA Swarm publication returned HTTP ${response.status}`,
      }
    }
    return {
      published: true as const,
      publicReadUrl: qaSwarmProjectionPublicReadUrl(baseUrl, input.projection.runRef),
      shareUrl: `${baseUrl}/qa/${encodeURIComponent(input.projection.runRef)}`,
    }
  })
