import type { IdentityDb } from './identity-db'
import { describe, expect, test } from 'vitest'

import {
  buildProgrammaticAgentHome,
  handleProgrammaticAgentHome,
  handleProgrammaticAgentSelfUpdate,
} from './agent-home-routes'
import {
  AGENT_TOKEN_PREFIX,
  type AgentCredentialLookup,
  type AgentRegistrationStore,
  sha256Hex,
} from './agent-registration'
import {
  buildPylonApiRegistrationRecord,
  publicPylonApiRegistrationProjection,
} from './pylon-api'

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
  readonly displayNameUpdates: Array<{
    displayName: string
    updatedAt: string
    userId: string
  }> = []

  private lookup: AgentCredentialLookup | undefined

  constructor(
    private readonly input: Readonly<{
      lookup?: AgentCredentialLookup
      token: string
      // When false, updateAgentDisplayName reports 0 rows changed so the route
      // can be exercised against a missing/non-updatable agent row (404 path).
      updatable?: boolean
    }>,
  ) {
    this.lookup = input.lookup
  }

  createAgentRegistration = async () => {}

  findAgentByTokenHash = async (tokenHash: string) =>
    tokenHash === (await sha256Hex(this.input.token)) ? this.lookup : undefined

  touchAgentCredential = async (credentialId: string) => {
    this.touched.push(credentialId)
  }

  updateAgentDisplayName = async (
    userId: string,
    displayName: string,
    updatedAt: string,
  ) => {
    if (this.input.updatable === false) {
      return 0
    }

    this.displayNameUpdates.push({ displayName, updatedAt, userId })

    // Mutate the in-memory lookup so a follow-up authenticated read reflects
    // the rename, mirroring the live user-row source of truth.
    if (this.lookup !== undefined) {
      this.lookup = {
        ...this.lookup,
        user: { ...this.lookup.user, displayName, updatedAt },
      }
    }

    return 1
  }
}

// CFG-4 Domain 2 (#8519): these tests always inject `agentStore`, so the
// identity handle is never reached — a throwing stub keeps that honest.
const stubIdentityDb: IdentityDb = {
  batch: () => Promise.reject(new Error('identityDb.batch should not be used')),
  query: () => Promise.reject(new Error('identityDb.query should not be used')),
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
      stubIdentityDb,
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
        stubIdentityDb,
      ),
    ).resolves.toMatchObject({ status: 401 })
  })
})

