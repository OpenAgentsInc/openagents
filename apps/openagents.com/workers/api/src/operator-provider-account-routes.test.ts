import { describe, expect, test } from 'vitest'

import {
  type ProviderAccountSanityClassification,
  type ProviderAccountSanityProbeResult,
  makeOperatorProviderAccountRoutes,
} from './operator-provider-account-routes'
import type { OperatorTargetUser } from './operator-targets'
import type {
  ProviderAccountAuthGrantRecord,
  ProviderAccountEventRecord,
  ProviderAccountRecord,
  ProviderAccountRepository,
  ProviderConnectionAttemptRecord,
} from './provider-account-domain'

const targetUser: OperatorTargetUser = {
  userId: 'user_chris',
  displayName: 'Chris',
  email: 'chris@openagents.com',
  githubUsername: 'OV1-Kenobi',
}

class FakeProviderAccountRepository implements ProviderAccountRepository {
  accounts: Array<ProviderAccountRecord> = []
  attempts: Array<ProviderConnectionAttemptRecord> = []
  events: Array<ProviderAccountEventRecord> = []
  grants: Array<ProviderAccountAuthGrantRecord> = []

  findAccountByRef = (
    userId: string,
    providerAccountRef: string,
  ): Promise<ProviderAccountRecord | undefined> =>
    Promise.resolve(
      this.accounts.find(
        account =>
          account.userId === userId &&
          account.providerAccountRef === providerAccountRef &&
          account.deletedAt === null,
      ),
    )

  findAccountByProviderAccountRef = (
    providerAccountRef: string,
  ): Promise<ProviderAccountRecord | undefined> =>
    Promise.resolve(
      this.accounts.find(
        account =>
          account.providerAccountRef === providerAccountRef &&
          account.deletedAt === null,
      ),
    )

  findReusableAccount = (
    userId: string,
  ): Promise<ProviderAccountRecord | undefined> =>
    Promise.resolve(
      this.accounts.find(
        account =>
          account.userId === userId &&
          account.status !== 'connected' &&
          account.deletedAt === null,
      ),
    )

  listAccountsForUser = (
    userId: string,
  ): Promise<ReadonlyArray<ProviderAccountRecord>> =>
    Promise.resolve(this.accounts.filter(account => account.userId === userId))

  listPendingAttemptsForUser = (
    userId: string,
  ): Promise<ReadonlyArray<ProviderConnectionAttemptRecord>> =>
    Promise.resolve(
      this.attempts.filter(
        attempt => attempt.userId === userId && attempt.status === 'pending',
      ),
    )

  findAttemptForUser = (
    userId: string,
    attemptId: string,
  ): Promise<
    | Readonly<{
        account: ProviderAccountRecord
        attempt: ProviderConnectionAttemptRecord
      }>
    | undefined
  > =>
    Promise.resolve(
      this.recordForAttempt(
        this.attempts.find(
          attempt => attempt.userId === userId && attempt.id === attemptId,
        ),
      ),
    )

  findAttemptById = (attemptId: string) =>
    Promise.resolve(
      this.recordForAttempt(
        this.attempts.find(attempt => attempt.id === attemptId),
      ),
    )

  saveStartedDeviceLogin = (
    account: ProviderAccountRecord,
    attempt: ProviderConnectionAttemptRecord,
    event: ProviderAccountEventRecord,
    accountAlreadyExists: boolean,
  ): Promise<void> => {
    if (accountAlreadyExists) {
      this.accounts = this.accounts.map(candidate =>
        candidate.id === account.id ? account : candidate,
      )
    } else {
      this.accounts.push(account)
    }

    this.attempts.push(attempt)
    this.events.push(event)

    return Promise.resolve()
  }

  recordConnectedAttempt = (
    account: ProviderAccountRecord,
    attempt: ProviderConnectionAttemptRecord,
    event: ProviderAccountEventRecord,
  ): Promise<ProviderAccountRecord> => {
    this.accounts = this.accounts.map(candidate =>
      candidate.id === account.id ? account : candidate,
    )
    this.attempts = this.attempts.map(candidate =>
      candidate.id === attempt.id ? attempt : candidate,
    )
    this.events.push(event)

    return Promise.resolve(account)
  }

  recordFailedAttempt = (
    account: ProviderAccountRecord,
    attempt: ProviderConnectionAttemptRecord,
    event: ProviderAccountEventRecord,
  ): Promise<ProviderAccountRecord> => {
    this.accounts = this.accounts.map(candidate =>
      candidate.id === account.id ? account : candidate,
    )
    this.attempts = this.attempts.map(candidate =>
      candidate.id === attempt.id ? attempt : candidate,
    )
    this.events.push(event)

    return Promise.resolve(account)
  }

  recordAccountHealth = (
    providerAccountRef: string,
    account: ProviderAccountRecord,
    event: ProviderAccountEventRecord,
  ): Promise<ProviderAccountRecord | undefined> => {
    const existing = this.accounts.find(
      candidate => candidate.providerAccountRef === providerAccountRef,
    )

    if (existing === undefined) {
      return Promise.resolve(undefined)
    }

    this.accounts = this.accounts.map(candidate =>
      candidate.providerAccountRef === providerAccountRef ? account : candidate,
    )
    this.events.push(event)

    return Promise.resolve(account)
  }

  createAuthGrant = (
    grant: ProviderAccountAuthGrantRecord,
    event: ProviderAccountEventRecord,
  ): Promise<ProviderAccountAuthGrantRecord> => {
    this.grants.push(grant)
    this.events.push(event)

    return Promise.resolve(grant)
  }

  findGrantByRef = (
    grantRef: string,
  ): Promise<ProviderAccountAuthGrantRecord | undefined> =>
    Promise.resolve(this.grants.find(grant => grant.grantRef === grantRef))

  markGrantUsed = (
    grant: ProviderAccountAuthGrantRecord,
    event: ProviderAccountEventRecord,
  ): Promise<ProviderAccountAuthGrantRecord> => {
    this.grants = this.grants.map(candidate =>
      candidate.id === grant.id ? grant : candidate,
    )
    this.events.push(event)

    return Promise.resolve(grant)
  }

  disconnectAccount = (): Promise<ProviderAccountRecord | undefined> =>
    Promise.resolve(undefined)

  private recordForAttempt = (
    attempt: ProviderConnectionAttemptRecord | undefined,
  ):
    | Readonly<{
        account: ProviderAccountRecord
        attempt: ProviderConnectionAttemptRecord
      }>
    | undefined => {
    if (attempt === undefined) {
      return undefined
    }

    const account = this.accounts.find(
      candidate => candidate.id === attempt.providerAccountId,
    )

    return account === undefined ? undefined : { account, attempt }
  }
}

const kv = (): KVNamespace => ({}) as KVNamespace

const db = (): D1Database =>
  ({
    batch: () => Promise.resolve([]),
    prepare: () => ({
      bind: () => ({
        run: () => Promise.resolve({ success: true }),
      }),
    }),
  }) as unknown as D1Database

type FakeLeaseRow = {
  lease_ref: string
  provider?: string
  provider_account_id: string
  provider_account_ref: string
  requested_action: string
  run_id: string | null
  assignment_id: string | null
  order_id: string | null
  user_id: string
  status: string
  expires_at: string
}

type FakeAccountRow = {
  id: string
  user_id: string
  provider_account_ref: string
  account_label: string | null
  operator_label: string | null
  provider: string
  status: string
  health: string
  secret_ref: string | null
  deleted_at: string | null
  low_credit_flag: number
  cooldown_until: string | null
  lease_limit: number
  operator_priority: number
  connected_at: string
  created_at: string
  last_selected_at: string | null
  last_sanity_check_at?: string | null
  last_sanity_check_result?: string | null
  last_parallel_probe_at?: string | null
  last_parallel_probe_result?: string | null
  last_successful_launch_at?: string | null
  last_failed_launch_at?: string | null
  recent_failure_class?: string | null
  reauth_required_reason?: string | null
  refill_note?: string | null
  operator_note?: string | null
}

type FakeFailoverReceiptRow = {
  id: string
  run_id: string | null
  assignment_id: string | null
  order_id: string | null
  requested_action: string
  previous_lease_ref: string | null
  previous_provider_account_ref: string | null
  next_lease_ref: string | null
  next_provider_account_ref: string | null
  failure_class: string
  account_state_action: string
  cooldown_until: string | null
  outcome: string
  attempt_number: number
  max_attempts: number
  customer_safe_status: string
  policy_version: string
  operator_summary: string
  customer_safe_summary: string | null
  created_at: string
}

