import { afterEach, describe, expect, test, vi } from 'vitest'

import {
  AUTH_EMAIL_OTP_CODE_TTL_SECONDS,
  AUTH_EMAIL_OTP_EXPIRES_AT_CLAIM,
  AUTH_EMAIL_OTP_ISSUED_AT_CLAIM,
  type AuthEmailOtpRateLimitPolicy,
  type AuthEmailOtpRateLimitRuntime,
  authEmailOtpClaimsAreFresh,
  authEmailOtpClientIp,
  authEmailOtpSendForm,
  reserveAuthEmailOtpSend,
  stampAuthEmailOtpClaims,
} from './auth/email-otp-hardening'
import worker, { authIssuerAllowsRedirectHostname } from './index'

type OpenAuthStorageRow = {
  expires_at: number | null
  key: string
  updated_at: string
  value_json: string
}

class MemoryD1Statement {
  constructor(
    private readonly rows: Map<string, OpenAuthStorageRow>,
    private readonly sql: string,
    private readonly bound: ReadonlyArray<unknown> = [],
  ) {}

  bind(...values: Array<unknown>): MemoryD1Statement {
    return new MemoryD1Statement(this.rows, this.sql, values)
  }

  first<T>(): Promise<T | null> {
    const key = this.bound[0]

    if (typeof key !== 'string') {
      return Promise.resolve(null)
    }

    const row = this.rows.get(key)

    if (row === undefined) {
      return Promise.resolve(null)
    }

    if (this.sql.includes('AND (expires_at IS NULL OR expires_at > ?)')) {
      const nowMs = Number(this.bound[1])

      if (row.expires_at !== null && row.expires_at <= nowMs) {
        return Promise.resolve(null)
      }
    }

    return Promise.resolve({ ...row } as T)
  }

  run(): Promise<D1Result> {
    if (this.sql.includes('INSERT INTO openauth_storage')) {
      const [key, valueJson, expiresAt, updatedAt] = this.bound

      if (
        typeof key === 'string' &&
        typeof valueJson === 'string' &&
        typeof updatedAt === 'string'
      ) {
        this.rows.set(key, {
          expires_at:
            typeof expiresAt === 'number' && Number.isFinite(expiresAt)
              ? expiresAt
              : null,
          key,
          updated_at: updatedAt,
          value_json: valueJson,
        })
      }
    }

    if (this.sql.includes('DELETE FROM openauth_storage WHERE key = ?')) {
      const key = this.bound[0]

      if (typeof key === 'string') {
        this.rows.delete(key)
      }
    }

    return Promise.resolve({ success: true } as D1Result)
  }

  all<T>(): Promise<D1Result<T>> {
    if (this.sql.includes('WHERE key LIKE ?')) {
      const prefix = String(this.bound[0] ?? '').replace(/%$/, '')
      const nowMs = Number(this.bound[1])
      const results = [...this.rows.values()]
        .filter(
          row =>
            row.key.startsWith(prefix) &&
            (row.expires_at === null || row.expires_at > nowMs),
        )
        .sort((left, right) => left.key.localeCompare(right.key))
        .map(row => ({ ...row })) as Array<T>

      return Promise.resolve({ results, success: true } as D1Result<T>)
    }

    return Promise.resolve({
      results: [...this.rows.values()]
        .sort((left, right) => left.key.localeCompare(right.key))
        .map(row => ({
          key: row.key,
          value_json: row.value_json,
        })) as Array<T>,
      success: true,
    } as D1Result<T>)
  }

  raw<T>(): Promise<T[]> {
    return Promise.resolve([])
  }
}

class MemoryD1 {
  private readonly rows = new Map<string, OpenAuthStorageRow>()

  prepare(sql: string): MemoryD1Statement {
    return new MemoryD1Statement(this.rows, sql)
  }

  dumpRows(): ReadonlyArray<Record<string, unknown>> {
    return [...this.rows.values()]
      .sort((left, right) => left.key.localeCompare(right.key))
      .map(row => ({
        key: row.key,
        value_json: row.value_json,
      }))
  }

  batch(statements: Array<MemoryD1Statement>): Promise<Array<D1Result>> {
    return Promise.all(statements.map(statement => statement.run()))
  }

  dump(): Promise<ArrayBuffer> {
    return Promise.resolve(new ArrayBuffer(0))
  }

