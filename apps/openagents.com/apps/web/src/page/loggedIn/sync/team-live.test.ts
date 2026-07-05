import {
  ChangelogEntry,
  EntityId as KhalaSyncEntityId,
  EntityType,
  SyncScope as KhalaSyncScope,
  SyncVersion,
} from '@openagentsinc/khala-sync'
import { describe, expect, test } from 'vitest'

import { authBootstrapFromSession } from '../../../domain/session'
import { TeamChatRoute, TeamFilesRoute } from '../../../route'
import { ReceivedSyncPatch } from '../message'
import { init } from '../model'
import { update } from '../update'
import {
  TEAM_LIVE_CHAT_MESSAGES_COLLECTION,
  TEAM_LIVE_THREAD_FILES_COLLECTION,
  teamLiveMessagesFromLiveFramePayload,
  teamLivePatchFromChangelogEntry,
  teamLiveWireScope,
} from './team-live'

const TEAM_ID = 'team_openagents_core'
const WIRE_SCOPE = `scope.team.${TEAM_ID}`
const LEGACY_SCOPE = `team:${TEAM_ID}`
const TEAM_ROUTE_REF = 'openagents-core-team'

const auth = authBootstrapFromSession({
  email: 'chris@openagents.com',
  name: 'Christopher David',
  userId: 'github:14167547',
})

const authWithTeam = {
  ...auth,
  teams: [
    {
      id: TEAM_ID,
      name: 'OpenAgents Core Team',
      slug: TEAM_ROUTE_REF,
      role: 'owner',
      members: [
        {
          userId: 'github:14167547',
          name: 'Christopher David',
          email: 'chris@openagents.com',
          avatarUrl: null,
          githubUsername: 'AtlantisPleb',
          githubId: '14167547',
          role: 'owner',
          status: 'active',
          joinedAt: '2026-06-03T00:00:00.000Z',
        },
      ],
    },
  ],
}

const entry = (
  input: Readonly<{
    entityId: string
    entityType: string
    op: 'upsert' | 'delete'
    postImageJson?: string
    version: number
  }>,
): ChangelogEntry =>
  new ChangelogEntry({
    scope: KhalaSyncScope.make(WIRE_SCOPE),
    version: SyncVersion.make(input.version),
    entityType: EntityType.make(input.entityType),
    entityId: KhalaSyncEntityId.make(input.entityId),
    op: input.op,
    committedAt: '2026-07-05T00:00:00.000Z',
    ...(input.postImageJson === undefined
      ? {}
      : { postImageJson: input.postImageJson }),
  })

const teamChatMessagePostImage = (
  overrides: Readonly<{
    authorName?: string | null
    authorAvatarUrl?: string | null
    authorGithubUsername?: string | null
    deletedAt?: string | null
    archivedAt?: string | null
  }> = {},
): string =>
  JSON.stringify({
    messageId: 'team_chat_1',
    teamId: TEAM_ID,
    projectId: null,
    authorUserId: 'github:14167547',
    authorName: overrides.authorName === undefined ? 'Ada Lovelace' : overrides.authorName,
    authorAvatarUrl:
      overrides.authorAvatarUrl === undefined
        ? 'https://avatars.example/ada.png'
        : overrides.authorAvatarUrl,
    authorGithubUsername:
      overrides.authorGithubUsername === undefined
        ? 'ada'
        : overrides.authorGithubUsername,
    kind: 'message',
    body: 'Hello team',
    autopilotThreadId: null,
    agentRunId: null,
    createdAt: '2026-07-05T00:00:00.000Z',
    updatedAt: '2026-07-05T00:00:00.000Z',
    deletedAt: overrides.deletedAt ?? null,
    archivedAt: overrides.archivedAt ?? null,
  })

const threadFilePostImage = (
  overrides: Readonly<{ deletedAt?: string | null; fileScope?: 'team' | 'personal' }> = {},
): string =>
  JSON.stringify({
    fileId: 'file_1',
    fileScope: overrides.fileScope ?? 'team',
    threadId: 'team:team_openagents_core:chat',
    teamId: TEAM_ID,
    ownerUserId: 'github:14167547',
    filename: 'report.pdf',
    contentType: 'application/pdf',
    sizeBytes: 1024,
    checksumSha256: null,
    uploadStatus: 'uploaded',
    scanStatus: 'passed',
    downloadEnabled: true,
    createdAt: '2026-07-05T00:00:00.000Z',
    updatedAt: '2026-07-05T00:00:00.000Z',
    deletedAt: overrides.deletedAt ?? null,
  })

