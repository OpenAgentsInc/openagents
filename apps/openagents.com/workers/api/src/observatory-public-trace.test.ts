import { describe, expect, test } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

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
      accepted: { state: 'accepted' },
      executable: { state: 'executable' },
      mapped: { state: 'mapped' },
      observed: { state: 'CONFIRMED' },
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
    expect(criterion?.relatedArtifacts).toHaveLength(3)
    expect(criterion?.mapped.state).toBe('mapped')
    expect(criterion?.observed.state).toBe('CONFIRMED')
    expect(criterion?.accepted.state).toBe('accepted')
  })

  test('binds every public criterion row to the committed reviewed Evidence Index', () => {
    const repositoryRoot = resolve(import.meta.dirname, '../../../../../')
    const index = JSON.parse(
      readFileSync(
        resolve(
          repositoryRoot,
          'assurance/openagents-desktop-mvp.evidence-index.json',
        ),
        'utf8',
      ),
    ) as {
      receipts: Array<{
        candidate: { ref: string }
        criterion_refs: Array<string>
        obligation_id: string
      }>
    }
    expect(index.receipts).toHaveLength(18)
    for (const row of index.receipts) {
      const criterion = openAgentsDesktopMvpPublicTrace.criteria.find(
        candidate => candidate.criterionRef === row.criterion_refs[0],
      )
      expect(criterion).toMatchObject({
        accepted: { dispositionRefs: [row.candidate.ref], state: 'accepted' },
        criterionRef: row.criterion_refs[0],
        mapped: { obligationRefs: [row.obligation_id], state: 'mapped' },
        observed: { receiptRefs: [row.candidate.ref], state: 'CONFIRMED' },
      })
    }
  })
})
