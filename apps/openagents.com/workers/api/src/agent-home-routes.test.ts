import { describe, expect, test } from 'vitest'

import {
  buildProgrammaticAgentHome,
  handleProgrammaticAgentHome,
} from './agent-home-routes'
import {
  AGENT_TOKEN_PREFIX,
  type AgentCredentialLookup,
  type AgentRegistrationStore,
  sha256Hex,
} from './agent-registration'

const agentLookup = (
  metadata: Record<string, unknown> = {},
): AgentCredentialLookup => ({
  credentialId: 'agent_credential_home',
  profileMetadataJson: JSON.stringify(metadata),
  tokenPrefix: `${AGENT_TOKEN_PREFIX}home`,
  user: {
    avatarUrl: null,
    createdAt: '2026-06-05T00:00:00.000Z',
    displayName: 'Home Test Agent',
    id: 'agent_home_user',
    kind: 'agent',
    primaryEmail: null,
    status: 'active',
    updatedAt: '2026-06-05T00:00:00.000Z',
  },
})

class MemoryAgentStore implements AgentRegistrationStore {
  readonly touched: Array<string> = []

  constructor(
    private readonly input: Readonly<{
      lookup?: AgentCredentialLookup
      token: string
    }>,
  ) {}

  createAgentRegistration = async () => {}

  findAgentByTokenHash = async (tokenHash: string) =>
    tokenHash === (await sha256Hex(this.input.token))
      ? this.input.lookup
      : undefined

  touchAgentCredential = async (credentialId: string) => {
    this.touched.push(credentialId)
  }
}

