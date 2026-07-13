import { describe, expect, test } from 'vitest'

import type {
  ProviderAccountAuthGrantRecord,
  ProviderAccountEventRecord,
  ProviderAccountRecord,
  ProviderAccountRepository,
} from './provider-account-domain'
import {
  reissueProviderAccountGrant,
  revokeProviderAccountGrant,
} from './provider-account-service'

const at = '2026-07-13T12:00:00.000Z'

const account: ProviderAccountRecord = {
  id: 'provider-account-row_1',
  userId: 'owner_1',
  teamId: null,
  provider: 'chatgpt_codex',
  authMode: 'oauth_device',
  status: 'connected',
  health: 'healthy',
  providerAccountRef: 'provider-account_1',
  secretRef: 'codex-auth://provider-account_1',
  accountLabel: null,
  planType: null,
  connectedAt: at,
  disconnectedAt: null,
  deniedAt: null,
  lastStatusAt: at,
  metadataJson: null,
  createdAt: at,
  updatedAt: at,
  deletedAt: null,
}

const sourceGrant: ProviderAccountAuthGrantRecord = {
  id: 'provider-grant-row_source',
  providerAccountId: account.id,
  userId: account.userId,
  teamId: null,
  threadId: 'thread_1',
  workroomId: 'work-context_1',
  runnerSessionId: 'session.source',
  provider: 'chatgpt_codex',
  providerAccountRef: account.providerAccountRef,
  providerSecretRef: 'codex-auth://provider-account_1',
  grantRef: 'codex-auth-grant_source',
  status: 'issued',
  requestedAction: 'portable_session_source',
  metadataJson: null,
  createdAt: at,
  updatedAt: at,
  expiresAt: '2026-07-13T14:00:00.000Z',
  usedAt: null,
  revokedAt: null,
  failedAt: null,
}

describe('portable capability grant authority', () => {
  test('revokes exactly one owner grant and reissues one replay-stable destination ref', async () => {
    const grants = [sourceGrant]
    const events: ProviderAccountEventRecord[] = []
    const repository = {
      findAccountByRef: async (userId: string, providerAccountRef: string) =>
        userId === account.userId && providerAccountRef === account.providerAccountRef
          ? account
          : undefined,
      findGrantByRef: async (grantRef: string) =>
        grants.find(grant => grant.grantRef === grantRef),
      createAuthGrant: async (
        grant: ProviderAccountAuthGrantRecord,
        event: ProviderAccountEventRecord,
      ) => {
        grants.push(grant)
        events.push(event)
        return grant
      },
      revokeGrant: async (
        grant: ProviderAccountAuthGrantRecord,
        event: ProviderAccountEventRecord,
      ) => {
        const index = grants.findIndex(candidate => candidate.id === grant.id)
        grants.splice(index, 1, grant)
        events.push(event)
        return grant
      },
    } as ProviderAccountRepository

    const revoked = await revokeProviderAccountGrant(
      repository,
      {
        actorId: 'service.portable-move',
        userId: account.userId,
        grantRef: sourceGrant.grantRef,
      },
      {
        makeId: () => 'provider-event_revoke',
        now: () => new Date('2026-07-13T12:01:00.000Z'),
      },
    )
    expect(revoked?.status).toBe('revoked')
    expect(account.status).toBe('connected')

    const input = {
      actorId: 'service.portable-move',
      userId: account.userId,
      sourceGrantRef: sourceGrant.grantRef,
      destinationGrantRef: 'codex-auth-grant_destination',
      requestedAction: 'portable_session_resume',
      runnerSessionId: 'session.destination',
    } as const
    const first = await reissueProviderAccountGrant(repository, input, {
      makeId: prefix => `${prefix}_destination`,
      now: () => new Date('2026-07-13T12:02:00.000Z'),
    })
    const replay = await reissueProviderAccountGrant(repository, input, {
      now: () => new Date('2026-07-13T12:03:00.000Z'),
    })

    expect(first?.grantRef).toBe('codex-auth-grant_destination')
    expect(replay?.grantRef).toBe(first?.grantRef)
    expect(grants).toHaveLength(2)
    expect(events.map(event => event.kind)).toEqual([
      'auth_grant_revoked',
      'auth_grant_issued',
    ])
    expect(JSON.stringify([revoked, first, replay])).not.toContain(
      'providerSecretRef',
    )
    await expect(
      reissueProviderAccountGrant(repository, {
        ...input,
        runnerSessionId: 'session.conflict',
      }),
    ).rejects.toThrow(/replay scope conflicts/)
    await expect(
      revokeProviderAccountGrant(repository, {
        actorId: 'service.portable-move',
        userId: 'owner_other',
        grantRef: first!.grantRef,
      }),
    ).resolves.toBeUndefined()
  })
})
