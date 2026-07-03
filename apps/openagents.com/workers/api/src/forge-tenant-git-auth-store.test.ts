import { readFileSync } from 'node:fs'
import { DatabaseSync } from 'node:sqlite'

import { describe, expect, test } from 'vitest'
import {
  compileAgentDefinitionToolRuntimePolicy,
  decodeAgentDefinition,
  type AgentDefinitionToolset,
} from '@openagentsinc/agent-runtime-schema'

import {
  FORGE_GIT_TOKEN_PREFIX,
  compileAgentDefinitionForgeGitAccessScopes,
  forgeGitAccessTokenHash,
  makeD1ForgeTenantGitAuthStore,
  type ForgeTenantGitAuthStore,
} from './forge-tenant-git-auth-store'

// Behavior contract oracle: background_agents.toolset.compiled_policy_enforced.v1

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

const compiledAgentPolicy = (toolset: AgentDefinitionToolset) =>
  compileAgentDefinitionToolRuntimePolicy(
    decodeAgentDefinition({
      schema: 'openagents.agent_definition.v1',
      id: 'agent_definition.forge.git_policy_test',
      ownerRef: 'agent:forge_policy_owner',
      name: 'Forge Git Policy Test',
      slug: 'forge-git-policy-test',
      goal: 'Compile definition toolsets to Forge git token scopes.',
      harness: { kind: 'khala' },
      toolset,
      triggers: [{ kind: 'manual', triggerRef: 'trigger.public.forge_policy.manual' }],
      lane: 'own_pylon',
      budget: { maxRunSeconds: 120, maxRunsPerDay: 3, maxCreditsPerDay: 0 },
      escalation: {
        channel: 'operator',
        askPolicy: {
          mode: 'operator_required',
          policyRef: 'policy.public.agent_definition.operator_required.v1',
        },
      },
      sourceRefs: ['issue.public.github.OpenAgentsInc.openagents.8192'],
      createdAt: '2026-07-03T00:00:00.000Z',
      updatedAt: '2026-07-03T00:00:00.000Z',
    }),
  )

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

  test('compiles agent-definition tool policy into Forge git token scopes', async () => {
    const { store } = makeStore()
    await store.upsertTenant({
      tenantRef: 'tenant.openagents',
      displayName: 'OpenAgents',
      nowIso,
    })
    const policy = compiledAgentPolicy({
      allow: ['tool.openagents.forge.git.receive_pack'],
      ask: ['tool.openagents.forge.git.upload_pack'],
      deny: ['tool.openagents.forge.git.admin'],
      networkPolicy: 'owner_scoped',
      secretPolicy: 'owner_scoped_refs_only',
    })

    const receivePackScopes = compileAgentDefinitionForgeGitAccessScopes({
      policy,
      requestedScopes: ['git:receive-pack'],
      invocationRef: 'invocation.public.forge.receive_pack',
    })
    expect(receivePackScopes).toMatchObject({
      status: 'allowed',
      scopes: ['git:receive-pack'],
    })

    const scopedToken = `${FORGE_GIT_TOKEN_PREFIX}compiled_0000000000000000`
    const minted = await store.mintGitAccessToken(
      {
        tenantRef: 'tenant.openagents',
        tokenRef: 'forge_git_token.compiled_receive',
        subjectRef: 'agent.public.background_agent',
        repositoryRef: 'repo.openagents.openagents',
        scopes: receivePackScopes.scopes,
        agentDefinitionToolPolicy: policy,
        expiresAt,
        sourceRefs: ['issue.public.github.OpenAgentsInc.openagents.8192'],
        nowIso,
      },
      { makeToken: () => scopedToken },
    )
    expect(minted.scopes.map(scope => scope.scope)).toEqual(['git:receive-pack'])

    const uploadPackScopes = compileAgentDefinitionForgeGitAccessScopes({
      policy,
      requestedScopes: ['git:upload-pack'],
      invocationRef: 'invocation.public.forge.upload_pack',
    })
    expect(uploadPackScopes).toMatchObject({
      status: 'operator_escalation_required',
      scopes: [],
      reasonRef: 'reason.agent_definition.forge_git_scope_requires_operator',
    })
    expect(uploadPackScopes.escalationRefs[0]).toMatch(
      /^escalation\.operator\.agent_definition\.[a-f0-9]{8}$/,
    )

    const adminScopes = compileAgentDefinitionForgeGitAccessScopes({
      policy,
      requestedScopes: ['git:admin'],
    })
    expect(adminScopes).toMatchObject({
      status: 'denied',
      scopes: [],
      reasonRef: 'reason.agent_definition.tool_denied',
      blockerRefs: ['blocker.agent_definition.tool_denied'],
    })

    await expect(
      store.mintGitAccessToken(
        {
          tenantRef: 'tenant.openagents',
          tokenRef: 'forge_git_token.rejected_admin',
          subjectRef: 'agent.public.background_agent',
          repositoryRef: 'repo.openagents.openagents',
          scopes: ['git:admin'],
          agentDefinitionToolPolicy: policy,
          expiresAt,
          sourceRefs: [],
          nowIso,
        },
        {
          makeToken: () =>
            `${FORGE_GIT_TOKEN_PREFIX}rejected_000000000000000`,
        },
      ),
    ).rejects.toThrow('compiled agent definition policy')
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
