import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect } from 'effect'
// Anti-staleness route coverage (#4752): openapi.json froze at 2026-06-05
// while shipped routes (including the tips receive-ladder route a green
// promise depends on) were missing from the contract surface. This suite
// statically scans the worker source for registered /api routes and fails
// when a registered route is neither documented in the OpenAPI paths nor
// listed in the explicit intentionally-undocumented allowlist below. New
// routes therefore cannot ship silently undocumented: either add an OpenAPI
// entry or make the omission an explicit, reviewable decision here.
import { readFileSync as fsReadFileSync, readdirSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, test } from 'vitest'

import { OpenAgentsOpenApiEndpoint } from './openagents-openapi'
import { handleOpenAgentsOpenApi } from './openagents-openapi-routes'
import { PublicProductPromisesVersion } from './product-promises'

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
  info: Readonly<{ title: string; version: string }>
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
    // #5057: the OpenAPI info.version must track the product-promise registry
    // version exactly so the contract surface can never silently lag the live
    // registry (projection-freshness invariant #5056).
    expect(body.info.version).toBe(PublicProductPromisesVersion)
    expect(
      operationAt(body, '/.well-known/openagents.json', 'get').operationId,
    ).toBe('getOpenAgentsCapabilityManifest')
    expect(operationAt(body, '/AGENTS.md', 'get').operationId).toBe(
      'getOpenAgentsAgentInstructions',
    )
    expect(operationAt(body, '/AGENTS-CORE.md', 'get').operationId).toBe(
      'getOpenAgentsCoreAgentInstructions',
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
      operationAt(
        body,
        '/api/public/artanis/tassadar-distillation-dataset',
        'get',
      ).operationId,
    ).toBe('getPublicArtanisTassadarDistillationDatasetReceipt')
    expect(
      operationAt(body, '/api/training/device-capabilities/a2', 'get')
        .operationId,
    ).toBe('readTrainingA2DeviceCapabilityDashboard')
    expect(
      operationAt(body, '/api/training/leaderboards', 'get').operationId,
    ).toBe('listTrainingLeaderboards')
    expect(
      operationAt(body, '/api/training/leaderboards/{lane}', 'get').operationId,
    ).toBe('getTrainingLeaderboardLane')
    expect(
      operationAt(body, '/api/public/training/runs/{trainingRunRef}', 'get')
        .operationId,
    ).toBe('getPublicTrainingRun')
    expect(
      operationAt(
        body,
        '/api/public/training/runs/{trainingRunRef}/settlements',
        'get',
      ).operationId,
    ).toBe('listPublicTrainingRunSettlements')
    expect(
      operationAt(
        body,
        '/api/training/runs/{trainingRunRef}/standby-dispatch-preflight',
        'post',
      ).operationId,
    ).toBe('preflightTrainingStandbyDispatch')
    expect(
      operationAt(
        body,
        '/api/training/runs/{trainingRunRef}/curtailment-drill-preflight',
        'post',
      ).operationId,
    ).toBe('preflightTrainingCurtailmentDrill')
    expect(
      operationAt(
        body,
        '/api/public/training/verification-challenges/{challengeRef}',
        'get',
      ).operationId,
    ).toBe('getPublicTrainingVerificationChallenge')
    expect(
      operationAt(body, '/api/public/training/full-pipeline-program', 'get')
        .operationId,
    ).toBe('getTrainingFullPipelineProgramStatus')
    expect(
      operationAt(body, '/api/public/training/marathon-operations', 'get')
        .operationId,
    ).toBe('getTrainingMarathonOperationsStatus')
    expect(
      operationAt(body, '/api/public/training/model-ladder-rungs', 'get')
        .operationId,
    ).toBe('getTrainingModelLadderRungsStatus')
    expect(
      operationAt(
        body,
        '/api/public/training/public-distributed-run-scale',
        'get',
      ).operationId,
    ).toBe('getTrainingPublicDistributedRunScaleStatus')
    expect(
      operationAt(
        body,
        '/api/public/pylon/largest-decentralized-training-claim',
        'get',
      ).operationId,
    ).toBe('getPylonLargestDecentralizedTrainingClaimStatus')
    expect(
      operationAt(body, '/api/public/training/public-gradient-windows', 'get')
        .operationId,
    ).toBe('getTrainingPublicGradientWindowsStatus')
    expect(
      operationAt(body, '/api/public/training/ablation-derisking-ledger', 'get')
        .operationId,
    ).toBe('getTrainingAblationDeriskingLedger')
    expect(
      operationAt(
        body,
        '/api/public/training/post-training-arc/instruct-sft-lane',
        'get',
      ).operationId,
    ).toBe('getTrainingPostTrainingInstructSftLane')
    expect(
      operationAt(
        body,
        '/api/public/training/post-training-arc/dpo-preference-workload',
        'get',
      ).operationId,
    ).toBe('getTrainingPostTrainingDpoPreferenceWorkload')
    expect(
      operationAt(
        body,
        '/api/public/training/post-training-arc/vibe-test-rubric',
        'get',
      ).operationId,
    ).toBe('getTrainingPostTrainingVibeTestRubric')
    expect(
      operationAt(
        body,
        '/api/public/models/tassadar-percepta-executor/architecture-receipts',
        'get',
      ).operationId,
    ).toBe('getTassadarPerceptaArchitectureReceipts')
    expect(
      operationAt(
        body,
        '/api/public/models/tassadar-percepta-executor/cpu-transform-training-receipts',
        'get',
      ).operationId,
    ).toBe('getTassadarPerceptaCpuTransformTrainingReceipts')
    expect(
      operationAt(body, '/api/public/tassadar-run-summary', 'get').operationId,
    ).toBe('getPublicTassadarRunSummary')
    const activityTimelineOperation = operationAt(
      body,
      '/api/public/activity-timeline',
      'get',
    )
    expect(activityTimelineOperation.operationId).toBe(
      'getPublicActivityTimeline',
    )
    expect(activityTimelineOperation.description).toEqual(
      expect.stringContaining('Source lag'),
    )
    expect(activityTimelineOperation.description).toEqual(
      expect.stringContaining('invalid_event_kind'),
    )
    expect(
      activityTimelineOperation.parameters?.map(parameter => parameter.name),
    ).toEqual(
      expect.arrayContaining([
        'since',
        'from',
        'to',
        'limit',
        'kind',
        'source',
      ]),
    )
    const activityTimelineStreamOperation = operationAt(
      body,
      '/api/public/activity-timeline/stream',
      'get',
    )
    expect(activityTimelineStreamOperation.operationId).toBe(
      'streamPublicActivityTimeline',
    )
    expect(activityTimelineStreamOperation.description).toEqual(
      expect.stringContaining('Last-Event-ID'),
    )
    const streamResponses = activityTimelineStreamOperation.responses as Record<
      string,
      { content?: Record<string, unknown> }
    >
    expect(streamResponses['200']?.content?.['text/event-stream']).toBeDefined()
    expect(
      activityTimelineStreamOperation.parameters?.map(
        parameter => parameter.name,
      ),
    ).toEqual(
      expect.arrayContaining([
        'since',
        'from',
        'to',
        'limit',
        'kind',
        'source',
      ]),
    )
    expect(operationAt(body, '/api/training/evals/a5', 'get').operationId).toBe(
      'readTrainingA5EvalDashboard',
    )
    expect(
      operationAt(body, '/api/hygiene-lane/debt-receipts', 'post').operationId,
    ).toBe('createHygieneLaneDebtReceipt')
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
    const inferenceModelsOperation = operationAt(body, '/api/v1/models', 'get')
    expect(inferenceModelsOperation.operationId).toBe('listInferenceModels')
    expect(inferenceModelsOperation.description).toEqual(
      expect.stringContaining('oa_free_tier_eligible'),
    )
    const khalaTokensServedOperation = operationAt(
      body,
      '/api/public/khala-tokens-served',
      'get',
    )
    expect(khalaTokensServedOperation.operationId).toBe(
      'getPublicKhalaTokensServed',
    )
    expect(khalaTokensServedOperation.description).toEqual(
      expect.stringContaining('tokensServed'),
    )
    const khalaTokensServedModelMixOperation = operationAt(
      body,
      '/api/public/khala-tokens-served/model-mix',
      'get',
    )
    expect(khalaTokensServedModelMixOperation.operationId).toBe(
      'getPublicKhalaTokensServedModelMix',
    )
    expect(khalaTokensServedModelMixOperation.description).toEqual(
      expect.stringContaining('canonical family aggregate rows'),
    )
    const khalaTokensServedDemandMixOperation = operationAt(
      body,
      '/api/public/khala-tokens-served/demand-mix',
      'get',
    )
    expect(khalaTokensServedDemandMixOperation.operationId).toBe(
      'getPublicKhalaTokensServedDemandMix',
    )
    expect(khalaTokensServedDemandMixOperation.description).toEqual(
      expect.stringContaining('demand/adoption mix'),
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
    expect(operationAt(body, '/api/autopilot/work', 'get').operationId).toBe(
      'listAutopilotWorkByPromise',
    )
    expect(
      operationAt(body, '/api/autopilot/work/{workOrderRef}', 'get')
        .operationId,
    ).toBe('getAutopilotWork')
    expect(
      operationAt(body, '/api/autopilot/work/{workOrderRef}/closeout', 'post')
        .operationId,
    ).toBe('recordAutopilotFallbackCloseout')
    expect(
      operationAt(body, '/api/autopilot/work/{workOrderRef}/review', 'post')
        .operationId,
    ).toBe('reviewAutopilotWork')
    expect(
      operationAt(body, '/api/autopilot/work/{workOrderRef}/events', 'get')
        .operationId,
    ).toBe('listAutopilotWorkEvents')
    expect(
      operationAt(body, '/api/autopilot/decisions', 'get').operationId,
    ).toBe('listAutopilotDecisions')
    expect(
      operationAt(
        body,
        '/api/autopilot/decisions/{decisionRef}/actions',
        'post',
      ).operationId,
    ).toBe('actOnAutopilotDecision')
    expect(
      operationAt(
        body,
        '/api/autopilot/decisions/{decisionRef}/actions',
        'post',
      ).parameters,
    ).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          in: 'header',
          name: 'Idempotency-Key',
          required: true,
        }),
      ]),
    )
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
      operationAt(body, '/api/public/training/runs/{trainingRunRef}', 'get')
        .security,
    ).toEqual([])
    expect(
      operationAt(
        body,
        '/api/public/training/runs/{trainingRunRef}/settlements',
        'get',
      ).security,
    ).toEqual([])
    expect(
      operationAt(
        body,
        '/api/public/training/verification-challenges/{challengeRef}',
        'get',
      ).security,
    ).toEqual([])
    expect(
      operationAt(body, '/api/public/training/marathon-operations', 'get')
        .security,
    ).toEqual([])
    expect(
      operationAt(body, '/api/public/training/model-ladder-rungs', 'get')
        .security,
    ).toEqual([])
    expect(
      operationAt(
        body,
        '/api/public/training/public-distributed-run-scale',
        'get',
      ).security,
    ).toEqual([])
    expect(
      operationAt(
        body,
        '/api/public/pylon/largest-decentralized-training-claim',
        'get',
      ).security,
    ).toEqual([])
    expect(
      operationAt(body, '/api/public/training/public-gradient-windows', 'get')
        .security,
    ).toEqual([])
    expect(
      operationAt(
        body,
        '/api/public/training/post-training-arc/vibe-test-rubric',
        'get',
      ).security,
    ).toEqual([])
    expect(
      operationAt(body, '/api/public/tassadar-run-summary', 'get').security,
    ).toEqual([])
    expect(
      operationAt(body, '/api/public/activity-timeline', 'get').security,
    ).toEqual([])
    expect(
      operationAt(body, '/api/public/activity-timeline/stream', 'get').security,
    ).toEqual([])
    expect(
      operationAt(
        body,
        '/api/public/tassadar-replays/first-real-settlement',
        'get',
      ).security,
    ).toEqual([])
    expect(
      operationAt(body, '/api/public/proof-replays', 'get').security,
    ).toEqual([])
    expect(
      operationAt(body, '/api/public/proof-replays', 'get').operationId,
    ).toBe('getPublicProofReplayBundle')
    expect(
      operationAt(
        body,
        '/api/public/tassadar-replays/first-real-settlement',
        'get',
      ).operationId,
    ).toBe('getPublicTassadarFirstRealSettlementReplay')
    expect(
      operationAt(body, '/api/hygiene-lane/debt-receipts', 'post').security,
    ).toEqual([{ adminBearer: [] }])
    expect(
      operationAt(
        body,
        '/api/training/runs/{trainingRunRef}/standby-dispatch-preflight',
        'post',
      ).security,
    ).toEqual([{ adminBearer: [] }])
    expect(
      operationAt(
        body,
        '/api/training/runs/{trainingRunRef}/curtailment-drill-preflight',
        'post',
      ).security,
    ).toEqual([{ adminBearer: [] }])
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
        'HygieneDebtReceiptCreateRequest',
        'HygieneDebtReceiptCreateResponse',
        'CustomerOrderEnvelope',
        'OperatorSiteEnvelope',
        'OperatorAdjutantAssignmentEnvelope',
        'OperatorEmailDeliveries',
        'TassadarPerceptaCpuTransformTrainingReceiptsEnvelope',
      ]),
    )
    expect(
      schemaProperties(body, 'CreateForumTopicRequest').bodyText?.maxLength,
    ).toBe(40000)
    expect(
      schemaProperties(body, 'CreateForumReplyRequest').bodyText?.maxLength,
    ).toBe(40000)
    const hygieneDebtReceiptRequest = schemaProperties(
      body,
      'HygieneDebtReceiptCreateRequest',
    )
    expect(hygieneDebtReceiptRequest.debtReceiptKeyInput).toEqual(
      expect.objectContaining({
        additionalProperties: false,
        required: [
          'debtReceiptRef',
          'objectiveDigest',
          'repoBaselineRef',
          'scopeDigest',
        ],
      }),
    )
    const hygieneDebtReceiptSourceRefs = hygieneDebtReceiptRequest.sourceRefs
    expect(hygieneDebtReceiptSourceRefs).toEqual(
      expect.objectContaining({ minItems: 1 }),
    )
    if (hygieneDebtReceiptSourceRefs === undefined) {
      throw new Error('Missing HygieneDebtReceiptCreateRequest.sourceRefs')
    }
    expect(
      (hygieneDebtReceiptSourceRefs.items as Readonly<Record<string, unknown>>)
        .maxLength,
    ).toBe(261)
    expect(hygieneDebtReceiptRequest.payableSats).toEqual(
      expect.objectContaining({ minimum: 1 }),
    )
    const hygieneDebtReceiptResponse = schemaProperties(
      body,
      'HygieneDebtReceiptCreateResponse',
    )
    expect(hygieneDebtReceiptResponse.debtReceipt).toEqual(
      expect.objectContaining({ additionalProperties: false }),
    )
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

