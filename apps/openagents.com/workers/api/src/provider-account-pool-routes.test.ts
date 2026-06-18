import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  AGENT_TOKEN_PREFIX,
  type AgentCredentialLookup,
  type AgentRegistrationStore,
  sha256Hex,
} from './agent-registration'
import {
  type ProviderAccountPoolResponse,
  makeProviderAccountPoolRoutes,
} from './provider-account-pool-routes'

const NOW = '2026-06-11T12:00:00.000Z'
const OWNER_USER_ID = 'github:1001'

type FakeAccountRow = {
  id: string
  user_id: string
  provider: string
  provider_account_ref: string
  account_label: string | null
  operator_label: string | null
  status: string
  health: string
  secret_ref: string | null
  deleted_at: string | null
  low_credit_flag: number
  cooldown_until: string | null
  lease_limit: number
  operator_priority: number
  connected_at: string | null
  created_at: string
  last_selected_at: string | null
  last_sanity_check_at: string | null
  last_sanity_check_result: string | null
  last_parallel_probe_at: string | null
  last_parallel_probe_result: string | null
  last_successful_launch_at: string | null
  last_failed_launch_at: string | null
  recent_failure_class: string | null
  reauth_required_reason: string | null
}

type FakeLeaseRow = {
  lease_ref: string
  provider_account_id: string
  provider_account_ref: string
  requested_action: string
  run_id: string | null
  assignment_id: string | null
  order_id: string | null
  user_id: string
  status: string
  started_at: string
  expires_at: string
  last_touched_at: string | null
}

class FakeD1Statement {
  constructor(
    private readonly database: FakePoolD1,
    private readonly query: string,
    private readonly values: Array<unknown> = [],
  ) {}

  bind = (...values: Array<unknown>) =>
    new FakeD1Statement(this.database, this.query, values)

  first = <T>(): Promise<T | null> =>
    Promise.resolve((this.database.first(this.query, this.values) as T) ?? null)

  all = <T>(): Promise<{ results: Array<T> }> =>
    Promise.resolve({
      results: this.database.all(this.query, this.values) as Array<T>,
    })

  run = () => Promise.resolve({ success: true })
}

class FakePoolD1 {
  accounts: Array<FakeAccountRow> = []
  leases: Array<FakeLeaseRow> = []

  asD1 = (): D1Database =>
    ({
      batch: (statements: Array<{ run: () => Promise<unknown> }>) =>
        Promise.all(statements.map(statement => statement.run())),
      prepare: (query: string) => new FakeD1Statement(this, query),
    }) as unknown as D1Database

  private activeLeaseCount = (accountId: string, now: string): number =>
    this.leases.filter(
      lease =>
        lease.provider_account_id === accountId &&
        lease.status === 'active' &&
        lease.expires_at > now,
    ).length

  first = (query: string, values: Array<unknown>): unknown => {
    if (
      query.includes('SELECT pa.provider_account_ref') &&
      query.includes('LIMIT 1')
    ) {
      const [now, userId] = values as [string, string]
      const selected = this.accounts
        .filter(
          account =>
            account.user_id === userId &&
            account.status === 'connected' &&
            account.health === 'healthy' &&
            account.secret_ref !== null &&
            account.deleted_at === null &&
            account.low_credit_flag === 0 &&
            account.reauth_required_reason === null &&
            (account.cooldown_until === null ||
              account.cooldown_until <= now) &&
            this.activeLeaseCount(account.id, now) < account.lease_limit,
        )
        .sort(
          (left, right) =>
            this.activeLeaseCount(left.id, now) -
              this.activeLeaseCount(right.id, now) ||
            left.provider.localeCompare(right.provider) ||
            left.operator_priority - right.operator_priority ||
            (
              left.last_selected_at ??
              left.connected_at ??
              left.created_at
            ).localeCompare(
              right.last_selected_at ?? right.connected_at ?? right.created_at,
            ) ||
            left.provider_account_ref.localeCompare(right.provider_account_ref),
        )[0]

      if (selected === undefined) {
        return null
      }

      return {
        provider_account_ref: selected.provider_account_ref,
        provider: selected.provider,
        account_label: selected.operator_label ?? selected.account_label,
        active_lease_count: this.activeLeaseCount(selected.id, NOW),
        lease_limit: selected.lease_limit,
        operator_priority: selected.operator_priority,
      }
    }

    return null
  }

