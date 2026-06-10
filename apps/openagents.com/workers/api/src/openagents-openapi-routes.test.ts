import { containsProviderSecretMaterial } from '@openagents/provider-account-schema'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { OpenAgentsOpenApiEndpoint } from './openagents-openapi'
import { handleOpenAgentsOpenApi } from './openagents-openapi-routes'

type OpenApiOperation = Readonly<{
  description?: string
  operationId: string
  parameters?: ReadonlyArray<Record<string, unknown>>
  responses?: Record<string, unknown>
  security: ReadonlyArray<Record<string, ReadonlyArray<string>>>
  tags: ReadonlyArray<string>
}>

type OpenApiDocument = Readonly<{
  components: Readonly<{
    schemas: Record<string, unknown>
    securitySchemes: Record<string, unknown>
  }>
  info: Readonly<{ title: string }>
  openapi: string
  paths: Record<string, Record<string, OpenApiOperation>>
}>

const operationAt = (
  document: OpenApiDocument,
  path: string,
  method: string,
): OpenApiOperation => {
  const pathItem = document.paths[path]
  expect(pathItem).toBeDefined()

  const apiOperation = pathItem?.[method]
  expect(apiOperation).toBeDefined()

  if (apiOperation === undefined) {
    throw new Error(`Missing OpenAPI operation ${method.toUpperCase()} ${path}`)
  }

  return apiOperation
}

const schemaProperties = (
  document: OpenApiDocument,
  name: string,
): Record<string, Record<string, unknown>> => {
  const schema = document.components.schemas[name]

  expect(schema).toEqual(
    expect.objectContaining({
      properties: expect.any(Object),
    }),
  )

  return (
    schema as Readonly<{
      properties: Record<string, Record<string, unknown>>
    }>
  ).properties
}

const runRoute = (method = 'GET'): Promise<Response> =>
  Effect.runPromise(
    handleOpenAgentsOpenApi(
      new Request(`https://openagents.com${OpenAgentsOpenApiEndpoint}`, {
        method,
      }),
    ),
  )

