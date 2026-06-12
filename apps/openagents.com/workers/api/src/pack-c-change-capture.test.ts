import { describe, expect, test } from 'vitest'

import {
  PACK_C_CHANGE_CAPTURE_VERSION,
  projectPackCChangeCapture,
} from './pack-c-change-capture'

describe('Pack C change capture projections', () => {
  const base = {
    authorityReceiptRefs: ['authority:github-write:approved'],
    baseCommitRef: '18aaf36d9',
    changeRef: 'change:pack-c:pc2',
    diagnosticRefs: ['diagnostic:tsc:summary'],
    fileCount: 3,
    fileSummaryRefs: ['file-summary:pack-c:pc2'],
    generatedAt: '2026-06-12T04:20:30.000Z',
    headCommitRef: '0ce8a0cd5',
    observedAt: '2026-06-12T04:20:00.000Z',
    patchDigestRef: 'patch-digest:sha256:pack-c-pc2',
    publicSafe: true,
    repositoryRef: 'repo:github:OpenAgentsInc/openagents',
    reviewCaveatRefs: ['review-caveat:summary-only'],
    staleAfterMs: 60_000,
    summaryRef: 'summary:change:pack-c:pc2',
    verificationRefs: ['verification:vitest:pack-c-pc2'],
    visibility: 'public' as const,
    worktreeIdentityStatus: 'ready' as const,
    worktreeRef: 'worktree:pack-c:pc2',
    writebackRequired: true,
  }

  test('projects review-ready change captures with digest and summary refs only', () => {
    const projection = projectPackCChangeCapture(base)

    expect(projection).toEqual({
      ageMs: 30_000,
      authorityReceiptRefs: ['authority:github-write:approved'],
      baseCommitRef: '18aaf36d9',
      blockerRefs: [],
      changeRef: 'change:pack-c:pc2',
      changeVersion: PACK_C_CHANGE_CAPTURE_VERSION,
      diagnosticRefs: ['diagnostic:tsc:summary'],
      fileCount: 3,
      fileSummaryRefs: ['file-summary:pack-c:pc2'],
      freshness: 'fresh',
      generatedAt: '2026-06-12T04:20:30.000Z',
      headCommitRef: '0ce8a0cd5',
      observedAt: '2026-06-12T04:20:00.000Z',
      patchDigestRef: 'patch-digest:sha256:pack-c-pc2',
      publicSafe: true,
      repositoryRef: 'repo:github:OpenAgentsInc/openagents',
      reviewCaveatRefs: ['review-caveat:summary-only'],
      staleAt: '2026-06-12T04:21:00.000Z',
      status: 'review_ready',
      summaryRef: 'summary:change:pack-c:pc2',
      verificationRefs: ['verification:vitest:pack-c-pc2'],
      visibility: 'public',
      worktreeIdentityStatus: 'ready',
      worktreeRef: 'worktree:pack-c:pc2',
      writebackRequired: true,
    })
  })

  test('blocks missing verification, patch digest, writeback authority, and unsafe public visibility', () => {
    const projection = projectPackCChangeCapture({
      ...base,
      authorityReceiptRefs: [],
      patchDigestRef: null,
      publicSafe: false,
      verificationRefs: [],
    })

    expect(projection.status).toBe('blocked')
    expect(projection.blockerRefs).toEqual([
      'pack-c-change-capture-blocker:change:pack-c:pc2:missing-verification',
      'pack-c-change-capture-blocker:change:pack-c:pc2:missing-patch-digest',
      'pack-c-change-capture-blocker:change:pack-c:pc2:missing-writeback-authority',
      'pack-c-change-capture-blocker:change:pack-c:pc2:unsafe-public-visibility',
    ])
  })

  test('blocks stale and blocked worktree identity states', () => {
    expect(
      projectPackCChangeCapture({
        ...base,
        worktreeIdentityStatus: 'stale',
      }).blockerRefs,
    ).toEqual([
      'pack-c-change-capture-blocker:change:pack-c:pc2:stale-worktree-identity',
    ])

    expect(
      projectPackCChangeCapture({
        ...base,
        worktreeIdentityStatus: 'blocked',
      }).blockerRefs,
    ).toEqual([
      'pack-c-change-capture-blocker:change:pack-c:pc2:blocked-worktree-identity',
    ])
  })

  test('marks stale captures separately from blocked captures', () => {
    const projection = projectPackCChangeCapture({
      ...base,
      generatedAt: '2026-06-12T04:25:00.000Z',
    })

    expect(projection).toMatchObject({
      ageMs: 300_000,
      freshness: 'stale',
      status: 'stale',
    })
  })

  test('rejects raw patches, raw file content, private repo data, local paths, and credentials', () => {
    expect(() =>
      projectPackCChangeCapture({
        ...base,
        patchDigestRef: 'diff --git a/private.ts b/private.ts',
      }),
    ).toThrow(/raw patch, private repo, local path, or shell material/)

    expect(() =>
      projectPackCChangeCapture({
        ...base,
        fileSummaryRefs: ['raw_file:export const secret = true'],
      }),
    ).toThrow(/raw patch, private repo, local path, or shell material/)

    expect(() =>
      projectPackCChangeCapture({
        ...base,
        repositoryRef: 'repo:github:OpenAgentsInc/private_repo',
      }),
    ).toThrow(/raw patch, private repo, local path, or shell material/)

    expect(() =>
      projectPackCChangeCapture({
        ...base,
        summaryRef: '/Users/christopherdavid/work/openagents/private.ts',
      }),
    ).toThrow(/raw patch, private repo, local path, or shell material/)

    expect(() =>
      projectPackCChangeCapture({
        ...base,
        verificationRefs: ['ghp_1234567890abcdef1234567890abcdef'],
      }),
    ).toThrow(/provider credential material|stable Pack C ref/)
  })
})