  all = (query: string, values: Array<unknown>): Array<unknown> => {
    if (query.includes('FROM provider_accounts pa')) {
      const [now, userId] = values as [string, string]

      return this.accounts
        .filter(
          account => account.user_id === userId && account.deleted_at === null,
        )
        .map(account => ({
          provider_account_ref: account.provider_account_ref,
          provider: account.provider,
          account_label: account.account_label,
          operator_label: account.operator_label,
          status: account.status,
          health: account.health,
          operator_priority: account.operator_priority,
          lease_limit: account.lease_limit,
          low_credit_flag: account.low_credit_flag,
          cooldown_until: account.cooldown_until,
          recent_failure_class: account.recent_failure_class,
          last_sanity_check_at: account.last_sanity_check_at,
          last_sanity_check_result: account.last_sanity_check_result,
          last_parallel_probe_at: account.last_parallel_probe_at,
          last_parallel_probe_result: account.last_parallel_probe_result,
          last_selected_at: account.last_selected_at,
          last_successful_launch_at: account.last_successful_launch_at,
          last_failed_launch_at: account.last_failed_launch_at,
          reauth_required_reason: account.reauth_required_reason,
          connected_at: account.connected_at,
          deleted_at: account.deleted_at,
          has_secret_ref: account.secret_ref === null ? 0 : 1,
          active_lease_count: this.activeLeaseCount(account.id, now),
        }))
    }

    if (query.includes('FROM provider_account_leases l')) {
      const [userId, now] = values as [string, string]

      return this.leases
        .filter(
          lease =>
            lease.user_id === userId &&
            lease.status === 'active' &&
            lease.expires_at > now,
        )
        .map(lease => {
          const account = this.accounts.find(
            candidate => candidate.id === lease.provider_account_id,
          )

          return {
            lease_ref: lease.lease_ref,
            provider_account_ref: lease.provider_account_ref,
            provider: account?.provider ?? 'chatgpt_codex',
            account_label:
              account?.operator_label ?? account?.account_label ?? null,
            requested_action: lease.requested_action,
            run_id: lease.run_id,
            assignment_id: lease.assignment_id,
            order_id: lease.order_id,
            started_at: lease.started_at,
            expires_at: lease.expires_at,
            last_touched_at: lease.last_touched_at,
            status: lease.status,
          }
        })
    }

    return []
  }
}

const accountRow = (
  overrides: Partial<FakeAccountRow> & Pick<FakeAccountRow, 'id'>,
): FakeAccountRow => ({
  user_id: OWNER_USER_ID,
  provider: 'chatgpt_codex',
  provider_account_ref: `provider-account_${overrides.id}`,
  account_label: `${overrides.id}@example.com`,
  operator_label: null,
  status: 'connected',
  health: 'healthy',
  secret_ref: `provider_secret_${overrides.id}`,
  deleted_at: null,
  low_credit_flag: 0,
  cooldown_until: null,
  lease_limit: 2,
  operator_priority: 100,
  connected_at: '2026-06-10T00:00:00.000Z',
  created_at: '2026-06-09T00:00:00.000Z',
  last_selected_at: null,
  last_sanity_check_at: null,
  last_sanity_check_result: null,
  last_parallel_probe_at: null,
  last_parallel_probe_result: null,
  last_successful_launch_at: null,
  last_failed_launch_at: null,
  recent_failure_class: null,
  reauth_required_reason: null,
  ...overrides,
})

class MemoryAgentStore implements AgentRegistrationStore {
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

  touchAgentCredential = async () => {}


  updateAgentDisplayName = async () => 0
}

const agentLookup = (
  metadata: Record<string, unknown> = {},
): AgentCredentialLookup => ({
  credentialId: 'agent_credential_pool',
  profileMetadataJson: JSON.stringify(metadata),
  tokenPrefix: `${AGENT_TOKEN_PREFIX}pool`,
  user: {
    avatarUrl: null,
    createdAt: '2026-06-05T00:00:00.000Z',
    displayName: 'Pool Test Agent',
    id: 'agent_pool_user',
    kind: 'agent',
    primaryEmail: null,
    status: 'active',
    updatedAt: '2026-06-05T00:00:00.000Z',
  },
})

