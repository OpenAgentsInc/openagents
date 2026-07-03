import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OmniWorkroomApprovalGatePolicy,
  OmniWorkroomApprovalGateUnsafe,
  OmniWorkroomOutboundDeliverableReviewDecision,
  OmniWorkroomOutboundDeliverableReviewInput,
  decideOmniWorkroomOutboundDeliverableReview,
} from './omni-workroom-approval-gates'

const policy = (
  overrides: Partial<OmniWorkroomApprovalGatePolicy> = {},
): OmniWorkroomApprovalGatePolicy =>
  new OmniWorkroomApprovalGatePolicy({
    approvalLevel: 'execute_with_approval',
    professionalReviewRequired: false,
    professionalReviewerRole: null,
    sourceRefs: ['policy.business_fulfillment.approval_ladder.v1'],
    workspaceRef: 'workspace.business.fulfillment.demo',
    ...overrides,
  })

const input = (
  overrides: Partial<OmniWorkroomOutboundDeliverableReviewInput> = {},
): OmniWorkroomOutboundDeliverableReviewInput =>
  new OmniWorkroomOutboundDeliverableReviewInput({
    approvalDecisionReceiptRefs: ['receipt.review.operator_approved.001'],
    deliverableRef: 'deliverable.business_site_revision.001',
    evidenceRefs: ['evidence.workroom.preview.001'],
    outboundActionKind: 'publish',
    policy: policy(),
    professionalReviewReceiptRefs: [],
    reviewerRoleRefs: [],
    sourceRefs: ['issue.8090'],
    workKind: 'site',
    workroomRef: 'workroom.business.fulfillment.001',
    ...overrides,
  })

describe('Omni workroom approval gates', () => {
  test('allows an execute-with-approval outbound deliverable only when a decision receipt is recorded', () => {
    const decision = decideOmniWorkroomOutboundDeliverableReview(input())

    expect(
      S.decodeUnknownSync(OmniWorkroomOutboundDeliverableReviewDecision)(
        decision,
      ),
    ).toEqual(decision)
    expect(decision).toMatchObject({
      approvalDecisionRecorded: true,
      approvalLevel: 'execute_with_approval',
      blockedExternalAction: false,
      blockerRefs: [],
      externalActionAllowed: true,
      outboundActionKind: 'publish',
      professionalReviewRecorded: true,
      professionalReviewRequired: false,
      receiptRefs: ['receipt.review.operator_approved.001'],
    })
    expect(decision.sourceRefs).toEqual([
      'issue.8090',
      'policy.business_fulfillment.approval_ladder.v1',
    ])
  })

  test('blocks draft and suggest ladder levels from external send, publish, file, or spend actions', () => {
    for (const approvalLevel of ['draft', 'suggest'] as const) {
      const decision = decideOmniWorkroomOutboundDeliverableReview(
        input({
          outboundActionKind: 'send',
          policy: policy({ approvalLevel }),
        }),
      )

      expect(decision.externalActionAllowed).toBe(false)
      expect(decision.blockerRefs).toContain(
        `blocker.workroom_approval_ladder.${approvalLevel}.external_action_not_allowed`,
      )
    }
  })

  test('blocks outbound deliverables at executable levels without a per-deliverable review decision receipt', () => {
    for (const approvalLevel of ['execute_with_approval', 'trusted'] as const) {
      const decision = decideOmniWorkroomOutboundDeliverableReview(
        input({
          approvalDecisionReceiptRefs: [],
          outboundActionKind: 'spend',
          policy: policy({ approvalLevel }),
        }),
      )

      expect(decision.externalActionAllowed).toBe(false)
      expect(decision.approvalDecisionRecorded).toBe(false)
      expect(decision.blockerRefs).toContain(
        'blocker.workroom_approval_ladder.decision_receipt_missing',
      )
    }
  })

  test('requires licensed-practitioner professional review receipts for legal-sensitive deliverables', () => {
    const blocked = decideOmniWorkroomOutboundDeliverableReview(
      input({
        outboundActionKind: 'file',
        policy: policy({
          professionalReviewRequired: true,
          professionalReviewerRole: 'licensed_practitioner',
        }),
        workKind: 'legal_sensitive',
      }),
    )

    expect(blocked.externalActionAllowed).toBe(false)
    expect(blocked.professionalReviewRequired).toBe(true)
    expect(blocked.professionalReviewerRole).toBe('licensed_practitioner')
    expect(blocked.blockerRefs).toEqual([
      'blocker.workroom_approval_ladder.professional_review_receipt_missing',
      'blocker.workroom_approval_ladder.professional_reviewer_role_missing',
    ])

    const allowed = decideOmniWorkroomOutboundDeliverableReview(
      input({
        outboundActionKind: 'file',
        policy: policy({
          professionalReviewRequired: true,
          professionalReviewerRole: 'licensed_practitioner',
        }),
        professionalReviewReceiptRefs: [
          'receipt.professional_review.licensed_practitioner.001',
        ],
        reviewerRoleRefs: ['role.licensed_practitioner.verified.001'],
        workKind: 'legal_sensitive',
      }),
    )

    expect(allowed.externalActionAllowed).toBe(true)
    expect(allowed.professionalReviewRecorded).toBe(true)
    expect(allowed.receiptRefs).toEqual([
      'receipt.professional_review.licensed_practitioner.001',
      'receipt.review.operator_approved.001',
    ])
  })

  test('defaults legal-sensitive work to licensed-practitioner review even when policy omits the role', () => {
    const decision = decideOmniWorkroomOutboundDeliverableReview(
      input({
        policy: policy({
          professionalReviewRequired: false,
          professionalReviewerRole: null,
        }),
        professionalReviewReceiptRefs: [
          'receipt.professional_review.licensed_practitioner.002',
        ],
        reviewerRoleRefs: ['role.licensed_practitioner.verified.002'],
        workKind: 'legal_sensitive',
      }),
    )

    expect(decision.professionalReviewRequired).toBe(true)
    expect(decision.professionalReviewerRole).toBe('licensed_practitioner')
    expect(decision.externalActionAllowed).toBe(true)
  })

  test('rejects private refs, raw email, provider, payment, settlement, and wallet material', () => {
    for (const badInput of [
      () =>
        decideOmniWorkroomOutboundDeliverableReview(
          input({ deliverableRef: 'deliverable.customer_email.raw' }),
        ),
      () =>
        decideOmniWorkroomOutboundDeliverableReview(
          input({ evidenceRefs: ['evidence.provider_payload.raw'] }),
        ),
      () =>
        decideOmniWorkroomOutboundDeliverableReview(
          input({ approvalDecisionReceiptRefs: ['receipt.payment_settled.001'] }),
        ),
      () =>
        decideOmniWorkroomOutboundDeliverableReview(
          input({
            policy: policy({
              sourceRefs: ['policy.wallet_secret.operator_notes'],
            }),
          }),
        ),
    ]) {
      expect(badInput).toThrow(OmniWorkroomApprovalGateUnsafe)
    }
  })
})