describe('teamLiveWireScope', () => {
  test('builds the new engine dotted scope from a team id', () => {
    expect(teamLiveWireScope(TEAM_ID)).toBe(WIRE_SCOPE)
  })
})

describe('teamLivePatchFromChangelogEntry — team_chat_message', () => {
  test('adapts a team_chat_message upsert into a team_chat_messages PUT the legacy reducer accepts directly', () => {
    const patch = teamLivePatchFromChangelogEntry(
      entry({
        entityId: 'team_chat_1',
        entityType: 'team_chat_message',
        op: 'upsert',
        postImageJson: teamChatMessagePostImage(),
        version: 3,
      }),
      LEGACY_SCOPE,
      TEAM_ROUTE_REF,
    )

    expect(patch).toBeDefined()
    expect(patch?.collection).toBe(TEAM_LIVE_CHAT_MESSAGES_COLLECTION)
    expect(patch?.op).toBe('put')
    expect(patch?.scope).toBe(LEGACY_SCOPE)
    expect(patch?.seq).toBe(3)
    expect(patch?.id).toBe('team_chat_1')
    expect(patch?.value).toMatchObject({
      id: 'team_chat_1',
      teamId: TEAM_ID,
      body: 'Hello team',
      author: {
        userId: 'github:14167547',
        name: 'Ada Lovelace',
        avatarUrl: 'https://avatars.example/ada.png',
        githubUsername: 'ada',
      },
    })
  })

  test('falls back to a non-blank author name for a historical row with no author-hydration JOIN', () => {
    const patch = teamLivePatchFromChangelogEntry(
      entry({
        entityId: 'team_chat_1',
        entityType: 'team_chat_message',
        op: 'upsert',
        postImageJson: teamChatMessagePostImage({
          authorName: null,
          authorAvatarUrl: null,
          authorGithubUsername: null,
        }),
        version: 1,
      }),
      LEGACY_SCOPE,
      TEAM_ROUTE_REF,
    )

    const author = (patch?.value as Record<string, unknown>).author as Record<
      string,
      unknown
    >
    expect(author.name).toBeTypeOf('string')
    expect((author.name as string).length).toBeGreaterThan(0)
    expect(author.userId).toBe('github:14167547')
    expect(author.avatarUrl).toBeNull()
    expect(author.githubUsername).toBeNull()
  })

  test('the adapted PUT patch round-trips through the real loggedIn update() reducer with a hydrated author', () => {
    const model = init(TeamChatRoute({ teamRef: TEAM_ROUTE_REF }), authWithTeam)
    const patch = teamLivePatchFromChangelogEntry(
      entry({
        entityId: 'team_chat_1',
        entityType: 'team_chat_message',
        op: 'upsert',
        postImageJson: teamChatMessagePostImage(),
        version: 1,
      }),
      LEGACY_SCOPE,
      TEAM_ROUTE_REF,
    )!

    const [nextModel] = update(model, ReceivedSyncPatch({ patch }))

    expect(nextModel.teamChatMessagesByTeam[TEAM_ID]).toHaveLength(1)
    expect(nextModel.teamChatMessagesByTeam[TEAM_ID]?.[0]).toMatchObject({
      id: 'team_chat_1',
      body: 'Hello team',
      author: {
        userId: 'github:14167547',
        name: 'Ada Lovelace',
        avatarUrl: 'https://avatars.example/ada.png',
        githubUsername: 'ada',
      },
    })
    expect(nextModel.sync.cursors[LEGACY_SCOPE]).toBe(1)
  })

  test('adapts a soft-deleted message into a delete op (message disappears rather than a stale ghost surviving)', () => {
    const patch = teamLivePatchFromChangelogEntry(
      entry({
        entityId: 'team_chat_1',
        entityType: 'team_chat_message',
        op: 'upsert',
        postImageJson: teamChatMessagePostImage({
          deletedAt: '2026-07-05T00:00:05.000Z',
        }),
        version: 4,
      }),
      LEGACY_SCOPE,
      TEAM_ROUTE_REF,
    )

    expect(patch?.op).toBe('delete')
    expect(patch?.id).toBe('team_chat_1')
    expect(patch?.value).toBeUndefined()
  })

  test('adapts an archived message into a delete op', () => {
    const patch = teamLivePatchFromChangelogEntry(
      entry({
        entityId: 'team_chat_1',
        entityType: 'team_chat_message',
        op: 'upsert',
        postImageJson: teamChatMessagePostImage({
          archivedAt: '2026-07-05T00:00:05.000Z',
        }),
        version: 4,
      }),
      LEGACY_SCOPE,
      TEAM_ROUTE_REF,
    )

    expect(patch?.op).toBe('delete')
  })

  test('adapts a hard delete op with no value', () => {
    const patch = teamLivePatchFromChangelogEntry(
      entry({
        entityId: 'team_chat_stale',
        entityType: 'team_chat_message',
        op: 'delete',
        version: 9,
      }),
      LEGACY_SCOPE,
      TEAM_ROUTE_REF,
    )

    expect(patch?.op).toBe('delete')
    expect(patch?.value).toBeUndefined()
  })
})

