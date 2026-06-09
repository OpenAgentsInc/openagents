import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ShareAccessService,
  type ShareProjectionRecord,
  type ShareViewer,
  audienceLabel,
} from './share-projections'

const now = '2026-06-04T21:00:00.000Z'

const owner: ShareViewer = {
  email: 'owner@openagents.com',
  name: 'Owner',
  userId: 'github:owner',
}

const recipient: ShareViewer = {
  email: 'teammate@openagents.com',
  name: 'Teammate',
  userId: 'github:teammate',
}

const stranger: ShareViewer = {
  email: 'stranger@example.com',
  name: 'Stranger',
  userId: 'github:stranger',
}

const publicAudience = { _tag: 'Public' as const }

const teamAudience = {
  _tag: 'TeamMembers' as const,
  teamId: 'team_openagents_core',
  teamName: 'OpenAgents Core Team',
}

const usersAudience = {
  _tag: 'Users' as const,
  recipients: [
    {
      displayName: 'Teammate',
      email: recipient.email,
      userId: recipient.userId,
    },
  ],
}

const shareRecord = (
  audience: ShareProjectionRecord['audience'],
  overrides: Partial<ShareProjectionRecord> = {},
): ShareProjectionRecord => {
  const projection = {
    schemaVersion: 'openagents.share_projection.v1' as const,
    id: '123e4567-e89b-42d3-a456-426614174000',
    url: 'https://openagents.com/share/123e4567-e89b-42d3-a456-426614174000',
    audience,
    audienceLabel: audienceLabel(audience),
    title: 'Shared run',
    subtitle: 'openagents/autopilot-omega@main · completed',
    source: { kind: 'agent-run' as const, id: 'run_1' },
    status: 'active' as const,
    createdAt: now,
    updatedAt: now,
    messages: [],
    files: [],
    artifacts: [],
    approvals: [],
    receipts: [],
    metrics: {
      eventCount: 0,
      tokenTotal: 0,
      toolCallCount: 0,
    },
  }

  return {
    audience,
    canonicalUrl: projection.url,
    createdAt: now,
    expiresAt: null,
    id: projection.id,
    ownerUserId: owner.userId,
    projectId: null,
    projection,
    redactionPolicyId: 'default',
    revokedAt: null,
    source: projection.source,
    status: 'active',
    summary: null,
    teamId: null,
    title: projection.title,
    updatedAt: now,
    ...overrides,
  }
}

const authorizeView = (record: ShareProjectionRecord, viewer?: ShareViewer) =>
  Effect.gen(function* () {
    const access = yield* ShareAccessService

    return yield* access.authorizeView(
      viewer === undefined
        ? { db: {} as D1Database, record }
        : { db: {} as D1Database, record, viewer },
    )
  }).pipe(
    Effect.match({
      onFailure: left => ({ _tag: 'Left' as const, left }),
      onSuccess: right => ({ _tag: 'Right' as const, right }),
    }),
    Effect.provide(ShareAccessService.layer),
  )

describe('share projection audience labels', () => {
  test('formats public, team, and direct-recipient labels', () => {
    expect(audienceLabel(publicAudience)).toBe('Shared publicly')
    expect(audienceLabel(teamAudience)).toBe(
      'Shared with members of OpenAgents Core Team',
    )
    expect(audienceLabel(usersAudience)).toBe('Shared with Teammate')
    expect(audienceLabel(usersAudience, recipient)).toBe('Shared with you')
  })
})

describe('share projection access', () => {
  test('allows public shares without a signed-in viewer', async () => {
    const result = await Effect.runPromise(
      authorizeView(shareRecord(publicAudience)),
    )

    expect(result._tag).toBe('Right')
    if (result._tag !== 'Right') {
      throw new Error('Expected public share authorization to succeed.')
    }
    expect(result.right.audienceLabel).toBe('Shared publicly')
  })

  test('requires auth before direct-recipient share access', async () => {
    const result = await Effect.runPromise(
      authorizeView(shareRecord(usersAudience)),
    )

    expect(result._tag).toBe('Left')
    if (result._tag !== 'Left') {
      throw new Error('Expected direct-recipient share authorization to fail.')
    }
    expect(result.left._tag).toBe('ShareProjectionAuthenticationRequired')
  })

  test('labels matching direct-recipient shares as shared with you', async () => {
    const result = await Effect.runPromise(
      authorizeView(shareRecord(usersAudience), recipient),
    )

    expect(result._tag).toBe('Right')
    if (result._tag !== 'Right') {
      throw new Error('Expected matching recipient authorization to succeed.')
    }
    expect(result.right.audienceLabel).toBe('Shared with you')
  })

  test('denies non-recipient direct share access', async () => {
    const result = await Effect.runPromise(
      authorizeView(shareRecord(usersAudience), stranger),
    )

    expect(result._tag).toBe('Left')
    if (result._tag !== 'Left') {
      throw new Error('Expected stranger authorization to fail.')
    }
    expect(result.left._tag).toBe('ShareProjectionForbidden')
  })

  test('marks revoked records before returning a projection', async () => {
    const result = await Effect.runPromise(
      authorizeView(
        shareRecord(publicAudience, {
          revokedAt: now,
          status: 'revoked',
        }),
      ),
    )

    expect(result._tag).toBe('Right')
    if (result._tag !== 'Right') {
      throw new Error('Expected revoked public projection to be returned.')
    }
    expect(result.right.status).toBe('revoked')
  })
})
