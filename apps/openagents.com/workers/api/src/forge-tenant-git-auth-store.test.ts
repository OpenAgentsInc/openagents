import { readFileSync } from 'node:fs'
import { DatabaseSync } from 'node:sqlite'

import { describe, expect, test } from 'vitest'

import {
  FORGE_GIT_TOKEN_PREFIX,
  forgeGitAccessTokenHash,
  makeD1ForgeTenantGitAuthStore,
  type ForgeTenantGitAuthStore,
} from './forge-tenant-git-auth-store'

class SqliteD1Statement {
  private bound: ReadonlyArray<unknown> = []

  constructor(
    private readonly db: DatabaseSync,
    private readonly sql: string,
  ) {}

  bind(...values: ReadonlyArray<unknown>): SqliteD1Statement {
    this.bound = values.map(value => (value === undefined ? null : value))
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

  async run(): Promise<{ success: true; results: [] }> {
    this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { results: [], success: true }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }
}

const migration = readFileSync(
  new URL('../migrations/0253_forge_tenant_git_access_tokens.sql', import.meta.url),
  'utf8',
)
const tenantIsolationPostureMigration = readFileSync(
  new URL('../migrations/0256_forge_tenant_isolation_posture.sql', import.meta.url),
  'utf8',
)

const makeStore = (): {
  db: DatabaseSync
  store: ForgeTenantGitAuthStore
} => {
  const db = new DatabaseSync(':memory:')
  db.exec('PRAGMA foreign_keys = ON')
  db.exec(migration)
  db.exec(tenantIsolationPostureMigration)
  return {
    db,
    store: makeD1ForgeTenantGitAuthStore(
      new SqliteD1(db) as unknown as D1Database,
    ),
  }
}

const nowIso = '2026-06-28T18:00:00.000Z'
const laterIso = '2026-06-28T18:05:00.000Z'
const expiresAt = '2026-06-28T19:00:00.000Z'
const token = `${FORGE_GIT_TOKEN_PREFIX}test_secret_00000000000000000000`

describe('forge tenant git auth store', () => {
  test('mints hashed tenant git tokens and authenticates exact repo/scope', async () => {
    const { db, store } = makeStore()
    await store.upsertTenant({
      tenantRef: 'tenant.openagents',
      displayName: 'OpenAgents',
      nowIso,
    })

    const minted = await store.mintGitAccessToken(
      {
        tenantRef: 'tenant.openagents',
        tokenRef: 'forge_git_token.receive_pack',
        subjectRef: 'agent.public.forge',
        repositoryRef: 'repo.openagents.openagents',
        scopes: ['git:receive-pack'],
        expiresAt,
        sourceRefs: ['github:OpenAgentsInc/openagents#6750'],
        nowIso,
      },
      { makeToken: () => token },
    )

    expect(minted.token).toBe(token)
    expect(minted.record.token_hash).toBe(await forgeGitAccessTokenHash(token))
    expect(minted.record.token_hash).not.toContain('test_secret')
    expect(minted.record.token_prefix).toBe(token.slice(0, 29))
    expect(minted.scopes.map(scope => scope.scope)).toEqual([
      'git:receive-pack',
    ])
    expect(
      JSON.stringify(
        db.prepare('SELECT * FROM forge_git_access_tokens').all(),
      ),
    ).not.toContain(token)

    const session = await store.authenticateGitAccessToken({
      token,
      repositoryRef: 'repo.openagents.openagents',
      requiredScope: 'git:receive-pack',
      nowIso: laterIso,
    })
    expect(session).toMatchObject({
      tenantRef: 'tenant.openagents',
      tokenRef: 'forge_git_token.receive_pack',
      subjectRef: 'agent.public.forge',
      repositoryRef: 'repo.openagents.openagents',
    })
    expect(session?.scopes).toEqual(['git:receive-pack'])
    expect(
      await store.authenticateGitAccessToken({
        token,
        repositoryRef: 'repo.openagents.openagents',
        requiredScope: 'git:upload-pack',
        nowIso: laterIso,
      }),
    ).toBeUndefined()
    expect(
      await store.authenticateGitAccessToken({
        token,
        repositoryRef: 'repo.other',
        requiredScope: 'git:receive-pack',
        nowIso: laterIso,
      }),
    ).toBeUndefined()

    const stored = await store.readGitAccessToken(
      'tenant.openagents',
      'forge_git_token.receive_pack',
    )
    expect(stored?.last_used_at).toBe(laterIso)
  })

  test('stores tenant confidential posture as public-safe refs', async () => {
    const { store } = makeStore()
    const tenant = await store.upsertTenant({
      tenantRef: 'tenant.external-fleet',
      displayName: 'External Fleet',
      confidentialWorkspaceMode: 'attested',
      attestationRef: 'attestation.forge.external_fleet.public',
      encryptedKnowledgePackRef: 'knowledge-pack.forge.external_fleet.encrypted',
      refusalReason: null,
      retentionPolicyRef: 'retention.forge.external_fleet.7d',
      nowIso,
    })

    expect(tenant).toMatchObject({
      tenant_ref: 'tenant.external-fleet',
      confidential_workspace_mode: 'attested',
      attestation_ref: 'attestation.forge.external_fleet.public',
      encrypted_knowledge_pack_ref: 'knowledge-pack.forge.external_fleet.encrypted',
      retention_policy_ref: 'retention.forge.external_fleet.7d',
    })
  })

  test('git:admin grants both upload-pack and receive-pack', async () => {
    const { store } = makeStore()
    await store.upsertTenant({
      tenantRef: 'tenant.openagents',
      displayName: 'OpenAgents',
      nowIso,
    })
    const adminToken = `${FORGE_GIT_TOKEN_PREFIX}admin_000000000000000000000000`
    await store.mintGitAccessToken(
      {
        tenantRef: 'tenant.openagents',
        tokenRef: 'forge_git_token.admin',
        subjectRef: 'agent.public.admin',
        repositoryRef: 'repo.openagents.openagents',
        scopes: ['git:admin'],
        expiresAt,
        sourceRefs: [],
        nowIso,
      },
      { makeToken: () => adminToken },
    )

    expect(
      await store.authenticateGitAccessToken({
        token: adminToken,
        repositoryRef: 'repo.openagents.openagents',
        requiredScope: 'git:upload-pack',
        nowIso: laterIso,
      }),
    ).toMatchObject({ subjectRef: 'agent.public.admin' })
    expect(
      await store.authenticateGitAccessToken({
        token: adminToken,
        repositoryRef: 'repo.openagents.openagents',
        requiredScope: 'git:receive-pack',
        nowIso: laterIso,
      }),
    ).toMatchObject({ subjectRef: 'agent.public.admin' })
  })

  test('expired, revoked, and suspended tenant tokens fail closed', async () => {
    const { store } = makeStore()
    await store.upsertTenant({
      tenantRef: 'tenant.openagents',
      displayName: 'OpenAgents',
      nowIso,
    })
    await store.upsertTenant({
      tenantRef: 'tenant.suspended',
      displayName: 'Suspended',
      state: 'suspended',
      nowIso,
    })

    await expect(
      store.mintGitAccessToken(
        {
          tenantRef: 'tenant.suspended',
          tokenRef: 'forge_git_token.suspended',
          subjectRef: 'agent.public.suspended',
          repositoryRef: 'repo.openagents.openagents',
          scopes: ['git:receive-pack'],
          expiresAt,
          sourceRefs: [],
          nowIso,
        },
        {
          makeToken: () =>
            `${FORGE_GIT_TOKEN_PREFIX}suspended_0000000000000000`,
        },
      ),
    ).rejects.toThrow('active tenant')

    const expiringToken = `${FORGE_GIT_TOKEN_PREFIX}expiring_000000000000000000`
    await store.mintGitAccessToken(
      {
        tenantRef: 'tenant.openagents',
        tokenRef: 'forge_git_token.expiring',
        subjectRef: 'agent.public.expiring',
        repositoryRef: 'repo.openagents.openagents',
        scopes: ['git:receive-pack'],
        expiresAt: '2026-06-28T18:01:00.000Z',
        sourceRefs: [],
        nowIso,
      },
      { makeToken: () => expiringToken },
    )
    expect(
      await store.authenticateGitAccessToken({
        token: expiringToken,
        repositoryRef: 'repo.openagents.openagents',
        requiredScope: 'git:receive-pack',
        nowIso: '2026-06-28T18:02:00.000Z',
      }),
    ).toBeUndefined()
    expect(
      (
        await store.readGitAccessToken(
          'tenant.openagents',
          'forge_git_token.expiring',
        )
      )?.state,
    ).toBe('expired')

    const revokedToken = `${FORGE_GIT_TOKEN_PREFIX}revoked_0000000000000000000`
    await store.mintGitAccessToken(
      {
        tenantRef: 'tenant.openagents',
        tokenRef: 'forge_git_token.revoked',
        subjectRef: 'agent.public.revoked',
        repositoryRef: 'repo.openagents.openagents',
        scopes: ['git:receive-pack'],
        expiresAt,
        sourceRefs: [],
        nowIso,
      },
      { makeToken: () => revokedToken },
    )
    await store.revokeGitAccessToken(
      'tenant.openagents',
      'forge_git_token.revoked',
      laterIso,
    )
    expect(
      await store.authenticateGitAccessToken({
        token: revokedToken,
        repositoryRef: 'repo.openagents.openagents',
        requiredScope: 'git:receive-pack',
        nowIso: laterIso,
      }),
    ).toBeUndefined()
  })
})