describe('teamLivePatchFromChangelogEntry — thread_file', () => {
  test('adapts a team-owned thread_file upsert, reconstructing downloadUrl/detailUrl exactly like the legacy publicThreadFile', () => {
    const patch = teamLivePatchFromChangelogEntry(
      entry({
        entityId: 'file_1',
        entityType: 'thread_file',
        op: 'upsert',
        postImageJson: threadFilePostImage(),
        version: 2,
      }),
      LEGACY_SCOPE,
      TEAM_ROUTE_REF,
    )

    expect(patch).toBeDefined()
    expect(patch?.collection).toBe(TEAM_LIVE_THREAD_FILES_COLLECTION)
    expect(patch?.op).toBe('put')
    expect(patch?.id).toBe('file_1')
    expect(patch?.value).toMatchObject({
      id: 'file_1',
      scope: 'team',
      teamId: TEAM_ID,
      filename: 'report.pdf',
      downloadUrl: '/api/thread-files/file_1/download',
      detailUrl: `/teams/${TEAM_ROUTE_REF}/files/file_1`,
      downloadEnabled: true,
    })
  })

  test('reconstructs the personal detailUrl form when fileScope is personal', () => {
    const patch = teamLivePatchFromChangelogEntry(
      entry({
        entityId: 'file_1',
        entityType: 'thread_file',
        op: 'upsert',
        postImageJson: threadFilePostImage({ fileScope: 'personal' }),
        version: 1,
      }),
      LEGACY_SCOPE,
      TEAM_ROUTE_REF,
    )

    expect((patch?.value as Record<string, unknown>).detailUrl).toBe(
      '/files/file_1',
    )
  })

  test('adapts a soft-deleted file into a delete op', () => {
    const patch = teamLivePatchFromChangelogEntry(
      entry({
        entityId: 'file_1',
        entityType: 'thread_file',
        op: 'upsert',
        postImageJson: threadFilePostImage({
          deletedAt: '2026-07-05T00:00:05.000Z',
        }),
        version: 3,
      }),
      LEGACY_SCOPE,
      TEAM_ROUTE_REF,
    )

    expect(patch?.op).toBe('delete')
    expect(patch?.id).toBe('file_1')
  })

  test('the adapted PUT patch round-trips through the real loggedIn update() reducer', () => {
    const model = init(TeamFilesRoute({ teamRef: TEAM_ROUTE_REF }), authWithTeam)
    const patch = teamLivePatchFromChangelogEntry(
      entry({
        entityId: 'file_1',
        entityType: 'thread_file',
        op: 'upsert',
        postImageJson: threadFilePostImage(),
        version: 1,
      }),
      LEGACY_SCOPE,
      TEAM_ROUTE_REF,
    )!

    const [nextModel] = update(model, ReceivedSyncPatch({ patch }))

    expect(nextModel.threadFilesByScope[`team-files:${TEAM_ID}`]).toHaveLength(
      1,
    )
    expect(
      nextModel.threadFilesByScope[`team-files:${TEAM_ID}`]?.[0],
    ).toMatchObject({
      id: 'file_1',
      downloadUrl: '/api/thread-files/file_1/download',
      detailUrl: `/teams/${TEAM_ROUTE_REF}/files/file_1`,
    })
  })
})

