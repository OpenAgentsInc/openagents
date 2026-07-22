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
import { resolvePortableCapabilityGrantFacts } from './portable-capability-grant-facts'

const at = '2026-07-13T12:00:00.000Z'

const account: ProviderAccountRecord = {
  id: 'provider-account-row_1',
  userId: 'owner_1',
  teamId: null,
  provider: 'chatgpt_codex',
  authMode: 'codex_device_auth',
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
  test('returns active owner facts without consuming grants or exposing material', async () => {
    let used = 0
    const provider = {
      findGrantByRef: async (ref: string) => ref === sourceGrant.grantRef ? sourceGrant : undefined,
      findAccountByRef: async () => account,
      markGrantUsed: async () => { used += 1; return sourceGrant },
    }
    const github = {
      findGrantByRef: async () => undefined,
      findUsableConnectionForUser: async () => undefined,
    }
    const facts = await resolvePortableCapabilityGrantFacts({
      ownerUserId: account.userId,
      grantRefs: [sourceGrant.grantRef],
      provider,
      github,
      now: () => new Date(at),
    })
    expect(facts).toEqual([{
      grantRef: sourceGrant.grantRef,
      kind: 'provider',
      ownerUserId: account.userId,
      status: 'issued',
      expiresAt: sourceGrant.expiresAt,
      providerAccountRef: account.providerAccountRef,
      runnerSessionId: sourceGrant.runnerSessionId,
    }])
    expect(used).toBe(0)
    expect(JSON.stringify(facts)).not.toMatch(/secret|material|token/i)
  })

  test('rejects duplicate, foreign, expired, and unusable facts', async () => {
    const provider = {
      findGrantByRef: async () => sourceGrant,
      findAccountByRef: async () => account,
    }
    const github = {
      findGrantByRef: async () => undefined,
      findUsableConnectionForUser: async () => undefined,
    }
    const base = { ownerUserId: account.userId, provider, github, now: () => new Date(at) }
    await expect(resolvePortableCapabilityGrantFacts({
      ...base, grantRefs: [sourceGrant.grantRef, sourceGrant.grantRef],
    })).rejects.toThrow(/scope is invalid/)
    await expect(resolvePortableCapabilityGrantFacts({
      ...base, ownerUserId: 'owner_other', grantRefs: [sourceGrant.grantRef],
    })).rejects.toThrow(/not active/)
    await expect(resolvePortableCapabilityGrantFacts({
      ...base, grantRefs: [sourceGrant.grantRef], now: () => new Date(sourceGrant.expiresAt),
    })).rejects.toThrow(/not active/)
    await expect(resolvePortableCapabilityGrantFacts({
      ...base, grantRefs: [sourceGrant.grantRef],
      provider: { ...provider, findAccountByRef: async () => undefined },
    })).rejects.toThrow(/not usable/)
  })

  test('reads GitHub facts from its repository without marking the grant used', async () => {
    let used = 0
    const githubGrant = {
      id: 'github-grant-row_1', connectionId: 'github-connection-row_1',
      userId: account.userId, runnerSessionId: 'session.github',
      connectionRef: 'github-connection_1', secretRef: 'github-secret_1',
      grantRef: 'github-write-grant_1', status: 'issued' as const,
      requestedAction: 'portable_session_source', metadataJson: null,
      createdAt: at, updatedAt: at, expiresAt: sourceGrant.expiresAt,
      usedAt: null, revokedAt: null, failedAt: null,
    }
    const github = {
      findGrantByRef: async () => githubGrant,
      findUsableConnectionForUser: async () => ({
        id: githubGrant.connectionId, userId: account.userId, githubId: '1',
        githubLogin: 'owner', connectionRef: githubGrant.connectionRef,
        secretRef: githubGrant.secretRef, scopes: ['repo'], status: 'connected' as const,
        health: 'healthy' as const, connectedAt: at, disconnectedAt: null,
        lastStatusAt: at, metadataJson: null, createdAt: at, updatedAt: at, deletedAt: null,
      }),
      markGrantUsed: async () => { used += 1; return githubGrant },
    }
    const provider = {
      findGrantByRef: async () => undefined,
      findAccountByRef: async () => undefined,
    }
    const facts = await resolvePortableCapabilityGrantFacts({
      ownerUserId: account.userId,
      grantRefs: [githubGrant.grantRef],
      provider,
      github,
      now: () => new Date(at),
    })
    expect(facts).toEqual([{
      grantRef: githubGrant.grantRef,
      kind: 'github',
      ownerUserId: account.userId,
      status: 'issued',
      expiresAt: githubGrant.expiresAt,
      runnerSessionId: githubGrant.runnerSessionId,
    }])
    expect(used).toBe(0)
  })

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
