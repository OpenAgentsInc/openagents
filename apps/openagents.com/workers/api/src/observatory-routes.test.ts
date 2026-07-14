import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  observatoryProjectionDigestPayload,
  openAgentsDesktopMvpPublicTrace,
  parseObservatoryPublicTraceProjection,
} from './observatory-public-trace'
import {
  OPENAGENTS_DESKTOP_MVP_OBSERVATORY_PATH,
  handleObservatoryTracePage,
  renderObservatoryTraceHtml,
} from './observatory-routes'

const request = (
  path: string = OPENAGENTS_DESKTOP_MVP_OBSERVATORY_PATH,
  init: RequestInit = {},
) => new Request(`https://openagents.com${path}`, init)

const reviewedVariant = async (
  patch: Partial<typeof openAgentsDesktopMvpPublicTrace>,
) => {
  const placeholder = `sha256:${'0'.repeat(64)}`
  const candidate = parseObservatoryPublicTraceProjection({
    ...openAgentsDesktopMvpPublicTrace,
    ...patch,
    projectionDigest: placeholder,
    publicationReview: {
      ...openAgentsDesktopMvpPublicTrace.publicationReview,
      reviewedProjectionDigest: placeholder,
    },
  })
  const digestBytes = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(observatoryProjectionDigestPayload(candidate)),
  )
  const projectionDigest = `sha256:${[...new Uint8Array(digestBytes)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')}`
  return {
    ...candidate,
    projectionDigest,
    publicationReview: {
      ...candidate.publicationReview,
      reviewedProjectionDigest: projectionDigest,
    },
  }
}

describe('/observer/traces/openagents-desktop-codex-workroom-mvp', () => {
  test('renders four separately labeled facts for every criterion', () => {
    const html = renderObservatoryTraceHtml(openAgentsDesktopMvpPublicTrace)
    expect(html).toContain('Observatory · AssuranceSpec protocol')
    expect(html).toContain('Criterion facts, not a score.')
    expect(html.match(/data-fact="mapped"/g)).toHaveLength(18)
    expect(html.match(/data-fact="executable"/g)).toHaveLength(18)
    expect(html.match(/data-fact="observed"/g)).toHaveLength(18)
    expect(html.match(/data-fact="accepted"/g)).toHaveLength(18)
    expect(html).toContain('locations only, never verdicts')
    expect(html).not.toContain('Observer protocol')
    expect(html).not.toContain('ObservatorySpec')
    expect(html).not.toMatch(/\b\d+%\b/)
  })

  test('GET serves the reviewed public projection', async () => {
    const response = await Effect.runPromise(
      handleObservatoryTracePage(request()),
    )
    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(response.headers.get('x-robots-tag')).toBe('index, follow')
    expect(await response.text()).toContain('CW-AC-18')
  })

  test('an unlisted projection is available only at its exact link and noindexed', async () => {
    const unlisted = await reviewedVariant({
      projectRef: 'unlisted-proof-snapshot',
      visibility: 'unlisted',
    })
    const exact = await Effect.runPromise(
      handleObservatoryTracePage(
        request('/observer/traces/unlisted-proof-snapshot'),
        unlisted,
      ),
    )
    const guessed = await Effect.runPromise(
      handleObservatoryTracePage(request('/observer/traces/guess'), unlisted),
    )
    expect(exact.status).toBe(200)
    expect(exact.headers.get('x-robots-tag')).toBe('noindex, nofollow')
    expect(guessed.status).toBe(404)
  })

  test('private projections are indistinguishable from missing', async () => {
    const response = await Effect.runPromise(
      handleObservatoryTracePage(request(), {
        ...openAgentsDesktopMvpPublicTrace,
        visibility: 'private' as const,
      }),
    )
    expect(response.status).toBe(404)
  })

  test('HEAD is empty and mutations are rejected', async () => {
    const head = await Effect.runPromise(
      handleObservatoryTracePage(request(undefined, { method: 'HEAD' })),
    )
    const post = await Effect.runPromise(
      handleObservatoryTracePage(request(undefined, { method: 'POST' })),
    )
    expect(head.status).toBe(200)
    expect(await head.text()).toBe('')
    expect(post.status).toBe(405)
    expect(post.headers.get('allow')).toBe('GET, HEAD')
  })
})