describe('teamLivePatchFromChangelogEntry — shared entity-type/decode guardrails', () => {
  test('returns undefined for an unrecognized entity type', () => {
    const patch = teamLivePatchFromChangelogEntry(
      entry({
        entityId: 'whatever',
        entityType: 'team',
        op: 'upsert',
        postImageJson: '{}',
        version: 1,
      }),
      LEGACY_SCOPE,
      TEAM_ROUTE_REF,
    )

    expect(patch).toBeUndefined()
  })

  test('returns undefined for an unparseable upsert post-image', () => {
    const patch = teamLivePatchFromChangelogEntry(
      entry({
        entityId: 'team_chat_1',
        entityType: 'team_chat_message',
        op: 'upsert',
        postImageJson: '{not json',
        version: 1,
      }),
      LEGACY_SCOPE,
      TEAM_ROUTE_REF,
    )

    expect(patch).toBeUndefined()
  })

  test('returns undefined when the post-image fails the entity contract decode', () => {
    const patch = teamLivePatchFromChangelogEntry(
      entry({
        entityId: 'team_chat_1',
        entityType: 'team_chat_message',
        op: 'upsert',
        postImageJson: JSON.stringify({ messageId: 'team_chat_1' }),
        version: 1,
      }),
      LEGACY_SCOPE,
      TEAM_ROUTE_REF,
    )

    expect(patch).toBeUndefined()
  })
})

describe('teamLiveMessagesFromLiveFramePayload', () => {
  test('fans out a DeltaFrame with a chat message + thread file entry into two ReceivedSyncPatch messages', () => {
    const payload = JSON.stringify({
      _tag: 'DeltaFrame',
      scope: WIRE_SCOPE,
      cursor: 2,
      entries: [
        {
          scope: WIRE_SCOPE,
          version: 1,
          entityType: 'team_chat_message',
          entityId: 'team_chat_1',
          op: 'upsert',
          postImageJson: teamChatMessagePostImage(),
          committedAt: '2026-07-05T00:00:00.000Z',
        },
        {
          scope: WIRE_SCOPE,
          version: 2,
          entityType: 'thread_file',
          entityId: 'file_1',
          op: 'upsert',
          postImageJson: threadFilePostImage(),
          committedAt: '2026-07-05T00:00:02.000Z',
        },
      ],
    })

    const messages = teamLiveMessagesFromLiveFramePayload(
      payload,
      LEGACY_SCOPE,
      TEAM_ROUTE_REF,
    )

    expect(messages).toHaveLength(2)
    expect(messages[0]).toMatchObject({
      _tag: 'ReceivedSyncPatch',
      patch: { collection: TEAM_LIVE_CHAT_MESSAGES_COLLECTION },
    })
    expect(messages[1]).toMatchObject({
      _tag: 'ReceivedSyncPatch',
      patch: { collection: TEAM_LIVE_THREAD_FILES_COLLECTION },
    })
  })

  test('ignores ping and mutation-ack frames', () => {
    expect(
      teamLiveMessagesFromLiveFramePayload(
        JSON.stringify({ _tag: 'PingFrame' }),
        LEGACY_SCOPE,
        TEAM_ROUTE_REF,
      ),
    ).toEqual([])
    expect(
      teamLiveMessagesFromLiveFramePayload(
        JSON.stringify({
          _tag: 'MutationAckFrame',
          clientId: 'client-1',
          lastMutationId: 1,
        }),
        LEGACY_SCOPE,
        TEAM_ROUTE_REF,
      ),
    ).toEqual([])
  })

  test('degrades a MustRefetchFrame to a scoped FailedSyncStream message', () => {
    const messages = teamLiveMessagesFromLiveFramePayload(
      JSON.stringify({
        _tag: 'MustRefetchFrame',
        scope: WIRE_SCOPE,
        reason: 'scope_reset',
      }),
      LEGACY_SCOPE,
      TEAM_ROUTE_REF,
    )

    expect(messages).toEqual([
      {
        _tag: 'FailedSyncStream',
        error: 'Team live-tail requested a refetch (scope_reset).',
        scope: LEGACY_SCOPE,
      },
    ])
  })

  test('returns a scoped FailedSyncStream message for an undecodable payload', () => {
    expect(
      teamLiveMessagesFromLiveFramePayload(
        '{not json',
        LEGACY_SCOPE,
        TEAM_ROUTE_REF,
      ),
    ).toEqual([
      {
        _tag: 'FailedSyncStream',
        error: 'Team live-tail message could not be decoded.',
        scope: LEGACY_SCOPE,
      },
    ])
  })
})