const srcRoot = import.meta.dirname

// Routes that are deliberately not part of the public OpenAPI contract
// surface today: browser-session product app internals, owner/operator and
// admin consoles, provider webhooks, and internal dispatcher patterns.
// Removing a route from the OpenAPI document requires adding it here, which
// keeps undocumented surface an explicit decision instead of silent drift.
const intentionallyUndocumentedApiRoutes: ReadonlyArray<string> = [
  // Khala app aliases and operator feedback surfaces. Public stats use the
  // documented `/api/public/...` routes; feedback is app/operator-internal.
  '/api/khala/feedback',
  '/api/khala/tokens',
  '/api/operator/khala/feedback',
  '/api/operator/khala/trace-review',
  '/api/operator/khala/unsupported-requests',
  // Agency / services-business vertical pack (internal omni + operator surfaces;
  // session/operator-gated, not part of the public OpenAPI surface yet):
  '/api/workspaces',
  '/api/workspaces/{param}',
  '/api/workspaces/{param}/engagement',
  '/api/autopilot/work/{param}/lane-c-fanout',
  '/api/lists/{param}',
  '/api/lists/{param}/subscribers',
  '/api/omni/evidence-bundles/{param}',
  '/api/omni/public-proof-bundles/{param}',
  '/api/omni/workrooms',
  '/api/omni/workrooms/{param}',
  '/api/omni/workrooms/{param}/handoff',
  '/api/omni/workrooms/{param}/lifecycle-decisions',
  '/api/omni/workrooms/{param}/source-authority',
  '/api/operator/email-sequences',
  '/api/operator/email-sequences/{param}/enroll',
  '/api/operator/email-sequences/{param}/status',
  // Inference referral payout dispatch (sub-EPIC #5475 / #5490): admin-gated,
  // owner-armed dispatch of one accrued inference referral payout through the
  // shared RL-2 rail. Operator surface; intentionally undocumented in OpenAPI.
  '/api/operator/inference/referral/payout/{param}/dispatch',
  '/api/operator/inference/pylon-fabric/smoke',
  '/api/operator/partners/payout-ledger/{param}',
  '/api/operator/partners/payout-ledger/{param}/transitions',
  '/api/operator/sites/orchestration/{param}',
  '/api/operator/sites/orchestration/{param}/advance',
  '/api/sites/forms/{param}/submit',
  '/api/tenant/client/workrooms/{param}',
  // Owner/admin and operator consoles (admin session or admin bearer only):
  '/api/admin/overview',
  '/api/admin/cf-browser-smoke',
  // Owner/admin agent token reissue console (admin bearer only): operator
  // credential surface, not part of the public OpenAPI contract.
  '/api/admin/agents/reissue-token',
  // Owner-gated inference cost / provider-lane analytics (#6232): internal cost
  // + provider data, owner session only, not part of the public OpenAPI surface.
  '/api/admin/inference-analytics',
  // Operator fleet status aggregator (#6427): admin/operator snapshot for
  // internal Artanis and fleet surfaces; public pages consume a later
  // public-safe projection, not this operator route directly.
  '/api/operator/fleet/status',
  '/api/admin/provider-accounts/usage',
  '/api/admin/sync/notify',
  // Self-hosted trace media blob serving (#6223): serves the public-safe R2 blob
  // bytes for a stored trace by uuid + blob key. Internal media-serving route,
  // not part of the typed JSON OpenAPI surface.
  '/api/traces/{param}/blob/{param}',
  '/api/operator/crm/accounts',
  '/api/operator/crm/accounts/{param}',
  '/api/operator/crm/commands',
  '/api/operator/crm/commands/{param}/approve',
  '/api/operator/crm/commands/{param}/reject',
  '/api/operator/crm/contacts',
  '/api/operator/crm/contacts/{param}',
  '/api/operator/crm/contacts/{param}/activities',
  '/api/operator/crm/contacts/{param}/commands/send-email',
  '/api/operator/crm/contacts/{param}/emails',
  '/api/operator/crm/contacts/{param}/engagement',
  '/api/operator/crm/contacts/{param}/gmail-writeback',
  '/api/operator/crm/contacts/{param}/render',
  '/api/operator/crm/contacts/{param}/resend-send',
  '/api/operator/crm/contacts/{param}/send',
  '/api/operator/crm/gmail-queue',
  '/api/operator/crm/import',
  '/api/operator/crm/import-runs',
  '/api/operator/crm/lists',
  '/api/operator/crm/mcp-grants',
  '/api/operator/crm/mcp-grants/{param}',
  '/api/operator/crm/opportunities',
  '/api/operator/crm/opportunities/{param}',
  '/api/operator/crm/send-batch',
  '/api/operator/crm/templates',
  '/api/operator/adjutant/assignments/{param}/current-run/clear',
  '/api/operator/adjutant/assignments/{param}/enrichment',
  '/api/operator/adjutant/assignments/{param}/enrichment/briefs/{param}/review',
  '/api/operator/adjutant/assignments/{param}/enrichment/enqueue',
  '/api/operator/adjutant/assignments/{param}/enrichment/plan',
  '/api/operator/adjutant/assignments/{param}/enrichment/refresh',
  '/api/operator/adjutant/assignments/{param}/enrichment/run',
  '/api/operator/adjutant/assignments/{param}/enrichment/source-cards/{param}/review',
  '/api/operator/adjutant/assignments/{param}/enrichment/source-refs',
  '/api/operator/adjutant/assignments/{param}/enrichment/source-refs/{param}/review',
  '/api/operator/adjutant/assignments/{param}/preflight',
  '/api/operator/adjutant/assignments/{param}/research-policy',
  '/api/operator/adjutant/assignments/{param}/task-packet',
  '/api/operator/adjutant/assignments/{param}/task-packet/keep-current',
  '/api/operator/artanis/approval-gates/{param}/approve',
  '/api/operator/artanis/approval-gates/{param}/reject',
  '/api/operator/artanis/console',
  '/api/operator/artanis/mind/smoke',
  '/api/operator/artanis/spend-decision',
  '/api/operator/autopilot/goals',
  '/api/operator/autopilot/goals/current',
  '/api/operator/autopilot/goals/{param}',
  '/api/operator/autopilot/goals/{param}/clear',
  '/api/operator/autopilot/goals/{param}/pause',
  '/api/operator/autopilot/goals/{param}/resume',
  '/api/operator/autopilot/goals/{param}/visibility',
  '/api/operator/autopilot/preflight',
  '/api/operator/buy-mode',
  '/api/operator/buy-mode/dispatch',
  '/api/operator/buy-mode/eval',
  '/api/operator/buy-mode/results/settle',
  '/api/operator/buy-mode/start',
  '/api/operator/ecommerce-campaign/receipts',
  '/api/operator/private-project-workspaces',
  '/api/operator/team-workspace-invites',
  '/api/operator/tips-buffer/payout',
  '/api/team-workspace-invites/accept',
  '/api/training/contributions/next-unpaired',
  '/api/operator/buy-mode/stop',
  '/api/operator/email-deliveries/review-ready-smoke',
  '/api/operator/orders/triage',
  '/api/operator/orders/triage/autopilot-foldover-inventory',
  '/api/operator/orders/triage/first-batch/assign',
  '/api/operator/orders/triage/first-batch/monitor',
  '/api/operator/orders/triage/first-batch/payment-policy',
  '/api/operator/orders/{param}/fulfillment/prepare',
  '/api/operator/orders/{param}/triage',
  '/api/operator/provider-accounts/chatgpt-codex/device-login/start',
  '/api/operator/provider-accounts/chatgpt-codex/device-login/{param}',
  '/api/operator/provider-accounts/chatgpt-codex/fleet-dashboard',
  '/api/operator/provider-accounts/chatgpt-codex/leases',
  '/api/operator/provider-accounts/chatgpt-codex/leases/active',
  '/api/operator/provider-accounts/chatgpt-codex/leases/explain',
  '/api/operator/provider-accounts/chatgpt-codex/leases/failover',
  '/api/operator/provider-accounts/chatgpt-codex/leases/failover-history',
  '/api/operator/provider-accounts/chatgpt-codex/leases/grant',
  '/api/operator/provider-accounts/chatgpt-codex/leases/release',
  '/api/operator/provider-accounts/chatgpt-codex/leases/touch',
  '/api/operator/provider-accounts/chatgpt-codex/sanity',
  '/api/operator/sites/builder-sessions/{param}/events',
  '/api/operator/sites/builder-sessions/{param}/versions',
  '/api/operator/sites/{param}/access',
  '/api/operator/sites/{param}/access-grants',
  '/api/operator/sites/{param}/build-validations/latest',
  '/api/operator/sites/{param}/deployments/{param}/disable',
  '/api/operator/sites/{param}/deployments/{param}/rollback',
  '/api/operator/sites/{param}/environment-values',
  '/api/operator/sites/{param}/events',
  '/api/operator/sites/{param}/generate',
  '/api/operator/sites/{param}/provisioning-plans',
  '/api/operator/sites/{param}/versions/{param}/source-exports',
  '/api/operator/tassadar/replay',
  '/api/operator/tips-buffer/funding-destination',
  '/api/operator/tips-buffer/status',
  '/api/operator/treasury/funding-destination',
  '/api/operator/treasury/payout',
  '/api/operator/treasury/spark-funding-destination',
  '/api/operator/treasury/status',
  '/api/operator/treasury/transactions/reconcile',
  // Browser-session product app internals (signed-in openagents.com app only):
  '/api/auth/teams',
  '/api/auth/totals',
  '/api/autopilot/fleet',
  '/api/autopilot/goals',
  '/api/autopilot/goals/current',
  '/api/autopilot/goals/{param}',
  '/api/autopilot/goals/{param}/clear',
  '/api/autopilot/goals/{param}/pause',
  '/api/autopilot/goals/{param}/resume',
  '/api/autopilot/goals/{param}/visibility',
  '/api/autopilot/missions',
  '/api/autopilot/onboarding/{param}',
  '/api/autopilot/onboarding/{param}/turn',
  '/api/autopilot/token-leaderboards',
  '/api/billing/auto-top-up-policy',
  '/api/billing/auto-top-up/run',
  '/api/billing/checkout',
  '/api/billing/coupons/redeem',
  '/api/billing/inference-credit',
  '/api/billing/stripe/checkout-return',
  '/api/billing/stripe/setup-intents',
  '/api/billing/stripe/setup-intents/save',
  '/api/billing/stripe/webhook',
  '/api/billing/summary',
  '/api/github-write/connections',
  '/api/github-write/connections/{param}/disconnect',
  '/api/github-write/grants/resolve',
  '/api/images/generate',
  '/api/images/{param}',
  // Inference referral revshare dashboard (sub-EPIC #5475 / #5491): browser-
  // session-scoped public-safe read of the signed-in referrer's inference
  // earnings. Internal product surface; intentionally undocumented in OpenAPI.
  '/api/inference/referral/dashboard',
  '/api/onboarding/billing/skip',
  '/api/onboarding/goal',
  '/api/share',
  '/api/share/{param}',
  '/api/share/{param}/v1/data',
  '/api/sites/referrals/overview',
  '/api/sites/{param}/access',
  '/api/sites/{param}/archive',
  '/api/sites/{param}/commerce/{param}',
  '/api/sites/{param}/delete',
  '/api/stats/token-usage/aggregate',
  '/api/stats/token-usage/events',
  '/api/stats/token-usage/leaderboard-preference',
  '/api/stats/token-usage/leaderboards',
  '/api/sync/{param}/{param}/{param}',
  '/api/teams/{param}/chat/messages',
  '/api/teams/{param}/files',
  '/api/teams/{param}/projects/{param}/chat/messages',
  '/api/thread-files',
  '/api/thread-files/{param}',
  '/api/thread-files/{param}/download',
  // Omni deployment/agent-run surfaces (session- or deployment-scoped app internals):
  '/api/omni/agent-runs',
  '/api/omni/agent-runs/{param}',
  '/api/omni/agent-runs/{param}/events',
  '/api/omni/agent-runs/{param}/events/ingest',
  '/api/omni/deployments',
  '/api/omni/deployments/{param}',
  '/api/omni/deployments/{param}/events',
  '/api/omni/deployments/{param}/events/ingest',
  '/api/omni/operator/agent-runs',
  '/api/omni/operator/agent-runs/{param}',
  '/api/omni/operator/autopilot/checklist',
  '/api/omni/operator/autopilot/preflight',
  '/api/omni/operator/billing/credits',
  '/api/omni/operator/billing/inference-credit',
  '/api/omni/operator/deployments',
  '/api/omni/operator/fleet',
  '/api/omni/operator/team-chat/messages',
  // Provider-account credential plumbing (owner-bound; never public contract surface):
  '/api/pylon/auth/openagents/device/start',
  '/api/pylon/auth/openagents/device/verify',
  '/api/pylon/auth/openagents/device/{param}',
  '/api/pylon/provider-accounts/chatgpt-codex/device-login/start',
  '/api/pylon/provider-accounts/chatgpt-codex/device-login/{param}',
  '/api/pylon/provider-accounts/chatgpt-codex/local-auth/import',
  '/api/provider-accounts',
  '/api/provider-accounts/anthropic/connect',
  '/api/provider-accounts/chatgpt-codex/device-login/start',
  '/api/provider-accounts/chatgpt-codex/device-login/{param}',
  '/api/provider-accounts/chatgpt-codex/device-login/{param}/connected',
  '/api/provider-accounts/chatgpt-codex/device-login/{param}/failed',
  '/api/provider-accounts/chatgpt-codex/grants/resolve',
  '/api/provider-accounts/google-gemini/connect',
  '/api/provider-accounts/google-gemini/grants/resolve',
  '/api/provider-accounts/google-gemini/models/{param}:streamGenerateContent',
  '/api/provider-accounts/{param}/disconnect',
  '/api/provider-accounts/{param}/grants',
  '/api/provider-accounts/{param}/health',
  // Blueprint program-registry internals:
  '/api/blueprint/action-submissions',
  '/api/blueprint/contracts',
  '/api/blueprint/contributions',
  '/api/blueprint/program-registry',
  '/api/blueprint/program-runs',
  '/api/blueprint/tassadar-modules',
  // Provider webhooks and internal callbacks:
  '/api/mdk',
  '/api/webhooks/resend',
  // Documented-alias and dispatcher patterns (concrete documented routes cover them):
  '/api/agent/sites/{param}/{param}',
  '/api/public/agents/{param}/current-goal',
]