describe('programmatic agent home', () => {
  test('builds available scoped customer order actions from active grants', () => {
    const home = buildProgrammaticAgentHome(
      {
        credential: {
          id: 'credential_1',
          lastUsedAt: '2026-06-05T00:01:00.000Z',
          profileMetadataJson: JSON.stringify({
            customerOrderGrants: [
              {
                expiresAt: null,
                ownerUserId: 'github:1',
                scopes: ['customer_orders.read', 'customer_orders.feedback'],
                status: 'active',
              },
              {
                expiresAt: '2026-06-04T00:00:00.000Z',
                ownerUserId: 'github:2',
                scopes: ['customer_orders.write'],
                status: 'active',
              },
            ],
          }),
          tokenPrefix: 'oa_agent_home',
        },
        user: agentLookup().user,
      },
      '2026-06-05T00:02:00.000Z',
    )

    expect(home.authority.customerOrderGrants).toEqual([
      {
        expiresAt: null,
        grantId: undefined,
        ownerUserId: 'github:1',
        scopes: ['customer_orders.read', 'customer_orders.feedback'],
        status: 'active',
      },
    ])
    expect(home.rateLimit).toMatchObject({
      paidRecovery: 'planned_not_live',
      paymentPreviewRequired: true,
      recoveryModes: expect.arrayContaining([
        'wait',
        'operator_review',
        'future_credit_top_up',
        'future_l402',
      ]),
      spendCapRequired: true,
    })
    expect(home.docs).toMatchObject({
      heartbeat: 'https://openagents.com/HEARTBEAT.md',
      packageMetadata: 'https://openagents.com/skill.json',
      rules: 'https://openagents.com/RULES.md',
    })
    expect(home.authorizedResources).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'agent_public_profile',
          status: 'available_public',
        }),
        expect.objectContaining({
          id: 'agent_notifications',
          status: 'available',
        }),
        expect.objectContaining({
          id: 'agent_notification_mark_read',
          status: 'available',
        }),
        expect.objectContaining({
          id: 'customer_orders',
          status: 'available_scoped',
        }),
        expect.objectContaining({
          id: 'customer_order_create',
          status: 'not_granted',
        }),
        expect.objectContaining({
          id: 'site_feedback_submit',
          status: 'available_scoped',
        }),
        expect.objectContaining({
          id: 'forum_post_reward_preview',
          status: 'available_contract',
        }),
        expect.objectContaining({
          id: 'forum_topic_create',
          status: 'available',
        }),
        expect.objectContaining({
          id: 'forum_reply_create',
          status: 'available',
        }),
        expect.objectContaining({
          id: 'forum_post_edit',
          status: 'available_owned',
        }),
        expect.objectContaining({
          id: 'forum_post_tombstone',
          status: 'available_owned',
        }),
        expect.objectContaining({
          id: 'forum_target_report',
          status: 'available',
        }),
        expect.objectContaining({
          id: 'forum_context_activity',
          status: 'available_public',
        }),
        expect.objectContaining({
          id: 'forum_launch_status',
          status: 'available_public',
        }),
        expect.objectContaining({
          id: 'forum_paid_action_confirm_payment',
          status: 'available_contract',
        }),
        expect.objectContaining({
          id: 'forum_receipt_lookup',
          status: 'available_public',
        }),
        expect.objectContaining({
          id: 'forum_watch_topic',
          status: 'available',
        }),
        expect.objectContaining({
          id: 'forum_bookmark_post',
          status: 'available',
        }),
        expect.objectContaining({
          id: 'forum_follow_actor',
          status: 'available',
        }),
        expect.objectContaining({
          id: 'public_agent_proposals',
          status: 'available_public_no_token',
        }),
        expect.objectContaining({
          id: 'public_agent_proposal_rate_limit_preview',
          status: 'not_granted',
        }),
        expect.objectContaining({
          id: 'public_agent_proposal_rate_limit_redeem',
          status: 'not_granted',
        }),
        expect.objectContaining({
          id: 'agent_hosted_search',
          status: 'available_free_limited',
        }),
        expect.objectContaining({
          id: 'agent_hosted_search_payment_preview',
          status: 'available_contract',
        }),
        expect.objectContaining({
          id: 'agent_hosted_search_payment_redeem',
          status: 'available_contract',
        }),
        expect.objectContaining({
          id: 'pylon_register',
          status: 'available',
        }),
        expect.objectContaining({
          id: 'pylon_heartbeat',
          status: 'available_owned',
        }),
        expect.objectContaining({
          id: 'pylon_wallet_readiness',
          status: 'available_owned',
        }),
        expect.objectContaining({
          id: 'pylon_payment_receipts',
          status: 'available_owned',
        }),
      ]),
    )
    expect(home.authority.liveScopes.forum).toEqual([
      'forum.bookmark',
      'forum.follow',
      'forum.notifications.acknowledge',
      'forum.notifications.read',
      'forum.watch',
      'forum.write',
    ])
    expect(home.authority.liveScopes.pylon).toEqual([
      'pylons.artifacts.write',
      'pylons.assignments.write',
      'pylons.heartbeat.write',
      'pylons.payment_receipts.write',
      'pylons.payout_target_admission.write',
      'pylons.read',
      'pylons.register',
      'pylons.settlement_status.write',
      'pylons.wallet_readiness.write',
    ])
    expect(home.forum.notifications).toMatchObject({
      href: 'https://openagents.com/api/agents/notifications',
      markReadHref:
        'https://openagents.com/api/agents/notifications/{notificationId}/read',
      summary: {
        totalCount: 0,
        unreadCount: 0,
      },
    })
    expect(home.authority.liveScopes.rateLimitRecovery).toEqual([])
    expect(home.authority.liveScopes.search).toEqual(['agent_search.basic'])
    expect(home.plannedOrGated).not.toEqual(
      expect.arrayContaining([
        expect.objectContaining({ id: 'agent_self_registration' }),
        expect.objectContaining({ id: 'public_no_token_proposal_intake' }),
        expect.objectContaining({
          id: 'agent_profiles_watches_bookmarks_notifications',
        }),
        expect.objectContaining({ id: 'forum_paid_actions_and_receipts' }),
      ]),
    )
  })

  test('builds available paid proposal recovery from active owner spend grants', () => {
    const home = buildProgrammaticAgentHome(
      {
        credential: {
          id: 'credential_1',
          lastUsedAt: '2026-06-05T00:01:00.000Z',
          profileMetadataJson: JSON.stringify({
            agentRateLimitRecoveryGrants: [
              {
                expiresAt: null,
                ownerUserId: 'github:owner',
                routeKeys: ['public_agent_proposals'],
                spendCap: {
                  amount: 100,
                  asset: 'bitcoin',
                  denomination: 'sats',
                },
                status: 'active',
              },
            ],
          }),
          tokenPrefix: 'oa_agent_home',
        },
        user: agentLookup().user,
      },
      '2026-06-05T00:02:00.000Z',
    )

    expect(home.authority.agentRateLimitRecoveryGrants).toEqual([
      {
        expiresAt: null,
        grantId: undefined,
        ownerUserId: 'github:owner',
        routeKeys: ['public_agent_proposals'],
        spendCap: {
          amount: 100,
          asset: 'bitcoin',
          denomination: 'sats',
        },
        status: 'active',
      },
    ])
    expect(home.authority.liveScopes.rateLimitRecovery).toEqual([
      'public_agent_proposals.recover',
    ])
    expect(home.authorizedResources).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'public_agent_proposal_rate_limit_preview',
          status: 'available_scoped',
        }),
        expect.objectContaining({
          id: 'public_agent_proposal_rate_limit_redeem',
          status: 'available_scoped',
        }),
      ]),
    )
  })

  test('builds available scoped Site action contracts from active grants', () => {
    const home = buildProgrammaticAgentHome(
      {
        credential: {
          id: 'credential_1',
          lastUsedAt: '2026-06-05T00:01:00.000Z',
          profileMetadataJson: JSON.stringify({
            agentSiteGrants: [
              {
                expiresAt: null,
                scopes: ['sites:preview:request', 'sites:version:save'],
                siteId: 'site_123',
                status: 'active',
              },
              {
                expiresAt: '2026-06-04T00:00:00.000Z',
                scopes: ['sites:deploy:request'],
                siteId: 'site_123',
                status: 'active',
              },
            ],
          }),
          tokenPrefix: 'oa_agent_home',
        },
        user: agentLookup().user,
      },
      '2026-06-05T00:02:00.000Z',
    )

    expect(home.authority.agentSiteGrants).toEqual([
      {
        expiresAt: null,
        grantId: undefined,
        ownerUserId: undefined,
        scopes: ['sites:preview:request', 'sites:version:save'],
        siteId: 'site_123',
        status: 'active',
      },
    ])
    expect(home.authority.liveScopes.agentSites).toEqual([
      'sites:preview:request',
      'sites:version:save',
    ])
    expect(home.authorizedResources).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'agent_site_preview_request',
          status: 'available_scoped',
        }),
        expect.objectContaining({
          id: 'agent_site_version_save',
          status: 'available_scoped',
        }),
        expect.objectContaining({
          id: 'agent_site_deploy_request',
          status: 'not_granted',
        }),
      ]),
    )
    expect(home.plannedOrGated).not.toEqual(
      expect.arrayContaining([
        expect.objectContaining({ id: 'scoped_agent_site_actions' }),
      ]),
    )
  })

  test('serves authenticated home and touches credential', async () => {
    const token = `${AGENT_TOKEN_PREFIX}home_token`
    const store = new MemoryAgentStore({
      lookup: agentLookup({
        customerOrderGrants: [
          {
            expiresAt: null,
            ownerUserId: 'github:1',
            scopes: ['customer_orders.write'],
            status: 'active',
          },
        ],
      }),
      token,
    })
    const response = await handleProgrammaticAgentHome(
      new Request('https://openagents.com/api/agents/home', {
        headers: { Authorization: `Bearer ${token}` },
      }),
      {} as D1Database,
      {
        agentStore: store,
        nowIso: () => '2026-06-05T00:03:00.000Z',
      },
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(response.headers.get('ratelimit-policy')).toBe('60;w=60')
    expect(response.headers.get('x-openagents-paid-recovery')).toBe(
      'planned_not_live',
    )
    await expect(response.json()).resolves.toMatchObject({
      home: {
        authenticated: true,
        authority: {
          liveScopes: {
            customerOrders: ['customer_orders.write'],
          },
        },
      },
    })
    expect(store.touched).toEqual(['agent_credential_home'])
  })

  test('rejects callers without an active agent token', async () => {
    await expect(
      handleProgrammaticAgentHome(
        new Request('https://openagents.com/api/agents/home'),
        {} as D1Database,
      ),
    ).resolves.toMatchObject({ status: 401 })
  })
})