class FakeD1Statement {
  constructor(
    private readonly database: FakeProviderAccountD1,
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

  run = () => {
    const changes = this.database.run(this.query, this.values)

    return Promise.resolve({ meta: { changes }, success: true })
  }
}

class FakeProviderAccountD1 {
  accounts: Array<FakeAccountRow> = []
  leases: Array<FakeLeaseRow> = []
  failoverReceipts: Array<FakeFailoverReceiptRow> = []

  asD1 = (): D1Database =>
    ({
      batch: (statements: Array<{ run: () => Promise<unknown> }>) =>
        Promise.all(statements.map(statement => statement.run())),
      prepare: (query: string) => new FakeD1Statement(this, query),
    }) as unknown as D1Database

  first = (query: string, values: Array<unknown>): unknown => {
    if (
      query.includes('FROM provider_account_leases') &&
      query.includes('WHERE lease_ref = ?')
    ) {
      const lease = this.leases.find(lease => lease.lease_ref === values[0])

      if (lease === undefined) {
        return null
      }

      if (query.includes("status = 'active'")) {
        const userId = values[1]
        const now = values[2]

        return lease.user_id === userId &&
          lease.status === 'active' &&
          typeof now === 'string' &&
          lease.expires_at > now
          ? lease
          : null
      }

      return lease
    }

    if (
      query.includes('FROM provider_accounts') &&
      query.includes('WHERE id = ?')
    ) {
      const account = this.accounts.find(
        candidate => candidate.id === values[0],
      )

      return account === undefined
        ? null
        : {
            account_label: account.operator_label ?? account.account_label,
          }
    }

    if (
      query.includes('SELECT pa.provider_account_ref') &&
      query.includes('LIMIT 1')
    ) {
      const [now, userId] = values as [string, string, string]
      const selected = this.selectEligibleAccount(userId, now)

      if (selected === undefined) {
        return null
      }

      return {
        provider_account_ref: selected.provider_account_ref,
        account_label: selected.operator_label ?? selected.account_label,
        active_lease_count: this.activeLeaseCount(selected.id, now),
        lease_limit: selected.lease_limit,
        operator_priority: selected.operator_priority,
      }
    }

    if (query.includes('INSERT INTO provider_account_leases')) {
      const [
        leaseId,
        leaseRef,
        requestedAction,
        runId,
        assignmentId,
        orderId,
        policyVersion,
        now,
        expiresAt,
      ] = values as [
        string,
        string,
        string,
        string | null,
        string | null,
        string | null,
        string,
        string,
        string,
      ]
      const selected = this.accounts
        .filter(account => {
          const activeLeaseCount = this.leases.filter(
            lease =>
              lease.provider_account_id === account.id &&
              lease.status === 'active' &&
              lease.expires_at > now,
          ).length

          return (
            account.user_id === values[11] &&
            (values[12] === null || account.provider === values[12]) &&
            account.status === 'connected' &&
            account.health === 'healthy' &&
            account.secret_ref !== null &&
            account.deleted_at === null &&
            account.low_credit_flag === 0 &&
            (account.reauth_required_reason === undefined ||
              account.reauth_required_reason === null) &&
            (account.cooldown_until === null ||
              account.cooldown_until <= now) &&
            activeLeaseCount < account.lease_limit
          )
        })
        .sort((left, right) => {
          const leftActive = this.activeLeaseCount(left.id, now)
          const rightActive = this.activeLeaseCount(right.id, now)

          return (
            leftActive - rightActive ||
            left.operator_priority - right.operator_priority ||
            (
              left.last_selected_at ??
              left.connected_at ??
              left.created_at
            ).localeCompare(
              right.last_selected_at ?? right.connected_at ?? right.created_at,
            ) ||
            left.provider_account_ref.localeCompare(right.provider_account_ref)
          )
        })[0]

      if (selected === undefined) {
        return null
      }

      const activeLeaseCount = this.activeLeaseCount(selected.id, now)
      this.leases.push({
        assignment_id: assignmentId,
        expires_at: expiresAt,
        lease_ref: leaseRef,
        order_id: orderId,
        provider: selected.provider,
        provider_account_id: selected.id,
        provider_account_ref: selected.provider_account_ref,
        requested_action: requestedAction,
        run_id: runId,
        status: 'active',
        user_id: selected.user_id,
      })

      return {
        id: leaseId,
        lease_ref: leaseRef,
        provider_account_id: selected.id,
        provider_account_ref: selected.provider_account_ref,
        requested_action: requestedAction,
        run_id: runId,
        assignment_id: assignmentId,
        order_id: orderId,
        selected_by_policy_version: policyVersion,
        selection_reason: `Selected connected healthy account with ${activeLeaseCount} active lease(s), priority ${selected.operator_priority}, and no cooldown, reconnect marker, or low-credit flag.`,
        selected_by_actor: 'operator_provider_account_routes',
        active_lease_count_before_selection: activeLeaseCount,
        operator_priority: selected.operator_priority,
        started_at: now,
        expires_at: expiresAt,
        last_touched_at: now,
        status: 'active',
      }
    }

    return null
  }

  all = (query: string, values: Array<unknown>): Array<unknown> => {
    if (query.includes('FROM provider_account_failover_receipts')) {
      const [userId, runId, , assignmentId, , orderId, , limit] = values as [
        string,
        string | null,
        string | null,
        string | null,
        string | null,
        string | null,
        string | null,
        number,
      ]

      return this.failoverReceipts
        .filter(receipt => {
          const previousLease = this.leases.find(
            lease => lease.lease_ref === receipt.previous_lease_ref,
          )
          const nextLease = this.leases.find(
            lease => lease.lease_ref === receipt.next_lease_ref,
          )

          return (
            (previousLease?.user_id ?? nextLease?.user_id) === userId &&
            (runId === null || receipt.run_id === runId) &&
            (assignmentId === null || receipt.assignment_id === assignmentId) &&
            (orderId === null || receipt.order_id === orderId)
          )
        })
        .sort((left, right) => right.created_at.localeCompare(left.created_at))
        .slice(0, limit)
    }

    if (query.includes('FROM provider_accounts pa')) {
      const [now, userId] = values as [string, string]

      return this.accounts
        .filter(account => account.user_id === userId)
        .map(account => ({
          provider_account_ref: account.provider_account_ref,
          account_label: account.account_label,
          operator_label: account.operator_label,
          status: account.status,
          health: account.health,
          operator_priority: account.operator_priority,
          lease_limit: account.lease_limit,
          low_credit_flag: account.low_credit_flag,
          cooldown_until: account.cooldown_until,
          recent_failure_class: account.recent_failure_class ?? null,
          last_sanity_check_at: account.last_sanity_check_at ?? null,
          last_sanity_check_result: account.last_sanity_check_result ?? null,
          last_parallel_probe_at: account.last_parallel_probe_at ?? null,
          last_parallel_probe_result:
            account.last_parallel_probe_result ?? null,
          last_selected_at: account.last_selected_at,
          last_successful_launch_at: account.last_successful_launch_at ?? null,
          last_failed_launch_at: account.last_failed_launch_at ?? null,
          reauth_required_reason: account.reauth_required_reason ?? null,
          refill_note: account.refill_note ?? null,
          operator_note: account.operator_note ?? null,
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
            account_label:
              account?.operator_label ?? account?.account_label ?? null,
            requested_action: lease.requested_action,
            run_id: lease.run_id,
            assignment_id: lease.assignment_id,
            order_id: lease.order_id,
            started_at: '2026-06-05T00:00:00.000Z',
            expires_at: lease.expires_at,
            last_touched_at: null,
            status: lease.status,
          }
        })
    }

    return []
  }

