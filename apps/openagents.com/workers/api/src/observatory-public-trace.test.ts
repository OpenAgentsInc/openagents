import { describe, expect, test } from 'vitest'

import {
  ObservatoryProjectionUnsafe,
  admitObservatoryProjectionForPublicRead,
  observatoryProjectionDigestMatches,
  openAgentsDesktopMvpPublicTrace,
  parseObservatoryPublicTraceProjection,
} from './observatory-public-trace'

describe('Observatory public trace projection', () => {
  test('keeps the four criterion facts independent and exposes no blended score', () => {
    const projection = parseObservatoryPublicTraceProjection(
      openAgentsDesktopMvpPublicTrace,
    )
    expect(projection.criteria).toHaveLength(18)
    expect(projection.criteria[0]).toMatchObject({
      accepted: { state: 'pending' },
      executable: { state: 'blocked' },
      mapped: { state: 'missing' },
      observed: { state: 'not_run' },
    })

    const keys = JSON.stringify(projection)
    for (const forbiddenAggregate of [
      'score',
      'percentage',
      'progress',
      'overallVerdict',
    ]) {
      expect(keys).not.toContain(`"${forbiddenAggregate}"`)
    }
  })

  test('rejects private source material before schema stripping', () => {
    expect(() =>
      parseObservatoryPublicTraceProjection({
        ...openAgentsDesktopMvpPublicTrace,
        rawPrompt: 'contents from /Users/someone/private-repo',
      }),
    ).toThrow(ObservatoryProjectionUnsafe)
  })

  test('also rejects innocuous excess keys instead of silently stripping them', () => {
    expect(() =>
      parseObservatoryPublicTraceProjection({
        ...openAgentsDesktopMvpPublicTrace,
        debug: true,
      }),
    ).toThrow()
  })

  test('requires review to bind the exact projection digest', () => {
    expect(() =>
      parseObservatoryPublicTraceProjection({
        ...openAgentsDesktopMvpPublicTrace,
        publicationReview: {
          ...openAgentsDesktopMvpPublicTrace.publicationReview,
          reviewedProjectionDigest: 'sha256:different-reviewed-artifact',
        },
      }),
    ).toThrow('Publication review does not bind')
  })

  test('the reviewed digest matches the canonical projected bytes', async () => {
    expect(
      await observatoryProjectionDigestMatches(openAgentsDesktopMvpPublicTrace),
    ).toBe(true)
  })

  test('private is never public, unlisted is exact-only, and public is discoverable', () => {
    const privateProjection = {
      ...openAgentsDesktopMvpPublicTrace,
      visibility: 'private' as const,
    }
    const unlistedProjection = {
      ...openAgentsDesktopMvpPublicTrace,
      visibility: 'unlisted' as const,
    }

    expect(
      admitObservatoryProjectionForPublicRead(privateProjection, 'exact'),
    ).toBeUndefined()
    expect(
      admitObservatoryProjectionForPublicRead(unlistedProjection, 'discovery'),
    ).toBeUndefined()
    expect(
      admitObservatoryProjectionForPublicRead(unlistedProjection, 'exact')
        ?.visibility,
    ).toBe('unlisted')
    expect(
      admitObservatoryProjectionForPublicRead(
        openAgentsDesktopMvpPublicTrace,
        'discovery',
      )?.visibility,
    ).toBe('public')
  })

  test('Related Artifacts remain location metadata, not criterion verdicts', () => {
    const [criterion] = openAgentsDesktopMvpPublicTrace.criteria
    expect(criterion?.relatedArtifacts).toHaveLength(2)
    expect(criterion?.mapped.state).toBe('missing')
    expect(criterion?.observed.state).toBe('not_run')
    expect(criterion?.accepted.state).toBe('pending')
  })
})
