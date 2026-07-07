import { describe, expect, test } from 'vitest'

import type { SyncSql } from '@openagentsinc/khala-sync-server'

import {
  CLOUD_RUNTIME_EXECUTION_TOKEN_AGENT_USER_ID,
  CLOUD_RUNTIME_EXECUTION_TOKEN_NAME,
  DEFAULT_EXECUTION_TOKEN_TTL_SECONDS,
  mintCloudRuntimeExecutionToken,
  revokeCloudRuntimeExecutionToken,
} from './khala-cloud-runtime-execution-token'

// A recording tagged-template SQL: captures every (text, values) pair and
// answers the revoke UPDATE ... RETURNING with a configurable row set.
type Captured = { text: string; values: ReadonlyArray<unknown> }

const makeRecordingSql = (
  updateReturns: (values: ReadonlyArray<unknown>) => ReadonlyArray<unknown> = () =>
    [{ id: 'x' }],
): { sql: SyncSql; captured: Array<Captured> } => {
  const captured: Array<Captured> = []
  const sql = (strings: TemplateStringsArray, ...values: Array<unknown>) => {
    const text = strings.join(' ')
    captured.push({ text, values })
    if (text.includes('UPDATE agent_credentials')) {
      return Promise.resolve([...updateReturns(values)])
    }
    // INSERT resolves to no rows.
    return Promise.resolve([])
  }
  return { captured, sql: sql as unknown as SyncSql }
}

describe('mintCloudRuntimeExecutionToken', () => {
  const fixedNow = () => '2026-07-07T00:00:00.000Z'

  test('user_id is the service agent, openauth_user_id is the owner; raw token once + TTL expiry', async () => {
    const { captured, sql } = makeRecordingSql()
    const minted = await mintCloudRuntimeExecutionToken(sql, {
      createToken: () => 'oa_agent_RAWTOKEN0123456789abcdef',
      hash: async value => `hash(${value})`,
      now: fixedNow,
      ownerUserId: 'github:14167547',
      uuid: () => 'fixed-uuid',
    })

    // raw token returned once, in memory only.
    expect(minted.rawToken).toBe('oa_agent_RAWTOKEN0123456789abcdef')
    // the mint still reports the OWNER (used for the receipt attribution).
    expect(minted.ownerUserId).toBe('github:14167547')
    // prefix is the first 20 chars, safe for logs.
    expect(minted.tokenPrefix).toBe('oa_agent_RAWTOKEN012')
    expect(minted.rawToken.startsWith(minted.tokenPrefix)).toBe(true)
    // TTL: expiry = createdAt + default ttl.
    expect(minted.createdAt).toBe('2026-07-07T00:00:00.000Z')
    expect(minted.expiresAt).toBe(
      new Date(
        Date.parse('2026-07-07T00:00:00.000Z') +
          DEFAULT_EXECUTION_TOKEN_TTL_SECONDS * 1000,
      ).toISOString(),
    )

    // ensures the service-agent user exists (idempotent) BEFORE the credential,
    // so the inference-gateway auth gate (kind='agent') can resolve user_id.
    const userUpsert = captured.find(
      c => c.text.includes('INSERT INTO users') && c.text.includes('ON CONFLICT'),
    )
    expect(userUpsert).toBeDefined()
    expect(userUpsert!.values[0]).toBe(CLOUD_RUNTIME_EXECUTION_TOKEN_AGENT_USER_ID)
    // kind='agent'/status='active' are SQL literals in the template text.
    expect(userUpsert!.text).toContain("'agent'")
    expect(userUpsert!.text).toContain("'active'")

    // exactly one INSERT into agent_credentials.
    const inserts = captured.filter(c => c.text.includes('INSERT INTO agent_credentials'))
    expect(inserts).toHaveLength(1)
    const values = inserts[0]!.values
    // (id, user_id, openauth_user_id, token_hash, token_prefix, name, created_at, expires_at)
    expect(values[0]).toBe(minted.credentialId)
    // user_id = the service AGENT user (so gateway auth resolves)…
    expect(values[1]).toBe(CLOUD_RUNTIME_EXECUTION_TOKEN_AGENT_USER_ID)
    // …openauth_user_id = the OWNER (so the receipt is owner-attributed).
    expect(values[2]).toBe('github:14167547')
    // only the HASH is persisted, never the raw token.
    expect(values[3]).toBe('hash(oa_agent_RAWTOKEN0123456789abcdef)')
    expect(values).not.toContain('oa_agent_RAWTOKEN0123456789abcdef')
    expect(values[4]).toBe('oa_agent_RAWTOKEN012')
    expect(values[5]).toBe(CLOUD_RUNTIME_EXECUTION_TOKEN_NAME)
    expect(values[6]).toBe('2026-07-07T00:00:00.000Z')
    expect(values[7]).toBe(minted.expiresAt)
  })

  test('honors an explicit TTL', async () => {
    const { sql } = makeRecordingSql()
    const minted = await mintCloudRuntimeExecutionToken(sql, {
      now: fixedNow,
      ownerUserId: 'github:1',
      ttlSeconds: 60,
    })
    expect(minted.expiresAt).toBe(
      new Date(Date.parse('2026-07-07T00:00:00.000Z') + 60_000).toISOString(),
    )
  })

  test('unique credential ids across mints', async () => {
    const { sql } = makeRecordingSql()
    let n = 0
    const mint = () =>
      mintCloudRuntimeExecutionToken(sql, {
        ownerUserId: 'github:1',
        uuid: () => `u${(n += 1)}`,
      })
    const a = await mint()
    const b = await mint()
    expect(a.credentialId).not.toBe(b.credentialId)
  })
})

describe('revokeCloudRuntimeExecutionToken', () => {
  const fixedNow = () => '2026-07-07T00:10:00.000Z'

  test('flips status and revoked_at, scoped to the credential id, returns 1 on success', async () => {
    const { captured, sql } = makeRecordingSql(() => [{ id: 'cred.1' }])
    const affected = await revokeCloudRuntimeExecutionToken(sql, {
      credentialId: 'cred.1',
      now: fixedNow,
    })
    expect(affected).toBe(1)
    const update = captured.find(c => c.text.includes('UPDATE agent_credentials'))
    expect(update).toBeDefined()
    expect(update!.text).toContain("status = 'revoked'")
    expect(update!.text).toContain('revoked_at =')
    expect(update!.text).toContain("status = 'active'")
    // revoked_at value then credential id are the bound params.
    expect(update!.values).toContain('2026-07-07T00:10:00.000Z')
    expect(update!.values).toContain('cred.1')
  })

  test('is a no-op (returns 0) when nothing was active to revoke', async () => {
    const { sql } = makeRecordingSql(() => [])
    const affected = await revokeCloudRuntimeExecutionToken(sql, {
      credentialId: 'absent',
    })
    expect(affected).toBe(0)
  })
})