  run = (query: string, values: Array<unknown>): number => {
    let changes = 0

    if (
      query.includes('UPDATE provider_accounts') &&
      query.includes('cooldown_until = NULL') &&
      query.includes('recent_failure_class = NULL')
    ) {
      const userId = values[2]
      const providerAccountRef = values[3]
      this.accounts = this.accounts.map(account => {
        if (
          account.user_id !== userId ||
          account.provider_account_ref !== providerAccountRef ||
          account.deleted_at !== null
        ) {
          return account
        }

        changes += 1

        return {
          ...account,
          cooldown_until: null,
          health:
            account.status === 'connected' &&
            (account.reauth_required_reason === undefined ||
              account.reauth_required_reason === null)
              ? 'healthy'
              : account.health,
          low_credit_flag: 0,
          recent_failure_class: null,
          refill_note: null,
        }
      })

      return changes
    }

    if (
      query.includes('UPDATE provider_account_leases') &&
      query.includes("SET status = 'failed'")
    ) {
      const [, failureClass, leaseRef] = values as [string, string, string]
      this.leases = this.leases.map(lease =>
        lease.lease_ref === leaseRef
          ? { ...lease, status: 'failed', failure_class: failureClass }
          : lease,
      ) as Array<FakeLeaseRow>
    }

    if (
      query.includes('UPDATE provider_accounts') &&
      query.includes('last_failed_launch_at')
    ) {
      const accountId = values[values.length - 1]
      this.accounts = this.accounts.map(account =>
        account.id === accountId
          ? {
              ...account,
              health: query.includes('COALESCE')
                ? ((values[0] as string | null) ?? account.health)
                : account.health,
              low_credit_flag:
                query.includes('low_credit_flag') && values[3] === 1
                  ? 1
                  : account.low_credit_flag,
              cooldown_until:
                query.includes('cooldown_until') &&
                typeof values[4] === 'string'
                  ? (values[4] as string)
                  : account.cooldown_until,
            }
          : account,
      )
    }

    if (
      query.includes('UPDATE provider_accounts') &&
      query.includes('last_selected_at')
    ) {
      const accountId = values[2]
      this.accounts = this.accounts.map(account =>
        account.id === accountId
          ? { ...account, last_selected_at: values[0] as string }
          : account,
      )
    }

    if (query.includes('INSERT INTO provider_account_failover_receipts')) {
      const [
        id,
        runId,
        assignmentId,
        orderId,
        requestedAction,
        previousLeaseRef,
        previousProviderAccountRef,
        nextLeaseRef,
        nextProviderAccountRef,
        failureClass,
        accountStateAction,
        cooldownUntil,
        outcome,
        attemptNumber,
        maxAttempts,
        customerSafeStatus,
        policyVersion,
        operatorSummary,
        customerSafeSummary,
        createdAt,
      ] = values
      this.failoverReceipts.push({
        account_state_action: accountStateAction as string,
        assignment_id: assignmentId as string | null,
        cooldown_until: cooldownUntil as string | null,
        created_at: createdAt as string,
        customer_safe_status: customerSafeStatus as string,
        customer_safe_summary: customerSafeSummary as string | null,
        failure_class: failureClass as string,
        id: id as string,
        max_attempts: maxAttempts as number,
        next_lease_ref: nextLeaseRef as string | null,
        next_provider_account_ref: nextProviderAccountRef as string | null,
        operator_summary: operatorSummary as string,
        order_id: orderId as string | null,
        outcome: outcome as string,
        policy_version: policyVersion as string,
        previous_lease_ref: previousLeaseRef as string | null,
        previous_provider_account_ref: previousProviderAccountRef as
          | string
          | null,
        requested_action: requestedAction as string,
        run_id: runId as string | null,
        attempt_number: attemptNumber as number,
      })
      changes += 1
    }

    return changes
  }

  private activeLeaseCount = (accountId: string, now: string): number =>
    this.leases.filter(
      lease =>
        lease.provider_account_id === accountId &&
        lease.status === 'active' &&
        lease.expires_at > now,
    ).length

