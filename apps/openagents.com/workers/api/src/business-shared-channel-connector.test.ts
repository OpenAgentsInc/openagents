import { describe, expect, test } from 'vitest'

import {
  BusinessSharedChannelConnectorInvariantError,
  assertBusinessSharedChannelConnectorReceipt,
  buildBusinessSharedChannelConnectorReceipt,
  publicBusinessSharedChannelConnectorProjection,
} from './business-shared-channel-connector'

const validInput = {
  channelRef: 'channel.business.engagement_001.shared',
  commandRef: 'command.shared_channel.mention_001.reply_draft',
  commandScope: 'engagement_reply_draft' as const,
  commandVerified: true,
  connectorRef: 'connector.shared_channel.slack.workspace_001',
  createdAt: '2026-07-03T12:00:00.000Z',
  engagementRef: 'engagement.business.opaque.001',
  inviteCreatedRef: 'invite.shared_channel.created.001',
  inviteRequestRef: 'request.shared_channel.invite.001',
  mentionRef: 'mention.shared_channel.verified.001',
  platform: 'slack' as const,
  replyDraftRef: 'draft.shared_channel.reply.001',
  requestedByActorRef: 'actor.business.operator.001',
  sourceRefs: [
    'github.public.issue.8101',
    'docs/fable/2026-07-02-business-fulfillment-engine-meditations.md#connector-lane',
  ],
  workspaceRef: 'workspace.business.fulfillment.001',
}

describe('business shared-channel connector', () => {
  test('records an opt-in connected channel run with draft-only reply authority', () => {
    const receipt = buildBusinessSharedChannelConnectorReceipt(validInput)

    expect(receipt).toMatchObject({
      autoInviteAllowed: false,
      commandScope: 'engagement_reply_draft',
      commandVerified: true,
      externalSendAuthorized: false,
      outboundAllowed: false,
      receiptKind: 'business.shared_channel_connector.connected_run',
      receiptRef:
        'receipt.business.shared_channel.engagement_business_opaque_001.command_shared_channel_mention_001_reply_draft',
      schema: 'openagents.business.shared_channel_connector.receipt.v1',
    })
    expect(receipt.sourceRefs).toContain('docs/fable/ROADMAP_BIZ.md#BF-6.2')
    expect(receipt.caveatRefs).toContain(
      'caveat.business.shared_channel.invite_on_request_never_auto',
    )
    expect(JSON.stringify(receipt)).not.toMatch(
      /@|raw_message|slack\.com\/archives|invite_url|access_token/i,
    )
    expect(() =>
      assertBusinessSharedChannelConnectorReceipt(receipt),
    ).not.toThrow()
  })

  test('blocks auto-invite, unverified command, and external-send widening', () => {
    expect(() =>
      buildBusinessSharedChannelConnectorReceipt({
        ...validInput,
        autoInviteAllowed: true,
      }),
    ).toThrow(BusinessSharedChannelConnectorInvariantError)

    expect(() =>
      buildBusinessSharedChannelConnectorReceipt({
        ...validInput,
        commandVerified: false,
      }),
    ).toThrow(/verified/)

    expect(() =>
      buildBusinessSharedChannelConnectorReceipt({
        ...validInput,
        externalSendAuthorized: true,
      }),
    ).toThrow(/external sends/)
  })

  test('rejects raw private channel or client-identifying refs', () => {
    expect(() =>
      buildBusinessSharedChannelConnectorReceipt({
        ...validInput,
        channelRef: 'https://slack.com/archives/C123/p456',
      }),
    ).toThrow(/public-safe ref/)

    expect(() =>
      buildBusinessSharedChannelConnectorReceipt({
        ...validInput,
        mentionRef: 'raw_message.customer_email@example.com',
      }),
    ).toThrow(/public-safe ref/)
  })

  test('public projection redacts connector internals while preserving gate evidence', () => {
    const receipt = buildBusinessSharedChannelConnectorReceipt(validInput)
    const projection = publicBusinessSharedChannelConnectorProjection(receipt)

    expect(projection).toEqual({
      autoInviteAllowed: false,
      caveatRefs: receipt.caveatRefs,
      commandScope: 'engagement_reply_draft',
      commandVerified: true,
      createdAt: '2026-07-03T12:00:00.000Z',
      engagementRef: 'engagement.business.opaque.001',
      externalSendAuthorized: false,
      outboundAllowed: false,
      platform: 'slack',
      receiptKind: 'business.shared_channel_connector.connected_run',
      receiptRef: receipt.receiptRef,
      schema: 'openagents.business.shared_channel_connector.receipt.v1',
      sourceRefs: receipt.sourceRefs,
      workspaceRef: 'workspace.business.fulfillment.001',
    })
    expect(projection).not.toHaveProperty('connectorRef')
    expect(projection).not.toHaveProperty('channelRef')
    expect(projection).not.toHaveProperty('mentionRef')
    expect(projection).not.toHaveProperty('replyDraftRef')
  })
})
