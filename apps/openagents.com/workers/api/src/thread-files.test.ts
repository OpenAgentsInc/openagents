import type { IdentityDb } from './identity-db'
import { describe, expect, test } from 'vitest'

import {
  insertThreadFileMessageReferences,
  listThreadFileReferences,
} from './thread-files'

type InsertedReference = Readonly<{
  fileId: string
  id: string
  messageId: string
  referenceKind: string
  teamId: string
  threadId: string
}>

type MemoryD1 = D1Database &
  Readonly<{
    insertedReferences: Array<InsertedReference>
  }>

const d1Meta = (): D1Meta & Record<string, unknown> => ({
  changed_db: true,
  changes: 0,
  duration: 0,
  last_row_id: 0,
  rows_read: 0,
  rows_written: 0,
  served_by: 'memory',
  served_by_primary: true,
  size_after: 0,
  timings: { sql_duration_ms: 0 },
})

const makeResult = <T>(results: Array<T> = []): D1Result<T> => ({
  meta: d1Meta(),
  results,
  success: true,
})

const referenceRows = [
  {
    author_avatar_url: 'https://avatars.githubusercontent.com/u/14167547?v=4',
    author_github_username: 'AtlantisPleb',
    author_name: 'Christopher David',
    author_user_id: 'github:14167547',
    body: 'Please inspect notes.txt before the Autopilot run.\n\nUse it as input.',
    created_at: '2026-06-03T00:00:04.000Z',
    file_id: 'file_1',
    id: 'thread_file_message_ref_1',
    message_id: 'team_chat_1',
    message_kind: 'message',
    reference_kind: 'message_attachment',
    team_id: 'team_openagents_core',
    team_ref: 'openagents-core-team',
    thread_id: 'team:team_openagents_core:chat',
  },
]

// CFG-4 Domain 2 (#8519): the author display fields come from the Postgres
// identity handle now — serve the fixture author for the users IN-list.
const referencesIdentityDb: IdentityDb = {
  batch: () => Promise.resolve(),
  query: (sql, params = []) =>
    Promise.resolve(
      sql.includes('FROM users') &&
        params.map(String).includes('github:14167547')
        ? [
            {
              avatar_url:
                'https://avatars.githubusercontent.com/u/14167547?v=4',
              created_at: '2026-06-01T00:00:00.000Z',
              deleted_at: null,
              display_name: 'Christopher David',
              github_id: '14167547',
              github_username: 'AtlantisPleb',
              id: 'github:14167547',
              kind: 'human',
              primary_email: null,
              status: 'active',
            },
          ]
        : [],
    ),
}

const makeStatement = (
  state: Pick<MemoryD1, 'insertedReferences'>,
  query: string,
): D1PreparedStatement => {
  let values: ReadonlyArray<unknown> = []
  let statement: D1PreparedStatement

  statement = {
    all: async <T = Record<string, unknown>>() => {
      if (query.includes('FROM thread_file_message_refs')) {
        // eslint-disable-next-line @typescript-eslint/consistent-type-assertions
        return makeResult<T>(referenceRows as Array<T>)
      }

      return makeResult<T>()
    },
    bind: (...nextValues: ReadonlyArray<unknown>) => {
      values = nextValues

      return statement
    },
    first: async <T = Record<string, unknown>>() =>
      Promise.resolve<T | null>(null),
    // eslint-disable-next-line @typescript-eslint/consistent-type-assertions
    raw: ((options?: { columnNames?: boolean }) =>
      Promise.resolve(
        options?.columnNames ? [[]] : [],
      )) as D1PreparedStatement['raw'],
    run: async <T = Record<string, unknown>>() => {
      if (query.includes('INSERT OR IGNORE INTO thread_file_message_refs')) {
        const [
          id,
          threadId,
          messageId,
          referenceKind,
          _createdAt,
          _updatedAt,
          fileId,
          teamId,
        ] = values

        state.insertedReferences.push({
          fileId: String(fileId),
          id: String(id),
          messageId: String(messageId),
          referenceKind: String(referenceKind),
          teamId: String(teamId),
          threadId: String(threadId),
        })
      }

      return makeResult<T>()
    },
  } satisfies D1PreparedStatement

  return statement
}

const makeMemoryD1 = (): MemoryD1 => {
  const state: Pick<MemoryD1, 'insertedReferences'> = {
    insertedReferences: [],
  }

  return {
    ...state,
    batch: async <T = unknown>(statements: Array<D1PreparedStatement>) =>
      Promise.all(statements.map(statement => statement.run<T>())),
    dump: async () => new ArrayBuffer(0),
    exec: async () => ({ count: 0, duration: 0 }),
    prepare: (query: string) => makeStatement(state, query),
    withSession: () =>
      ({
        batch: async <T = unknown>(statements: Array<D1PreparedStatement>) =>
          Promise.all(statements.map(statement => statement.run<T>())),
        getBookmark: () => null,
        prepare: (query: string) => makeStatement(state, query),
      }) satisfies D1DatabaseSession,
  } satisfies MemoryD1
}

describe('thread file message references', () => {
  test('inserts one team-scoped reference per unique file id', async () => {
    const db = makeMemoryD1()

    await insertThreadFileMessageReferences(db, {
      fileIds: ['file_1', 'file_1', 'file_2', ''],
      messageId: 'team_chat_1',
      referenceKind: 'autopilot_input',
      teamId: 'team_openagents_core',
      threadId: 'team:team_openagents_core:chat',
    })

    expect(db.insertedReferences).toHaveLength(2)
    expect(db.insertedReferences.map(reference => reference.fileId)).toEqual([
      'file_1',
      'file_2',
    ])
    expect(db.insertedReferences[0]).toMatchObject({
      messageId: 'team_chat_1',
      referenceKind: 'autopilot_input',
      teamId: 'team_openagents_core',
      threadId: 'team:team_openagents_core:chat',
    })
  })

  test('lists references with chat anchors and compact excerpts', async () => {
    const references = await listThreadFileReferences(
      makeMemoryD1(),
      referencesIdentityDb,
      'file_1',
    )

    expect(references).toEqual([
      {
        author: {
          avatarUrl: 'https://avatars.githubusercontent.com/u/14167547?v=4',
          githubUsername: 'AtlantisPleb',
          name: 'Christopher David',
          userId: 'github:14167547',
        },
        body: 'Please inspect notes.txt before the Autopilot run.\n\nUse it as input.',
        createdAt: '2026-06-03T00:00:04.000Z',
        excerpt:
          'Please inspect notes.txt before the Autopilot run. Use it as input.',
        fileId: 'file_1',
        href: '/teams/openagents-core-team/chat#message-team_chat_1',
        id: 'thread_file_message_ref_1',
        messageId: 'team_chat_1',
        messageKind: 'message',
        referenceKind: 'message_attachment',
        teamId: 'team_openagents_core',
        threadId: 'team:team_openagents_core:chat',
      },
    ])
  })
})
