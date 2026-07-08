import { readFileSync } from 'node:fs'
import { DatabaseSync } from 'node:sqlite'

import { describe, expect, test } from 'vitest'

import {
  codexAccessToAuthMaterial,
  deleteConnectedCodexAuthFromCustody,
  issueShortLivedCodexAccessFromCustody,
  makeD1ProviderAccountTokenCustodyStore,
  makeProviderAccountTokenCustodyCipher,
  providerAccountTokenCustodyCipherFromEnv,
  storeConnectedCodexAuthInCustody,
} from './provider-account-token-custody'

class SqliteD1Statement {
  private bound: ReadonlyArray<unknown> = []

  constructor(
    private readonly db: DatabaseSync,
    private readonly sql: string,
  ) {}

  bind(...values: ReadonlyArray<unknown>): SqliteD1Statement {
    this.bound = values.map(value => value === undefined ? null : value)

    return this
  }

  async first<T = Record<string, unknown>>(): Promise<T | null> {
    return (this.db.prepare(this.sql).get(...(this.bound as never[])) ??
      null) as T | null
  }

  async all<T = Record<string, unknown>>(): Promise<{ results: Array<T> }> {
    return {
      results: this.db.prepare(this.sql).all(...(this.bound as never[])) as Array<T>,
    }
  }

  async run(): Promise<{
    readonly meta: { readonly changes: number }
    readonly results: []
    readonly success: true
  }> {
    const result = this.db.prepare(this.sql).run(...(this.bound as never[]))

    return {
      meta: { changes: Number(result.changes ?? 0) },
      results: [],
      success: true,
    }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }

  async batch(
    statements: ReadonlyArray<SqliteD1Statement>,
  ): Promise<ReadonlyArray<unknown>> {
    return Promise.all(statements.map(statement => statement.run()))
  }
}

const migration = readFileSync(
  new URL('../migrations/0283_provider_account_token_custody.sql', import.meta.url),
  'utf8',
)

const base64 = (bytes: Uint8Array): string =>
  btoa(Array.from(bytes, byte => String.fromCharCode(byte)).join(''))

const makeIdFactory = () => {
  let index = 0

  return (prefix: string): string => {
    index += 1

    return `${prefix}_${index}`
  }
}

const makeDb = (): Readonly<{ d1: D1Database; raw: DatabaseSync }> => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(migration)

  return {
    d1: new SqliteD1(raw) as unknown as D1Database,
    raw,
  }
}

const makeCipher = () =>
  makeProviderAccountTokenCustodyCipher({
    keyBytes: new Uint8Array(32).fill(7),
    keyId: 'test-key',
  })

const ownerUserId = 'openauth-owner-1'
const providerAccountRef = 'provider-account_codex_1'

