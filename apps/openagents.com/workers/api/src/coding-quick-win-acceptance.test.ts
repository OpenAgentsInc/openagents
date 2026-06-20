import { describe, expect, it } from 'vitest'

import {
  CodingQuickWinAcceptanceInvariantError,
  buildCodingQuickWinAcceptanceEvidence,
  codingQuickWinAcceptedEvidenceRef,
  publicCodingQuickWinAcceptanceProjection,
} from './coding-quick-win-acceptance'

describe('coding-quick-win-acceptance', () => {
  it('builds accepted evidence for an approved diff', () => {
    const evidence = buildCodingQuickWinAcceptanceEvidence({
      diffRef: 'pr-url',
      acceptedByUserId: 'user_123',
      acceptanceAction: 'diff_approved',
      attestationRef: 'review-url',
    })

    expect(evidence.evidenceKind).toBe('coding_quick_win_acceptance')
    expect(evidence.offeringPromiseId).toBe('business.coding_quick_win.v1')
    expect(evidence.isAccepted).toBe(true)
    expect(codingQuickWinAcceptedEvidenceRef(evidence)).toBe('review-url')
  })

  it('builds accepted evidence for a merged diff', () => {
    const evidence = buildCodingQuickWinAcceptanceEvidence({
      diffRef: 'pr-url',
      acceptedByUserId: 'user_123',
      acceptanceAction: 'diff_merged',
      attestationRef: 'merge-sha',
    })

    expect(evidence.isAccepted).toBe(true)
    expect(codingQuickWinAcceptedEvidenceRef(evidence)).toBe('merge-sha')
  })

  it('builds unaccepted evidence for a rejected diff', () => {
    const evidence = buildCodingQuickWinAcceptanceEvidence({
      diffRef: 'pr-url',
      acceptedByUserId: 'user_123',
      acceptanceAction: 'diff_rejected',
      attestationRef: 'rejection-url',
    })

    expect(evidence.isAccepted).toBe(false)
    expect(() => codingQuickWinAcceptedEvidenceRef(evidence)).toThrow(
      CodingQuickWinAcceptanceInvariantError,
    )
  })

  it('requires all fields', () => {
    expect(() =>
      buildCodingQuickWinAcceptanceEvidence({
        diffRef: '  ',
        acceptedByUserId: 'user_123',
        acceptanceAction: 'diff_approved',
        attestationRef: 'review-url',
      }),
    ).toThrow(/diffRef is required/)

    expect(() =>
      buildCodingQuickWinAcceptanceEvidence({
        diffRef: 'pr-url',
        acceptedByUserId: '',
        acceptanceAction: 'diff_approved',
        attestationRef: 'review-url',
      }),
    ).toThrow(/acceptedByUserId is required/)

    expect(() =>
      buildCodingQuickWinAcceptanceEvidence({
        diffRef: 'pr-url',
        acceptedByUserId: 'user_123',
        acceptanceAction: 'diff_approved',
        attestationRef: '  ',
      }),
    ).toThrow(/attestationRef is required/)
  })

  it('projects safely', () => {
    const evidence = buildCodingQuickWinAcceptanceEvidence({
      diffRef: 'pr-url',
      acceptedByUserId: 'user_123',
      acceptanceAction: 'diff_approved',
      attestationRef: 'review-url',
    })

    const projection = publicCodingQuickWinAcceptanceProjection(evidence)
    expect(projection).toEqual({
      evidenceKind: 'coding_quick_win_acceptance',
      offeringPromiseId: 'business.coding_quick_win.v1',
      acceptanceAction: 'diff_approved',
      isAccepted: true,
    })
    // @ts-expect-error - Ensure private fields are dropped
    expect(projection.acceptedByUserId).toBeUndefined()
    // @ts-expect-error
    expect(projection.attestationRef).toBeUndefined()
  })
})