describe('OpenAgents OpenAPI route', () => {
  test('serves stable public API docs with auth metadata and no-store headers', async () => {
    const response = await runRoute()
    const body = (await response.json()) as OpenApiDocument
    const serialized = JSON.stringify(body)

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.openapi).toBe('3.1.0')
    expect(body.info.title).toBe('OpenAgents Autopilot API')
    expect(
      operationAt(body, '/.well-known/openagents.json', 'get').operationId,
    ).toBe('getOpenAgentsCapabilityManifest')
    expect(operationAt(body, '/AGENTS.md', 'get').operationId).toBe(
      'getOpenAgentsAgentInstructions',
    )
    expect(operationAt(body, '/HEARTBEAT.md', 'get').operationId).toBe(
      'getOpenAgentsAgentHeartbeat',
    )
    expect(operationAt(body, '/RULES.md', 'get').operationId).toBe(
      'getOpenAgentsAgentRules',
    )
    expect(operationAt(body, '/skill.json', 'get').operationId).toBe(
      'getOpenAgentsCompanionMetadata',
    )
    expect(operationAt(body, '/api/public/proof/otec', 'get').operationId).toBe(
      'getPublicOtecProof',
    )
    expect(
      operationAt(body, '/api/training/device-capabilities/a2', 'get')
        .operationId,
    ).toBe('readTrainingA2DeviceCapabilityDashboard')
    expect(operationAt(body, '/api/agents/register', 'post').operationId).toBe(
      'registerProgrammaticAgent',
    )
    expect(operationAt(body, '/api/agents/claims', 'post').operationId).toBe(
      'requestAgentOwnerClaim',
    )
    expect(
      operationAt(body, '/api/agents/claims/{claimId}', 'get').operationId,
    ).toBe('getAgentOwnerClaimStatus')
    expect(
      operationAt(body, '/api/agents/claims/{claimId}/approve', 'post')
        .operationId,
    ).toBe('approveAgentOwnerClaim')
    expect(
      operationAt(body, '/api/agents/claims/{claimId}/x/challenge', 'post')
        .operationId,
    ).toBe('startAgentOwnerXClaimChallenge')
    expect(
      operationAt(body, '/api/agents/claims/{claimId}/x/verify', 'post')
        .operationId,
    ).toBe('verifyAgentOwnerXClaimTweet')
    expect(
      operationAt(body, '/api/agents/claims/{claimId}/reject', 'post')
        .operationId,
    ).toBe('rejectAgentOwnerClaim')
    expect(operationAt(body, '/api/agents/proposals', 'post').operationId).toBe(
      'submitPublicAgentProposal',
    )
    expect(
      operationAt(body, '/api/agents/proposals/rate-limit/preview', 'post')
        .operationId,
    ).toBe('previewPublicAgentProposalRateLimitRecovery')
    expect(
      operationAt(body, '/api/agents/proposals/rate-limit/redeem', 'post')
        .operationId,
    ).toBe('redeemPublicAgentProposalRateLimitRecovery')
    expect(
      operationAt(body, '/api/agents/proposals/{proposalId}', 'get')
        .operationId,
    ).toBe('getPublicAgentProposal')
    expect(
      operationAt(body, '/api/operator/agent-proposals', 'get').operationId,
    ).toBe('listOperatorAgentProposals')
    expect(
      operationAt(body, '/api/operator/agent-proposals/{proposalId}', 'get')
        .operationId,
    ).toBe('getOperatorAgentProposal')
    expect(
      operationAt(
        body,
        '/api/operator/agent-proposals/{proposalId}/promote',
        'post',
      ).operationId,
    ).toBe('promoteOperatorAgentProposal')
    expect(
      operationAt(
        body,
        '/api/operator/agent-proposals/{proposalId}/reject',
        'post',
      ).operationId,
    ).toBe('rejectOperatorAgentProposal')
    expect(
      operationAt(body, '/api/operator/artanis/pylon-marketplace/jobs', 'post')
        .operationId,
    ).toBe('createOperatorPylonMarketplaceJobIntake')
    expect(
      operationAt(
        body,
        '/api/operator/artanis/pylon-marketplace/jobs/{intakeRef}/triage',
        'post',
      ).operationId,
    ).toBe('triageOperatorPylonMarketplaceJobIntake')
    expect(
      operationAt(body, '/api/developer/signature-packages/validate', 'post')
        .operationId,
    ).toBe('validateSignaturePackage')
    expect(operationAt(body, '/api/omni/sdk-seed', 'get').operationId).toBe(
      'getOmniApiSdkSeed',
    )
    expect(operationAt(body, '/api/agents/me', 'get').operationId).toBe(
      'getProgrammaticAgentMe',
    )
    expect(operationAt(body, '/api/agents/home', 'get').operationId).toBe(
      'getProgrammaticAgentHome',
    )
    expect(operationAt(body, '/api/agents/search', 'post').operationId).toBe(
      'runAgentHostedSearch',
    )
    expect(
      operationAt(body, '/api/agents/search/payments/preview', 'post')
        .operationId,
    ).toBe('previewAgentHostedSearchPayment')
    expect(
      operationAt(body, '/api/agents/search/payments/redeem', 'post')
        .operationId,
    ).toBe('redeemAgentHostedSearchPayment')
    expect(
      operationAt(body, '/api/agents/scoped-grants', 'get').operationId,
    ).toBe('listOwnerAgentScopedGrants')
    expect(
      operationAt(body, '/api/agents/scoped-grants', 'post').operationId,
    ).toBe('createOwnerAgentScopedGrant')
    expect(
      operationAt(body, '/api/agents/scoped-grants/{grantId}/revoke', 'post')
        .operationId,
    ).toBe('revokeOwnerAgentScopedGrant')
    expect(
      operationAt(body, '/api/agents/profiles/{agentRef}', 'get').operationId,
    ).toBe('getPublicAgentProfile')
    expect(
      operationAt(body, '/api/agents/notifications', 'get').operationId,
    ).toBe('listAgentNotifications')
    expect(operationAt(body, '/api/autopilot/work', 'post').operationId).toBe(
      'createAutopilotWork',
    )
    expect(
      operationAt(body, '/api/autopilot/work/{workOrderRef}', 'get')
        .operationId,
    ).toBe('getAutopilotWork')
    expect(
      operationAt(body, '/api/autopilot/work/{workOrderRef}/events', 'get')
        .operationId,
    ).toBe('listAutopilotWorkEvents')
    expect(operationAt(body, '/api/auth/session', 'get').operationId).toBe(
      'getAuthSession',
    )
    expect(operationAt(body, '/api/onboarding', 'get').operationId).toBe(
      'getOnboardingStatus',
    )
    expect(
      operationAt(body, '/api/onboarding/repository/select', 'post')
        .operationId,
    ).toBe('selectOnboardingRepository')
    expect(operationAt(body, '/api/forum', 'get').operationId).toBe(
      'getForumBoardIndex',
    )
    expect(operationAt(body, '/api/forum/search', 'get').operationId).toBe(
      'searchForum',
    )
    expect(
      operationAt(body, '/api/forum/forums/{forumId}/topics', 'post')
        .operationId,
    ).toBe('createForumTopic')
    expect(
      operationAt(body, '/api/forum/forums/{forumId}/watches', 'post')
        .operationId,
    ).toBe('watchForum')
    expect(
      operationAt(body, '/api/forum/actors/{actorRef}/profile', 'get')
        .operationId,
    ).toBe('getForumActorProfile')
    expect(
      operationAt(body, '/api/forum/actors/{actorRef}/follows', 'post')
        .operationId,
    ).toBe('followForumActor')
    expect(
      operationAt(body, '/api/forum/topics/{topicId}/watches', 'post')
        .operationId,
    ).toBe('watchForumTopic')
    expect(
      operationAt(body, '/api/forum/topics/{topicId}/bookmarks', 'post')
        .operationId,
    ).toBe('bookmarkForumTopic')
    expect(
      operationAt(body, '/api/forum/topics/{topicId}/posts', 'post')
        .operationId,
    ).toBe('createForumReplyPost')
    expect(
      operationAt(body, '/api/forum/posts/{postId}/bookmarks', 'post')
        .operationId,
    ).toBe('bookmarkForumPost')
    expect(
      operationAt(body, '/api/forum/posts/{postId}', 'get').operationId,
    ).toBe('getForumPost')
    expect(
      operationAt(body, '/api/forum/posts/{postId}/rewards', 'post')
        .operationId,
    ).toBe('previewForumPostReward')
    expect(
      operationAt(body, '/api/forum/posts/{postId}/direct-tips', 'post')
        .operationId,
    ).toBe('submitForumPostDirectTip')
    expect(
      operationAt(body, '/api/forum/direct-tips/{attemptId}', 'get')
        .operationId,
    ).toBe('getForumDirectTip')
    expect(
      operationAt(body, '/api/forum/paid-actions/mdk/webhooks', 'post')
        .operationId,
    ).toBe('reconcileForumDirectTipMdkWebhook')
    expect(
      operationAt(body, '/api/forum/posts/{postId}/down-signals', 'post')
        .operationId,
    ).toBe('previewForumPostDownSignal')
    expect(
      operationAt(body, '/api/forum/paid-actions/preview', 'post').operationId,
    ).toBe('previewForumPaidAction')
    expect(
      operationAt(body, '/api/forum/paid-actions/redeem', 'post').operationId,
    ).toBe('redeemForumPaidAction')
    expect(
      operationAt(body, '/api/forum/receipts/{receiptId}', 'get').operationId,
    ).toBe('getForumReceipt')
    expect(
      operationAt(body, '/api/customer-orders/active', 'get').operationId,
    ).toBe('getActiveCustomerOrder')
    expect(
      operationAt(
        body,
        '/api/customer-orders/{orderId}/fulfillment-artifacts',
        'get',
      ).operationId,
    ).toBe('listCustomerOrderFulfillmentArtifacts')
    expect(operationAt(body, '/api/sites', 'get').operationId).toBe(
      'listSiteLibrary',
    )
    expect(
      operationAt(body, '/api/sites/builder-sessions', 'post').operationId,
    ).toBe('createSiteBuilderSession')
    expect(
      operationAt(
        body,
        '/api/sites/builder-sessions/{sessionId}/messages',
        'post',
      ).operationId,
    ).toBe('appendSiteBuilderMessage')
    expect(
      operationAt(
        body,
        '/api/sites/builder-sessions/{sessionId}/files/read',
        'get',
      ).operationId,
    ).toBe('readSiteBuilderFile')
    expect(
      operationAt(body, '/api/sites/{siteId}/commerce/discovery', 'get')
        .operationId,
    ).toBe('getSitePaymentDiscovery')
    expect(
      operationAt(body, '/api/sites/{siteId}/commerce/checkout-intents', 'post')
        .operationId,
    ).toBe('createSiteCommerceCheckoutIntent')
    expect(
      operationAt(
        body,
        '/api/sites/{siteId}/commerce/payment-proofs/{checkoutIntentRef}',
        'get',
      ).operationId,
    ).toBe('readSiteCommercePaymentProof')
    expect(
      operationAt(body, '/api/sites/{siteId}/commerce/l402/challenges', 'post')
        .operationId,
    ).toBe('createSiteCommerceL402Challenge')
    expect(
      operationAt(body, '/api/sites/{siteId}/commerce/l402/redemptions', 'post')
        .operationId,
    ).toBe('redeemSiteCommerceL402Challenge')
    expect(
      operationAt(body, '/api/sites/{siteId}/commerce/payout-bridges', 'post')
        .operationId,
    ).toBe('createSiteCommercePayoutBridge')
    expect(
      operationAt(body, '/r/site/{publicSourceRef}', 'get').operationId,
    ).toBe('captureSiteReferral')
    expect(operationAt(body, '/api/agent/sites', 'post').operationId).toBe(
      'createAgentSiteProjectContract',
    )
    expect(
      operationAt(body, '/api/agent/sites/{siteId}/previews', 'post')
        .operationId,
    ).toBe('createAgentSitePreviewContract')
    expect(
      operationAt(body, '/api/agent/sites/{siteId}/versions', 'post')
        .operationId,
    ).toBe('createAgentSiteVersionContract')
    expect(
      operationAt(body, '/api/agent/sites/{siteId}/deploy-requests', 'post')
        .operationId,
    ).toBe('createAgentSiteDeployRequestContract')
    expect(operationAt(body, '/api/operator/sites', 'get').operationId).toBe(
      'listOperatorSites',
    )
    expect(
      operationAt(body, '/api/operator/sites/{siteId}/versions', 'post')
        .operationId,
    ).toBe('saveOperatorSiteVersion')
    expect(
      operationAt(
        body,
        '/api/operator/adjutant/orders/{orderId}/assign',
        'post',
      ).operationId,
    ).toBe('createOrderAdjutantAssignment')
    expect(
      operationAt(body, '/api/operator/email-deliveries', 'get').operationId,
    ).toBe('listOperatorEmailDeliveries')
    expect(operationAt(body, '/api/public/proof/otec', 'get').security).toEqual(
      [],
    )
    expect(
      operationAt(body, '/api/customer-orders/active', 'get').security,
    ).toEqual([{ browserSession: [] }, { agentBearer: [] }])
    expect(operationAt(body, '/api/customer-orders', 'post').security).toEqual([
      { browserSession: [] },
      { agentBearer: [] },
    ])
    expect(
      operationAt(body, '/api/customer-orders', 'post').parameters,
    ).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          in: 'header',
          name: 'Idempotency-Key',
          required: false,
        }),
      ]),
    )
    expect(operationAt(body, '/api/agent/sites', 'post').security).toEqual([
      { agentBearer: [] },
    ])
    expect(operationAt(body, '/api/sites', 'get').security).toEqual([
      { browserSession: [] },
    ])
    expect(
      operationAt(body, '/api/sites/{siteId}/commerce/checkout-intents', 'post')
        .parameters,
    ).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          in: 'header',
          name: 'Idempotency-Key',
          required: true,
        }),
      ]),
    )
    expect(
      operationAt(body, '/api/sites/{siteId}/commerce/payout-bridges', 'post')
        .security,
    ).toEqual([{ adminBearer: [] }])
    expect(
      operationAt(
        body,
        '/api/sites/{siteId}/commerce/payment-proofs/{checkoutIntentRef}',
        'get',
      ).security,
    ).toEqual([])
    expect(
      operationAt(body, '/api/sites/{siteId}/commerce/payout-bridges', 'post')
        .parameters,
    ).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          in: 'header',
          name: 'Idempotency-Key',
          required: true,
        }),
      ]),
    )
    expect(
      operationAt(body, '/api/agent/sites/{siteId}/deploy-requests', 'post')
        .parameters,
    ).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          in: 'header',
          name: 'Idempotency-Key',
          required: true,
        }),
      ]),
    )
    expect(operationAt(body, '/api/operator/sites', 'get').security).toEqual([
      { adminSession: [] },
    ])
    expect(operationAt(body, '/api/agents/register', 'post').security).toEqual(
      [],
    )
    expect(operationAt(body, '/api/agents/claims', 'post').security).toEqual([])
    expect(
      operationAt(body, '/api/agents/claims/{claimId}', 'get').security,
    ).toEqual([{ agentClaimToken: [] }, { agentBearer: [] }])
    expect(
      operationAt(body, '/api/agents/claims/{claimId}/approve', 'post')
        .security,
    ).toEqual([{ browserSession: [] }])
    expect(
      operationAt(body, '/api/agents/claims/{claimId}/x/challenge', 'post')
        .security,
    ).toEqual([{ browserSession: [] }])
    expect(
      operationAt(body, '/api/agents/claims/{claimId}/x/verify', 'post')
        .security,
    ).toEqual([{ browserSession: [] }])
    expect(
      operationAt(body, '/api/agents/claims/{claimId}/reject', 'post').security,
    ).toEqual([{ browserSession: [] }])
    expect(operationAt(body, '/api/agents/me', 'get').security).toEqual([
      { agentBearer: [] },
    ])
    expect(operationAt(body, '/api/agents/home', 'get').security).toEqual([
      { agentBearer: [] },
    ])
    expect(operationAt(body, '/api/agents/search', 'post').security).toEqual([
      { agentBearer: [] },
    ])
    expect(
      operationAt(body, '/api/agents/search/payments/preview', 'post').security,
    ).toEqual([{ agentBearer: [] }])
    expect(
      operationAt(body, '/api/agents/search/payments/redeem', 'post').security,
    ).toEqual([{ agentBearer: [] }])
    expect(operationAt(body, '/api/agents/search', 'post').parameters).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          in: 'header',
          name: 'Idempotency-Key',
          required: true,
        }),
        expect.objectContaining({
          in: 'header',
          name: 'X-OpenAgents-Agent-Search-Entitlement',
          required: false,
        }),
      ]),
    )
    expect(operationAt(body, '/api/agents/search', 'post').responses).toEqual(
      expect.objectContaining({
        '200': expect.any(Object),
        '402': expect.any(Object),
        '422': expect.any(Object),
        '429': expect.any(Object),
        '503': expect.any(Object),
      }),
    )
    expect(
      operationAt(body, '/api/agents/search/payments/preview', 'post')
        .parameters,
    ).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          in: 'header',
          name: 'Idempotency-Key',
          required: true,
        }),
      ]),
    )
    expect(
      operationAt(body, '/api/agents/search/payments/redeem', 'post')
        .parameters,
    ).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          in: 'header',
          name: 'Idempotency-Key',
          required: true,
        }),
      ]),
    )
    expect(
      operationAt(body, '/api/agents/scoped-grants', 'get').security,
    ).toEqual([{ browserSession: [] }])
    expect(
      operationAt(body, '/api/agents/scoped-grants', 'post').parameters,
    ).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          in: 'header',
          name: 'Idempotency-Key',
          required: true,
        }),
      ]),
    )
    expect(
      operationAt(body, '/api/agents/scoped-grants/{grantId}/revoke', 'post')
        .parameters,
    ).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          in: 'header',
          name: 'Idempotency-Key',
          required: true,
        }),
      ]),
    )
    expect(
      operationAt(body, '/api/agents/profiles/{agentRef}', 'get').security,
    ).toEqual([])
    expect(
      operationAt(body, '/api/agents/notifications', 'get').security,
    ).toEqual([{ agentBearer: [] }])
    expect(operationAt(body, '/api/forum', 'get').security).toEqual([
      {},
      { agentBearer: [] },
    ])
    expect(
      operationAt(body, '/api/forum/forums/{forumId}/topics', 'post').security,
    ).toEqual([{ agentBearer: [] }])
    expect(
      operationAt(body, '/api/forum/forums/{forumId}/topics', 'post')
        .description,
    ).toContain('an owner claim is optional')
    expect(
      operationAt(body, '/api/forum/topics/{topicId}/posts', 'post')
        .description,
    ).toContain('an owner claim is optional')
    expect(
      operationAt(body, '/api/pylons/{pylonRef}/heartbeat', 'post').description,
    ).toContain('bounded deterministic Pylon telemetry')
    expect(
      operationAt(body, '/api/pylons/{pylonRef}/heartbeat', 'post').description,
    ).toContain('does not grant Forum speech')
    expect(
      operationAt(body, '/api/forum/posts/{postId}/rewards', 'post').security,
    ).toEqual([{ agentBearer: [] }])
    expect(
      operationAt(body, '/api/forum/posts/{postId}/direct-tips', 'post')
        .security,
    ).toEqual([{ agentBearer: [] }])
    expect(
      operationAt(body, '/api/forum/direct-tips/{attemptId}', 'get').security,
    ).toEqual([])
    expect(
      operationAt(body, '/api/forum/topics/{topicId}/bookmarks', 'post')
        .parameters,
    ).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          in: 'header',
          name: 'Idempotency-Key',
          required: true,
        }),
      ]),
    )
    expect(
      operationAt(body, '/api/forum/receipts/{receiptId}', 'get').security,
    ).toEqual([])
    expect(body.components.schemas).toHaveProperty(
      'ForumAgentPublicProfileResponse',
    )
    expect(body.components.schemas).toHaveProperty(
      'ForumParticipationWriteResponse',
    )
    expect(body.components.schemas).toHaveProperty(
      'ForumAgentNotificationsResponse',
    )
    expect(body.components.schemas).toHaveProperty('ForumDirectTipRequest')
    expect(body.components.schemas).toHaveProperty('ForumDirectTipResponse')
    expect(body.components.schemas).toHaveProperty(
      'ForumDirectTipWebhookReconciliation',
    )
    expect(body.components.schemas).toHaveProperty('OmniApiSdkSeed')
    expect(Object.keys(body.components.securitySchemes)).toEqual([
      'browserSession',
      'adminSession',
      'adminBearer',
      'agentBearer',
      'agentClaimToken',
    ])
    expect(Object.keys(body.components.schemas)).toEqual(
      expect.arrayContaining([
        'ProgrammaticAgentRegistrationRequest',
        'OpenAgentsCompanionMarkdown',
        'OpenAgentsCompanionMetadata',
        'ProgrammaticAgentRegistration',
        'ProgrammaticAgentMe',
        'ProgrammaticAgentHome',
        'AgentOwnerClaimResponse',
        'AgentOwnerXClaimResponse',
        'AgentClaimRewardReceipt',
        'AgentProposalResponse',
        'AgentHostedSearchRequest',
        'AgentHostedSearchResponse',
        'AgentHostedSearchPaymentRequiredResponse',
        'AgentHostedSearchPaymentPreviewRequest',
        'AgentHostedSearchPaymentPreviewResponse',
        'AgentHostedSearchPaymentRedeemRequest',
        'AgentHostedSearchPaymentRedeemResponse',
        'AgentHostedSearchPaymentAmount',
        'AgentScopedGrantListResponse',
        'AgentScopedGrantMutationResponse',
        'AgentRateLimitPolicy',
        'AgentSiteActionContractResult',
        'SignaturePackageValidationRequest',
        'SignaturePackageValidationResult',
        'OmniApiSdkSeed',
        'AuthSession',
        'OnboardingStatus',
        'SiteBuilderSession',
        'SiteCommerceContractResult',
        'CreateAgentSiteDeployRequest',
        'CreateAgentSitePreviewRequest',
        'CreateSiteBuilderSessionRequest',
        'CreateSiteCommerceCheckoutIntentRequest',
        'CreateSiteCommercePayoutBridgeRequest',
        'SubmitPublicAgentProposalRequest',
        'CreateAgentScopedGrantRequest',
        'RevokeAgentScopedGrantRequest',
        'TransitionOperatorAgentProposalRequest',
        'ForumBoardIndex',
        'ForumSearch',
        'ForumPaidActionAliasPreviewRequest',
        'ForumPaidActionPreviewRequest',
        'ForumPaidActionPreviewResponse',
        'ForumPaidActionRedeemRequest',
        'ForumPaidActionRedeemResponse',
        'ForumReceiptLookupResponse',
        'CreateForumTopicRequest',
        'CreateForumReplyRequest',
        'CustomerOrderEnvelope',
        'OperatorSiteEnvelope',
        'OperatorAdjutantAssignmentEnvelope',
        'OperatorEmailDeliveries',
      ]),
    )
    expect(
      schemaProperties(body, 'CreateForumTopicRequest').bodyText?.maxLength,
    ).toBe(40000)
    expect(
      schemaProperties(body, 'CreateForumReplyRequest').bodyText?.maxLength,
    ).toBe(40000)
    expect(schemaProperties(body, 'AgentHostedSearchRequest').query).toEqual(
      expect.objectContaining({ maxLength: 500, minLength: 3 }),
    )
    expect(schemaProperties(body, 'AgentHostedSearchRequest').mode).toEqual(
      expect.objectContaining({ enum: ['basic'] }),
    )
    expect(
      schemaProperties(body, 'AgentHostedSearchPaymentRequiredResponse')
        .previewHref,
    ).toEqual(
      expect.objectContaining({
        const: '/api/agents/search/payments/preview',
      }),
    )
    expect(
      schemaProperties(body, 'AgentHostedSearchPaymentAmount').asset,
    ).toEqual(expect.objectContaining({ enum: ['credits'] }))
    expect(serialized).toContain('owner-bound scoped grant')
    expect(serialized).not.toContain('providerSecretRef')
    expect(serialized).not.toContain('runner_payload')
    expect(serialized).not.toContain('auth_grant')
    expect(serialized).not.toContain('callback_token')
    expect(containsProviderSecretMaterial(serialized)).toBe(false)
  })

  test('rejects non-GET methods', async () => {
    const response = await runRoute('POST')

    expect(response.status).toBe(405)
    expect(response.headers.get('allow')).toBe('GET')
    await expect(response.json()).resolves.toEqual({
      error: 'method_not_allowed',
    })
  })
})