const sourceFiles = (): ReadonlyArray<string> =>
  readdirSync(srcRoot, { encoding: 'utf8', recursive: true })
    .filter(file => file.endsWith('.ts'))
    .filter(file => !file.endsWith('.test.ts'))
    .filter(file => !file.includes('node_modules'))
    .map(file => join(srcRoot, file))

const expandRoutePattern = (pattern: string): ReadonlyArray<string> => {
  const results: Array<string> = []
  const queue: Array<string> = [pattern]

  while (queue.length > 0) {
    const current = queue.pop()

    if (current === undefined) {
      break
    }

    const optionalGroup = /\(\?:((?:[^()]|\([^()]*\))*)\)\?/.exec(current)
    if (optionalGroup !== null) {
      queue.push(current.replace(optionalGroup[0], ''))
      queue.push(current.replace(optionalGroup[0], optionalGroup[1] ?? ''))
      continue
    }

    const alternation = /\((?:\?:)?([A-Za-z0-9_|-]+\|[A-Za-z0-9_|-]+)\)/.exec(
      current,
    )
    if (alternation !== null) {
      for (const option of (alternation[1] ?? '').split('|')) {
        queue.push(current.replace(alternation[0], option))
      }
      continue
    }

    const normalized = current
      .replace(/\(\[\^\/\]\+\)/g, '{param}')
      .replace(/\[\^\/\]\+/g, '{param}')
      .replace(/\(\.\+\)/g, '{param}')

    if (!/[()\\^$?*+[\]]/.test(normalized)) {
      results.push(normalized)
    }
  }

  return results
}