  exec(): Promise<D1ExecResult> {
    return Promise.resolve({
      count: 0,
      duration: 0,
    })
  }
}

const testRuntime = (nowMs: number): AuthEmailOtpRateLimitRuntime => ({
  nowIso: () => '2026-06-16T12:00:00.000Z',
  nowMs: () => nowMs,
})

const relaxedPolicy: AuthEmailOtpRateLimitPolicy = {
  email: { limit: 10, windowSeconds: 60 },
  global: { limit: 100, windowSeconds: 60 },
  ip: { limit: 10, windowSeconds: 60 },
}

const makeDb = (): MemoryD1 => new MemoryD1()

const storedRows = (db: MemoryD1): ReadonlyArray<Record<string, unknown>> =>
  db.dumpRows()

describe('auth email OTP hardening', () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

  test('auth issuer redirect host allowlist includes staging but rejects sibling worker hosts', () => {
    expect(authIssuerAllowsRedirectHostname('openagents.com')).toBe(true)
    expect(authIssuerAllowsRedirectHostname('auth.openagents.com')).toBe(true)
    expect(
      authIssuerAllowsRedirectHostname(
        'openagents-staging.openagents.workers.dev',
      ),
    ).toBe(true)
    expect(authIssuerAllowsRedirectHostname('localhost')).toBe(true)
    expect(authIssuerAllowsRedirectHostname('127.0.0.1')).toBe(true)

    expect(authIssuerAllowsRedirectHostname('openagents.example.com')).toBe(
      false,
    )
    expect(
      authIssuerAllowsRedirectHostname('random.openagents.workers.dev'),
    ).toBe(false)
    expect(
      authIssuerAllowsRedirectHostname(
        'openagents-staging.evil.workers.dev',
      ),
    ).toBe(false)
    expect(authIssuerAllowsRedirectHostname('www.openagents.com')).toBe(false)
  })

  test('stamps a short server-side expiry claim and rejects stale claims', () => {
    const nowMs = Date.parse('2026-06-16T12:00:00.000Z')
    const claims: Record<string, string> = { email: 'user@example.com' }

    stampAuthEmailOtpClaims(claims, testRuntime(nowMs))

    expect(claims[AUTH_EMAIL_OTP_ISSUED_AT_CLAIM]).toBe(
      '2026-06-16T12:00:00.000Z',
    )
    expect(claims[AUTH_EMAIL_OTP_EXPIRES_AT_CLAIM]).toBe(
      '2026-06-16T12:10:00.000Z',
    )
    expect(authEmailOtpClaimsAreFresh(claims, testRuntime(nowMs))).toBe(true)
    expect(
      authEmailOtpClaimsAreFresh(
        claims,
        testRuntime(nowMs + AUTH_EMAIL_OTP_CODE_TTL_SECONDS * 1000 + 1),
      ),
    ).toBe(false)
  })

  test('rate limits by target email without storing raw email or IP in keys', async () => {
    const db = makeDb()
    const policy: AuthEmailOtpRateLimitPolicy = {
      ...relaxedPolicy,
      email: { limit: 2, windowSeconds: 60 },
    }
    const input = {
      email: 'USER@example.com',
      ipAddress: '203.0.113.10',
    }

    await expect(
      reserveAuthEmailOtpSend(
        db as unknown as D1Database,
        input,
        testRuntime(0),
        policy,
      ),
    ).resolves.toMatchObject({ _tag: 'Allowed' })
    await expect(
      reserveAuthEmailOtpSend(
        db as unknown as D1Database,
        input,
        testRuntime(0),
        policy,
      ),
    ).resolves.toMatchObject({ _tag: 'Allowed' })
    await expect(
      reserveAuthEmailOtpSend(
        db as unknown as D1Database,
        input,
        testRuntime(0),
        policy,
      ),
    ).resolves.toMatchObject({
      _tag: 'RateLimited',
      retryAfterSeconds: 60,
      scope: 'email',
    })

    const serializedRows = JSON.stringify(storedRows(db))

    expect(serializedRows).not.toContain('USER@example.com')
    expect(serializedRows).not.toContain('user@example.com')
    expect(serializedRows).not.toContain('203.0.113.10')
  })

  test('rate limits by IP before one address can fan out across recipients', async () => {
    const db = makeDb()
    const policy: AuthEmailOtpRateLimitPolicy = {
      ...relaxedPolicy,
      ip: { limit: 2, windowSeconds: 60 },
    }
    const reserve = (email: string) =>
      reserveAuthEmailOtpSend(
        db as unknown as D1Database,
        { email, ipAddress: '198.51.100.9' },
        testRuntime(0),
        policy,
      )

    await expect(reserve('a@example.com')).resolves.toMatchObject({
      _tag: 'Allowed',
    })
    await expect(reserve('b@example.com')).resolves.toMatchObject({
      _tag: 'Allowed',
    })
    await expect(reserve('c@example.com')).resolves.toMatchObject({
      _tag: 'RateLimited',
      scope: 'ip',
    })
  })

  test('rate limits globally even when IPs and target emails rotate', async () => {
    const db = makeDb()
    const policy: AuthEmailOtpRateLimitPolicy = {
      ...relaxedPolicy,
      global: { limit: 2, windowSeconds: 60 },
    }
    const reserve = (email: string, ipAddress: string) =>
      reserveAuthEmailOtpSend(
        db as unknown as D1Database,
        { email, ipAddress },
        testRuntime(0),
        policy,
      )

    await expect(
      reserve('a@example.com', '198.51.100.1'),
    ).resolves.toMatchObject({
      _tag: 'Allowed',
    })
    await expect(
      reserve('b@example.com', '198.51.100.2'),
    ).resolves.toMatchObject({
      _tag: 'Allowed',
    })
    await expect(
      reserve('c@example.com', '198.51.100.3'),
    ).resolves.toMatchObject({
      _tag: 'RateLimited',
      scope: 'global',
    })
  })

  test('detects only code request and resend form posts', () => {
    const requestForm = new FormData()
    requestForm.set('action', 'request')
    requestForm.set('email', ' USER@Example.COM ')
    const verifyForm = new FormData()
    verifyForm.set('action', 'verify')
    verifyForm.set('code', '123456')

    expect(authEmailOtpSendForm(requestForm)).toEqual({
      action: 'request',
      email: 'user@example.com',
    })
    expect(authEmailOtpSendForm(verifyForm)).toBeUndefined()
  })

  test('uses Cloudflare client IP first, then forwarded IP, then unknown', () => {
    expect(
      authEmailOtpClientIp(
        new Request('https://auth.openagents.com/authorize', {
          headers: {
            'cf-connecting-ip': '203.0.113.9',
            'x-forwarded-for': '198.51.100.1',
          },
        }),
      ),
    ).toBe('203.0.113.9')
    expect(
      authEmailOtpClientIp(
        new Request('https://auth.openagents.com/authorize', {
          headers: { 'x-forwarded-for': '198.51.100.1, 198.51.100.2' },
        }),
      ),
    ).toBe('198.51.100.1')
    expect(
      authEmailOtpClientIp(
        new Request('https://auth.openagents.com/authorize'),
      ),
    ).toBe('unknown')
  })

  test('auth host fails closed to the email form when sender config is missing', async () => {
    const db = makeDb()
    const response = await worker.fetch(
      new Request('https://auth.openagents.com/code/authorize', {
        body: new URLSearchParams({
          action: 'request',
          email: 'user@example.com',
        }),
        headers: {
          accept: 'text/html',
          'cf-connecting-ip': '203.0.113.44',
          'content-type': 'application/x-www-form-urlencoded',
        },
        method: 'POST',
      }) as never,
      {
        GITHUB_CLIENT_ID: 'github-client',
        GITHUB_CLIENT_SECRET: 'github-secret',
        OPENAGENTS_APP_URL: 'https://openagents.com',
        OPENAGENTS_DB: db as unknown as D1Database,
        OPENAUTH_CLIENT_ID: 'openagents-web',
        OPENAUTH_ISSUER_URL: 'https://auth.openagents.com',
      } as never,
      {
        passThroughOnException: () => undefined,
        waitUntil: () => undefined,
      } as never,
    )
    const body = await response.text()
    const stored = JSON.stringify(storedRows(db))

    expect(response.status).toBe(200)
    expect(body).toContain('We couldn&#39;t send a sign-in code right now')
    expect(body).toContain('name="email"')
    expect(stored).not.toContain('"type":"code"')
    expect(stored).not.toContain('"code"')
  })
})