describe('provider account token custody', () => {
  test('stores Codex OAuth refresh tokens encrypted and serves short-lived access only', async () => {
    const { d1, raw } = makeDb()
    const store = makeD1ProviderAccountTokenCustodyStore(d1)
    const cipher = await makeCipher()
    const makeId = makeIdFactory()
    const secretRef = await storeConnectedCodexAuthInCustody(store, cipher, {
      auth: {
        type: 'oauth',
        access: 'access-secret-1',
        refresh: 'refresh-secret-1',
        expires: Date.parse('2026-07-03T18:00:00.000Z'),
        accountId: 'acct_1',
        idToken: 'id-token-secret-1',
      },
      makeId,
      nowIso: '2026-07-03T17:00:00.000Z',
      ownerUserId,
      providerAccountRef,
    })

    expect(secretRef).toBe(`codex-auth://${providerAccountRef}`)

    const rawRows = raw
      .prepare('SELECT * FROM provider_account_token_custody')
      .all()
    const rawJson = JSON.stringify(rawRows)
    expect(rawJson).not.toContain('refresh-secret-1')
    expect(rawJson).not.toContain('access-secret-1')
    expect(rawJson).not.toContain('id-token-secret-1')

    const access = await issueShortLivedCodexAccessFromCustody(store, cipher, {
      makeId,
      now: new Date('2026-07-03T17:10:00.000Z'),
      ownerUserId,
      providerAccountRef,
    })
    const material = codexAccessToAuthMaterial(access)

    expect(access).toMatchObject({
      access: 'access-secret-1',
      accountId: 'acct_1',
      expires: Date.parse('2026-07-03T18:00:00.000Z'),
      idToken: 'id-token-secret-1',
      providerAccountRef,
    })
    expect(material.authContentEnv).toBe('OPENCODE_AUTH_CONTENT')
    expect(material.authContentJson).toContain('access-secret-1')
    expect(material.authContentJson).not.toContain('refresh-secret-1')

    const auditRows = raw
      .prepare(
        'SELECT event_kind, status FROM provider_account_token_custody_audit ORDER BY created_at',
      )
      .all() as Array<{ event_kind: string; status: string }>
    expect(auditRows).toEqual([
      { event_kind: 'auth_stored', status: 'succeeded' },
      { event_kind: 'access_issued', status: 'succeeded' },
    ])
  })

  test('denies access when the owner does not match the custody row', async () => {
    const { d1 } = makeDb()
    const store = makeD1ProviderAccountTokenCustodyStore(d1)
    const cipher = await makeCipher()

    await storeConnectedCodexAuthInCustody(store, cipher, {
      auth: {
        type: 'oauth',
        access: 'access-secret-1',
        refresh: 'refresh-secret-1',
        expires: Date.parse('2026-07-03T18:00:00.000Z'),
      },
      makeId: makeIdFactory(),
      nowIso: '2026-07-03T17:00:00.000Z',
      ownerUserId,
      providerAccountRef,
    })

    await expect(
      issueShortLivedCodexAccessFromCustody(store, cipher, {
        now: new Date('2026-07-03T17:10:00.000Z'),
        ownerUserId: 'openauth-other-owner',
        providerAccountRef,
      }),
    ).rejects.toMatchObject({
      _tag: 'ProviderAccountNotFound',
    })
  })

  test('deletes owner-scoped Codex custody material with an audit receipt', async () => {
    const { d1, raw } = makeDb()
    const store = makeD1ProviderAccountTokenCustodyStore(d1)
    const cipher = await makeCipher()
    const makeId = makeIdFactory()

    await storeConnectedCodexAuthInCustody(store, cipher, {
      auth: {
        type: 'oauth',
        access: 'access-secret-1',
        refresh: 'refresh-secret-1',
        expires: Date.parse('2026-07-03T18:00:00.000Z'),
      },
      makeId,
      nowIso: '2026-07-03T17:00:00.000Z',
      ownerUserId,
      providerAccountRef,
    })

    const deleted = await deleteConnectedCodexAuthFromCustody(store, {
      actorRef: `owner:${ownerUserId}`,
      makeId,
      nowIso: '2026-07-03T17:30:00.000Z',
      ownerUserId,
      providerAccountRef,
    })

    expect(deleted).toBe(true)
    expect(raw.prepare('SELECT COUNT(*) AS count FROM provider_account_token_custody').get()).toEqual({ count: 0 })
    await expect(
      issueShortLivedCodexAccessFromCustody(store, cipher, {
        now: new Date('2026-07-03T17:31:00.000Z'),
        ownerUserId,
        providerAccountRef,
      }),
    ).rejects.toMatchObject({ _tag: 'ProviderAccountNotFound' })
    expect(
      raw
        .prepare(
          'SELECT event_kind, status FROM provider_account_token_custody_audit ORDER BY rowid',
        )
        .all(),
    ).toEqual([
      { event_kind: 'auth_stored', status: 'succeeded' },
      { event_kind: 'auth_deleted', status: 'succeeded' },
    ])
  })

  test('refreshes near-expiry access and persists rotated refresh tokens atomically with audit', async () => {
    const { d1, raw } = makeDb()
    const store = makeD1ProviderAccountTokenCustodyStore(d1)
    const cipher = await makeCipher()
    const makeId = makeIdFactory()
    const refreshCalls: Array<string> = []

    await storeConnectedCodexAuthInCustody(store, cipher, {
      auth: {
        type: 'oauth',
        access: 'old-access-secret',
        refresh: 'old-refresh-secret',
        expires: Date.parse('2026-07-03T17:03:00.000Z'),
      },
      makeId,
      nowIso: '2026-07-03T17:00:00.000Z',
      ownerUserId,
      providerAccountRef,
    })

    const access = await issueShortLivedCodexAccessFromCustody(store, cipher, {
      makeId,
      now: new Date('2026-07-03T17:01:00.000Z'),
      ownerUserId,
      providerAccountRef,
      refreshCodexOAuthAuth: async auth => {
        refreshCalls.push(auth.refresh)

        return {
          status: 'refreshed',
          auth: {
            type: 'oauth',
            access: 'new-access-secret',
            refresh: 'new-refresh-secret',
            expires: Date.parse('2026-07-03T18:30:00.000Z'),
          },
        }
      },
    })

    expect(access.access).toBe('new-access-secret')
    expect(refreshCalls).toEqual(['old-refresh-secret'])

    const rawJson = JSON.stringify(
      raw.prepare('SELECT * FROM provider_account_token_custody').all(),
    )
    expect(rawJson).not.toContain('old-refresh-secret')
    expect(rawJson).not.toContain('new-refresh-secret')
    expect(rawJson).not.toContain('new-access-secret')

    const refreshedAccess = await issueShortLivedCodexAccessFromCustody(
      store,
      cipher,
      {
        makeId,
        now: new Date('2026-07-03T17:05:00.000Z'),
        ownerUserId,
        providerAccountRef,
        refreshCodexOAuthAuth: async () => {
          throw new Error('cached access should not refresh')
        },
      },
    )
    expect(refreshedAccess.access).toBe('new-access-secret')

    const auditRows = raw
      .prepare(
        'SELECT event_kind, status FROM provider_account_token_custody_audit ORDER BY rowid',
      )
      .all() as Array<{ event_kind: string; status: string }>
    expect(auditRows).toEqual([
      { event_kind: 'auth_stored', status: 'succeeded' },
      { event_kind: 'refresh_succeeded', status: 'succeeded' },
      { event_kind: 'access_issued', status: 'succeeded' },
    ])
  })

  test('records typed refresh failures without exposing refresh material', async () => {
    const { d1, raw } = makeDb()
    const store = makeD1ProviderAccountTokenCustodyStore(d1)
    const cipher = await makeCipher()
    const makeId = makeIdFactory()

    await storeConnectedCodexAuthInCustody(store, cipher, {
      auth: {
        type: 'oauth',
        access: 'old-access-secret',
        refresh: 'old-refresh-secret',
        expires: Date.parse('2026-07-03T17:03:00.000Z'),
      },
      makeId,
      nowIso: '2026-07-03T17:00:00.000Z',
      ownerUserId,
      providerAccountRef,
    })

    await expect(
      issueShortLivedCodexAccessFromCustody(store, cipher, {
        makeId,
        now: new Date('2026-07-03T17:01:00.000Z'),
        ownerUserId,
        providerAccountRef,
        refreshCodexOAuthAuth: async () => ({
          status: 'failed',
          code: 'refresh_token_invalidated',
          failureClass: 'token_invalidated',
          providerStatus: 400,
        }),
      }),
    ).rejects.toMatchObject({
      _tag: 'ProviderTokenCustodyRefreshFailed',
      failureClass: 'token_invalidated',
      providerStatus: 400,
    })

    const auditRows = raw
      .prepare(
        'SELECT event_kind, status, error_tag, error_message, metadata_json FROM provider_account_token_custody_audit ORDER BY rowid',
      )
      .all() as Array<{
      event_kind: string
      status: string
      error_tag: string | null
      error_message: string | null
      metadata_json: string | null
    }>

    expect(auditRows.at(-1)).toEqual({
      event_kind: 'refresh_failed',
      status: 'failed',
      error_tag: 'token_invalidated',
      error_message: 'refresh_token_invalidated',
      metadata_json: '{"providerStatus":400}',
    })
    expect(JSON.stringify(auditRows)).not.toContain('old-refresh-secret')
  })

  test('imports AES-GCM custody keys from base64 env values', async () => {
    const keyBytes = new Uint8Array(32).fill(9)
    const cipher = await providerAccountTokenCustodyCipherFromEnv({
      PROVIDER_TOKEN_CUSTODY_AES_KEY_B64: base64(keyBytes),
      PROVIDER_TOKEN_CUSTODY_AES_KEY_ID: 'env-key',
    })
    const encrypted = await cipher.encryptText('secret-value')

    expect(encrypted.keyId).toBe('env-key')
    expect(await cipher.decryptText(encrypted)).toBe('secret-value')
    expect(base64(keyBytes)).toMatch(/^[A-Za-z0-9+/=]+$/)
  })
})