  private selectEligibleAccount = (
    userId: string,
    now: string,
    requiredProvider: string | null = null,
  ): FakeAccountRow | undefined =>
    this.accounts
      .filter(account => {
        const activeLeaseCount = this.activeLeaseCount(account.id, now)

        return (
          account.user_id === userId &&
          (requiredProvider === null ||
            account.provider === requiredProvider) &&
          account.status === 'connected' &&
          account.health === 'healthy' &&
          account.secret_ref !== null &&
          account.deleted_at === null &&
          account.low_credit_flag === 0 &&
          (account.reauth_required_reason === undefined ||
            account.reauth_required_reason === null) &&
          (account.cooldown_until === null || account.cooldown_until <= now) &&
          activeLeaseCount < account.lease_limit
        )
      })
      .sort((left, right) => {
        const leftActive = this.activeLeaseCount(left.id, now)
        const rightActive = this.activeLeaseCount(right.id, now)

        return (
          leftActive - rightActive ||
          left.operator_priority - right.operator_priority ||
          (
            left.last_selected_at ??
            left.connected_at ??
            left.created_at
          ).localeCompare(
            right.last_selected_at ?? right.connected_at ?? right.created_at,
          ) ||
          left.provider_account_ref.localeCompare(right.provider_account_ref)
        )
      })[0]
}

const connectedAccount = (
  overrides: Partial<ProviderAccountRecord> = {},
): ProviderAccountRecord => ({
  id: 'provider_account_1',
  userId: targetUser.userId,
  teamId: null,
  provider: 'chatgpt_codex',
  authMode: 'chatgpt_device_code',
  status: 'connected',
  health: 'healthy',
  providerAccountRef: 'provider-account_ref_test',
  secretRef: 'codex-auth://provider-account_ref_test',
  accountLabel: 'account 1',
  planType: null,
  connectedAt: '2026-06-05T00:00:00.000Z',
  disconnectedAt: null,
  deniedAt: null,
  lastStatusAt: '2026-06-05T00:00:00.000Z',
  metadataJson: null,
  createdAt: '2026-06-05T00:00:00.000Z',
  updatedAt: '2026-06-05T00:00:00.000Z',
  deletedAt: null,
  ...overrides,
})

const fakeAccountRow = (
  id: string,
  providerAccountRef: string,
  overrides: Partial<FakeAccountRow> = {},
): FakeAccountRow => ({
  id,
  user_id: targetUser.userId,
  provider_account_ref: providerAccountRef,
  account_label: providerAccountRef,
  operator_label: null,
  provider: 'chatgpt_codex',
  status: 'connected',
  health: 'healthy',
  secret_ref: `codex-auth://${providerAccountRef}`,
  deleted_at: null,
  low_credit_flag: 0,
  cooldown_until: null,
  lease_limit: 1,
  operator_priority: 100,
  connected_at: '2026-06-05T00:00:00.000Z',
  created_at: '2026-06-05T00:00:00.000Z',
  last_selected_at: null,
  ...overrides,
})

const makeRoutes = (
  repository: FakeProviderAccountRepository,
  options: Readonly<{
    authMaterialAvailable?: boolean
    authorized?: boolean
    pollStatus?: 'connected' | 'failed' | 'pending'
    probeResolvedGrant?: (input: {
      account: ProviderAccountRecord
      leaseId: string | null
      probeId: string | null
    }) => Promise<ProviderAccountSanityProbeResult>
    sanityClassification?: ProviderAccountSanityClassification
  }> = {},
) => {
  const startedSecrets = new Map<
    string,
    Readonly<{ deviceAuthId: string; userCode: string }>
  >()
  const connectedAuths = new Map<string, unknown>()

  return {
    connectedAuths,
    routes: makeOperatorProviderAccountRoutes({
      deleteStartedCodexDeviceLogin: () => attemptId => {
        startedSecrets.delete(attemptId)

        return Promise.resolve()
      },
      makeProviderAccountRepository: () => repository,
      pollDeviceLogin: () =>
        Promise.resolve(
          options.pollStatus === 'failed'
            ? { status: 'failed' as const, reason: 'rate limited' }
            : options.pollStatus === 'pending'
              ? { status: 'pending' as const }
              : {
                  status: 'connected' as const,
                  accountLabel: 'Connected account',
                  auth: {
                    type: 'oauth' as const,
                    access: 'access-token-secret',
                    refresh: 'refresh-token-secret',
                    expires: 1_800_000_000_000,
                  },
                },
        ),
      probeResolvedGrant: input =>
        options.probeResolvedGrant === undefined
          ? Promise.resolve(options.sanityClassification ?? 'healthy')
          : options.probeResolvedGrant(input),
      readConnectedCodexAuthMaterial: () =>
        Promise.resolve(
          options.authMaterialAvailable === false
            ? undefined
            : {
                authContentEnv: 'OPENCODE_AUTH_CONTENT',
                authContentJson: '{"access":"access-token-secret"}',
              },
        ),
      readSelectedOperatorTargetUser: () => Promise.resolve(targetUser),
      readStartedCodexDeviceLogin: () => attemptId =>
        Promise.resolve(startedSecrets.get(attemptId)),
      requireAdminApiToken: () => Promise.resolve(options.authorized !== false),
      startDeviceLogin: () =>
        Promise.resolve({
          deviceAuthId: 'device-auth-secret',
          verificationUrl: 'https://auth.openai.com/codex/device',
          userCode: 'ABCD-EFGH',
          expiresAt: '2099-01-01T00:00:00.000Z',
          intervalSeconds: 5,
        }),
      storeConnectedCodexAuth: () => input => {
        connectedAuths.set(input.providerAccountRef, input.auth)

        return Promise.resolve(`codex-auth://${input.providerAccountRef}`)
      },
      storeStartedCodexDeviceLogin: () => input => {
        startedSecrets.set(input.attemptId, {
          deviceAuthId: input.deviceAuthId,
          userCode: input.userCode,
        })

        return Promise.resolve()
      },
    }),
  }
}

const run = async (
  repository: FakeProviderAccountRepository,
  path: string,
  init: RequestInit,
  options?: Parameters<typeof makeRoutes>[1],
  database: D1Database = db(),
): Promise<Response> =>
  makeRoutes(repository, options).routes.routeOperatorProviderAccountRequest(
    new Request(`https://openagents.com${path}`, init),
    {
      AUTH_STORAGE: kv(),
      OPENAGENTS_DB: database,
    },
  ) ?? Promise.reject(new Error('route did not match'))

describe('operator provider account routes', () => {
  test('starts a ChatGPT device login with only safe ceremony fields', async () => {
    const repository = new FakeProviderAccountRepository()
    const response = await run(
      repository,
      '/api/operator/provider-accounts/chatgpt-codex/device-login/start',
      {
        body: JSON.stringify({
          accountLabel: 'account 1',
          createNew: true,
          email: 'chris@openagents.com',
        }),
        headers: { authorization: 'Bearer expected' },
        method: 'POST',
      },
    )
    const body = await response.text()

    expect(response.status).toBe(201)
    expect(JSON.parse(body)).toMatchObject({
      status: 'pending',
      targetUser: {
        userId: 'user_chris',
        email: 'chris@openagents.com',
      },
      verificationUrl: 'https://auth.openai.com/codex/device',
      userCode: 'ABCD-EFGH',
      intervalSeconds: 5,
      nextPollCommand: expect.stringContaining(
        'scripts/provider-chatgpt-device-login.mjs poll provider_attempt_',
      ),
    })
    expect(body).not.toContain('device-auth-secret')
    expect(body).not.toContain('access-token-secret')
    expect(body).not.toContain('refresh-token-secret')
    expect(repository.accounts).toHaveLength(1)
    expect(repository.attempts).toHaveLength(1)
  })

  test('rejects missing admin authorization', async () => {
    const response = await run(
      new FakeProviderAccountRepository(),
      '/api/operator/provider-accounts/chatgpt-codex/device-login/start',
      {
        body: JSON.stringify({ createNew: true }),
        method: 'POST',
      },
      { authorized: false },
    )

    expect(response.status).toBe(401)
    await expect(response.json()).resolves.toEqual({ error: 'unauthorized' })
  })

  test('polls a pending attempt into a connected account without printing tokens', async () => {
    const repository = new FakeProviderAccountRepository()
    const { connectedAuths, routes } = makeRoutes(repository)
    const start = await routes.routeOperatorProviderAccountRequest(
      new Request(
        'https://openagents.com/api/operator/provider-accounts/chatgpt-codex/device-login/start',
        {
          body: JSON.stringify({ accountLabel: 'account 1', createNew: true }),
          method: 'POST',
        },
      ),
      {
        AUTH_STORAGE: kv(),
        OPENAGENTS_DB: db(),
      },
    )
    const startBody = (await start?.json()) as { attemptId: string }
    const poll = await routes.routeOperatorProviderAccountRequest(
      new Request(
        `https://openagents.com/api/operator/provider-accounts/chatgpt-codex/device-login/${startBody.attemptId}`,
      ),
      {
        AUTH_STORAGE: kv(),
        OPENAGENTS_DB: db(),
      },
    )
    const pollText = await poll?.text()
    const pollBody = JSON.parse(pollText ?? '{}')

    expect(poll?.status).toBe(200)
    expect(pollBody).toMatchObject({
      status: 'connected',
      failureReason: null,
      providerAccountStatus: 'connected',
      providerAccountHealth: 'healthy',
      accountLabel: 'Connected account',
    })
    expect(pollText).not.toContain('access-token-secret')
    expect(pollText).not.toContain('refresh-token-secret')
    expect(connectedAuths.size).toBe(1)
  })

  test('poll returns a typed redacted failed state', async () => {
    const repository = new FakeProviderAccountRepository()
    const { routes } = makeRoutes(repository, { pollStatus: 'failed' })
    const start = await routes.routeOperatorProviderAccountRequest(
      new Request(
        'https://openagents.com/api/operator/provider-accounts/chatgpt-codex/device-login/start',
        {
          body: JSON.stringify({ createNew: true }),
          method: 'POST',
        },
      ),
      {
        AUTH_STORAGE: kv(),
        OPENAGENTS_DB: db(),
      },
    )
    const startBody = (await start?.json()) as { attemptId: string }
    const poll = await routes.routeOperatorProviderAccountRequest(
      new Request(
        `https://openagents.com/api/operator/provider-accounts/chatgpt-codex/device-login/${startBody.attemptId}`,
      ),
      {
        AUTH_STORAGE: kv(),
        OPENAGENTS_DB: db(),
      },
    )

    expect(poll?.status).toBe(200)
    await expect(poll?.json()).resolves.toMatchObject({
      status: 'failed',
      failureReason: 'device_login_failed',
      providerAccountStatus: 'denied',
      providerAccountHealth: 'requires_reauth',
    })
  })

  test('sanity checks one account through grant resolution without printing secrets', async () => {
    const repository = new FakeProviderAccountRepository()
    repository.accounts.push(connectedAccount())

    const response = await run(
      repository,
      '/api/operator/provider-accounts/chatgpt-codex/sanity',
      {
        body: JSON.stringify({
          providerAccountRef: 'provider-account_ref_test',
        }),
        method: 'POST',
      },
    )
    const text = await response.text()
    const body = JSON.parse(text)

    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      summary: {
        total: 1,
        healthy: 1,
        requiresAttention: 0,
      },
      checks: [
        {
          providerAccountRef: 'provider-account_ref_test',
          accountLabel: 'account 1',
          classification: 'healthy',
          health: 'healthy',
          status: 'connected',
        },
      ],
    })
    expect(text).not.toContain('access-token-secret')
    expect(text).not.toContain('refresh-token-secret')
    expect(text).not.toContain('authContentJson')
    expect(text).not.toContain('codex-auth://')
    expect(text).not.toContain('codex-auth-grant_')
    expect(repository.grants).toHaveLength(1)
    expect(repository.grants[0]?.status).toBe('used')
    expect(repository.events.map(event => event.kind)).toEqual([
      'auth_grant_issued',
      'auth_grant_used',
      'account_health_updated',
    ])
  })

