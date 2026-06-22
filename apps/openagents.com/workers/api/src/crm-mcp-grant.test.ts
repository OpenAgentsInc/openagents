import { describe, expect, test } from 'vitest'

import {
  crmMcpAdminPrincipal,
  listCrmMcpGrants,
  mintCrmMcpGrant,
  resolveCrmMcpGrantPrincipal,
  revokeCrmMcpGrant,
} from './crm-mcp-grant'

type GrantRow = {
  id: string
  grant_ref: string
  token_hash: string
  tenant_ref: string
  authority_classes_json: string
  label: string | null
  status: string
  created_at: string
  expires_at: string | null
}

const makeGrantDb = () => {
  const rows: Array<GrantRow> = []
  const statement = (q: string, bound: ReadonlyArray<unknown> = []): D1PreparedStatement =>
    ({
      bind: (...v: ReadonlyArray<unknown>) => statement(q, v),
      first: <T,>() => {
        if (q.includes('WHERE token_hash')) {
          const hash = String(bound[0] ?? '')
          return Promise.resolve((rows.find(r => r.token_hash === hash && r.status === 'active') ?? null) as T | null)
        }
        return Promise.resolve(null as T | null)
      },
      all: <T,>() => {
        const tenant = String(bound[0] ?? '')
        return Promise.resolve({
          meta: {} as D1Meta,
          results: rows.filter(r => r.tenant_ref === tenant) as unknown as Array<T>,
          success: true,
        } as D1Result<T>)
      },
      run: () => {
        if (q.includes('INSERT INTO crm_mcp_grants')) {
          const [id, grantRef, tokenHash, tenantRef, authoritiesJson, label, createdAt, expiresAt] = bound
          rows.push({
            authority_classes_json: String(authoritiesJson),
            created_at: String(createdAt),
            expires_at: expiresAt === null ? null : String(expiresAt),
            grant_ref: String(grantRef),
            id: String(id),
            label: label === null ? null : String(label),
            status: 'active',
            tenant_ref: String(tenantRef),
            token_hash: String(tokenHash),
          })
          return Promise.resolve({ meta: { changes: 1 } as D1Meta, results: [], success: true } as unknown as D1Result)
        }
        if (q.includes('UPDATE crm_mcp_grants')) {
          const [tenantRef, grantRef] = bound
          const row = rows.find(
            r => r.tenant_ref === String(tenantRef) && r.grant_ref === String(grantRef) && r.status === 'active',
          )
          let changes = 0
          if (row !== undefined) {
            row.status = 'revoked'
            changes = 1
          }
          return Promise.resolve({ meta: { changes } as D1Meta, results: [], success: true } as unknown as D1Result)
        }
        return Promise.resolve({ meta: {} as D1Meta, results: [], success: true } as unknown as D1Result)
      },
      raw: () => Promise.reject(new Error('raw')),
    }) as unknown as D1PreparedStatement
  return {
    db: {
      batch: () => Promise.reject(new Error('batch')),
      dump: () => Promise.reject(new Error('dump')),
      exec: () => Promise.reject(new Error('exec')),
      prepare: (q: string) => statement(q),
      withSession: () => {
        throw new Error('session')
      },
    } as unknown as D1Database,
    rows,
  }
}

const NOW = '2026-06-22T00:00:00.000Z'
const runtime = { makeId: (p: string) => `${p}_t`, nowIso: () => NOW }

describe('crmMcpAdminPrincipal', () => {
  test('grants the full CRM authority set on the bound tenant', () => {
    const principal = crmMcpAdminPrincipal('tenant.acme', NOW)
    expect(principal.tenantRef).toBe('tenant.acme')
    const authorities = principal.grants.map(g => g.authorityClass)
    expect(authorities).toContain('operator_read')
    expect(authorities).toContain('approval_resolution')
    expect(principal.grants.every(g => g.decision === 'granted')).toBe(true)
  })
})

describe('mint + resolve scoped grant', () => {
  test('a minted token resolves to a principal with bound tenant + declared authorities', async () => {
    const { db } = makeGrantDb()
    const minted = await mintCrmMcpGrant(
      db,
      { authorities: ['operator_read'], label: 'read bot', tenantRef: 'tenant.acme' },
      runtime,
    )
    expect(minted.token.startsWith('oa_mcp_')).toBe(true)
    expect(minted.summary.tenantRef).toBe('tenant.acme')

    const principal = await resolveCrmMcpGrantPrincipal(db, minted.token, NOW)
    expect(principal).not.toBeNull()
    expect(principal?.tenantRef).toBe('tenant.acme')
    expect(principal?.grants.map(g => g.authorityClass)).toEqual(['operator_read'])
  })

  test('an unknown token resolves to null', async () => {
    const { db } = makeGrantDb()
    expect(await resolveCrmMcpGrantPrincipal(db, 'oa_mcp_nope', NOW)).toBeNull()
  })

  test('an expired grant resolves to null', async () => {
    const { db } = makeGrantDb()
    const minted = await mintCrmMcpGrant(
      db,
      { authorities: ['operator_read'], expiresAt: '2020-01-01T00:00:00.000Z', tenantRef: 'tenant.acme' },
      runtime,
    )
    expect(await resolveCrmMcpGrantPrincipal(db, minted.token, NOW)).toBeNull()
  })

  test('minting with no valid authority throws', async () => {
    const { db } = makeGrantDb()
    await expect(
      mintCrmMcpGrant(db, { authorities: ['bogus'], tenantRef: 'tenant.acme' }, runtime),
    ).rejects.toThrow()
  })

  test('revoking a grant makes its token stop resolving', async () => {
    const { db } = makeGrantDb()
    const minted = await mintCrmMcpGrant(db, { authorities: ['operator_read'], tenantRef: 'tenant.acme' }, runtime)
    expect(await resolveCrmMcpGrantPrincipal(db, minted.token, NOW)).not.toBeNull()
    const revoked = await revokeCrmMcpGrant(db, 'tenant.acme', minted.grantRef, runtime)
    expect(revoked).toBe(true)
    expect(await resolveCrmMcpGrantPrincipal(db, minted.token, NOW)).toBeNull()
  })

  test('listCrmMcpGrants returns the tenant grants', async () => {
    const { db } = makeGrantDb()
    await mintCrmMcpGrant(db, { authorities: ['operator_read'], tenantRef: 'tenant.acme' }, runtime)
    const grants = await listCrmMcpGrants(db, 'tenant.acme')
    expect(grants).toHaveLength(1)
    expect(grants[0]?.authorities).toEqual(['operator_read'])
  })
})
