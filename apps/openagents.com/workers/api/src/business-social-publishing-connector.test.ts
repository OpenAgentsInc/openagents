import { describe, expect, test } from 'vitest'

import {
  BusinessSocialPublishingConnectorInvariantError,
  buildBusinessSocialPublishingConnectorReceipt,
  publicBusinessSocialPublishingConnectorProjection,
  publishApprovedBusinessSocialPost,
  type BusinessSocialPlatformPublisher,
} from './business-social-publishing-connector'
import {
  OmniWorkroomApprovalGatePolicy,
  OmniWorkroomOutboundDeliverableReviewInput,
  decideOmniWorkroomOutboundDeliverableReview,
} from './omni-workroom-approval-gates'

const approvalPolicy = (
  approvalLevel: 'draft' | 'suggest' | 'execute_with_approval' | 'trusted' =
    'execute_with_approval',
) =>
  new OmniWorkroomApprovalGatePolicy({
    approvalLevel,
    professionalReviewRequired: false,
    professionalReviewerRole: null,
    sourceRefs: ['policy.business_fulfillment.approval_ladder.v1'],
    workspaceRef: 'workspace.business.fulfillment.001',
  })

const approvalDecision = (
  overrides: Partial<OmniWorkroomOutboundDeliverableReviewInput> = {},
) =>
  decideOmniWorkroomOutboundDeliverableReview(
    new OmniWorkroomOutboundDeliverableReviewInput({
      approvalDecisionReceiptRefs: ['receipt.review.social_post_approved.001'],
      deliverableRef: 'deliverable.social_post.campaign_001',
      evidenceRefs: ['evidence.social_post.preview.001'],
      outboundActionKind: 'publish',
      policy: approvalPolicy(),
      professionalReviewReceiptRefs: [],
      reviewerRoleRefs: [],
      sourceRefs: ['github.public.issue.8102'],
      workKind: 'business',
      workroomRef: 'workroom.business.fulfillment.001',
      ...overrides,
    }),
  )

const validInput = {
  approvalDecision: approvalDecision(),
  approvedPostRef: 'artifact.social_post.approved.001',
  campaignRef: 'campaign.business.social.001',
  connectorRef: 'connector.social.x.workspace_001',
  createdAt: '2026-07-03T12:00:00.000Z',
  engagementRef: 'engagement.business.opaque.001',
  platform: 'x' as const,
  publishRequestRef: 'request.social_publish.x.001',
  sourceRefs: [
    'github.public.issue.8102',
    'docs/fable/2026-07-02-business-fulfillment-engine-meditations.md#connector-lane',
  ],
}

