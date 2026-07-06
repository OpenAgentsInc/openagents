import type { IdentityDb } from './identity-db'
import { Effect, Layer } from 'effect'
import { WorkerEnvironment } from 'effect-cf'
import { describe, expect, test } from 'vitest'

import { OpenAgentsDatabase } from './bindings'
import {
  ThreadFileRepository,
  type ThreadFileRepositoryShape,
  type ThreadFileRow,
  makeD1ThreadFileRepository,
  setThreadFileDownloadEnabled,
} from './thread-files'

const d1Meta = (): D1Meta & Record<string, unknown> => ({
  changed_db: false,
  changes: 1,
  duration: 0,
  last_row_id: 0,
  rows_read: 0,
  rows_written: 1,
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

const makePreparedStatement = (
  onRun: (values: ReadonlyArray<unknown>) => void,
): D1PreparedStatement => {
  let values: ReadonlyArray<unknown> = []

  function raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  function raw<T = unknown[]>(options?: {
    columnNames?: false
  }): Promise<Array<T>>
  function raw<T = unknown[]>(options?: {
    columnNames?: boolean
  }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
    return options?.columnNames === true
      ? Promise.resolve([[]])
      : Promise.resolve([])
  }

  const statement: D1PreparedStatement = {
    all: <T = Record<string, unknown>>() => Promise.resolve(makeResult<T>()),
    bind: (...nextValues: ReadonlyArray<unknown>) => {
      values = nextValues

      return statement
    },
    first: <T = Record<string, unknown>>() => Promise.resolve<T | null>(null),
    raw,
    run: <T = Record<string, unknown>>() => {
      onRun(values)

      return Promise.resolve(makeResult<T>())
    },
  }

  return statement
}

type QueryBinding = Readonly<{
  query: string
  values: ReadonlyArray<unknown>
}>

const jsonFixture = <T>(value: unknown): T => JSON.parse(JSON.stringify(value))

const threadFileRow = (
  overrides: Partial<ThreadFileRow> = {},
): ThreadFileRow => ({
  checksum_sha256: 'checksum',
  content_type: 'text/plain',
  created_at: '2026-06-04T00:00:00.000Z',
  download_enabled: 1,
  filename: 'notes.txt',
  id: 'file_1',
  object_key: 'thread-files/personal/user_1/thread_1/file_1/notes.txt',
  owner_user_id: 'user_1',
  scope: 'personal',
  size_bytes: 12,
  team_id: null,
  team_ref: null,
  thread_id: 'thread_1',
  ...overrides,
})

const makeScriptedD1 = (script: {
  all?: (query: string, values: ReadonlyArray<unknown>) => Array<unknown>
  first?: (query: string, values: ReadonlyArray<unknown>) => unknown | null
  run?: (query: string, values: ReadonlyArray<unknown>) => void
}): Readonly<{ bindings: Array<QueryBinding>; db: D1Database }> => {
  const bindings: Array<QueryBinding> = []

  const db: D1Database = {
    batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
      Promise.all(statements.map(statement => statement.run<T>())),
    dump: () => Promise.resolve(new ArrayBuffer(0)),
    exec: () => Promise.resolve({ count: 0, duration: 0 }),
    prepare: (query: string) => {
      let values: ReadonlyArray<unknown> = []

      function raw<T = unknown[]>(options: {
        columnNames: true
      }): Promise<[Array<string>, ...Array<T>]>
      function raw<T = unknown[]>(options?: {
        columnNames?: false
      }): Promise<Array<T>>
      function raw<T = unknown[]>(options?: {
        columnNames?: boolean
      }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
        return options?.columnNames === true
          ? Promise.resolve([[]])
          : Promise.resolve([])
      }

      const statement: D1PreparedStatement = {
        all: <T = Record<string, unknown>>() => {
          bindings.push({ query, values })

          return Promise.resolve(
            makeResult<T>(
              jsonFixture<Array<T>>(script.all?.(query, values) ?? []),
            ),
          )
        },
        bind: (...nextValues: ReadonlyArray<unknown>) => {
          values = nextValues

          return statement
        },
        first: <T = Record<string, unknown>>() => {
          bindings.push({ query, values })

          const value = script.first?.(query, values) ?? null

          return Promise.resolve(value === null ? null : jsonFixture<T>(value))
        },
        raw,
        run: <T = Record<string, unknown>>() => {
          bindings.push({ query, values })
          script.run?.(query, values)

          return Promise.resolve(makeResult<T>())
        },
      }

      return statement
    },
    withSession: () => ({
      batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
        Promise.all(statements.map(statement => statement.run<T>())),
      getBookmark: () => null,
      prepare: query => db.prepare(query),
    }),
  }

  return { bindings, db }
}

// CFG-4 Domain 2 (#8519): author display fields come from the Postgres
// identity handle now — serve the fixture author for the users IN-list.
const serviceIdentityDb: IdentityDb = {
  batch: () => Promise.resolve(),
  query: (sql, params = []) =>
    Promise.resolve(
      sql.includes('FROM users') && params.map(String).includes('user_2')
        ? [
            {
              avatar_url: 'https://example.com/avatar.png',
              created_at: '2026-06-01T00:00:00.000Z',
              deleted_at: null,
              display_name: 'Octo Cat',
              github_id: '583231',
              github_username: 'octocat',
              id: 'user_2',
              kind: 'human',
              primary_email: null,
              status: 'active',
            },
          ]
        : [],
    ),
}

describe('ThreadFileRepository', () => {
  test('updates download visibility through the D1 repository', async () => {
    const writes: Array<ReadonlyArray<unknown>> = []
    const db: D1Database = {
      batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
        Promise.all(statements.map(statement => statement.run<T>())),
      dump: () => Promise.resolve(new ArrayBuffer(0)),
      exec: () => Promise.resolve({ count: 0, duration: 0 }),
      prepare: () =>
        makePreparedStatement(values => {
          writes.push(values)
        }),
      withSession: () => ({
        batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
          Promise.all(statements.map(statement => statement.run<T>())),
        getBookmark: () => null,
        prepare: query => db.prepare(query),
      }),
    }
    const repository = makeD1ThreadFileRepository(db, serviceIdentityDb, {
      runtime: {
        now: () => new Date('2026-06-04T00:00:00.000Z'),
        randomId: prefix => `${prefix}_test`,
      },
    })

    await Effect.runPromise(
      repository.setDownloadEnabled({
        downloadEnabled: false,
        fileId: 'file_1',
      }),
    )

    expect(writes).toEqual([[0, '2026-06-04T00:00:00.000Z', 'file_1']])
  })

  test('uses the injected repository service', async () => {
    const updates: Array<
      Readonly<{ downloadEnabled: boolean; fileId: string }>
    > = []
    const unused = () => Effect.die(new Error('unused test repository method'))
    const repository: ThreadFileRepositoryShape = {
      insert: unused,
      insertMessageReferences: unused,
      listPersonal: unused,
      listReferences: unused,
      listTeam: unused,
      readById: unused,
      readDetail: unused,
      setDownloadEnabled: input =>
        Effect.sync(() => {
          updates.push(input)
        }),
    }
    const layer = Layer.succeed(ThreadFileRepository, repository)

    await Effect.runPromise(
      setThreadFileDownloadEnabled({
        downloadEnabled: true,
        fileId: 'file_2',
      }).pipe(Effect.provide(layer)),
    )

    expect(updates).toEqual([{ downloadEnabled: true, fileId: 'file_2' }])
  })

  test('lists team files through the D1 repository', async () => {
    const { bindings, db } = makeScriptedD1({
      all: () => [
        threadFileRow({
          id: 'file_team_1',
          object_key: 'thread-files/team/team_1/thread_1/file_team_1/spec.pdf',
          owner_user_id: 'user_2',
          scope: 'team',
          team_id: 'team_1',
          team_ref: 'core-team',
          thread_id: 'thread_1',
        }),
      ],
    })
    const repository = makeD1ThreadFileRepository(db, serviceIdentityDb)

    const files = await Effect.runPromise(
      repository.listTeam({ teamId: 'team_1', threadId: 'thread_1' }),
    )

    expect(bindings.map(binding => binding.values)).toEqual([
      ['team_1', 'thread_1'],
    ])
    expect(files).toEqual([
      expect.objectContaining({
        detailUrl: '/teams/core-team/files/file_team_1',
        downloadEnabled: true,
        id: 'file_team_1',
        scope: 'team',
        teamId: 'team_1',
        threadId: 'thread_1',
      }),
    ])
  })

  test('provides the repository from the effect-cf D1 binding layer', async () => {
    const { bindings, db } = makeScriptedD1({
      all: () => [
        threadFileRow({
          id: 'file_effect_cf',
          object_key:
            'thread-files/team/team_1/thread_1/file_effect_cf/spec.pdf',
          owner_user_id: 'user_2',
          scope: 'team',
          team_id: 'team_1',
          team_ref: 'core-team',
          thread_id: 'thread_1',
        }),
      ],
    })
    const workerEnvironmentLayer = Layer.succeed(WorkerEnvironment, {
      OPENAGENTS_DB: db,
    })
    const repositoryLayer = ThreadFileRepository.effectCfLayer(serviceIdentityDb).pipe(
      Layer.provide(OpenAgentsDatabase.layer),
      Layer.provide(workerEnvironmentLayer),
    )

    const files = await Effect.runPromise(
      Effect.gen(function* () {
        const repository = yield* ThreadFileRepository

        return yield* repository.listTeam({ teamId: 'team_1' })
      }).pipe(Effect.provide(repositoryLayer)),
    )

    expect(bindings.map(binding => binding.values)).toEqual([['team_1']])
    expect(files).toEqual([
      expect.objectContaining({
        detailUrl: '/teams/core-team/files/file_effect_cf',
        id: 'file_effect_cf',
      }),
    ])
  })

  test('inserts and reloads a file through the D1 repository', async () => {
    const { bindings, db } = makeScriptedD1({
      first: () =>
        threadFileRow({
          checksum_sha256:
            'd2a40d09912d514c42d87a6fdfc3cdbf6d36623c8a6222a3ac40e8fa9f7a5f0e',
          id: 'file_inserted',
          object_key:
            'thread-files/personal/user_1/thread_1/file_inserted/upload.txt',
          size_bytes: 19,
        }),
    })
    const repository = makeD1ThreadFileRepository(db, serviceIdentityDb)

    const file = await Effect.runPromise(
      repository.insert({
        checksumSha256:
          'd2a40d09912d514c42d87a6fdfc3cdbf6d36623c8a6222a3ac40e8fa9f7a5f0e',
        contentType: 'text/plain',
        filename: 'upload.txt',
        id: 'file_inserted',
        objectKey:
          'thread-files/personal/user_1/thread_1/file_inserted/upload.txt',
        ownerUserId: 'user_1',
        scope: 'personal',
        sizeBytes: 19,
        teamId: null,
        threadId: 'thread_1',
      }),
    )

    expect(bindings.map(binding => binding.values)).toEqual([
      [
        'file_inserted',
        'personal',
        'thread_1',
        null,
        'user_1',
        'upload.txt',
        'text/plain',
        19,
        'thread-files/personal/user_1/thread_1/file_inserted/upload.txt',
        'd2a40d09912d514c42d87a6fdfc3cdbf6d36623c8a6222a3ac40e8fa9f7a5f0e',
        expect.any(String),
        expect.any(String),
      ],
      ['file_inserted'],
    ])
    expect(file).toEqual(
      expect.objectContaining({
        id: 'file_inserted',
        ownerUserId: 'user_1',
        sizeBytes: 19,
      }),
    )
  })

  test('assembles detail references through the D1 repository', async () => {
    const { db } = makeScriptedD1({
      all: () => [
        {
          author_avatar_url: 'https://example.com/avatar.png',
          author_github_username: 'octocat',
          author_name: 'Octo Cat',
          author_user_id: 'user_2',
          body: 'Attached the design notes for review.',
          created_at: '2026-06-04T00:05:00.000Z',
          file_id: 'file_team_1',
          id: 'ref_1',
          message_id: 'message_1',
          message_kind: 'message',
          project_id: null,
          project_ref: null,
          reference_kind: 'message_attachment',
          team_id: 'team_1',
          team_ref: 'core-team',
          thread_id: 'team:team_1:chat',
        },
      ],
    })
    const repository = makeD1ThreadFileRepository(db, serviceIdentityDb)

    const detail = await Effect.runPromise(
      repository.readDetail({
        readActiveTeamMembershipRole: () => Promise.resolve('admin'),
        row: threadFileRow({
          id: 'file_team_1',
          owner_user_id: 'user_2',
          scope: 'team',
          team_id: 'team_1',
          team_ref: 'core-team',
          thread_id: 'team:team_1:chat',
        }),
        userId: 'user_1',
      }),
    )

    expect(detail.canManage).toBe(true)
    expect(detail.references).toEqual([
      expect.objectContaining({
        author: expect.objectContaining({
          githubUsername: 'octocat',
          userId: 'user_2',
        }),
        fileId: 'file_team_1',
        href: '/teams/core-team/chat#message-message_1',
        messageId: 'message_1',
        referenceKind: 'message_attachment',
      }),
    ])
  })
})