type PoolSession = Readonly<{ user: Readonly<{ userId: string }> }>

const makeHandler = (input: {
  database: FakePoolD1
  session?: PoolSession | undefined
  agentStore?: AgentRegistrationStore
}) => {
  const env = { OPENAGENTS_DB: input.database.asD1() }
  const routes = makeProviderAccountPoolRoutes<PoolSession, typeof env>({
    agentStore: () =>
      input.agentStore ??
      new MemoryAgentStore({ token: `${AGENT_TOKEN_PREFIX}unused` }),
    appendRefreshedSessionCookies: response => response,
    nowIso: () => NOW,
    requireBrowserSession: () => Promise.resolve(input.session),
  })

  return (request: Request) =>
    Effect.runPromise(
      routes.handleProviderAccountPoolApi(request, env, {} as ExecutionContext),
    )
}

const poolRequest = (headers: Record<string, string> = {}): Request =>
  new Request('https://openagents.com/api/provider-accounts/pool', {
    headers,
    method: 'GET',
  })

const seededDatabase = (): FakePoolD1 => {
  const database = new FakePoolD1()

  database.accounts = [
    accountRow({ id: 'acct_healthy' }),
    accountRow({
      id: 'acct_anthropic',
      provider: 'anthropic_claude',
      account_label: 'claude-api-key',
      operator_priority: 110,
    }),
    accountRow({
      id: 'acct_cooling',
      cooldown_until: '2026-06-11T12:05:00.000Z',
      recent_failure_class: 'rate_limited',
      last_failed_launch_at: '2026-06-11T11:50:00.000Z',
    }),
    accountRow({
      id: 'acct_low_credit',
      low_credit_flag: 1,
      recent_failure_class: 'low_credits',
    }),
    accountRow({
      id: 'acct_expired',
      status: 'expired',
      health: 'requires_reauth',
      secret_ref: null,
      reauth_required_reason: 'token_invalidated',
    }),
  ]
  database.leases = [
    {
      lease_ref: 'provider-account-lease_ref_1',
      provider_account_id: 'acct_healthy',
      provider_account_ref: 'provider-account_acct_healthy',
      requested_action: 'autopilot_coder_run',
      run_id: 'run_1',
      assignment_id: null,
      order_id: 'autopilot_work_order.1',
      user_id: OWNER_USER_ID,
      status: 'active',
      started_at: '2026-06-11T11:30:00.000Z',
      expires_at: '2026-06-11T12:30:00.000Z',
      last_touched_at: '2026-06-11T11:45:00.000Z',
    },
  ]

  return database
}

