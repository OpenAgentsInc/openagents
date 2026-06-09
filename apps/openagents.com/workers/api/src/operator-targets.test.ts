import { describe, expect, test } from 'vitest'

import {
  readOperatorTargetByIdentity,
  readOperatorTargetByUserId,
  readOperatorTargetUser,
} from './operator-targets'

type QueryBinding = Readonly<{
  query: string
  values: ReadonlyArray<unknown>
}>

const d1Meta = (): D1Meta & Record<string, unknown> => ({
  changed_db: false,
  changes: 0,
  duration: 0,
  last_row_id: 0,
  rows_read: 1,
  rows_written: 0,
  served_by: 'memory',
  served_by_primary: true,
  size_after: 0,
  timings: { sql_duration_ms: 0 },
})

const jsonFixture = <T>(value: unknown): T => JSON.parse(JSON.stringify(value))

const makeResult = <T>(results: Array<T> = []): D1Result<T> => ({
  meta: d1Meta(),
  results,
  success: true,
})

const makeScriptedD1 = (row: unknown | null) => {
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

          return Promise.resolve(row === null ? null : jsonFixture<T>(row))
        },
        raw,
        run: <T = Record<string, unknown>>() =>
          Promise.resolve(makeResult<T>()),
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

const targetRow = {
  display_name: 'Christopher David',
  github_username: 'AtlantisPleb',
  primary_email: 'chris@openagents.com',
  user_id: 'github:14167547',
}

describe('operator target repository helpers', () => {
  test('reads targets by user id', async () => {
    const { bindings, db } = makeScriptedD1(targetRow)

    await expect(
      readOperatorTargetByUserId(db, 'github:14167547'),
    ).resolves.toEqual({
      displayName: 'Christopher David',
      email: 'chris@openagents.com',
      githubUsername: 'AtlantisPleb',
      userId: 'github:14167547',
    })
    expect(bindings[0]?.values).toEqual(['github:14167547'])
  })

  test('normalizes identity selectors', async () => {
    const { bindings, db } = makeScriptedD1(targetRow)

    await readOperatorTargetByIdentity(db, '@AtlantisPleb')

    expect(bindings[0]?.values).toEqual([
      'atlantispleb',
      'atlantispleb',
      'atlantispleb',
    ])
  })

  test('falls back to the configured default identity', async () => {
    const { bindings, db } = makeScriptedD1(null)

    await expect(
      readOperatorTargetUser(db, {}, 'chris@openagents.com'),
    ).resolves.toBeUndefined()
    expect(bindings[0]?.values).toEqual([
      'chris@openagents.com',
      'chris@openagents.com',
      'chris@openagents.com',
    ])
  })
})