describe('business social publishing connector', () => {
  test('publishes an approved X post through an injected connector and returns a per-post receipt', async () => {
    const calls: Array<unknown> = []
    const publisher: BusinessSocialPlatformPublisher = async request => {
      calls.push(request)
      return {
        platformPostRef: 'x.post.public.opaque_001',
        platformReceiptRef: 'receipt.x.publish.opaque_001',
      }
    }

    const receipt = await publishApprovedBusinessSocialPost(validInput, publisher)

    expect(calls).toEqual([
      {
        approvedPostRef: 'artifact.social_post.approved.001',
        campaignRef: 'campaign.business.social.001',
        connectorRef: 'connector.social.x.workspace_001',
        engagementRef: 'engagement.business.opaque.001',
        platform: 'x',
        publishRequestRef: 'request.social_publish.x.001',
        toolRef: 'tool.business.social_publish.x.v1',
        workspaceRef: 'workspace.business.fulfillment.001',
      },
    ])
    expect(receipt).toMatchObject({
      approvalDecisionReceiptRef: 'receipt.review.social_post_approved.001',
      credentialMaterialInModelContext: false,
      externalActionKind: 'publish',
      platform: 'x',
      publishAllowed: true,
      rawProviderPayloadInModelContext: false,
      receiptKind: 'business.social_publishing_connector.post_published',
      receiptRef:
        'receipt.business.social_publish.campaign_business_social_001.x_post_public_opaque_001',
      schema: 'openagents.business.social_publishing_connector.receipt.v1',
      toolRef: 'tool.business.social_publish.x.v1',
    })
    expect(receipt.sourceRefs).toContain('docs/fable/ROADMAP_BIZ.md#BF-6.3')
    expect(receipt.caveatRefs).toContain(
      'caveat.business.social_publishing.approval_gate_required',
    )
    expect(JSON.stringify(receipt)).not.toMatch(
      /@|access_token|raw_post|provider_payload|customer_email|x\.com\//i,
    )
  })

  test('blocks posting when the BF-4.3 approval gate has not authorized publish', async () => {
    const blockedDecision = approvalDecision({
      approvalDecisionReceiptRefs: [],
    })
    let called = false

    await expect(
      publishApprovedBusinessSocialPost(
        { ...validInput, approvalDecision: blockedDecision },
        async () => {
          called = true
          return {
            platformPostRef: 'x.post.public.opaque_001',
            platformReceiptRef: 'receipt.x.publish.opaque_001',
          }
        },
      ),
    ).rejects.toThrow(BusinessSocialPublishingConnectorInvariantError)
    expect(called).toBe(false)
  })

  test('blocks non-publish approval decisions and draft/suggest ladder levels', () => {
    expect(() =>
      buildBusinessSocialPublishingConnectorReceipt({
        ...validInput,
        approvalDecision: approvalDecision({ outboundActionKind: 'send' }),
        platformPostRef: 'x.post.public.opaque_001',
        platformReceiptRef: 'receipt.x.publish.opaque_001',
      }),
    ).toThrow(/publish decisions/)

    for (const approvalLevel of ['draft', 'suggest'] as const) {
      expect(() =>
        buildBusinessSocialPublishingConnectorReceipt({
          ...validInput,
          approvalDecision: approvalDecision({
            policy: approvalPolicy(approvalLevel),
          }),
          platformPostRef: 'x.post.public.opaque_001',
          platformReceiptRef: 'receipt.x.publish.opaque_001',
        }),
      ).toThrow(/approval gate/)
    }
  })

  test('rejects unsafe refs so raw posts, provider payloads, credentials, and client identifiers stay out of receipts', () => {
    expect(() =>
      buildBusinessSocialPublishingConnectorReceipt({
        ...validInput,
        approvedPostRef: 'raw_post.hello_world',
        platformPostRef: 'x.post.public.opaque_001',
        platformReceiptRef: 'receipt.x.publish.opaque_001',
      }),
    ).toThrow(/public-safe ref/)

    expect(() =>
      buildBusinessSocialPublishingConnectorReceipt({
        ...validInput,
        connectorRef: 'connector.social.x.access_token.sk_test',
        platformPostRef: 'x.post.public.opaque_001',
        platformReceiptRef: 'receipt.x.publish.opaque_001',
      }),
    ).toThrow(/public-safe ref/)

    expect(() =>
      buildBusinessSocialPublishingConnectorReceipt({
        ...validInput,
        engagementRef: 'engagement.customer_email@example.com',
        platformPostRef: 'x.post.public.opaque_001',
        platformReceiptRef: 'receipt.x.publish.opaque_001',
      }),
    ).toThrow(/public-safe ref/)
  })

  test('public projection keeps receipt and approval evidence but omits connector internals', () => {
    const receipt = buildBusinessSocialPublishingConnectorReceipt({
      ...validInput,
      platformPostRef: 'x.post.public.opaque_001',
      platformReceiptRef: 'receipt.x.publish.opaque_001',
    })

    const projection = publicBusinessSocialPublishingConnectorProjection(receipt)

    expect(projection).toEqual({
      approvalDecisionReceiptRef: 'receipt.review.social_post_approved.001',
      approvedPostRef: 'artifact.social_post.approved.001',
      campaignRef: 'campaign.business.social.001',
      caveatRefs: receipt.caveatRefs,
      createdAt: '2026-07-03T12:00:00.000Z',
      credentialMaterialInModelContext: false,
      deliverableRef: 'deliverable.social_post.campaign_001',
      engagementRef: 'engagement.business.opaque.001',
      externalActionKind: 'publish',
      platform: 'x',
      platformPostRef: 'x.post.public.opaque_001',
      platformReceiptRef: 'receipt.x.publish.opaque_001',
      publishAllowed: true,
      rawProviderPayloadInModelContext: false,
      receiptKind: 'business.social_publishing_connector.post_published',
      receiptRef: receipt.receiptRef,
      schema: 'openagents.business.social_publishing_connector.receipt.v1',
      sourceRefs: receipt.sourceRefs,
      toolRef: 'tool.business.social_publish.x.v1',
      workspaceRef: 'workspace.business.fulfillment.001',
    })
    expect(projection).not.toHaveProperty('connectorRef')
    expect(projection).not.toHaveProperty('publishRequestRef')
  })
})
