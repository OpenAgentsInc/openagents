import { describe, expect, test } from 'vitest'

import type {
  ProviderAccountAuthGrantRow,
  ProviderAccountRow,
  ProviderAccountRepository,
} from './provider-account-domain'
import { makeAuthoritativePostgresProviderGrantRepository } from './provider-account-postgres-grant-repository'
import { issueProviderAccountGrant, resolveProviderAccountGrant } from './provider-account-service'

const accountRow = (): ProviderAccountRow => ({
  id: 'account-id-1', user_id: 'github:1', team_id: null,
  provider: 'chatgpt_codex', auth_mode: 'chatgpt_device_code',
  status: 'connected', health: 'healthy',
  provider_account_ref: 'provider-account-1',
  secret_ref: 'codex-auth://provider-account-1', account_label: 'codex-2',
  plan_type: null, connected_at: '2026-07-12T22:00:00.000Z',
  disconnected_at: null, denied_at: null,
  last_status_at: '2026-07-12T22:00:00.000Z', metadata_json: '{}',
  created_at: '2026-07-12T22:00:00.000Z', updated_at: '2026-07-12T22:00:00.000Z',
  deleted_at: null,
})

const row = (): ProviderAccountAuthGrantRow => ({
  id: 'grant-id-1',
  provider_account_id: 'account-id-1',
  user_id: 'github:1',
  team_id: null,
  thread_id: 'thread-1',
  workroom_id: 'workroom-1',
  runner_session_id: 'turn-1',
  provider: 'chatgpt_codex',
  provider_account_ref: 'provider-account-1',
  provider_secret_ref: 'codex-auth://provider-account-1',
  grant_ref: 'codex-auth-grant-1',
  status: 'issued',
  requested_action: 'agent_computer_codex_turn',
  metadata_json: '{}',
  created_at: '2026-07-12T22:00:00.000Z',
  updated_at: '2026-07-12T22:00:00.000Z',
  expires_at: '2026-07-13T00:00:00.000Z',
  used_at: null,
  revoked_at: null,
  failed_at: null,
})

const repositoryFixture = () => {
  let current = row()
  let account = accountRow()
  let auditCount = 0
  const repository = makeAuthoritativePostgresProviderGrantRepository(
    {} as ProviderAccountRepository,
    async (text, params) => {
      if (text.includes('FROM provider_accounts')) {
        return account.user_id === params[0] &&
          (params.length === 1 || account.provider_account_ref === params[1])
          ? [account] : []
      }
      if (text.startsWith('SELECT')) {
        return current.grant_ref === params[0] ? [current] : []
      }
      if (text.includes('WITH issued AS')) {
        current = {
          ...current,
          id: String(params[0]), provider_account_id: String(params[1]),
          user_id: String(params[2]), provider_account_ref: String(params[8]),
          provider_secret_ref: String(params[9]), grant_ref: String(params[10]),
          status: 'issued', created_at: String(params[14]), updated_at: String(params[15]),
          expires_at: String(params[16]), used_at: null,
        }
        auditCount += 1
        return [current]
      }
      if (current.id !== params[2] || current.status !== 'issued') return []
      current = {
        ...current,
        status: 'used',
        used_at: String(params[0]),
        updated_at: String(params[1]),
      }
      auditCount += 1
      return [current]
    },
  )
  return {
    auditCount: () => auditCount,
    current: () => current,
    repository,
  }
}

describe('authoritative Postgres provider grant repository', () => {
  test('atomically consumes the same Postgres row exactly once', async () => {
    const fixture = repositoryFixture()
    const input = {
      actorId: 'agent-1',
      grantRef: 'codex-auth-grant-1',
      providerAccountRef: 'provider-account-1',
      runnerSessionId: 'turn-1',
    }
    const used = await resolveProviderAccountGrant(fixture.repository, input, {
      now: () => new Date('2026-07-12T22:10:00.000Z'),
      makeId: () => 'event-1',
    })
    expect(used?.status).toBe('used')
    expect(fixture.current().status).toBe('used')
    expect(fixture.auditCount()).toBe(1)
    await expect(resolveProviderAccountGrant(fixture.repository, input)).rejects.toMatchObject({
      _tag: 'ProviderGrantNotIssued',
    })
    expect(fixture.auditCount()).toBe(1)
  })

  test('selects custody-backed healthy account and issues into Postgres authority', async () => {
    const fixture = repositoryFixture()
    const issued = await issueProviderAccountGrant(fixture.repository, {
      providerAccountRef: 'provider-account-1', userId: 'github:1',
      runnerSessionId: 'turn-2', requestedAction: 'agent_computer_codex_turn',
    }, {
      now: () => new Date('2026-07-12T22:20:00.000Z'),
      makeId: prefix => `${prefix}-2`,
    })
    expect(issued?.status).toBe('issued')
    expect(fixture.current().provider_secret_ref).toBe('codex-auth://provider-account-1')
    expect(fixture.auditCount()).toBe(1)
  })

  test('account and runner mismatches fail closed without consuming the row', async () => {
    const fixture = repositoryFixture()
    await expect(
      resolveProviderAccountGrant(fixture.repository, {
        actorId: 'agent-1',
        grantRef: 'codex-auth-grant-1',
        providerAccountRef: 'provider-account-other',
        runnerSessionId: 'turn-1',
      }),
    ).rejects.toMatchObject({ _tag: 'ProviderGrantAccountMismatch' })
    await expect(
      resolveProviderAccountGrant(fixture.repository, {
        actorId: 'agent-1',
        grantRef: 'codex-auth-grant-1',
        providerAccountRef: 'provider-account-1',
        runnerSessionId: 'turn-other',
      }),
    ).rejects.toMatchObject({ _tag: 'ProviderGrantRunnerSessionMismatch' })
    expect(fixture.current().status).toBe('issued')
    expect(fixture.auditCount()).toBe(0)
  })
})