describe('provider account pool projection route', () => {
  test('browser session reads the account-pool dashboard projection', async () => {
    const handler = makeHandler({
      database: seededDatabase(),
      session: { user: { userId: OWNER_USER_ID } },
    })
    const response = await handler(poolRequest())
    const body = (await response.json()) as ProviderAccountPoolResponse

    expect(response.status).toBe(200)
    expect(body.generatedAt).toBe(NOW)
    expect(body.provider).toBe('all_connected_provider_accounts')
    expect(body.policyVersion).toBe('provider-account-lease-policy:v2')
    expect(body.staleness).toMatchObject({
      composition: 'live_at_read',
      contractVersion: 'projection_staleness.v1',
      maxStalenessSeconds: 0,
    })
    expect(body.staleness.rebuildsOn).toContain(
      'provider_account_lease_acquired',
    )

    expect(body.accounts).toHaveLength(5)
    expect(body.summary).toEqual({
      total: 5,
      eligible: 2,
      activeLeaseCount: 1,
      lowCredit: 1,
      requiresReauth: 1,
      cooldown: 1,
      unhealthy: 0,
    })

    const anthropic = body.accounts.find(
      account =>
        account.providerAccountRef === 'provider-account_acct_anthropic',
    )
    expect(anthropic).toMatchObject({
      provider: 'anthropic_claude',
      accountLabel: 'claude-api-key',
      eligibility: 'eligible',
    })

    const healthy = body.accounts.find(
      account => account.providerAccountRef === 'provider-account_acct_healthy',
    )
    expect(healthy).toMatchObject({
      eligibility: 'eligible',
      eligibilityReasons: [],
      activeLeaseCount: 1,
      leaseLimit: 2,
      lowCredit: false,
      reconnect: { needed: false, reason: null },
    })

    const cooling = body.accounts.find(
      account => account.providerAccountRef === 'provider-account_acct_cooling',
    )
    expect(cooling).toMatchObject({
      eligibility: 'ineligible',
      eligibilityReasons: ['cooldown'],
      cooldownUntil: '2026-06-11T12:05:00.000Z',
      cooldownRemainingSeconds: 300,
      recentFailureClass: 'rate_limited',
      reconnect: { needed: false, reason: null },
    })

    const lowCredit = body.accounts.find(
      account =>
        account.providerAccountRef === 'provider-account_acct_low_credit',
    )
    expect(lowCredit).toMatchObject({
      eligibility: 'ineligible',
      eligibilityReasons: ['low_credit'],
      lowCredit: true,
    })

    const expired = body.accounts.find(
      account => account.providerAccountRef === 'provider-account_acct_expired',
    )
    expect(expired).toMatchObject({
      eligibility: 'ineligible',
      reconnect: { needed: true, reason: 'requires_reauth' },
    })
    expect(expired?.eligibilityReasons).toEqual(
      expect.arrayContaining([
        'status:expired',
        'health:requires_reauth',
        'missing_server_auth_material',
        'reauth_required:token_invalidated',
      ]),
    )

    expect(body.activeLeases).toHaveLength(1)
    expect(body.activeLeases[0]).toMatchObject({
      leaseRef: 'provider-account-lease_ref_1',
      providerAccountRef: 'provider-account_acct_healthy',
      requestedAction: 'autopilot_coder_run',
      runId: 'run_1',
      status: 'active',
    })

    expect(body.nextSelection).toMatchObject({
      status: 'selected',
      providerAccountRef: 'provider-account_acct_anthropic',
      provider: 'anthropic_claude',
    })
  })

  test('projection never exposes provider secret refs or token material', async () => {
    const handler = makeHandler({
      database: seededDatabase(),
      session: { user: { userId: OWNER_USER_ID } },
    })
    const response = await handler(poolRequest())
    const raw = await response.text()

    expect(response.status).toBe(200)
    expect(raw).not.toContain('provider_secret_')
    expect(raw).not.toContain('secretRef')
    expect(raw).not.toContain('secret_ref')
  })

  test('registered agent with customer_orders.read grant reads the owner pool', async () => {
    const token = `${AGENT_TOKEN_PREFIX}pool_token_1`
    const handler = makeHandler({
      database: seededDatabase(),
      agentStore: new MemoryAgentStore({
        lookup: agentLookup({
          customerOrderGrants: [
            {
              expiresAt: null,
              ownerUserId: OWNER_USER_ID,
              scopes: ['customer_orders.read'],
              status: 'active',
            },
          ],
        }),
        token,
      }),
    })
    const response = await handler(
      poolRequest({ authorization: `Bearer ${token}` }),
    )
    const body = (await response.json()) as ProviderAccountPoolResponse

    expect(response.status).toBe(200)
    expect(body.generatedAt).toBe(NOW)
    expect(body.accounts).toHaveLength(5)
    expect(body.summary.eligible).toBe(2)
  })

  test('registered agent without an owner grant is refused', async () => {
    const token = `${AGENT_TOKEN_PREFIX}pool_token_2`
    const handler = makeHandler({
      database: seededDatabase(),
      agentStore: new MemoryAgentStore({
        lookup: agentLookup({}),
        token,
      }),
    })
    const response = await handler(
      poolRequest({ authorization: `Bearer ${token}` }),
    )

    expect(response.status).toBe(403)
  })

  test('anonymous browser request is unauthorized', async () => {
    const handler = makeHandler({ database: seededDatabase() })
    const response = await handler(poolRequest())

    expect(response.status).toBe(401)
  })

  test('non-GET methods are rejected', async () => {
    const handler = makeHandler({
      database: seededDatabase(),
      session: { user: { userId: OWNER_USER_ID } },
    })
    const response = await handler(
      new Request('https://openagents.com/api/provider-accounts/pool', {
        method: 'POST',
      }),
    )

    expect(response.status).toBe(405)
  })
})