describe('programmatic agent self displayName update (#5333)', () => {
  const token = `${AGENT_TOKEN_PREFIX}rename_token`

  const renameRequest = (
    body: unknown,
    headers: Record<string, string> = {},
  ): Request =>
    new Request('https://openagents.com/api/agents/me', {
      body: JSON.stringify(body),
      headers: {
        Authorization: `Bearer ${token}`,
        'Content-Type': 'application/json',
        'Idempotency-Key': 'rename-key-1',
        ...headers,
      },
      method: 'PATCH',
    })

  test('renames self and propagates to GET /api/agents/me and pylon projection', async () => {
    const store = new MemoryAgentStore({ lookup: agentLookup(), token })

    const response = await handleProgrammaticAgentSelfUpdate(
      renameRequest({ displayName: '  Trigger Pylon#1  ' }),
      {} as D1Database,
      stubIdentityDb,
      {
        agentStore: store,
        makeReceiptNonce: () => 'fixed-nonce',
        nowIso: () => '2026-06-18T00:00:00.000Z',
      },
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    const payload = (await response.json()) as {
      agent: { user: { displayName: string; updatedAt: string } }
      receipt: { changed: boolean; ref: string }
      updated: boolean
    }
    // Trimmed by the shared AgentDisplayName constraint.
    expect(payload.agent.user.displayName).toBe('Trigger Pylon#1')
    expect(payload.agent.user.updatedAt).toBe('2026-06-18T00:00:00.000Z')
    expect(payload.updated).toBe(true)
    expect(payload.receipt.changed).toBe(true)
    expect(payload.receipt.ref).toMatch(/^agent_display_name_update\.[a-f0-9]{32}$/)

    // Self-only: the store recorded an update for the authenticated user id.
    expect(store.displayNameUpdates).toEqual([
      {
        displayName: 'Trigger Pylon#1',
        updatedAt: '2026-06-18T00:00:00.000Z',
        userId: 'agent_home_user',
      },
    ])

    // GET /api/agents/me reflects the renamed user row (it reads the same
    // source the rename wrote to).
    const meResponse = await handleProgrammaticAgentHome(
      new Request('https://openagents.com/api/agents/home', {
        headers: { Authorization: `Bearer ${token}` },
      }),
      {} as D1Database,
      stubIdentityDb,
      { agentStore: store, nowIso: () => '2026-06-18T00:01:00.000Z' },
    )
    const mePayload = (await meResponse.json()) as {
      home: { agent: { user: { displayName: string } } }
    }
    expect(mePayload.home.agent.user.displayName).toBe('Trigger Pylon#1')

    // A Pylon registration projection built from the renamed session
    // displayName carries the new name (pylon-api-routes builds the projection
    // displayName from session.user.displayName).
    const session = await store.findAgentByTokenHash(await sha256Hex(token))
    const registration = buildPylonApiRegistrationRecord({
      credentialId: 'agent_credential_home',
      displayName: session?.user.displayName ?? '',
      makeId: () => 'reg1',
      nowIso: '2026-06-18T00:02:00.000Z',
      ownerAgentTokenPrefix: `${AGENT_TOKEN_PREFIX}home`,
      ownerAgentUserId: 'agent_home_user',
      request: {},
    })
    const projection = publicPylonApiRegistrationProjection(
      registration,
      '2026-06-18T00:02:00.000Z',
    )
    expect(projection.displayName).toBe('Trigger Pylon#1')
  })

  test('repeated identical rename is idempotent and is a no-op write', async () => {
    const store = new MemoryAgentStore({ lookup: agentLookup(), token })

    const first = await handleProgrammaticAgentSelfUpdate(
      renameRequest({ displayName: 'Renamed Agent' }),
      {} as D1Database,
      stubIdentityDb,
      { agentStore: store, nowIso: () => '2026-06-18T00:00:00.000Z' },
    )
    expect(first.status).toBe(200)
    expect(store.displayNameUpdates).toHaveLength(1)

    const second = await handleProgrammaticAgentSelfUpdate(
      renameRequest({ displayName: 'Renamed Agent' }),
      {} as D1Database,
      stubIdentityDb,
      { agentStore: store, nowIso: () => '2026-06-18T00:05:00.000Z' },
    )
    expect(second.status).toBe(200)
    const secondPayload = (await second.json()) as {
      receipt: { changed: boolean; ref: string }
      updated: boolean
    }
    expect(secondPayload.updated).toBe(true)
    expect(secondPayload.receipt.changed).toBe(false)
    expect(secondPayload.receipt.ref).toMatch(/\.noop$/)
    // No second write was issued because the name already matched.
    expect(store.displayNameUpdates).toHaveLength(1)
  })

  test('rejects an unauthenticated / non-self caller with 401', async () => {
    const store = new MemoryAgentStore({ lookup: agentLookup(), token })

    const response = await handleProgrammaticAgentSelfUpdate(
      new Request('https://openagents.com/api/agents/me', {
        body: JSON.stringify({ displayName: 'Hijack' }),
        headers: {
          Authorization: `Bearer ${AGENT_TOKEN_PREFIX}wrong_token`,
          'Idempotency-Key': 'rename-key-1',
        },
        method: 'PATCH',
      }),
      {} as D1Database,
      stubIdentityDb,
      { agentStore: store },
    )

    expect(response.status).toBe(401)
    expect(store.displayNameUpdates).toHaveLength(0)
  })

  test('rejects an invalid displayName with 400', async () => {
    const store = new MemoryAgentStore({ lookup: agentLookup(), token })

    const empty = await handleProgrammaticAgentSelfUpdate(
      renameRequest({ displayName: '   ' }),
      {} as D1Database,
      stubIdentityDb,
      { agentStore: store },
    )
    expect(empty.status).toBe(400)
    await expect(empty.json()).resolves.toMatchObject({
      error: 'invalid_display_name',
    })

    const tooLong = await handleProgrammaticAgentSelfUpdate(
      renameRequest({ displayName: 'x'.repeat(121) }),
      {} as D1Database,
      stubIdentityDb,
      { agentStore: store },
    )
    expect(tooLong.status).toBe(400)

    expect(store.displayNameUpdates).toHaveLength(0)
  })

  test('rejects a missing Idempotency-Key with the standard 400', async () => {
    const store = new MemoryAgentStore({ lookup: agentLookup(), token })

    const response = await handleProgrammaticAgentSelfUpdate(
      new Request('https://openagents.com/api/agents/me', {
        body: JSON.stringify({ displayName: 'Renamed Agent' }),
        headers: { Authorization: `Bearer ${token}` },
        method: 'PATCH',
      }),
      {} as D1Database,
      stubIdentityDb,
      { agentStore: store },
    )

    expect(response.status).toBe(400)
    await expect(response.json()).resolves.toMatchObject({
      error: 'idempotency_key_required',
    })
    expect(store.displayNameUpdates).toHaveLength(0)
  })

  test('returns 404 when the agent user row is not updatable', async () => {
    const store = new MemoryAgentStore({
      lookup: agentLookup(),
      token,
      updatable: false,
    })

    const response = await handleProgrammaticAgentSelfUpdate(
      renameRequest({ displayName: 'Renamed Agent' }),
      {} as D1Database,
      stubIdentityDb,
      { agentStore: store, nowIso: () => '2026-06-18T00:00:00.000Z' },
    )

    expect(response.status).toBe(404)
    await expect(response.json()).resolves.toMatchObject({
      error: 'agent_not_found',
    })
  })

  test('rejects non-PATCH methods with 405 advertising GET and PATCH', async () => {
    const store = new MemoryAgentStore({ lookup: agentLookup(), token })

    const response = await handleProgrammaticAgentSelfUpdate(
      new Request('https://openagents.com/api/agents/me', {
        headers: { Authorization: `Bearer ${token}` },
        method: 'PUT',
      }),
      {} as D1Database,
      stubIdentityDb,
      { agentStore: store },
    )

    expect(response.status).toBe(405)
    expect(response.headers.get('allow')).toBe('GET, PATCH')
  })
})
