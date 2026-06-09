import { describe, expect, test } from 'vitest'

import {
  insertTeamChatMessage,
  makeTeamChatMessageId,
  makeTeamChatThreadId,
  publicTeamChatMessage,
  teamChatRunSummaryFromUnknown,
  updateTeamChatMessageRunSummary,
} from './team-chat'

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

type QueryBinding = Readonly<{
  query: string
  values: ReadonlyArray<unknown>
}>

const jsonFixture = <T>(value: unknown): T => JSON.parse(JSON.stringify(value))

const messageRow = (overrides: Record<string, unknown> = {}) => ({
  agent_run_id: null,
  author_avatar_url: null,
  author_github_username: 'AtlantisPleb',
  author_name: 'Christopher David',
  author_user_id: 'user_1',
  autopilot_thread_id: null,
  body: 'Ship the audit work',
  created_at: '2026-06-04T00:00:00.000Z',
  id: 'team_chat_1',
  kind: 'message' as const,
  metadata_json: null,
  project_id: null,
  team_id: 'team_1',
  ...overrides,
})

const makeScriptedD1 = (script: {
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
        all: <T = Record<string, unknown>>() =>
          Promise.resolve(makeResult<T>()),
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

describe('team chat repository helpers', () => {
  test('builds deterministic chat identifiers from injected UUIDs', () => {
    const options = { makeUuid: () => 'uuid_1' }

    expect(makeTeamChatMessageId(options)).toBe('team_chat_uuid_1')
    expect(makeTeamChatThreadId(options)).toBe('uuid_1')
  })

  test('projects public messages with strict run summary metadata', () => {
    const runSummary = {
      backend: 'gcloud_vm',
      durationSeconds: 12,
      eventCount: 4,
      repository: 'OpenAgentsInc/autopilot-omega',
      runId: 'run_1',
      runtime: 'codex',
      status: 'completed',
      tokenTotal: 128,
      toolCallCount: 3,
      updatedAt: '2026-06-04T00:01:00.000Z',
    }

    expect(
      publicTeamChatMessage(
        messageRow({
          metadata_json: JSON.stringify({
            launchError: 'not used',
            runSummary,
          }),
        }),
      ),
    ).toMatchObject({
      launchError: 'not used',
      runSummary,
    })
    expect(teamChatRunSummaryFromUnknown({ runId: 'run_2' })).toBeUndefined()
  })

  test('inserts a message and reloads the public projection', async () => {
    const { bindings, db } = makeScriptedD1({
      first: () => messageRow({ id: 'team_chat_inserted' }),
    })

    const message = await insertTeamChatMessage(
      db,
      {
        authorUserId: 'user_1',
        body: 'Continue',
        kind: 'message',
        teamId: 'team_1',
      },
      {
        makeUuid: () => 'uuid_1',
        now: () => new Date('2026-06-04T00:00:00.000Z'),
      },
    )

    expect(message.id).toBe('team_chat_inserted')
    expect(bindings[0]?.values.slice(0, 9)).toEqual([
      'team_chat_uuid_1',
      'team_1',
      null,
      'user_1',
      'message',
      'Continue',
      null,
      null,
      '{}',
    ])
    expect(bindings[0]?.values[9]).toBe('2026-06-04T00:00:00.000Z')
    expect(bindings[0]?.values[10]).toBe('2026-06-04T00:00:00.000Z')
    expect(bindings[1]?.values).toEqual(['team_chat_uuid_1'])
  })

  test('merges run summaries into existing metadata', async () => {
    const runSummary = {
      backend: 'shc_vm' as const,
      durationSeconds: null,
      eventCount: 1,
      repository: 'repo',
      runId: 'run_1',
      runtime: 'codex',
      status: 'running' as const,
      tokenTotal: 0,
      toolCallCount: 0,
      updatedAt: '2026-06-04T00:02:00.000Z',
    }
    const { bindings, db } = makeScriptedD1({
      first: () =>
        messageRow({
          metadata_json: JSON.stringify({
            launchError: 'old',
            runSummary,
          }),
        }),
    })

    const message = await updateTeamChatMessageRunSummary(
      db,
      {
        messageId: 'team_chat_1',
        metadataJson: JSON.stringify({ launchError: 'old' }),
        runSummary,
      },
      {
        now: () => new Date('2026-06-04T00:03:00.000Z'),
      },
    )

    expect(message?.launchError).toBe('old')
    expect(message?.runSummary?.runId).toBe('run_1')
    expect(bindings[0]?.values[0]).toBe(
      JSON.stringify({
        launchError: 'old',
        runSummary,
      }),
    )
    expect(bindings[0]?.values[1]).toBe('2026-06-04T00:03:00.000Z')
    expect(bindings[0]?.values[2]).toBe('team_chat_1')
  })
})