  test('sanity --all checks connected accounts for the selected target user', async () => {
    const repository = new FakeProviderAccountRepository()
    repository.accounts.push(
      connectedAccount(),
      connectedAccount({
        id: 'provider_account_2',
        providerAccountRef: 'provider-account_ref_second',
        secretRef: 'codex-auth://provider-account_ref_second',
      }),
      connectedAccount({
        id: 'provider_account_3',
        providerAccountRef: 'provider-account_ref_other_user',
        secretRef: 'codex-auth://provider-account_ref_other_user',
        userId: 'user_other',
      }),
    )

    const response = await run(
      repository,
      '/api/operator/provider-accounts/chatgpt-codex/sanity',
      {
        body: JSON.stringify({
          all: true,
          email: 'chris@openagents.com',
        }),
        method: 'POST',
      },
    )
    const body = (await response.json()) as Readonly<{
      checks: ReadonlyArray<Readonly<{ providerAccountRef: string }>>
    }>

    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      summary: {
        total: 2,
        healthy: 2,
        requiresAttention: 0,
      },
    })
    expect(body.checks.map(check => check.providerAccountRef)).toEqual([
      'provider-account_ref_test',
      'provider-account_ref_second',
    ])
  })

  test('sanity records requires_reauth when auth material is missing', async () => {
    const repository = new FakeProviderAccountRepository()
    repository.accounts.push(connectedAccount())

    const response = await run(
      repository,
      '/api/operator/provider-accounts/chatgpt-codex/sanity',
      {
        body: JSON.stringify({
          providerAccountRef: 'provider-account_ref_test',
        }),
        method: 'POST',
      },
      { authMaterialAvailable: false },
    )
    const body = (await response.json()) as Readonly<{
      checks: ReadonlyArray<Readonly<{ providerAccountRef: string }>>
    }>

    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      summary: {
        total: 1,
        healthy: 0,
        requiresAttention: 1,
      },
      checks: [
        {
          classification: 'requires_reauth',
          health: 'requires_reauth',
        },
      ],
    })
    expect(repository.accounts[0]?.health).toBe('requires_reauth')
  })

  test('sanity maps token-invalidated provider probes to requires_reauth without leaking auth material', async () => {
    const repository = new FakeProviderAccountRepository()
    repository.accounts.push(connectedAccount())

    const response = await run(
      repository,
      '/api/operator/provider-accounts/chatgpt-codex/sanity',
      {
        body: JSON.stringify({
          providerAccountRef: 'provider-account_ref_test',
        }),
        method: 'POST',
      },
      {
        probeResolvedGrant: async () => ({
          classification: 'healthy',
          providerFailureClass: 'token_invalidated',
          providerStatus: 401,
        }),
      },
    )
    const text = await response.text()
    const body = JSON.parse(text)

    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      summary: {
        total: 1,
        healthy: 0,
        requiresAttention: 1,
      },
      checks: [
        {
          classification: 'requires_reauth',
          failureClass: 'token_invalidated',
          health: 'requires_reauth',
          terminalStatus: 'failed',
        },
      ],
    })
    expect(repository.accounts[0]?.health).toBe('requires_reauth')
    expect(repository.events[2]?.summary).toContain('token_invalidated')
    expect(text).not.toContain('access-token-secret')
    expect(text).not.toContain('refresh-token-secret')
    expect(text).not.toContain('authContentJson')
    expect(text).not.toContain('codex-auth://')
    expect(text).not.toContain('codex-auth-grant_')
  })

  test.each<ProviderAccountSanityClassification>([
    'low_credit',
    'rate_limited',
    'quota_exhausted',
    'provider_outage',
    'grant_resolution_failed',
    'launch_probe_failed',
    'unknown_failure',
  ])(
    'sanity maps %s probe results to unhealthy output',
    async classification => {
      const repository = new FakeProviderAccountRepository()
      repository.accounts.push(connectedAccount())

      const response = await run(
        repository,
        '/api/operator/provider-accounts/chatgpt-codex/sanity',
        {
          body: JSON.stringify({
            providerAccountRef: 'provider-account_ref_test',
          }),
          method: 'POST',
        },
        { sanityClassification: classification },
      )
      const body = await response.json()

      expect(response.status).toBe(200)
      expect(body).toMatchObject({
        summary: {
          total: 1,
          healthy: 0,
          requiresAttention: 1,
        },
        checks: [
          {
            classification,
            health: 'unhealthy',
          },
        ],
      })
      expect(repository.accounts[0]?.health).toBe('unhealthy')
    },
  )

  test('parallel sanity probes isolate account state with per-account probe receipts', async () => {
    const repository = new FakeProviderAccountRepository()
    repository.accounts.push(
      connectedAccount(),
      connectedAccount({
        id: 'provider_account_2',
        providerAccountRef: 'provider-account_ref_second',
        secretRef: 'codex-auth://provider-account_ref_second',
      }),
      connectedAccount({
        id: 'provider_account_3',
        providerAccountRef: 'provider-account_ref_third',
        secretRef: 'codex-auth://provider-account_ref_third',
      }),
    )
    const activeAccountRefs = new Set<string>()
    let maxActive = 0

    const response = await run(
      repository,
      '/api/operator/provider-accounts/chatgpt-codex/sanity',
      {
        body: JSON.stringify({
          all: true,
          email: 'chris@openagents.com',
          parallel: 3,
        }),
        method: 'POST',
      },
      {
        probeResolvedGrant: async ({ account, leaseId, probeId }) => {
          expect(leaseId).toEqual(
            expect.stringContaining('provider_probe_lease'),
          )
          expect(probeId).toEqual(expect.stringContaining('provider_probe'))
          activeAccountRefs.add(account.providerAccountRef)
          maxActive = Math.max(maxActive, activeAccountRefs.size)
          await Promise.resolve()
          activeAccountRefs.delete(account.providerAccountRef)

          return {
            classification: 'healthy',
            observedProviderAccountRef: account.providerAccountRef,
          }
        },
      },
    )
    const text = await response.text()
    const body = JSON.parse(text)

    expect(response.status).toBe(200)
    expect(body.summary).toMatchObject({
      total: 3,
      healthy: 3,
      requiresAttention: 0,
      collisionCount: 0,
    })
    expect(body.probeRunId).toEqual(
      expect.stringContaining('provider_parallel_probe_run'),
    )
    expect(body.parallel).toBe(3)
    expect(body.checks).toHaveLength(3)
    expect(
      body.checks.every(
        (
          check: Readonly<{
            collisionClass: string
            leaseId: string
            probeId: string
            terminalStatus: string
          }>,
        ) =>
          check.collisionClass === 'none' &&
          check.terminalStatus === 'passed' &&
          check.leaseId.startsWith('provider_probe_lease') &&
          check.probeId.startsWith('provider_probe'),
      ),
    ).toBe(true)
    expect(text).not.toContain('access-token-secret')
    expect(text).not.toContain('codex-auth://')
    expect(text).not.toContain('codex-auth-grant_')
    expect(repository.grants).toHaveLength(3)
    expect(maxActive).toBeGreaterThan(1)
  })

  test('parallel sanity probes classify wrong-account collision symptoms', async () => {
    const repository = new FakeProviderAccountRepository()
    repository.accounts.push(
      connectedAccount(),
      connectedAccount({
        id: 'provider_account_2',
        providerAccountRef: 'provider-account_ref_second',
        secretRef: 'codex-auth://provider-account_ref_second',
      }),
    )

    const response = await run(
      repository,
      '/api/operator/provider-accounts/chatgpt-codex/sanity',
      {
        body: JSON.stringify({
          all: true,
          parallel: 2,
        }),
        method: 'POST',
      },
      {
        probeResolvedGrant: async ({ account }) =>
          account.providerAccountRef === 'provider-account_ref_test'
            ? {
                classification: 'healthy',
                observedProviderAccountRef: 'provider-account_ref_second',
              }
            : {
                classification: 'healthy',
                observedProviderAccountRef: account.providerAccountRef,
              },
      },
    )
    const body = (await response.json()) as Readonly<{
      checks: ReadonlyArray<Readonly<{ providerAccountRef: string }>>
    }>

    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      summary: {
        total: 2,
        healthy: 1,
        requiresAttention: 1,
        collisionCount: 1,
      },
    })
    expect(
      body.checks.find(
        (check: Readonly<{ providerAccountRef: string }>) =>
          check.providerAccountRef === 'provider-account_ref_test',
      ),
    ).toMatchObject({
      classification: 'unknown_failure',
      health: 'unhealthy',
      collisionClass: 'wrong_account_identity',
      terminalStatus: 'failed',
    })
  })

  test('issues an active lease-bound auth grant through the admin route', async () => {
    const repository = new FakeProviderAccountRepository()
    const database = new FakeProviderAccountD1()
    repository.accounts.push(connectedAccount())
    database.leases.push({
      assignment_id: 'artanis.assignment.pylon-launch.test',
      expires_at: '2099-01-01T00:00:00.000Z',
      lease_ref: 'provider-account-lease_ref_artanis',
      order_id: null,
      provider_account_id: 'provider_account_1',
      provider_account_ref: 'provider-account_ref_test',
      requested_action: 'artanis.pylon_launch_bootstrap',
      run_id: 'artanis.bootstrap.pylon-launch.test',
      status: 'active',
      user_id: targetUser.userId,
    })

    const response = await run(
      repository,
      '/api/operator/provider-accounts/chatgpt-codex/leases/grant',
      {
        body: JSON.stringify({
          email: 'chris@openagents.com',
          leaseRef: 'provider-account-lease_ref_artanis',
          runnerSessionId: 'artanis.bootstrap.pylon-launch.test',
          workroomId: 'workroom.artanis.pylon-launch.test',
        }),
        method: 'POST',
      },
      undefined,
      database.asD1(),
    )
    const text = await response.text()
    const body = JSON.parse(text)

    expect(response.status).toBe(201)
    expect(body).toMatchObject({
      leaseRef: 'provider-account-lease_ref_artanis',
      providerAccountRef: 'provider-account_ref_test',
      requestedAction: 'artanis.pylon_launch_bootstrap',
      runId: 'artanis.bootstrap.pylon-launch.test',
      assignmentId: 'artanis.assignment.pylon-launch.test',
      grant: {
        grantRef: expect.stringContaining('codex-auth-grant_'),
        requestedAction: 'artanis.pylon_launch_bootstrap',
        runnerSessionId: 'artanis.bootstrap.pylon-launch.test',
        status: 'issued',
        workroomId: 'workroom.artanis.pylon-launch.test',
      },
    })
    expect(text).not.toContain('access-token-secret')
    expect(text).not.toContain('refresh-token-secret')
    expect(text).not.toContain('authContentJson')
    expect(text).not.toContain('codex-auth://')
    expect(repository.grants).toHaveLength(1)
    expect(repository.grants[0]).toMatchObject({
      providerAccountRef: 'provider-account_ref_test',
      requestedAction: 'artanis.pylon_launch_bootstrap',
      runnerSessionId: 'artanis.bootstrap.pylon-launch.test',
      status: 'issued',
      workroomId: 'workroom.artanis.pylon-launch.test',
    })
  })

  test('denies expired lease grant issue without creating a grant', async () => {
    const repository = new FakeProviderAccountRepository()
    const database = new FakeProviderAccountD1()
    repository.accounts.push(connectedAccount())
    database.leases.push({
      assignment_id: 'assignment_expired',
      expires_at: '2000-01-01T00:00:00.000Z',
      lease_ref: 'provider-account-lease_ref_expired',
      order_id: null,
      provider_account_id: 'provider_account_1',
      provider_account_ref: 'provider-account_ref_test',
      requested_action: 'artanis.pylon_launch_bootstrap',
      run_id: 'run_expired',
      status: 'active',
      user_id: targetUser.userId,
    })

    const response = await run(
      repository,
      '/api/operator/provider-accounts/chatgpt-codex/leases/grant',
      {
        body: JSON.stringify({
          email: 'chris@openagents.com',
          leaseRef: 'provider-account-lease_ref_expired',
        }),
        method: 'POST',
      },
      undefined,
      database.asD1(),
    )

    expect(response.status).toBe(404)
    await expect(response.json()).resolves.toEqual({ error: 'not_found' })
    expect(repository.grants).toHaveLength(0)
  })

  test('failover creates a retry receipt with redacted customer projection', async () => {
    const repository = new FakeProviderAccountRepository()
    const database = new FakeProviderAccountD1()
    database.accounts.push(
      {
        id: 'provider_account_failed',
        user_id: targetUser.userId,
        provider_account_ref: 'provider-account_ref_failed',
        account_label: 'failed account',
        operator_label: null,
        provider: 'chatgpt_codex',
        status: 'connected',
        health: 'healthy',
        secret_ref: 'codex-auth://failed-secret',
        deleted_at: null,
        low_credit_flag: 0,
        cooldown_until: null,
        lease_limit: 1,
        operator_priority: 100,
        connected_at: '2026-06-05T00:00:00.000Z',
        created_at: '2026-06-05T00:00:00.000Z',
        last_selected_at: '2026-06-05T00:05:00.000Z',
      },
      {
        id: 'provider_account_next',
        user_id: targetUser.userId,
        provider_account_ref: 'provider-account_ref_next',
        account_label: 'next account',
        operator_label: null,
        provider: 'chatgpt_codex',
        status: 'connected',
        health: 'healthy',
        secret_ref: 'codex-auth://next-secret',
        deleted_at: null,
        low_credit_flag: 0,
        cooldown_until: null,
        lease_limit: 1,
        operator_priority: 100,
        connected_at: '2026-06-05T00:00:00.000Z',
        created_at: '2026-06-05T00:00:00.000Z',
        last_selected_at: '2026-06-05T00:00:00.000Z',
      },
    )
    database.leases.push({
      assignment_id: 'assignment_1',
      expires_at: '2099-01-01T00:00:00.000Z',
      lease_ref: 'provider-account-lease_ref_failed',
      order_id: 'order_1',
      provider_account_id: 'provider_account_failed',
      provider_account_ref: 'provider-account_ref_failed',
      requested_action: 'customer_order_fulfillment',
      run_id: 'run_1',
      status: 'active',
      user_id: targetUser.userId,
    })

    const response = await run(
      repository,
      '/api/operator/provider-accounts/chatgpt-codex/leases/failover',
      {
        body: JSON.stringify({
          previousLeaseRef: 'provider-account-lease_ref_failed',
          failureClass: 'rate_limited',
          requestedAction: 'customer_order_fulfillment',
          attemptNumber: 1,
          maxAttempts: 3,
          email: 'chris@openagents.com',
        }),
        method: 'POST',
      },
      undefined,
      database.asD1(),
    )
    const text = await response.text()
    const body = JSON.parse(text)

    expect(response.status).toBe(201)
    expect(body).toMatchObject({
      outcome: 'retrying',
      receiptId: expect.stringContaining('provider_account_failover_receipt'),
      previousProviderAccountRef: 'provider-account_ref_failed',
      nextLease: {
        providerAccountRef: 'provider-account_ref_next',
      },
      customerSafeStatus:
        'Work is retrying with another account after a temporary provider limit.',
    })
    expect(database.failoverReceipts).toHaveLength(1)
    expect(database.failoverReceipts[0]).toMatchObject({
      run_id: 'run_1',
      assignment_id: 'assignment_1',
      order_id: 'order_1',
      previous_provider_account_ref: 'provider-account_ref_failed',
      next_provider_account_ref: 'provider-account_ref_next',
      failure_class: 'rate_limited',
      account_state_action: 'timed_cooldown',
      outcome: 'retrying',
      policy_version: 'provider-account-lease-policy:v2',
      customer_safe_summary:
        'Work is retrying through another connected execution account.',
    })
    expect(text).not.toContain('codex-auth://')
    expect(text).not.toContain('access-token')
    expect(text).not.toContain('grant')
  })

  test('failover records an exhausted fleet blocker receipt and exposes history safely', async () => {
    const repository = new FakeProviderAccountRepository()
    const database = new FakeProviderAccountD1()
    database.accounts.push({
      id: 'provider_account_failed',
      user_id: targetUser.userId,
      provider_account_ref: 'provider-account_ref_failed',
      account_label: 'failed account',
      operator_label: null,
      provider: 'chatgpt_codex',
      status: 'connected',
      health: 'healthy',
      secret_ref: 'codex-auth://failed-secret',
      deleted_at: null,
      low_credit_flag: 0,
      cooldown_until: null,
      lease_limit: 1,
      operator_priority: 100,
      connected_at: '2026-06-05T00:00:00.000Z',
      created_at: '2026-06-05T00:00:00.000Z',
      last_selected_at: null,
    })
    database.leases.push({
      assignment_id: 'assignment_1',
      expires_at: '2099-01-01T00:00:00.000Z',
      lease_ref: 'provider-account-lease_ref_failed',
      order_id: 'order_1',
      provider_account_id: 'provider_account_failed',
      provider_account_ref: 'provider-account_ref_failed',
      requested_action: 'customer_order_fulfillment',
      run_id: 'run_1',
      status: 'active',
      user_id: targetUser.userId,
    })

    const failover = await run(
      repository,
      '/api/operator/provider-accounts/chatgpt-codex/leases/failover',
      {
        body: JSON.stringify({
          previousLeaseRef: 'provider-account-lease_ref_failed',
          failureClass: 'low_credits',
          requestedAction: 'customer_order_fulfillment',
          attemptNumber: 3,
          maxAttempts: 3,
          email: 'chris@openagents.com',
        }),
        method: 'POST',
      },
      undefined,
      database.asD1(),
    )
    const failoverText = await failover.text()
    const failoverBody = JSON.parse(failoverText)

    expect(failover.status).toBe(409)
    expect(failoverBody).toMatchObject({
      outcome: 'blocked',
      receiptId: expect.stringContaining('provider_account_failover_receipt'),
      nextLease: null,
      customerSafeStatus:
        'Work is blocked until another eligible account is available.',
    })

    const history = await run(
      repository,
      '/api/operator/provider-accounts/chatgpt-codex/leases/failover-history',
      {
        body: JSON.stringify({
          email: 'chris@openagents.com',
          assignmentId: 'assignment_1',
        }),
        method: 'POST',
      },
      undefined,
      database.asD1(),
    )
    const historyText = await history.text()
    const historyBody = JSON.parse(historyText)

    expect(history.status).toBe(200)
    expect(historyBody).toMatchObject({
      total: 1,
      receipts: [
        {
          receiptId: failoverBody.receiptId,
          outcome: 'blocked',
          failureClass: 'low_credits',
          accountStateAction: 'low_credit_cooldown',
          previousProviderAccountRef: 'provider-account_ref_failed',
          nextProviderAccountRef: null,
          policyVersion: 'provider-account-lease-policy:v2',
          customerSafeSummary:
            'Work is waiting for operator capacity before it can continue.',
        },
      ],
    })
    expect(historyText).not.toContain('codex-auth://')
    expect(historyText).not.toContain('access-token')
    expect(historyText).not.toContain('grant')
  })

  test('fleet dashboard projects account states and blocks stale reconnect markers without secrets', async () => {
    const repository = new FakeProviderAccountRepository()
    const database = new FakeProviderAccountD1()
    database.accounts.push(
      fakeAccountRow(
        'provider_account_healthy',
        'provider-account_ref_healthy',
        {
          account_label: 'healthy',
          last_sanity_check_at: '2026-06-05T01:00:00.000Z',
          last_sanity_check_result: 'healthy',
          last_parallel_probe_at: '2026-06-05T01:05:00.000Z',
          last_parallel_probe_result: 'healthy',
          last_successful_launch_at: '2026-06-05T01:10:00.000Z',
        },
      ),
      fakeAccountRow(
        'provider_account_stale_reauth',
        'provider-account_ref_stale_reauth',
        {
          account_label: 'stale reauth marker',
          operator_priority: 1,
          reauth_required_reason: 'token_invalidated',
        },
      ),
      fakeAccountRow(
        'provider_account_low_credit',
        'provider-account_ref_low',
        {
          account_label: 'low credit',
          low_credit_flag: 1,
          refill_note: 'Refill before overnight use.',
        },
      ),
      fakeAccountRow(
        'provider_account_rate_limited',
        'provider-account_ref_rate_limited',
        {
          account_label: 'rate limited',
          health: 'unhealthy',
          cooldown_until: '2099-01-01T00:00:00.000Z',
          last_failed_launch_at: '2026-06-05T01:20:00.000Z',
          recent_failure_class: 'rate_limited',
        },
      ),
      fakeAccountRow('provider_account_reauth', 'provider-account_ref_reauth', {
        account_label: 'reauth',
        health: 'requires_reauth',
        reauth_required_reason: 'token_invalidated',
      }),
      fakeAccountRow('provider_account_busy', 'provider-account_ref_busy', {
        account_label: 'busy',
        last_selected_at: '2026-06-05T01:30:00.000Z',
      }),
    )
    database.leases.push({
      assignment_id: 'assignment_busy',
      expires_at: '2099-01-01T00:00:00.000Z',
      lease_ref: 'provider-account-lease_ref_busy',
      order_id: 'order_busy',
      provider_account_id: 'provider_account_busy',
      provider_account_ref: 'provider-account_ref_busy',
      requested_action: 'customer_order_fulfillment',
      run_id: 'run_busy',
      status: 'active',
      user_id: targetUser.userId,
    })

    const response = await run(
      repository,
      '/api/operator/provider-accounts/chatgpt-codex/fleet-dashboard',
      {
        body: JSON.stringify({ email: 'chris@openagents.com' }),
        method: 'POST',
      },
      undefined,
      database.asD1(),
    )
    const text = await response.text()
    const body = JSON.parse(text)

    expect(response.status).toBe(200)
    expect(body.summary).toMatchObject({
      total: 6,
      eligible: 1,
      activeLeaseCount: 1,
      lowCredit: 1,
      requiresReauth: 2,
      cooldown: 1,
      unhealthy: 1,
    })
    expect(body.selector).toMatchObject({
      status: 'selected',
      providerAccountRef: 'provider-account_ref_healthy',
    })
    expect(body.accounts).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          providerAccountRef: 'provider-account_ref_healthy',
          eligibility: 'eligible',
          sanityCommand:
            'node scripts/provider-chatgpt-device-login.mjs sanity provider-account_ref_healthy',
          reconnectCommand:
            'node scripts/provider-chatgpt-device-login.mjs start --providerAccountRef provider-account_ref_healthy',
        }),
        expect.objectContaining({
          providerAccountRef: 'provider-account_ref_stale_reauth',
          eligibility: 'ineligible',
          eligibilityReasons: expect.arrayContaining([
            'reauth_required:token_invalidated',
          ]),
          reauthRequiredReason: 'token_invalidated',
        }),
        expect.objectContaining({
          providerAccountRef: 'provider-account_ref_low',
          eligibility: 'ineligible',
          eligibilityReasons: expect.arrayContaining(['low_credit']),
        }),
        expect.objectContaining({
          providerAccountRef: 'provider-account_ref_rate_limited',
          eligibility: 'ineligible',
          eligibilityReasons: expect.arrayContaining([
            'health:unhealthy',
            'cooldown',
          ]),
          recentFailureClass: 'rate_limited',
        }),
        expect.objectContaining({
          providerAccountRef: 'provider-account_ref_reauth',
          eligibility: 'ineligible',
          eligibilityReasons: expect.arrayContaining([
            'health:requires_reauth',
          ]),
          reauthRequiredReason: 'token_invalidated',
        }),
        expect.objectContaining({
          providerAccountRef: 'provider-account_ref_busy',
          eligibility: 'ineligible',
          eligibilityReasons: expect.arrayContaining(['lease_limit_reached']),
          activeLeaseCount: 1,
          leaseLimit: 1,
        }),
      ]),
    )
    expect(body.activeLeases).toHaveLength(1)
    expect(text).not.toContain('codex-auth://')
    expect(text).not.toContain('access-token')
    expect(text).not.toContain('auth.json')
    expect(text).not.toContain('grant')
  })

  test('operator reset clears account cooldown and rate-limit markers without secrets', async () => {
    const repository = new FakeProviderAccountRepository()
    const database = new FakeProviderAccountD1()
    database.accounts.push(
      fakeAccountRow(
        'provider_account_rate_limited',
        'provider-account_ref_rate_limited',
        {
          account_label: 'rate limited',
          cooldown_until: '2099-01-01T00:00:00.000Z',
          health: 'unhealthy',
          low_credit_flag: 1,
          recent_failure_class: 'rate_limited',
          refill_note: 'Refill before reuse.',
        },
      ),
    )

    const response = await run(
      repository,
      '/api/operator/accounts/reset',
      {
        body: JSON.stringify({
          email: 'chris@openagents.com',
          providerAccountRef: 'provider-account_ref_rate_limited',
        }),
        method: 'POST',
      },
      undefined,
      database.asD1(),
    )
    const text = await response.text()
    const body = JSON.parse(text)

    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      ok: true,
      providerAccountRef: 'provider-account_ref_rate_limited',
      resetAt: expect.any(String),
    })
    expect(database.accounts[0]).toMatchObject({
      cooldown_until: null,
      health: 'healthy',
      low_credit_flag: 0,
      recent_failure_class: null,
      refill_note: null,
    })
    expect(text).not.toContain('codex-auth://')
    expect(text).not.toContain('access-token')
    expect(text).not.toContain('grant')
  })

  test('operator reset is admin-gated and scoped to the selected target user', async () => {
    const repository = new FakeProviderAccountRepository()
    const database = new FakeProviderAccountD1()
    database.accounts.push(
      fakeAccountRow('provider_account_other', 'provider-account_ref_other', {
        user_id: 'user_other',
        cooldown_until: '2099-01-01T00:00:00.000Z',
        health: 'unhealthy',
        recent_failure_class: 'rate_limited',
      }),
    )

    const unauthorized = await run(
      repository,
      '/api/operator/accounts/reset',
      {
        body: JSON.stringify({
          email: 'chris@openagents.com',
          providerAccountRef: 'provider-account_ref_other',
        }),
        method: 'POST',
      },
      { authorized: false },
      database.asD1(),
    )

    expect(unauthorized.status).toBe(401)

    const crossOwner = await run(
      repository,
      '/api/operator/accounts/reset',
      {
        body: JSON.stringify({
          email: 'chris@openagents.com',
          providerAccountRef: 'provider-account_ref_other',
        }),
        method: 'POST',
      },
      undefined,
      database.asD1(),
    )

    expect(crossOwner.status).toBe(404)
    expect(database.accounts[0]).toMatchObject({
      cooldown_until: '2099-01-01T00:00:00.000Z',
      health: 'unhealthy',
      recent_failure_class: 'rate_limited',
    })
  })

  test('operator reset accepts accountRefHash alias for the Artanis accounts dashboard', async () => {
    const repository = new FakeProviderAccountRepository()
    const database = new FakeProviderAccountD1()
    database.accounts.push(
      fakeAccountRow(
        'provider_account_rate_limited',
        'provider-account_ref_rate_limited',
        {
          cooldown_until: '2099-01-01T00:00:00.000Z',
          health: 'unhealthy',
          recent_failure_class: 'rate_limited',
        },
      ),
    )

    const response = await run(
      repository,
      '/api/operator/accounts/reset',
      {
        body: JSON.stringify({
          accountRefHash: 'provider-account_ref_rate_limited',
          email: 'chris@openagents.com',
        }),
        method: 'POST',
      },
      undefined,
      database.asD1(),
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toMatchObject({
      ok: true,
      providerAccountRef: 'provider-account_ref_rate_limited',
    })
    expect(database.accounts[0]).toMatchObject({
      cooldown_until: null,
      health: 'healthy',
      recent_failure_class: null,
    })
  })

  test('operator reset rejects non-POST methods and missing provider account ref', async () => {
    const repository = new FakeProviderAccountRepository()
    const database = new FakeProviderAccountD1()

    const wrongMethod = await run(
      repository,
      '/api/operator/accounts/reset',
      { method: 'GET' },
      undefined,
      database.asD1(),
    )

    expect(wrongMethod.status).toBe(405)

    const missingRef = await run(
      repository,
      '/api/operator/accounts/reset',
      {
        body: JSON.stringify({ email: 'chris@openagents.com' }),
        method: 'POST',
      },
      undefined,
      database.asD1(),
    )

    expect(missingRef.status).toBe(400)
    await expect(missingRef.json()).resolves.toEqual({
      error: 'bad_request',
      reason: 'providerAccountRef or accountRefHash is required',
    })
  })

  test('lease acquisition skips accounts with stale reconnect markers', async () => {
    const repository = new FakeProviderAccountRepository()
    const database = new FakeProviderAccountD1()
    database.accounts.push(
      fakeAccountRow(
        'provider_account_stale_reauth',
        'provider-account_ref_stale_reauth',
        {
          account_label: 'stale reauth marker',
          operator_priority: 1,
          reauth_required_reason: 'token_invalidated',
        },
      ),
      fakeAccountRow(
        'provider_account_healthy',
        'provider-account_ref_healthy',
        {
          account_label: 'healthy',
          operator_priority: 100,
        },
      ),
    )

    const response = await run(
      repository,
      '/api/operator/provider-accounts/chatgpt-codex/leases',
      {
        body: JSON.stringify({
          email: 'chris@openagents.com',
          requestedAction: 'customer_order_fulfillment',
        }),
        method: 'POST',
      },
      undefined,
      database.asD1(),
    )
    const text = await response.text()
    const body = JSON.parse(text)

    expect(response.status).toBe(201)
    expect(body).toMatchObject({
      providerAccountRef: 'provider-account_ref_healthy',
      requestedAction: 'customer_order_fulfillment',
    })
    expect(text).not.toContain('codex-auth://')
    expect(text).not.toContain('access-token')
    expect(text).not.toContain('grant')
  })

  test('leases and grants a required Google Gemini provider account', async () => {
    const repository = new FakeProviderAccountRepository()
    const database = new FakeProviderAccountD1()
    repository.accounts.push(
      connectedAccount({
        authMode: 'api_key',
        id: 'provider_account_gemini',
        provider: 'google_gemini',
        providerAccountRef: 'provider-account_ref_gemini',
        secretRef:
          'provider-account://google-gemini/user-api-key/provider-account_ref_gemini',
      }),
    )
    database.accounts.push(
      fakeAccountRow('provider_account_codex', 'provider-account_ref_codex', {
        operator_priority: 1,
      }),
      fakeAccountRow('provider_account_gemini', 'provider-account_ref_gemini', {
        operator_priority: 50,
        provider: 'google_gemini',
        secret_ref:
          'provider-account://google-gemini/user-api-key/provider-account_ref_gemini',
      }),
    )

    const leaseResponse = await run(
      repository,
      '/api/operator/provider-accounts/chatgpt-codex/leases',
      {
        body: JSON.stringify({
          email: 'chris@openagents.com',
          requestedAction: 'm13.google_gemini_live_run',
          requiredProvider: 'google_gemini',
          runId: 'run.m13.google_gemini.live',
        }),
        method: 'POST',
      },
      undefined,
      database.asD1(),
    )
    const leaseText = await leaseResponse.text()
    const leaseBody = JSON.parse(leaseText) as { leaseRef: string }

    expect(leaseResponse.status).toBe(201)
    expect(leaseBody).toMatchObject({
      providerAccountRef: 'provider-account_ref_gemini',
      requestedAction: 'm13.google_gemini_live_run',
      runId: 'run.m13.google_gemini.live',
    })
    expect(leaseText).not.toContain('user-api-key')

    const grantResponse = await run(
      repository,
      '/api/operator/provider-accounts/chatgpt-codex/leases/grant',
      {
        body: JSON.stringify({
          email: 'chris@openagents.com',
          leaseRef: leaseBody.leaseRef,
          runnerSessionId: 'run.m13.google_gemini.live',
        }),
        method: 'POST',
      },
      undefined,
      database.asD1(),
    )
    const grantText = await grantResponse.text()
    const grantBody = JSON.parse(grantText)

    expect(grantResponse.status).toBe(201)
    expect(grantBody).toMatchObject({
      leaseRef: leaseBody.leaseRef,
      providerAccountRef: 'provider-account_ref_gemini',
      grant: {
        grantRef: expect.stringContaining('provider-auth-grant_'),
        requestedAction: 'm13.google_gemini_live_run',
        runnerSessionId: 'run.m13.google_gemini.live',
        status: 'issued',
      },
    })
    expect(grantText).not.toContain('user-api-key')
    expect(repository.grants).toHaveLength(1)
    expect(repository.grants[0]).toMatchObject({
      provider: 'google_gemini',
      providerAccountRef: 'provider-account_ref_gemini',
      requestedAction: 'm13.google_gemini_live_run',
      runnerSessionId: 'run.m13.google_gemini.live',
      status: 'issued',
    })
  })

  test('rejects an unknown required provider instead of leasing any account', async () => {
    const repository = new FakeProviderAccountRepository()
    const database = new FakeProviderAccountD1()
    database.accounts.push(
      fakeAccountRow('provider_account_codex', 'provider-account_ref_codex'),
    )

    const response = await run(
      repository,
      '/api/operator/provider-accounts/chatgpt-codex/leases',
      {
        body: JSON.stringify({
          email: 'chris@openagents.com',
          requestedAction: 'm13.google_gemini_live_run',
          requiredProvider: 'gemini',
          runId: 'run.m13.google_gemini.live',
        }),
        method: 'POST',
      },
      undefined,
      database.asD1(),
    )
    const text = await response.text()

    expect(response.status).toBe(400)
    expect(text).toContain('requiredProvider')
    expect(database.leases).toHaveLength(0)
  })
})
