import { describe, expect, test } from 'vitest'

import type { AutopilotWorkProjection } from '../model'
import {
  buildForgeTeamSharedMemoryInput,
  projectForgeTeamSharedMemory,
} from './team-shared-memory'

const baseInput = {
  generatedAt: '2026-06-18T01:00:00.000Z',
  projectionRef: 'team-shared-memory-projection.public.work_1',
  snapshotRef: 'team-shared-memory-snapshot.public.work_1',
  versionRef: 'team-shared-memory-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

describe('Forge team shared memory projection', () => {
  test('projects scoped shared memory as refs-only non-authoritative state', () => {
    const view = projectForgeTeamSharedMemory({
      ...baseInput,
      entries: [
        {
          applicationReceiptRefs: ['memory-application.public.work_1.repo_style'],
          evidenceRefs: ['evidence.public.review.accepted'],
          freshness: 'fresh',
          kind: 'repo_style',
          memoryRef: 'shared-memory.public.repo_style',
          ownerRefs: ['owner.public.user_1'],
          policyRefs: ['policy.public.team_memory.visible'],
          redactionClass: 'team_ref',
          retrievalPolicyRefs: ['retrieval-policy.public.semantic_typed'],
          reviewState: 'accepted',
          scope: 'team',
          semanticQueryRefs: ['semantic-query.public.repo_style'],
          teamRefs: ['team.public.engineering'],
          typedQueryRefs: ['typed-query.public.repo_style'],
          visibility: 'team',
        },
      ],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      accepted: 1,
      pendingReview: 0,
      publicVisible: 0,
      stale: 0,
      teamVisible: 1,
      total: 1,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      deploymentAuthority: false,
      memoryCreateAuthority: false,
      memoryDeleteAuthority: false,
      memoryPromotionAuthority: false,
      memoryUpdateAuthority: false,
      modelCallAuthority: false,
      promptAssemblyAuthority: false,
      publicClaimAuthority: false,
      semanticRetrievalAuthority: false,
      settlementAuthority: false,
      teamRecordMutationAuthority: false,
      toolGrantAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing shared memory state as empty', () => {
    const view = projectForgeTeamSharedMemory({
      generatedAt: '2026-06-18T01:00:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.snapshotRef).toBeNull()
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks stale shared memory evidence', () => {
    const view = projectForgeTeamSharedMemory({
      ...baseInput,
      entries: [
        {
          freshness: 'stale',
          kind: 'budget_caveat',
          memoryRef: 'shared-memory.public.stale',
          policyRefs: ['policy.public.memory.retention'],
          redactionClass: 'private_ref',
          reviewState: 'accepted',
          scope: 'mission',
          visibility: 'private',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-team-shared-memory-blocker:work.public.work_1:stale-shared-memory-evidence:shared-memory.public.stale',
    )
  })

  test('blocks team-visible memory without team and policy refs', () => {
    const view = projectForgeTeamSharedMemory({
      ...baseInput,
      entries: [
        {
          freshness: 'fresh',
          kind: 'reviewer_preference',
          memoryRef: 'shared-memory.public.team_missing_policy',
          redactionClass: 'team_ref',
          reviewState: 'accepted',
          scope: 'team',
          visibility: 'team',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-team-shared-memory-blocker:work.public.work_1:team-memory-policy-missing:shared-memory.public.team_missing_policy',
    )
  })

  test('blocks public memory without public-safe redaction and policy refs', () => {
    const view = projectForgeTeamSharedMemory({
      ...baseInput,
      entries: [
        {
          freshness: 'fresh',
          kind: 'onboarding_note',
          memoryRef: 'shared-memory.public.public_missing_redaction',
          redactionClass: 'team_ref',
          reviewState: 'accepted',
          scope: 'public',
          visibility: 'public',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-team-shared-memory-blocker:work.public.work_1:public-memory-redaction-policy-missing:shared-memory.public.public_missing_redaction',
    )
  })

  test('blocks applied memory without application receipts', () => {
    const view = projectForgeTeamSharedMemory({
      ...baseInput,
      entries: [
        {
          freshness: 'fresh',
          kind: 'accepted_fix',
          memoryRef: 'shared-memory.public.applied_without_receipt',
          policyRefs: ['policy.public.memory.apply'],
          redactionClass: 'private_ref',
          reviewState: 'accepted',
          scope: 'repository',
          typedQueryRefs: ['typed-query.public.accepted_fix'],
          visibility: 'private',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-team-shared-memory-blocker:work.public.work_1:applied-memory-receipt-missing:shared-memory.public.applied_without_receipt',
    )
  })

  test('blocks deleted memory without tombstone or deletion receipts', () => {
    const view = projectForgeTeamSharedMemory({
      ...baseInput,
      entries: [
        {
          freshness: 'fresh',
          kind: 'denied_path',
          memoryRef: 'shared-memory.public.deleted_without_tombstone',
          policyRefs: ['policy.public.memory.delete'],
          redactionClass: 'private_ref',
          reviewState: 'deleted',
          scope: 'project',
          visibility: 'private',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-team-shared-memory-blocker:work.public.work_1:deleted-memory-tombstone-missing:shared-memory.public.deleted_without_tombstone',
    )
  })

  test('blocks promoted memory without consent and policy refs', () => {
    const view = projectForgeTeamSharedMemory({
      ...baseInput,
      entries: [
        {
          freshness: 'fresh',
          kind: 'build_command',
          memoryRef: 'shared-memory.public.promotion_missing_consent',
          promotionRefs: ['memory-promotion.public.personal_to_team'],
          redactionClass: 'team_ref',
          reviewState: 'accepted',
          scope: 'team',
          teamRefs: ['team.public.engineering'],
          visibility: 'team',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-team-shared-memory-blocker:work.public.work_1:team-memory-policy-missing:shared-memory.public.promotion_missing_consent',
    )
    expect(view.blockerRefs).toContain(
      'forge-team-shared-memory-blocker:work.public.work_1:memory-promotion-consent-missing:shared-memory.public.promotion_missing_consent',
    )
  })

  test('blocks populated shared memory entries without snapshot refs', () => {
    const view = projectForgeTeamSharedMemory({
      entries: [
        {
          freshness: 'fresh',
          kind: 'repo_style',
          memoryRef: 'shared-memory.public.no_snapshot',
          policyRefs: ['policy.public.memory.read'],
          redactionClass: 'private_ref',
          reviewState: 'accepted',
          scope: 'repository',
          visibility: 'private',
        },
      ],
      generatedAt: '2026-06-18T01:00:00.000Z',
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-team-shared-memory-blocker:work.public.no_snapshot:missing-team-shared-memory-snapshot-ref',
    )
  })

  test('omits unsafe private shared memory material before projection', () => {
    const view = projectForgeTeamSharedMemory({
      ...baseInput,
      blockerRefs: [
        'shared-memory-blocker.public.safe',
        'raw memory /Users/christopher/memory.md',
      ],
      entries: [
        {
          applicationReceiptRefs: ['memory-application.public.safe'],
          blockerRefs: ['entry-shared-memory-blocker.public.safe'],
          consentRefs: ['consent.public.safe'],
          deletionReceiptRefs: ['memory-deletion.public.safe'],
          evidenceRefs: ['evidence.public.safe', 'memory body /Users/christopher/private.md'],
          expiryRefs: ['expiry.public.safe'],
          freshness: 'fresh',
          kind: 'repo_style',
          memoryRef: 'shared-memory.public.safe',
          ownerRefs: ['owner.public.safe', 'raw prompt sk-private'],
          policyRefs: ['policy.public.safe', 'bearer token private'],
          promotionRefs: ['promotion.public.safe'],
          redactionClass: 'team_ref',
          retrievalPolicyRefs: ['retrieval-policy.public.safe'],
          reviewRefs: ['review.public.safe', 'provider payload sk-private'],
          reviewState: 'accepted',
          scope: 'team',
          semanticQueryRefs: ['semantic-query.public.safe'],
          teamRefs: ['team.public.safe'],
          tombstoneRefs: ['tombstone.public.safe'],
          typedQueryRefs: ['typed-query.public.safe'],
          visibility: 'team',
        },
      ],
    })

    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.evidenceRefs).toEqual(['evidence.public.safe'])
    expect(view.entries[0]?.policyRefs).toEqual(['policy.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-team-shared-memory-blocker:work.public.work_1:unsafe-team-shared-memory-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw memory')
    expect(payload).not.toContain('memory body')
    expect(payload).not.toContain('raw prompt')
    expect(payload).not.toContain('provider payload')
    expect(payload).not.toContain('bearer token')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      generatedAt: '2026-06-18T01:01:00.000Z',
      teamSharedMemory: {
        entries: [
          {
            freshness: 'fresh',
            kind: 'repo_style',
            memoryRef: 'shared-memory.public.work_2',
            policyRefs: ['policy.public.work_2'],
            redactionClass: 'private_ref',
            reviewState: 'accepted',
            scope: 'repository',
            visibility: 'private',
          },
        ],
        projectionRef: 'team-shared-memory-projection.public.work_2',
        snapshotRef: 'team-shared-memory-snapshot.public.work_2',
        versionRef: 'team-shared-memory-version.public.v2',
      },
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeTeamSharedMemoryInput(work)).toEqual({
      entries: [
        {
          freshness: 'fresh',
          kind: 'repo_style',
          memoryRef: 'shared-memory.public.work_2',
          policyRefs: ['policy.public.work_2'],
          redactionClass: 'private_ref',
          reviewState: 'accepted',
          scope: 'repository',
          visibility: 'private',
        },
      ],
      generatedAt: '2026-06-18T01:01:00.000Z',
      projectionRef: 'team-shared-memory-projection.public.work_2',
      snapshotRef: 'team-shared-memory-snapshot.public.work_2',
      versionRef: 'team-shared-memory-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
