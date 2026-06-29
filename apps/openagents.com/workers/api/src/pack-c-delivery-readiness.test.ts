import { describe, expect, test } from 'vitest'

import {
  PACK_C_DELIVERY_READINESS_VERSION,
  projectPackCDeliveryReadiness,
} from './pack-c-delivery-readiness'

describe('Pack C delivery readiness projections', () => {
  const base = {
    acceptanceReceiptRefs: [],
    agentDeliveryRefs: ['agent-delivery:pack-c:pc4'],
    caveatRefs: ['caveat:pack-c:pr-draft-only'],
    changeCaptureRefs: ['change-capture:pack-c:pc4'],
    changeCaptureStatus: 'review_ready' as const,
    deliveryReceiptRefs: ['delivery-receipt:pack-c:pc4'],
    deliveryRef: 'delivery:pack-c:pc4',
    generatedAt: '2026-06-12T05:05:00.000Z',
    githubWritebackAuthorityRefs: ['github-writeback-authority:pack-c:pc4'],
    humanMergeCaveatRefs: ['human-merge-caveat:pack-c:pc4'],
    marketDeliveryRefs: ['market-delivery:pack-c:pc4'],
    observedAt: '2026-06-12T05:00:00.000Z',
    publicSafe: true,
    repositoryIdentityRef: 'repo-identity:openagents:main',
    repositoryIdentityStatus: 'ready' as const,
    reviewRefs: ['review:pack-c:pc4'],
    settlementReceiptRefs: [],
    staleAfterMs: 15 * 60 * 1000,
    verificationRefs: ['verification:pack-c:pc4'],
    visibility: 'public' as const,
    worktreeIdentityRef: 'worktree-identity:pack-c:pc4',
    worktreeIdentityStatus: 'ready' as const,
  }

  test('marks PR draft readiness as ready when refs and caveats are present', () => {
    const projection = projectPackCDeliveryReadiness(base)

    expect(projection).toMatchObject({
      deliveryReadinessVersion: PACK_C_DELIVERY_READINESS_VERSION,
      deliveryRef: 'delivery:pack-c:pc4',
      freshness: 'fresh',
      status: 'ready',
      blockerRefs: [],
      repositoryIdentityRef: 'repo-identity:openagents:main',
      worktreeIdentityRef: 'worktree-identity:pack-c:pc4',
      changeCaptureRefs: ['change-capture:pack-c:pc4'],
      verificationRefs: ['verification:pack-c:pc4'],
      githubWritebackAuthorityRefs: ['github-writeback-authority:pack-c:pc4'],
      reviewRefs: ['review:pack-c:pc4'],
      humanMergeCaveatRefs: ['human-merge-caveat:pack-c:pc4'],
      authorityBoundary: {
        acceptanceAuthority: 'separate_receipt_required',
        agentDeliveryAuthority: 'evidence_only',
        humanMergeAuthority: 'not_delegated',
        marketDeliveryAuthority: 'evidence_only',
        prDraftWritebackAuthority: 'ready',
        settlementAuthority: 'separate_receipt_required',
      },
    })
  })

  test('blocks missing authority, verification, review, change capture, and unsafe visibility', () => {
    const projection = projectPackCDeliveryReadiness({
      ...base,
      changeCaptureRefs: [],
      githubWritebackAuthorityRefs: [],
      humanMergeCaveatRefs: [],
      publicSafe: false,
      reviewRefs: [],
      verificationRefs: [],
    })

    expect(projection.status).toBe('blocked')
    expect(projection.blockerRefs).toEqual([
      'pack-c-delivery-readiness-blocker:delivery:pack-c:pc4:missing-change-capture',
      'pack-c-delivery-readiness-blocker:delivery:pack-c:pc4:missing-writeback-authority',
      'pack-c-delivery-readiness-blocker:delivery:pack-c:pc4:missing-verification',
      'pack-c-delivery-readiness-blocker:delivery:pack-c:pc4:missing-review',
      'pack-c-delivery-readiness-blocker:delivery:pack-c:pc4:missing-human-merge-caveat',
      'pack-c-delivery-readiness-blocker:delivery:pack-c:pc4:unsafe-public-visibility',
    ])
  })

  test('blocks stale identity and stale change capture evidence', () => {
    const projection = projectPackCDeliveryReadiness({
      ...base,
      changeCaptureStatus: 'stale',
      generatedAt: '2026-06-12T06:00:00.000Z',
      repositoryIdentityStatus: 'stale',
      staleAfterMs: 5 * 60 * 1000,
      worktreeIdentityStatus: 'stale',
    })

    expect(projection.status).toBe('blocked')
    expect(projection.freshness).toBe('stale')
    expect(projection.blockerRefs).toEqual([
      'pack-c-delivery-readiness-blocker:delivery:pack-c:pc4:stale-repository-identity',
      'pack-c-delivery-readiness-blocker:delivery:pack-c:pc4:stale-worktree-identity',
      'pack-c-delivery-readiness-blocker:delivery:pack-c:pc4:stale-change-capture',
      'pack-c-delivery-readiness-blocker:delivery:pack-c:pc4:stale-delivery-readiness',
    ])
  })

  test('keeps blockers visible under a scoped exception', () => {
    const projection = projectPackCDeliveryReadiness({
      ...base,
      githubWritebackAuthorityRefs: [],
      scopedExceptionRef: 'scoped-exception:pack-c:pc4',
      verificationRefs: [],
    })

    expect(projection.status).toBe('scoped_exception')
    expect(projection.scopedExceptionRef).toBe('scoped-exception:pack-c:pc4')
    expect(projection.blockerRefs).toEqual([
      'pack-c-delivery-readiness-blocker:delivery:pack-c:pc4:missing-writeback-authority',
      'pack-c-delivery-readiness-blocker:delivery:pack-c:pc4:missing-verification',
    ])
    expect(projection.authorityBoundary.prDraftWritebackAuthority).toBe(
      'scoped_exception',
    )
  })

  test('does not let market or agent deliveries satisfy merge, acceptance, or settlement authority', () => {
    const evidenceOnly = projectPackCDeliveryReadiness({
      ...base,
      agentDeliveryRefs: ['agent-delivery:pack-c:pc4:generated'],
      marketDeliveryRefs: ['market-delivery:pack-c:pc4:listed'],
    })

    expect(evidenceOnly.status).toBe('ready')
    expect(evidenceOnly.authorityBoundary).toMatchObject({
      acceptanceAuthority: 'separate_receipt_required',
      agentDeliveryAuthority: 'evidence_only',
      humanMergeAuthority: 'not_delegated',
      marketDeliveryAuthority: 'evidence_only',
      settlementAuthority: 'separate_receipt_required',
    })

    const receipted = projectPackCDeliveryReadiness({
      ...base,
      acceptanceReceiptRefs: ['acceptance-receipt:pack-c:pc4'],
      settlementReceiptRefs: ['settlement-receipt:pack-c:pc4'],
    })

    expect(receipted.authorityBoundary).toMatchObject({
      acceptanceAuthority: 'receipt_present',
      settlementAuthority: 'receipt_present',
    })
  })

  test('rejects raw patches, shell logs, private repo data, local paths, provider payloads, credentials, and payment material', () => {
    expect(() =>
      projectPackCDeliveryReadiness({
        ...base,
        changeCaptureRefs: ['diff --git a/src/file.ts b/src/file.ts'],
      }),
    ).toThrow(/raw patch, raw shell, private repo, local path, or payment material/)

    expect(() =>
      projectPackCDeliveryReadiness({
        ...base,
        deliveryReceiptRefs: ['raw_shell:bun test && gh issue close'],
      }),
    ).toThrow(/raw patch, raw shell, private repo, local path, or payment material/)

    expect(() =>
      projectPackCDeliveryReadiness({
        ...base,
        repositoryIdentityRef: 'private_repo:customer-source',
      }),
    ).toThrow(/raw patch, raw shell, private repo, local path, or payment material/)

    expect(() =>
      projectPackCDeliveryReadiness({
        ...base,
        worktreeIdentityRef: '/Users/christopherdavid/work/openagents',
      }),
    ).toThrow(/raw patch, raw shell, private repo, local path, or payment material/)

    expect(() =>
      projectPackCDeliveryReadiness({
        ...base,
        caveatRefs: ['provider_payload:raw-response'],
      }),
    ).toThrow(/raw patch, raw shell, private repo, local path, or payment material/)

    expect(() =>
      projectPackCDeliveryReadiness({
        ...base,
        reviewRefs: ['ghp_1234567890abcdef1234567890abcdef'],
      }),
    ).toThrow(/provider credential material|stable Pack C delivery ref/)

    expect(() =>
      projectPackCDeliveryReadiness({
        ...base,
        settlementReceiptRefs: ['payment_preimage:secret'],
      }),
    ).toThrow(/raw patch, raw shell, private repo, local path, or payment material/)
  })
})
