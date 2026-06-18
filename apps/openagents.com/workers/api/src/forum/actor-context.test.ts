import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type AgentCredentialLookup,
  type AgentRegistrationRecord,
  type AgentRegistrationStore,
  createProgrammaticAgentRegistration,
} from '../agent-registration'
import {
  type ForumHumanSessionActor,
  ForumWriterAuthFailure,
  ForumWriterGrant,
  type ForumWriterGrant as ForumWriterGrantType,
  authenticateForumAgentToken,
  buildForumWriterContext,
  humanForumWriterActor,
} from './index'

class MemoryAgentRegistrationStore implements AgentRegistrationStore {
  readonly registrations: Array<AgentRegistrationRecord> = []
  readonly lookups = new Map<string, AgentCredentialLookup>()
  readonly touches: Array<
    Readonly<{ credentialId: string; lastUsedAt: string }>
  > = []

  createAgentRegistration(record: AgentRegistrationRecord): Promise<void> {
    this.registrations.push(record)
    this.lookups.set(record.credential.tokenHash, {
      user: record.user,
      credentialId: record.credential.id,
      profileMetadataJson: record.profile.metadataJson,
      tokenPrefix: record.credential.tokenPrefix,
    })

    return Promise.resolve()
  }

  findAgentByTokenHash(
    tokenHash: string,
  ): Promise<AgentCredentialLookup | undefined> {
    return Promise.resolve(this.lookups.get(tokenHash))
  }

  touchAgentCredential(
    credentialId: string,
    lastUsedAt: string,
  ): Promise<void> {
    this.touches.push({ credentialId, lastUsedAt })

    return Promise.resolve()
  }

  updateAgentDisplayName(): Promise<number> {
    return Promise.resolve(0)
  }
}

const makeUuidFactory = (values: ReadonlyArray<string>) => {
  const queue = Array.from(values)

  return (): string => {
    const value = queue.shift()

    if (value === undefined) {
      throw new Error('uuid factory exhausted')
    }

    return value
  }
}

const voidForumId = '55555555-1111-4111-8111-555555555555'

const activeVoidGrant = (
  overrides: Record<string, unknown> = {},
): ForumWriterGrantType =>
  S.decodeUnknownSync(ForumWriterGrant)({
    expiresAtEpochMillis: 1_800_000_000_000,
    forumIds: [voidForumId],
    ownerUserId: null,
    scopes: ['forum.read', 'forum.void.write'],
    status: 'active',
    teamId: null,
    ...overrides,
  })

const humanSession: ForumHumanSessionActor = {
  email: 'ben@example.com',
  login: 'ben-silone',
  name: 'Ben Silone',
  userId: 'user_ben',
}

const makeAgent = async () => {
  const store = new MemoryAgentRegistrationStore()
  const registration = await createProgrammaticAgentRegistration(
    store,
    {
      displayName: 'Void Posting Agent',
      externalId: 'void-agent-1',
      slug: 'void-posting-agent',
    },
    {
      makeToken: () => 'oa_agent_forum_secret',
      makeUuid: makeUuidFactory(['agent-user', 'credential', 'identity']),
      now: () => '2026-06-05T22:30:00.000Z',
    },
  )

  return { registration, store }
}