const registeredApiRoutes = (): ReadonlyArray<string> => {
  const routes = new Set<string>()

  for (const file of sourceFiles()) {
    const content = fsReadFileSync(file, 'utf8')

    for (const match of content.matchAll(
      /pathname\s*===\s*\n?\s*'(\/api\/[^']*)'/g,
    )) {
      routes.add(match[1] ?? '')
    }

    if (file.endsWith('/index.ts')) {
      for (const match of content.matchAll(/path:\s*'(\/api\/[^']*)'/g)) {
        routes.add(match[1] ?? '')
      }
    }

    for (const match of content.matchAll(/\/\^(\\\/api\\\/[^\n]*?)\$\//g)) {
      const pattern = (match[1] ?? '').replace(/\\\//g, '/')
      for (const expanded of expandRoutePattern(pattern)) {
        routes.add(expanded)
      }
    }
  }

  return [...routes].sort()
}

const normalizeRouteParams = (path: string): string =>
  path.replace(/\{[^}]+\}/g, '{param}')

const routeMatchesDocumentedPath = (
  route: string,
  documented: string,
): boolean => {
  const routeSegments = route.split('/')
  const documentedSegments = documented.split('/')

  return (
    routeSegments.length === documentedSegments.length &&
    documentedSegments.every(
      (segment, index) =>
        segment === '{param}' || segment === routeSegments[index],
    )
  )
}

