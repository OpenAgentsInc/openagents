import { describe, expect, test } from 'vitest'

import {
  CHATGPT_CODEX_PROVIDER,
  type ProviderAccountBundle,
  type PublicProviderAccount,
} from './provider-accounts'
import {
  connectedProviderAccountRef,
  latestProviderAccountHealthSummary,
  providerAccountLaunchBlockMessage,
  providerAccountReconnectReason,
} from './provider-launch'

const account = (
  overrides: Partial<PublicProviderAccount> = {},
): PublicProviderAccount => ({
  authMode: 'chatgpt_device_code',
  connectedAt: '2026-06-04T00:00:00.000Z',
  createdAt: '2026-06-04T00:00:00.000Z',
  hasSecretRef: true,
  health: 'healthy',
  id: 'provider_account_1',
  lastStatusAt: '2026-06-04T00:00:00.000Z',
  provider: CHATGPT_CODEX_PROVIDER,
  providerAccountRef: 'provider-account_1',
  publicStatus: 'connected',
  status: 'connected',
  updatedAt: '2026-06-04T00:00:00.000Z',
  ...overrides,
})

const bundle = (
  accounts: ReadonlyArray<PublicProviderAccount>,
): ProviderAccountBundle => ({
  accounts,
  attempts: [],
})

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

const makeResult = <T>(results: Array<T> = []): D1Result<T> => ({
  meta: d1Meta(),
  results,
  success: true,
})

const jsonFixture = <T>(value: unknown): T => JSON.parse(JSON.stringify(value))

const makeHealthSummaryD1 = (
  summary: string | null,
): Readonly<{ db: D1Database; values: Array<ReadonlyArray<unknown>> }> => {
  const values: Array<ReadonlyArray<unknown>> = []
  const db: D1Database = {
    batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
      Promise.all(statements.map(statement => statement.run<T>())),
    dump: () => Promise.resolve(new ArrayBuffer(0)),
    exec: () => Promise.resolve({ count: 0, duration: 0 }),
    prepare: () => {
      let bound: ReadonlyArray<unknown> = []

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
          bound = nextValues

          return statement
        },
        first: <T = Record<string, unknown>>() => {
          values.push(bound)

          return Promise.resolve(
            summary === null ? null : jsonFixture<T>({ summary }),
          )
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

  return { db, values }
}

describe('provider launch helpers', () => {
  test('selects connected provider account refs', () => {
    expect(
      connectedProviderAccountRef(
        bundle([
          account({
            health: 'requires_reauth',
            providerAccountRef: 'provider-account_bad',
          }),
          account({ providerAccountRef: 'provider-account_good' }),
        ]),
        undefined,
      ),
    ).toBe('provider-account_good')
    expect(
      connectedProviderAccountRef(
        bundle([account({ id: 'account_id', providerAccountRef: 'ref_1' })]),
        'account_id',
      ),
    ).toBe('ref_1')
  })

  test('explains reconnect reasons from status and latest health', () => {
    expect(
      providerAccountReconnectReason(
        account({ health: 'requires_reauth' }),
        undefined,
      ),
    ).toBe('The saved ChatGPT login needs to be refreshed.')
    expect(
      providerAccountReconnectReason(
        account(),
        'token_invalidated while checking account',
      ),
    ).toBe('ChatGPT/Codex account token was invalidated by OpenAI.')
  })

  test('reads latest health summary and builds launch block message', async () => {
    const { db, values } = makeHealthSummaryD1('token_invalidated')
    const message = await providerAccountLaunchBlockMessage(
      db,
      'github:1',
      bundle([account({ accountLabel: 'Main ChatGPT' })]),
      undefined,
    )

    expect(message).toContain('Main ChatGPT cannot launch Autopilot.')
    expect(message).toContain('token was invalidated')
    expect(values[0]).toEqual([
      'github:1',
      'provider_account_1',
      'provider-account_1',
    ])
    await expect(
      latestProviderAccountHealthSummary(db, 'github:1', account()),
    ).resolves.toBe('token_invalidated')
  })
})
