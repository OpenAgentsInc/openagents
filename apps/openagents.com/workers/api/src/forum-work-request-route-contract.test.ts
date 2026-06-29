import { describe, expect, test } from 'vitest'

import {
  ForumWorkRequestBodyValidationError,
  decodeCreateForumWorkRequestBody,
  workRequestMatchesInput,
} from './forum-work-request-route-contract'
import {
  type ForumWorkRequestRecord,
  normalizeForumWorkRequestInput,
} from './forum-work-requests'

const workRequestRecord: ForumWorkRequestRecord = {
  budgetMsats: 100_000_000,
  budgetSats: 100_000,
  createdAt: '2026-06-18T15:23:43.476Z',
  deadlineRef: 'deadline.public.debt_receipt.20260625',
  firstPostId: 'post-1',
  idempotencyKey: 'debt-receipt:receipt_public_debt_5334_template',
  jobEventId: 'job-1',
  jobEventKind: 5934,
  jobResultKind: 6934,
  objectiveRef: 'receipt.public.debt.5334.template',
  publicProjection: {
    classificationCaveatRef: 'classification.public_forum_projection',
    customerSafe: true,
    dataClassification: 'public',
    excludedPrivateRefs: [],
    publicSafe: true,
    redactionPolicyRef: 'redaction.forum.public.v1',
    safeArtifactRefs: ['artifact.forum.work_request.wr-1'],
    safeReceiptRefs: [],
    trustTier: 'reviewed',
  },
  quoteCount: 0,
  relayUrl: 'wss://relay.test.openagents.dev',
  repositoryRefs: ['repo.public.openagents'],
  requesterActorRef: 'actor.public.trigger',
  requiredCapabilityRefs: ['capability.pylon.local_claude_agent'],
  state: 'open',
  title: 'Debt receipt template',
  topicId: 'topic-1',
  updatedAt: '2026-06-18T15:23:43.476Z',
  verificationCommandRef: 'command.public.debt_receipt.regenerate_and_diff',
  workRequestId: 'wr-1',
}

describe('Forum work request route contract', () => {
  test('decodes public ref-only create bodies', () => {
    expect(
      decodeCreateForumWorkRequestBody({
        budgetSats: 100_000,
        deadlineRef: 'deadline.public.debt_receipt.20260625',
        objectiveRef: 'receipt.public.debt.5334.template',
        repositoryRefs: ['repo.public.openagents'],
        requestedSlug: 'debt-receipt-template',
        requiredCapabilityRefs: ['capability.pylon.local_claude_agent'],
        title: 'Debt receipt template',
        verificationCommandRef:
          'command.public.debt_receipt.regenerate_and_diff',
      }),
    ).toMatchObject({
      budgetSats: 100_000,
      objectiveRef: 'receipt.public.debt.5334.template',
      requestedSlug: 'debt-receipt-template',
    })
  })

  test('rejects raw prompt material before schema decoding', () => {
    expect(() =>
      decodeCreateForumWorkRequestBody({
        budgetSats: 100_000,
        deadlineRef: 'deadline.public.debt_receipt.20260625',
        objectiveRef: 'receipt.public.debt.5334.template',
        rawPrompt: 'clone the private repo and fix it',
        title: 'Debt receipt template',
        verificationCommandRef:
          'command.public.debt_receipt.regenerate_and_diff',
      }),
    ).toThrow(ForumWorkRequestBodyValidationError)
  })

  test('matches normalized input to an existing work request without route dependencies', () => {
    const input = normalizeForumWorkRequestInput({
      budgetSats: 100_000,
      deadlineRef: workRequestRecord.deadlineRef,
      objectiveRef: workRequestRecord.objectiveRef,
      repositoryRefs: workRequestRecord.repositoryRefs,
      requiredCapabilityRefs: workRequestRecord.requiredCapabilityRefs,
      title: workRequestRecord.title,
      verificationCommandRef: workRequestRecord.verificationCommandRef,
    })

    expect(workRequestMatchesInput(workRequestRecord, input)).toBe(true)
    expect(
      workRequestMatchesInput(workRequestRecord, {
        ...input,
        requiredCapabilityRefs: ['capability.public.different'],
      }),
    ).toBe(false)
  })
})