describe('Forum actor context', () => {
  test('creates a Forum writer context from a valid registered agent token', async () => {
    const { registration, store } = await makeAgent()
    const actor = await Effect.runPromise(
      authenticateForumAgentToken(
        store,
        registration.credential.token,
        () => '2026-06-05T22:31:00.000Z',
      ),
    )
    const context = await Effect.runPromise(
      buildForumWriterContext({
        actor,
        grant: activeVoidGrant(),
        nowEpochMillis: () => 1_780_000_000_000,
        requiredScope: 'forum.void.write',
        targetForumId: voidForumId,
      }),
    )

    expect(context).toMatchObject({
      actor: {
        actorRef: `agent:${registration.user.id}`,
        displayName: 'Void Posting Agent',
        groupRefs: ['agents'],
        isAgent: true,
        slug: 'void-posting-agent',
      },
      actorKind: 'agent',
      authKind: 'agent_bearer_token',
      grantedScopes: ['forum.read', 'forum.void.write'],
      targetForumId: voidForumId,
    })
    expect(JSON.stringify(context)).not.toContain('forum_secret')
    expect(store.touches).toStrictEqual([
      {
        credentialId: registration.credential.id,
        lastUsedAt: '2026-06-05T22:31:00.000Z',
      },
    ])
  })

  test('creates a Forum writer context from a valid human browser session', async () => {
    const actor = await Effect.runPromise(humanForumWriterActor(humanSession))
    const context = await Effect.runPromise(
      buildForumWriterContext({
        actor,
        grant: activeVoidGrant({ ownerUserId: humanSession.userId }),
        nowEpochMillis: () => 1_780_000_000_000,
        requiredScope: 'forum.void.write',
        targetForumId: voidForumId,
        targetOwnerUserId: humanSession.userId,
      }),
    )

    expect(context).toMatchObject({
      actor: {
        actorRef: 'user:user_ben',
        displayName: 'Ben Silone',
        groupRefs: ['humans'],
        isAgent: false,
        slug: 'ben-silone',
      },
      actorKind: 'human',
      authKind: 'browser_session',
    })
  })

  test('fails closed for missing, malformed, and inactive agent credentials', async () => {
    const { store } = await makeAgent()

    await expect(
      Effect.runPromise(authenticateForumAgentToken(store, undefined)),
    ).rejects.toMatchObject({
      failureKind: 'missing_credentials',
    })
    await expect(
      Effect.runPromise(
        authenticateForumAgentToken(store, 'not-an-agent-token'),
      ),
    ).rejects.toMatchObject({
      failureKind: 'malformed_credentials',
    })
    await expect(
      Effect.runPromise(authenticateForumAgentToken(store, 'oa_agent_missing')),
    ).rejects.toMatchObject({
      failureKind: 'expired_credentials',
    })
  })

  test('fails closed for missing grant, under-scoped grant, expired grant, and wrong forum', async () => {
    const actor = await Effect.runPromise(humanForumWriterActor(humanSession))
    const baseInput = {
      actor,
      nowEpochMillis: () => 1_780_000_000_000,
      requiredScope: 'forum.void.write' as const,
      targetForumId: voidForumId,
    }

    await expect(
      Effect.runPromise(
        buildForumWriterContext({ ...baseInput, grant: undefined }),
      ),
    ).rejects.toMatchObject({
      failureKind: 'under_scoped',
    })
    await expect(
      Effect.runPromise(
        buildForumWriterContext({
          ...baseInput,
          grant: activeVoidGrant({ scopes: ['forum.read'] }),
        }),
      ),
    ).rejects.toMatchObject({
      failureKind: 'under_scoped',
    })
    await expect(
      Effect.runPromise(
        buildForumWriterContext({
          ...baseInput,
          grant: activeVoidGrant({ expiresAtEpochMillis: 1_700_000_000_000 }),
        }),
      ),
    ).rejects.toMatchObject({
      failureKind: 'expired_credentials',
    })
    await expect(
      Effect.runPromise(
        buildForumWriterContext({
          ...baseInput,
          grant: activeVoidGrant({
            forumIds: ['33333333-3333-4333-8333-333333333333'],
          }),
        }),
      ),
    ).rejects.toMatchObject({
      failureKind: 'wrong_forum',
    })
  })

  test('fails closed for wrong owner/team bindings and payment proof without permission', async () => {
    const actor = await Effect.runPromise(humanForumWriterActor(humanSession))
    const baseInput = {
      actor,
      grant: activeVoidGrant({
        ownerUserId: 'user_expected',
        teamId: 'team_expected',
      }),
      nowEpochMillis: () => 1_780_000_000_000,
      requiredScope: 'forum.void.write' as const,
      targetForumId: voidForumId,
    }

    await expect(
      Effect.runPromise(
        buildForumWriterContext({
          ...baseInput,
          targetOwnerUserId: 'user_other',
        }),
      ),
    ).rejects.toMatchObject({
      failureKind: 'wrong_owner',
    })
    await expect(
      Effect.runPromise(
        buildForumWriterContext({
          ...baseInput,
          targetOwnerUserId: 'user_expected',
          targetTeamId: 'team_other',
        }),
      ),
    ).rejects.toMatchObject({
      failureKind: 'wrong_team',
    })
    await expect(
      Effect.runPromise(
        buildForumWriterContext({
          actor,
          grant: activeVoidGrant({ scopes: ['forum.read'] }),
          nowEpochMillis: () => 1_780_000_000_000,
          paymentProofRef: 'l402.proof.redacted',
          requiredScope: 'forum.void.write',
          targetForumId: voidForumId,
        }),
      ),
    ).rejects.toBeInstanceOf(ForumWriterAuthFailure)
    await expect(
      Effect.runPromise(
        buildForumWriterContext({
          actor,
          grant: activeVoidGrant({ scopes: ['forum.read'] }),
          nowEpochMillis: () => 1_780_000_000_000,
          paymentProofRef: 'l402.proof.redacted',
          requiredScope: 'forum.void.write',
          targetForumId: voidForumId,
        }),
      ),
    ).rejects.toMatchObject({
      failureKind: 'payment_not_authority',
    })
  })
})
