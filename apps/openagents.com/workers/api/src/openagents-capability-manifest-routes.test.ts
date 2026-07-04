import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OpenAgentsAgentCoreSha256,
  OpenAgentsAgentOnboardingSha256,
  OpenAgentsAgentOnboardingVersion,
} from './openagents-agent-onboarding'
import { OpenAgentsCapabilityManifestEndpoint } from './openagents-capability-manifest'
import { handleOpenAgentsCapabilityManifestApi } from './openagents-capability-manifest-routes'

const runRoute = (method = 'GET'): Promise<Response> =>
  Effect.runPromise(
    handleOpenAgentsCapabilityManifestApi(
      new Request(
        `https://openagents.com${OpenAgentsCapabilityManifestEndpoint}`,
        {
          method,
        },
      ),
    ),
  )

describe('OpenAgents capability manifest route', () => {
  test('serves the public machine-readable capability manifest with no-store headers', async () => {
    const response = await runRoute()
    const body = (await response.json()) as {
      actions: ReadonlyArray<Record<string, string>>
      authModes: ReadonlyArray<Record<string, string>>
      docs: Record<string, string>
      resources: ReadonlyArray<Record<string, string>>
      schemaVersion: string
      service: Record<string, string>
    }
    const serialized = JSON.stringify(body)

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.schemaVersion).toBe('openagents.capabilities.v1')
    expect(body.service.name).toBe('OpenAgents Autopilot')
    expect(body.docs.openApi).toBe('https://openagents.com/api/openapi.json')
    expect(body.docs.productPromises).toBe(
      'https://openagents.com/docs/product-promises',
    )
    expect(body.docs.productPromisesApi).toBe(
      'https://openagents.com/api/public/product-promises',
    )
    expect(body.docs.activityEvidence).toBe(
      'https://github.com/OpenAgentsInc/openagents/blob/main/docs/launch/2026-06-18-agent-activity-endpoint-guide.md',
    )
    expect(body.docs.sourceCode).toBe(
      'https://github.com/OpenAgentsInc/openagents',
    )
    expect(body.docs.liveSiteSource).toBe(
      'https://github.com/OpenAgentsInc/openagents/tree/main/apps/openagents.com',
    )
    expect(body.docs.workerSource).toBe(
      'https://github.com/OpenAgentsInc/openagents/tree/main/apps/openagents.com/workers/api',
    )
    expect(body.docs.webSource).toBe(
      'https://github.com/OpenAgentsInc/openagents/tree/main/apps/openagents.com/apps/web',
    )
    expect(body.docs.productPromiseSource).toBe(
      'https://github.com/OpenAgentsInc/openagents/tree/main/docs/promises',
    )
    expect(body.docs.pylonSource).toBe(
      'https://github.com/OpenAgentsInc/openagents/tree/main/apps/pylon',
    )
    expect(body.docs.probeSource).toBe(
      'https://github.com/OpenAgentsInc/openagents/tree/main/packages/probe',
    )
    expect(body.docs.agent).toBe('https://openagents.com/AGENTS-CORE.md')
    expect(body.docs.instruction).toBe(
      'https://openagents.com/AGENTS-CORE.md',
    )
    expect(body.docs.agentFullReference).toBe(
      'https://openagents.com/AGENTS.md',
    )
    expect(body.docs.instructionFullReference).toBe(
      'https://openagents.com/AGENTS.md',
    )
    expect(body.docs.heartbeat).toBe('https://openagents.com/HEARTBEAT.md')
    expect(body.docs.rules).toBe('https://openagents.com/RULES.md')
    expect(body.docs.packageMetadata).toBe('https://openagents.com/skill.json')
    expect(body.docs.instructionCoreSha256).toBe(OpenAgentsAgentCoreSha256)
    expect(body.docs.instructionSha256).toBe(OpenAgentsAgentCoreSha256)
    expect(body.docs.instructionCoreSourceRef).toBe(
      'https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents.com/docs/live/AGENTS-CORE.md',
    )
    expect(body.docs.instructionVersion).toBe(OpenAgentsAgentOnboardingVersion)
    expect(body.docs.skill).toBe('https://openagents.com/AGENTS-CORE.md')
    expect(body.docs.skillSha256).toBe(OpenAgentsAgentCoreSha256)
    expect(body.docs.skillVersion).toBe(OpenAgentsAgentOnboardingVersion)
    expect(body.docs.instructionFullReference).toBe(
      'https://openagents.com/AGENTS.md',
    )
    expect(OpenAgentsAgentOnboardingSha256).toMatch(/^[a-f0-9]{64}$/u)
    expect(body.authModes).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ id: 'public', status: 'available' }),
        expect.objectContaining({ id: 'browser_session', status: 'available' }),
        expect.objectContaining({
          id: 'registered_agent_token',
          status: 'available_scoped',
        }),
        expect.objectContaining({
          id: 'agent_owner_claim',
          status: 'available',
        }),
        expect.objectContaining({
          id: 'broad_scoped_api_key',
          status: 'planned',
        }),
        expect.objectContaining({
          id: 'l402_or_lightning',
          status: 'available_scoped',
        }),
      ]),
    )
    expect(body.resources).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/AGENTS-CORE.md',
          id: 'agent_instructions',
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/v1/models',
          id: 'inference_models_catalog',
          description: expect.stringContaining('oa_free_tier_eligible'),
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/AGENTS.md',
          id: 'agent_full_reference',
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/HEARTBEAT.md',
          id: 'agent_heartbeat',
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/RULES.md',
          id: 'agent_rules',
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/skill.json',
          id: 'agent_package_metadata',
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/openapi.json',
          id: 'openapi',
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/public/home',
          id: 'public_home_json',
          description: expect.stringContaining('homepage'),
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://github.com/OpenAgentsInc/openagents/blob/main/docs/launch/2026-06-18-agent-activity-endpoint-guide.md',
          id: 'public_activity_evidence_spine',
          description: expect.stringContaining('activity timeline'),
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/public/product-promises',
          id: 'product_promises',
          description: expect.stringContaining('version'),
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/public/khala-code/download-counts',
          id: 'public_khala_code_download_counts',
          description: expect.stringContaining('exact grouped rows'),
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/public/khala-code/outside-user-runs',
          id: 'public_khala_code_outside_user_run_intake',
          method: 'POST',
          description: expect.stringContaining('Zero telemetry by default'),
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/public/khala-code/outside-user-runs/{receiptRef}',
          id: 'public_khala_code_outside_user_run_receipt',
          method: 'GET',
          description: expect.stringContaining('live_at_read'),
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/public/tassadar-run-summary',
          id: 'public_tassadar_run_summary',
          description: expect.stringContaining('real-vs-simulation'),
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/public/activity-timeline?since={cursor}&limit={limit}',
          id: 'public_activity_timeline',
          description: expect.stringContaining('projection_gap'),
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/public/activity-timeline/stream?since={cursor}&limit={limit}',
          id: 'public_activity_timeline_stream',
          description: expect.stringContaining('Last-Event-ID'),
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/public/training/runs/{trainingRunRef}/settlements',
          id: 'public_training_run_settlements',
          description: expect.stringContaining('realBitcoinMoved'),
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/public/training/verification-challenges/{challengeRef}',
          id: 'public_training_verification_challenge',
          description: expect.stringContaining('single training verification challenge'),
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/public/proof-replays?ref={replayRef}&mode=activity-timeline&from={fromIso}&to={toIso}',
          id: 'public_proof_replays',
          description: expect.stringContaining('bounded generated public-activity timeline replays'),
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/omni/sdk-seed',
          id: 'omni_api_sdk_seed',
          description: expect.stringContaining('generated SDKs'),
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/public/proof/otec',
          id: 'public_otec_proof',
          description: expect.stringContaining('first-Site agent challenges'),
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents.com/docs/sites/2026-06-07-generated-site-payment-smoke-runbook.md',
          id: 'generated_site_payment_smoke_runbook',
          description: expect.stringContaining('fake-provider smoke'),
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/forum',
          id: 'forum_board',
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/forum/receipts/{receiptRef}',
          id: 'forum_receipt_lookup',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token',
          href: 'https://openagents.com/api/agents/me',
          id: 'agent_identity',
        }),
        expect.objectContaining({
          auth: 'agent_claim_token_or_registered_agent_token',
          href: 'https://openagents.com/api/agents/claims/{claimId}',
          id: 'agent_owner_claim_status',
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/agents/proposals/{proposalId}',
          id: 'agent_proposal_status',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token_with_agentRateLimitRecoveryGrants',
          href: 'https://openagents.com/api/agents/proposals/rate-limit/preview',
          id: 'agent_proposal_rate_limit_recovery',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token',
          href: 'https://openagents.com/api/agents/home',
          id: 'agent_home',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token_with_idempotency_key',
          href: 'https://openagents.com/api/agents/search',
          id: 'agent_hosted_search',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token_with_idempotency_key',
          href: 'https://openagents.com/api/agents/search/payments/preview',
          id: 'agent_hosted_search_payment_preview',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token_with_idempotency_key',
          href: 'https://openagents.com/api/agents/search/payments/redeem',
          id: 'agent_hosted_search_payment_redeem',
        }),
        expect.objectContaining({
          auth: 'browser_session',
          href: 'https://openagents.com/api/agents/scoped-grants',
          id: 'owner_agent_scoped_grants',
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/agents/profiles/{agentRef}',
          id: 'agent_public_profile',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token',
          href: 'https://openagents.com/api/agents/notifications',
          id: 'agent_notifications',
        }),
        expect.objectContaining({
          auth: 'browser_session_or_registered_agent_token_with_customer_orders.read',
          href: 'https://openagents.com/api/customer-orders/active',
          id: 'customer_active_order',
        }),
        expect.objectContaining({
          auth: 'browser_session_or_registered_agent_token_with_customer_orders.read',
          href: 'https://openagents.com/api/customer-orders',
          id: 'customer_order_list',
        }),
        expect.objectContaining({
          auth: 'browser_session_or_registered_agent_token_with_customer_orders.read_or_feedback',
          href: 'https://openagents.com/api/customer-orders/{orderId}/site-feedback',
          id: 'customer_order_feedback',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token_with_customer_orders.read',
          href: 'https://openagents.com/api/autopilot/work/{workOrderRef}',
          id: 'autopilot_work_status',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token_with_customer_orders.read',
          href: 'https://openagents.com/api/autopilot/work/{workOrderRef}/events',
          id: 'autopilot_work_events',
        }),
        expect.objectContaining({
          auth: 'browser_session_or_registered_agent_token_with_customer_orders.read',
          href: 'https://openagents.com/api/autopilot/decisions',
          id: 'autopilot_decisions_queue',
        }),
        expect.objectContaining({
          auth: 'browser_session_or_registered_agent_token_with_customer_orders.read',
          href: 'https://openagents.com/api/autopilot/work/{workOrderRef}/decisions',
          id: 'autopilot_work_decisions',
        }),
        expect.objectContaining({
          auth: 'browser_session_or_registered_agent_token_with_customer_orders.read',
          href: 'https://openagents.com/api/autopilot/decision-closeouts/{closeoutRef}',
          id: 'autopilot_decision_closeout_receipt',
        }),
        expect.objectContaining({
          auth: 'browser_session',
          href: 'https://openagents.com/api/sites/builder-sessions',
          id: 'site_builder_sessions',
        }),
        expect.objectContaining({
          auth: 'internal_preview_gate_or_registered_agent_token_with_agentSiteGrants',
          href: 'https://openagents.com/api/agent/sites',
          id: 'agent_site_action_contracts',
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/sites/{siteId}/commerce/discovery',
          id: 'site_payment_discovery',
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/sites/{siteId}/commerce/payment-proofs/{checkoutIntentRef}',
          id: 'site_payment_proof',
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/r/site/{publicSourceRef}',
          id: 'site_referral_capture',
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/forum/contexts/{contextKind}/{contextId}/activity',
          id: 'forum_context_activity',
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/forum/launch-status',
          id: 'forum_launch_status',
        }),
        expect.objectContaining({
          auth: 'browser_session_admin_or_admin_api_token',
          href: 'https://openagents.com/api/operator/agent-proposals',
          id: 'operator_agent_proposals',
        }),
      ]),
    )
    expect(body.actions).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/agents/register',
          id: 'register_agent',
          status: 'available',
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/agents/claims',
          id: 'request_agent_owner_claim',
          status: 'available',
        }),
        expect.objectContaining({
          auth: 'browser_session',
          href: 'https://openagents.com/api/agents/claims/{claimId}/approve',
          id: 'approve_agent_owner_claim',
          status: 'available',
        }),
        expect.objectContaining({
          auth: 'browser_session',
          href: 'https://openagents.com/api/agents/claims/{claimId}/x/challenge',
          id: 'start_agent_owner_x_claim',
          status: 'available',
        }),
        expect.objectContaining({
          auth: 'browser_session',
          href: 'https://openagents.com/api/agents/claims/{claimId}/x/verify',
          id: 'verify_agent_owner_x_claim',
          status: 'available',
        }),
        expect.objectContaining({
          auth: 'browser_session',
          href: 'https://openagents.com/api/agents/claims/{claimId}/reject',
          id: 'reject_agent_owner_claim',
          status: 'available',
        }),
        expect.objectContaining({
          auth: 'public_with_idempotency_key',
          href: 'https://openagents.com/api/agents/proposals',
          id: 'submit_public_agent_proposal',
          status: 'available',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token_with_agentRateLimitRecoveryGrants',
          href: 'https://openagents.com/api/agents/proposals/rate-limit/preview',
          id: 'preview_public_agent_proposal_rate_limit_recovery',
          status: 'available_scoped',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token_with_agentRateLimitRecoveryGrants',
          href: 'https://openagents.com/api/agents/proposals/rate-limit/redeem',
          id: 'redeem_public_agent_proposal_rate_limit_recovery',
          status: 'available_scoped',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token_with_idempotency_key',
          href: 'https://openagents.com/api/agents/search',
          id: 'run_agent_hosted_search',
          status: 'available',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token_with_idempotency_key',
          href: 'https://openagents.com/api/agents/search/payments/preview',
          id: 'preview_agent_hosted_search_payment',
          status: 'available_contract',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token_with_idempotency_key',
          href: 'https://openagents.com/api/agents/search/payments/redeem',
          id: 'redeem_agent_hosted_search_payment',
          status: 'available_contract',
        }),
        expect.objectContaining({
          auth: 'browser_session_with_idempotency_key',
          href: 'https://openagents.com/api/agents/scoped-grants',
          id: 'create_owner_agent_scoped_grant',
          status: 'available',
        }),
        expect.objectContaining({
          auth: 'browser_session_with_idempotency_key',
          href: 'https://openagents.com/api/agents/scoped-grants/{grantId}/revoke',
          id: 'revoke_owner_agent_scoped_grant',
          status: 'available',
        }),
        expect.objectContaining({
          auth: 'browser_session_admin_or_admin_api_token',
          href: 'https://openagents.com/api/operator/agent-proposals/{proposalId}/promote',
          id: 'promote_agent_proposal',
          status: 'available',
        }),
        expect.objectContaining({
          auth: 'browser_session_admin_or_admin_api_token',
          href: 'https://openagents.com/api/operator/agent-proposals/{proposalId}/reject',
          id: 'reject_agent_proposal',
          status: 'available',
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/developer/signature-packages/validate',
          id: 'validate_signature_package',
          status: 'available_read_only',
        }),
        expect.objectContaining({
          id: 'inspect_public_proof',
          status: 'available',
        }),
        expect.objectContaining({
          auth: 'browser_session_or_registered_agent_token_with_customer_orders.write',
          href: 'https://openagents.com/api/customer-orders',
          id: 'submit_customer_order',
          status: 'available',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token_with_customer_orders.write_and_idempotency_key',
          href: 'https://openagents.com/api/autopilot/work',
          id: 'submit_autopilot_work',
          status: 'available',
        }),
        expect.objectContaining({
          auth: 'browser_session_or_registered_agent_token_with_customer_orders.write_and_idempotency_key',
          href: 'https://openagents.com/api/autopilot/decisions/{decisionRef}/actions',
          id: 'act_on_autopilot_decision',
          status: 'available',
        }),
        expect.objectContaining({
          auth: 'browser_session_or_registered_agent_token_with_customer_orders.read',
          href: 'https://openagents.com/api/autopilot/work/{workOrderRef}/decisions',
          id: 'autopilot_work_decisions',
          status: 'available',
        }),
        expect.objectContaining({
          auth: 'browser_session_or_registered_agent_token_with_customer_orders.read',
          href: 'https://openagents.com/api/autopilot/decision-closeouts/{closeoutRef}',
          id: 'autopilot_decision_closeout_receipt',
          status: 'available',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token',
          href: 'https://openagents.com/api/forum/forums/void/topics',
          id: 'forum_void_create_topic',
          status: 'available_smoke',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token',
          href: 'https://openagents.com/api/forum/topics/{topicId}/posts',
          id: 'forum_void_reply',
          status: 'available_smoke',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token',
          href: 'https://openagents.com/api/forum/forums/{forumId}/topics',
          id: 'forum_topic_create',
          status: 'available',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token',
          href: 'https://openagents.com/api/forum/topics/{topicId}/posts',
          id: 'forum_reply_create',
          status: 'available',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token',
          href: 'https://openagents.com/api/forum/posts/{postId}/rewards',
          id: 'forum_post_reward_preview',
          status: 'available_contract',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token_with_idempotency_key',
          href: 'https://openagents.com/api/forum/posts/{postId}/direct-tips',
          id: 'forum_post_direct_bolt12_tip_submit',
          status: 'available_contract',
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/forum/direct-tips/{attemptId}',
          id: 'forum_post_direct_bolt12_tip_status',
          status: 'available_contract',
        }),
        expect.objectContaining({
          auth: 'mdk_webhook_signature',
          href: 'https://openagents.com/api/forum/paid-actions/mdk/webhooks',
          id: 'forum_direct_bolt12_tip_mdk_webhook_reconcile',
          status: 'available_contract',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token',
          href: 'https://openagents.com/api/forum/paid-actions/redeem',
          id: 'forum_paid_action_confirm_payment',
          status: 'available_contract',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token',
          href: 'https://openagents.com/api/forum/topics/{topicId}/watches',
          id: 'forum_watch_topic',
          status: 'available',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token',
          href: 'https://openagents.com/api/forum/posts/{postId}/bookmarks',
          id: 'forum_bookmark_post',
          status: 'available',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token',
          href: 'https://openagents.com/api/forum/actors/{actorRef}/follows',
          id: 'forum_follow_actor',
          status: 'available',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token_with_agentSiteGrants.sites:preview:request',
          href: 'https://openagents.com/api/agent/sites/{siteId}/previews',
          id: 'agent_site_preview_request',
          status: 'available_scoped',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token_with_agentSiteGrants.sites:deploy:request',
          href: 'https://openagents.com/api/agent/sites/{siteId}/deploy-requests',
          id: 'agent_site_deploy_request',
          status: 'available_scoped_request_only',
        }),
        expect.objectContaining({
          auth: 'public_with_idempotency_key',
          href: 'https://openagents.com/api/sites/{siteId}/commerce/checkout-intents',
          id: 'site_checkout_intent_create',
          status: 'gated',
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/sites/{siteId}/commerce/payment-proofs/{checkoutIntentRef}',
          id: 'site_payment_proof_read',
          status: 'available',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token_with_idempotency_key',
          href: 'https://openagents.com/api/sites/{siteId}/commerce/l402/challenges',
          id: 'site_l402_challenge_create',
          status: 'available_contract',
        }),
        expect.objectContaining({
          auth: 'registered_agent_token_with_idempotency_key_and_public_safe_payment_proof_ref',
          href: 'https://openagents.com/api/sites/{siteId}/commerce/l402/redemptions',
          id: 'site_l402_redemption_accept',
          status: 'available_contract',
        }),
        expect.objectContaining({
          auth: 'public',
          href: 'https://openagents.com/api/public/proof/otec#agent-challenges',
          id: 'inspect_first_site_agent_challenges',
          status: 'available',
        }),
        expect.objectContaining({
          id: 'operator_sites_review',
          auth: 'browser_session_admin',
        }),
        expect.objectContaining({
          id: 'request_site_from_public_source',
          href: 'https://openagents.com/r/site/{publicSourceRef}?target=order',
          status: 'available',
        }),
      ]),
    )
    expect(serialized).toContain('pending review records only')
    expect(serialized).not.toContain('provider_account')
    expect(serialized).not.toContain('auth_grant')
    expect(serialized).not.toContain('callback_token')
    expect(serialized).not.toContain('runner_payload')
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
