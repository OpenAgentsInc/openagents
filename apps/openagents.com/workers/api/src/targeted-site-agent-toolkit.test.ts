import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  TargetedSiteAgentToolkitForbidden,
  TargetedSiteAgentToolkitValidationError,
  agentToolkitActionContract,
  createTargetedSiteAgentToolkitGrant,
  publicTargetedSiteAgentToolkitActionProjection,
  recordTargetedSiteAgentToolkitAction,
} from './targeted-site-agent-toolkit'

type Campaign = Readonly<{
  archived_at: string | null
  id: string
  owner_user_id: string
}>

type Grant = Readonly<{
  agent_ref: string
  approval_policy: 'operator_approval' | 'owner_approval' | 'auto_dry_run_only'
  archived_at: string | null
  campaign_id: string
  created_at: string
  daily_send_cap: number
  dry_run_default: number
  expires_at: string | null
  id: string
  idempotency_key: string
  metadata_json: string
  owner_user_id: string
  revoked_at: string | null
  scopes_json: string
  spend_cap_cents: number
  status: 'active' | 'revoked' | 'expired'
  suppression_policy_ref: string | null
  updated_at: string
}>

type Action = Readonly<{
  action_kind:
    | 'discover_prospects'
    | 'capture_site'
    | 'audit_site'
    | 'generate_preview'
    | 'send_outreach_request'
    | 'record_metric'
    | 'propose_reward'
  agent_ref: string
  approval_state: 'not_required' | 'requested' | 'approved' | 'rejected'
  archived_at: string | null
  campaign_id: string
  created_at: string
  dry_run: number
  grant_id: string
  id: string
  idempotency_key: string
  metadata_json: string
  reason: string | null
  receipt_ref: string
  requested_cost_cents: number
  requested_send_count: number
  result_state: 'accepted' | 'blocked' | 'rejected'
  suppression_state: 'unknown' | 'clear' | 'suppressed' | 'manual_review'
}>

class AgentToolkitStore {
  actions: Array<Action> = []
  campaigns: Array<Campaign> = [
    {
      archived_at: null,
      id: 'targeted_site_campaign_1',
      owner_user_id: 'user_owner',
    },
    {
      archived_at: '2026-06-05T21:00:00.000Z',
      id: 'targeted_site_campaign_archived',
      owner_user_id: 'user_owner',
    },
  ]
  grants: Array<Grant> = []
}

const runtime = {
  makeActionId: () => 'targeted_site_agent_action_generated',
  makeGrantId: () => 'targeted_site_agent_grant_generated',
  nowIso: () => '2026-06-05T21:10:00.000Z',
}

class AgentToolkitStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: AgentToolkitStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM targeted_site_campaigns')) {
      const campaignId = String(this.values[0])
      const campaign =
        this.store.campaigns.find(
          item => item.id === campaignId && item.archived_at === null,
        ) ?? null

      return Promise.resolve(campaign as T | null)
    }

    if (this.query.includes('FROM targeted_site_agent_toolkit_grants')) {
      return Promise.resolve(this.findGrant() as T | null)
    }

    if (this.query.includes('FROM targeted_site_agent_toolkit_actions')) {
      if (this.query.includes('SUM(requested_send_count)')) {
        return Promise.resolve({ used_send_count: this.usedSendCount() } as T)
      }

      return Promise.resolve(this.findAction() as T | null)
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (
      this.query.includes(
        'INSERT OR IGNORE INTO targeted_site_agent_toolkit_grants',
      )
    ) {
      const idempotencyKey = String(this.values[1])

      if (
        this.store.grants.every(item => item.idempotency_key !== idempotencyKey)
      ) {
        this.store.grants.push({
          agent_ref: String(this.values[4]),
          approval_policy: this.values[10] as Grant['approval_policy'],
          archived_at: null,
          campaign_id: String(this.values[2]),
          created_at: String(this.values[13]),
          daily_send_cap: Number(this.values[8]),
          dry_run_default: Number(this.values[6]),
          expires_at: this.values[14] as string | null,
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          metadata_json: String(this.values[11]),
          owner_user_id: String(this.values[3]),
          revoked_at: null,
          scopes_json: String(this.values[5]),
          spend_cap_cents: Number(this.values[7]),
          status: 'active',
          suppression_policy_ref: this.values[9] as string | null,
          updated_at: String(this.values[13]),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (
      this.query.includes(
        'INSERT OR IGNORE INTO targeted_site_agent_toolkit_actions',
      )
    ) {
      const idempotencyKey = String(this.values[1])

      if (
        this.store.actions.every(
          item => item.idempotency_key !== idempotencyKey,
        )
      ) {
        this.store.actions.push({
          action_kind: this.values[5] as Action['action_kind'],
          agent_ref: String(this.values[4]),
          approval_state: this.values[10] as Action['approval_state'],
          archived_at: null,
          campaign_id: String(this.values[3]),
          created_at: String(this.values[15]),
          dry_run: Number(this.values[6]),
          grant_id: String(this.values[2]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          metadata_json: String(this.values[14]),
          reason: this.values[13] as string | null,
          receipt_ref: String(this.values[12]),
          requested_cost_cents: Number(this.values[7]),
          requested_send_count: Number(this.values[8]),
          result_state: this.values[11] as Action['result_state'],
          suppression_state: this.values[9] as Action['suppression_state'],
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    return Promise.resolve({ results: [] } as unknown as D1Result<T>)
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(options?: {
    columnNames?: boolean
  }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
    return options?.columnNames === true ? Promise.resolve([[]]) : Promise.resolve([])
  }

  private findGrant(): Grant | null {
    if (this.query.includes('idempotency_key = ?')) {
      const idempotencyKey = String(this.values[0])

      return (
        this.store.grants.find(
          item =>
            item.idempotency_key === idempotencyKey &&
            item.archived_at === null,
        ) ?? null
      )
    }

    const grantId = String(this.values[0])

    return (
      this.store.grants.find(
        item =>
          item.id === grantId &&
          item.status === 'active' &&
          item.archived_at === null,
      ) ?? null
    )
  }

  private findAction(): Action | null {
    const idempotencyKey = String(this.values[0])

    return (
      this.store.actions.find(
        item =>
          item.idempotency_key === idempotencyKey &&
          item.archived_at === null,
      ) ?? null
    )
  }

  private usedSendCount(): number {
    const grantId = String(this.values[0])
    const dayStart = String(this.values[1])

    return this.store.actions
      .filter(
        item =>
          item.grant_id === grantId &&
          item.result_state === 'accepted' &&
          item.created_at >= dayStart &&
          item.archived_at === null,
      )
      .reduce((sum, item) => sum + item.requested_send_count, 0)
  }
}

const agentToolkitDb = (store: AgentToolkitStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new AgentToolkitStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const createGrant = (
  store: AgentToolkitStore,
  overrides: Partial<Parameters<typeof createTargetedSiteAgentToolkitGrant>[1]> = {},
) =>
  Effect.runPromise(
    createTargetedSiteAgentToolkitGrant(
      agentToolkitDb(store),
      {
        agentRef: 'agent_site_builder_1',
        campaignId: 'targeted_site_campaign_1',
        dailySendCap: 2,
        id: 'targeted_site_agent_grant_1',
        idempotencyKey: 'grant:agent:1',
        ownerUserId: 'user_owner',
        scopes: [
          'campaign:discover',
          'campaign:capture',
          'campaign:audit',
          'campaign:outreach:request',
        ],
        spendCapCents: 500,
        ...overrides,
      },
      runtime,
    ),
  )

describe('targeted Site agent toolkit', () => {
  test('creates scoped grants idempotently and exposes action contracts', async () => {
    const store = new AgentToolkitStore()
    const grant = await createGrant(store)
    const replay = await createGrant(store, { agentRef: 'agent_changed' })
    const contract = agentToolkitActionContract(grant)

    expect(replay.agentRef).toBe('agent_site_builder_1')
    expect(contract).toMatchObject({
      approvalPolicy: 'auto_dry_run_only',
      campaignId: 'targeted_site_campaign_1',
      dailySendCap: 2,
      dryRunDefault: true,
      grantId: 'targeted_site_agent_grant_1',
      spendCapCents: 500,
    })
    expect(contract.scopes).toContain('campaign:outreach:request')
    expect(store.grants).toHaveLength(1)
  })

  test('enforces owner/admin grant authority', async () => {
    await expect(
      Effect.runPromise(
        createTargetedSiteAgentToolkitGrant(
          agentToolkitDb(new AgentToolkitStore()),
          {
            agentRef: 'agent_site_builder_1',
            campaignId: 'targeted_site_campaign_1',
            idempotencyKey: 'grant:forbidden',
            ownerUserId: 'user_other',
            scopes: ['campaign:discover'],
          },
          runtime,
        ),
      ),
    ).rejects.toBeInstanceOf(TargetedSiteAgentToolkitForbidden)

    const store = new AgentToolkitStore()
    const adminGrant = await createGrant(store, {
      idempotencyKey: 'grant:admin',
      isAdmin: true,
      ownerUserId: 'user_other',
    })

    expect(adminGrant.ownerUserId).toBe('user_owner')
  })

  test('records accepted dry-run actions idempotently and redacts projection', async () => {
    const store = new AgentToolkitStore()
    const grant = await createGrant(store)
    const action = await Effect.runPromise(
      recordTargetedSiteAgentToolkitAction(
        agentToolkitDb(store),
        {
          actionKind: 'send_outreach_request',
          grantId: grant.id,
          id: 'targeted_site_agent_action_1',
          idempotencyKey: 'action:outreach:1',
          requestedCostCents: 50,
          requestedSendCount: 1,
        },
        runtime,
      ),
    )
    const replay = await Effect.runPromise(
      recordTargetedSiteAgentToolkitAction(
        agentToolkitDb(store),
        {
          actionKind: 'send_outreach_request',
          grantId: grant.id,
          idempotencyKey: 'action:outreach:1',
        },
        runtime,
      ),
    )
    const projection = publicTargetedSiteAgentToolkitActionProjection(action)

    expect(action).toMatchObject({
      approvalState: 'not_required',
      dryRun: true,
      receiptRef:
        'targeted_site_agent_toolkit:send_outreach_request:action:outreach:1',
      resultState: 'accepted',
    })
    expect(replay.requestedSendCount).toBe(1)
    expect(projection).toEqual({
      actionKind: 'send_outreach_request',
      approvalState: 'not_required',
      campaignId: 'targeted_site_campaign_1',
      createdAt: '2026-06-05T21:10:00.000Z',
      dryRun: true,
      receiptRef:
        'targeted_site_agent_toolkit:send_outreach_request:action:outreach:1',
      resultState: 'accepted',
    })
    expect(projection).not.toHaveProperty('agentRef')
    expect(projection).not.toHaveProperty('metadata')
    expect(projection).not.toHaveProperty('reason')
  })

  test('rejects missing scope and blocks caps and suppression', async () => {
    const store = new AgentToolkitStore()
    const grant = await createGrant(store, {
      scopes: ['campaign:discover'],
    })
    const rejected = await Effect.runPromise(
      recordTargetedSiteAgentToolkitAction(
        agentToolkitDb(store),
        {
          actionKind: 'send_outreach_request',
          grantId: grant.id,
          idempotencyKey: 'action:missing-scope',
        },
        runtime,
      ),
    )
    const spendBlocked = await Effect.runPromise(
      recordTargetedSiteAgentToolkitAction(
        agentToolkitDb(store),
        {
          actionKind: 'discover_prospects',
          grantId: grant.id,
          idempotencyKey: 'action:spend-cap',
          requestedCostCents: 501,
        },
        runtime,
      ),
    )
    const suppressionBlocked = await Effect.runPromise(
      recordTargetedSiteAgentToolkitAction(
        agentToolkitDb(store),
        {
          actionKind: 'discover_prospects',
          grantId: grant.id,
          idempotencyKey: 'action:suppressed',
          suppressionState: 'suppressed',
        },
        runtime,
      ),
    )

    expect(rejected).toMatchObject({
      reason: 'missing required scope campaign:outreach:request',
      resultState: 'rejected',
    })
    expect(spendBlocked).toMatchObject({
      reason: 'requested cost exceeds spend cap',
      resultState: 'blocked',
    })
    expect(suppressionBlocked).toMatchObject({
      reason: 'suppression state is suppressed',
      resultState: 'blocked',
    })
  })

  test('enforces daily send caps using accepted same-day sends', async () => {
    const store = new AgentToolkitStore()
    const grant = await createGrant(store)

    await Effect.runPromise(
      recordTargetedSiteAgentToolkitAction(
        agentToolkitDb(store),
        {
          actionKind: 'send_outreach_request',
          grantId: grant.id,
          idempotencyKey: 'action:first-send',
          requestedSendCount: 2,
        },
        runtime,
      ),
    )

    const blocked = await Effect.runPromise(
      recordTargetedSiteAgentToolkitAction(
        agentToolkitDb(store),
        {
          actionKind: 'send_outreach_request',
          grantId: grant.id,
          idempotencyKey: 'action:second-send',
          requestedSendCount: 1,
        },
        runtime,
      ),
    )

    expect(blocked).toMatchObject({
      reason: 'requested sends exceed daily send cap',
      resultState: 'blocked',
    })
  })

  test('requires approval for non-dry-run owner approval grants', async () => {
    const store = new AgentToolkitStore()
    const grant = await createGrant(store, {
      approvalPolicy: 'owner_approval',
      dryRunDefault: false,
      idempotencyKey: 'grant:owner-approval',
      spendCapCents: 500,
    })
    const requested = await Effect.runPromise(
      recordTargetedSiteAgentToolkitAction(
        agentToolkitDb(store),
        {
          actionKind: 'capture_site',
          grantId: grant.id,
          idempotencyKey: 'action:approval-requested',
          requestedCostCents: 100,
        },
        runtime,
      ),
    )
    const approved = await Effect.runPromise(
      recordTargetedSiteAgentToolkitAction(
        agentToolkitDb(store),
        {
          actionKind: 'capture_site',
          approvalState: 'approved',
          grantId: grant.id,
          idempotencyKey: 'action:approval-approved',
          requestedCostCents: 100,
        },
        runtime,
      ),
    )

    expect(requested).toMatchObject({
      approvalState: 'requested',
      reason: 'non-dry-run action requires approval',
      resultState: 'blocked',
    })
    expect(approved).toMatchObject({
      approvalState: 'approved',
      reason: null,
      resultState: 'accepted',
    })
  })

  test('rejects private provider, email, payment, and wallet material', async () => {
    const store = new AgentToolkitStore()
    await expect(
      Effect.runPromise(
        createTargetedSiteAgentToolkitGrant(
          agentToolkitDb(store),
          {
            agentRef: 'agent_site_builder_1',
            campaignId: 'targeted_site_campaign_1',
            idempotencyKey: 'grant:private-material',
            metadata: { rawEmail: 'ben@example.com' },
            ownerUserId: 'user_owner',
            scopes: ['campaign:discover'],
          },
          runtime,
        ),
      ),
    ).rejects.toBeInstanceOf(TargetedSiteAgentToolkitValidationError)
  })
})