const documentedPaths = async (): Promise<ReadonlyArray<string>> => {
  const response = await runRoute()
  const body: { paths?: Record<string, unknown> } = JSON.parse(
    await response.text(),
  )

  return Object.keys(body.paths ?? {})
}

describe('OpenAgents OpenAPI registered-route coverage', () => {
  test('every registered /api route is documented or explicitly allowlisted', async () => {
    const documented = (await documentedPaths()).map(normalizeRouteParams)
    const allowlisted = new Set(
      intentionallyUndocumentedApiRoutes.map(normalizeRouteParams),
    )
    const registered = registeredApiRoutes().map(normalizeRouteParams)

    expect(registered.length).toBeGreaterThan(150)

    const missing = registered.filter(
      route =>
        !allowlisted.has(route) &&
        !documented.some(path => routeMatchesDocumentedPath(route, path)),
    )

    expect(missing).toEqual([])
  })

  test('allowlisted routes do not shadow documented paths', async () => {
    const documented = (await documentedPaths()).map(normalizeRouteParams)

    const shadowed = intentionallyUndocumentedApiRoutes
      .map(normalizeRouteParams)
      .filter(route =>
        documented.some(path => routeMatchesDocumentedPath(route, path)),
      )

    expect(shadowed).toEqual([])
  })
})
