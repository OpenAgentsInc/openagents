import { readFileSync } from 'node:fs'
import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  authorizeForgeControlPlaneBearer,
  makeForgeControlPlaneRoutes,
} from './forge-control-plane-routes'
import {
  makeD1ForgeCoordinationStore,
  type ForgeCoordinationStore,
} from './forge-coordination-store'
import {
  makeD1ForgeGitCanonicalStore,
  type ForgeGitCanonicalStore,
} from './forge-git-canonical-store'

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

  async run(): Promise<{ success: true; results: []; meta: { changes: number } }> {
    const result = this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { meta: { changes: Number(result.changes) }, results: [], success: true }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }
}

const coordinationMigration = readFileSync(
  new URL('../migrations/0251_forge_coordination_source_of_truth.sql', import.meta.url),
  'utf8',
)
const controlPlaneReceiptsMigration = readFileSync(
  new URL('../migrations/0254_forge_control_plane_receipts.sql', import.meta.url),
  'utf8',
)
const githubMirrorReceiptsMigration = readFileSync(
  new URL('../migrations/0259_forge_github_mirror_receipts.sql', import.meta.url),
  'utf8',
)
const canonicalGitMigration = readFileSync(
  new URL('../migrations/0255_forge_git_canonical_store.sql', import.meta.url),
  'utf8',
)

const makeStores = (): Readonly<{
  canonicalStore: ForgeGitCanonicalStore
  coordinationStore: ForgeCoordinationStore
}> => {
  const db = new DatabaseSync(':memory:')
  db.exec('PRAGMA foreign_keys = ON')
  db.exec(coordinationMigration)
  db.exec(controlPlaneReceiptsMigration)
  db.exec(githubMirrorReceiptsMigration)
  db.exec(canonicalGitMigration)
  const d1 = new SqliteD1(db) as unknown as D1Database
  return {
    canonicalStore: makeD1ForgeGitCanonicalStore(d1),
    coordinationStore: makeD1ForgeCoordinationStore(d1),
  }
}

const now = '2026-06-28T17:00:00.000Z'
const controlPlaneToken = 'forge-control-token'

const authHeaders = (scopes: string): HeadersInit => ({
  authorization: `Bearer ${controlPlaneToken}`,
  'content-type': 'application/json',
  'x-openagents-forge-tenant-ref': 'tenant.openagents',
  'x-openagents-forge-scopes': scopes,
})

const githubMainSha = '1234567890abcdef1234567890abcdef12345678'
const makeGitHubFetch = (sha: string = githubMainSha): typeof fetch =>
  (async () =>
    Response.json({
      ref: 'refs/heads/main',
      object: {
        sha,
        type: 'commit',
        url: `https://api.github.com/repos/OpenAgentsInc/openagents/git/commits/${sha}`,
      },
    })) as typeof fetch

type JsonRequestInit = Omit<RequestInit, 'body'> & Readonly<{ json?: unknown }>

const requestJson = (
  path: string,
  init: JsonRequestInit = {},
): Request =>
  new Request(`https://openagents.com${path}`, {
    ...init,
    ...(init.json === undefined ? {} : { body: JSON.stringify(init.json) }),
    headers: {
      ...(init.headers ?? {}),
      ...(init.json === undefined ? {} : { 'content-type': 'application/json' }),
    },
  })

const makeHarness = () => {
  const { canonicalStore, coordinationStore } = makeStores()
  const routes = makeForgeControlPlaneRoutes({
    authorizeControlPlaneBearer: (request, _env, scope) =>
      authorizeForgeControlPlaneBearer(request, controlPlaneToken, scope),
    fetch: makeGitHubFetch(),
    makeCanonicalStore: () => canonicalStore,
    makeStore: () => coordinationStore,
    nowIso: () => now,
    requireAdminApiToken: () => Promise.resolve(false),
  })
  const run = (request: Request): Promise<Response> => {
    const effect = routes.routeForgeControlPlaneRequest(request, {})

    if (effect === undefined) {
      throw new Error(`unmatched Forge route: ${request.url}`)
    }

    return Effect.runPromise(effect)
  }

  return { canonicalStore, run, store: coordinationStore }
}

describe('Forge control-plane routes', () => {
  test('rejects Forge smart-Git tokens on control-plane routes', async () => {
    const { run } = makeHarness()
    const response = await run(
      requestJson('/api/forge/work-records', {
        json: {
          tenantRef: 'tenant.openagents',
          issueRef: 'issue.forge.6770',
          title: 'Control plane API',
          state: 'open',
        },
        headers: {
          authorization: 'Bearer oa_forge_git_deadbeef',
          'x-openagents-forge-scopes': 'forge:work:write',
        },
        method: 'POST',
      }),
    )
    const body = (await response.json()) as { error: string }

    expect(response.status).toBe(403)
    expect(body.error).toBe('forge_control_plane_git_token_rejected')
  })

  test('requires the dedicated Forge scope for the requested mutation', async () => {
    const { run } = makeHarness()
    const response = await run(
      requestJson('/api/forge/work-records', {
        json: {
          tenantRef: 'tenant.openagents',
          issueRef: 'issue.forge.6770',
          title: 'Control plane API',
          state: 'open',
        },
        headers: authHeaders('forge:work:read'),
        method: 'POST',
      }),
    )
    const body = (await response.json()) as { error: string }

    expect(response.status).toBe(403)
    expect(body.error).toBe('forge_control_plane_forbidden')
  })

  test('rejects control-plane reads for the wrong tenant scope', async () => {
    const { run } = makeHarness()
    const response = await run(
      new Request(
        'https://openagents.com/api/forge/work-records?tenantRef=tenant.external',
        { headers: authHeaders('forge:work:read') },
      ),
    )
    const body = (await response.json()) as { error: string; reason: string }

    expect(response.status).toBe(403)
    expect(body.error).toBe('forge_control_plane_wrong_tenant')
    expect(body.reason).toContain('scoped to one tenant')
  })

  test('rejects control-plane mutations for the wrong tenant scope', async () => {
    const { run } = makeHarness()
    const response = await run(
      requestJson('/api/forge/work-records', {
        json: {
          tenantRef: 'tenant.external',
          issueRef: 'issue.forge.external',
          title: 'External tenant work',
          state: 'open',
        },
        headers: authHeaders('forge:work:write'),
        method: 'POST',
      }),
    )
    const body = (await response.json()) as { error: string }

    expect(response.status).toBe(403)
    expect(body.error).toBe('forge_control_plane_wrong_tenant')
  })

  test('creates work, change, and status rows through scoped live routes', async () => {
    const { run, store } = makeHarness()
    const scopes = [
      'forge:work:write',
      'forge:work:read',
      'forge:change:write',
      'forge:change:read',
      'forge:status:write',
    ].join(' ')

    const workResponse = await run(
      requestJson('/api/forge/work-records', {
        json: {
          tenantRef: 'tenant.openagents',
          issueRef: 'issue.forge.6770',
          githubIssueNumber: 6770,
          title: 'Expose Forge control-plane routes',
          state: 'open',
          sourceRefs: ['github:OpenAgentsInc/openagents#6770'],
        },
        headers: authHeaders(scopes),
        method: 'POST',
      }),
    )
    expect(workResponse.status).toBe(201)

    const changeResponse = await run(
      requestJson('/api/forge/changes', {
        json: {
          tenantRef: 'tenant.openagents',
          prRef: 'pr.forge.6770',
          issueRef: 'issue.forge.6770',
          changeRef: 'change.forge.6770',
          state: 'ready',
          baseHead: '8e0c9b2eaf84c821caf555cae233a0d27e94d4ab',
          patchHead: '9e0c9b2eaf84c821caf555cae233a0d27e94d4ac',
          sourceRefs: ['github:OpenAgentsInc/openagents#6770'],
        },
        headers: authHeaders(scopes),
        method: 'POST',
      }),
    )
    expect(changeResponse.status).toBe(201)

    const statusResponse = await run(
      requestJson('/api/forge/changes/change.forge.6770/status', {
        json: {
          tenantRef: 'tenant.openagents',
          statusRef: 'status.forge.6770.open',
          state: 'open',
          actorRef: 'agent.public.forge',
          sourceRefs: ['github:OpenAgentsInc/openagents#6770'],
        },
        headers: authHeaders(scopes),
        method: 'PATCH',
      }),
    )
    expect(statusResponse.status).toBe(201)

    await expect(store.listIssues('tenant.openagents', 10)).resolves.toHaveLength(1)
    await expect(store.listChanges('tenant.openagents', 10)).resolves.toHaveLength(1)
    await expect(store.listStatuses('tenant.openagents', 10)).resolves.toHaveLength(1)

    const listResponse = await run(
      new Request(
        'https://openagents.com/api/forge/work-records?tenantRef=tenant.openagents',
        { headers: authHeaders(scopes) },
      ),
    )
    const listBody = (await listResponse.json()) as {
      workRecords: ReadonlyArray<unknown>
    }

    expect(listResponse.status).toBe(200)
    expect(listBody.workRecords).toHaveLength(1)
  })

  test('records verification receipts and promotion decisions', async () => {
    const { run } = makeHarness()
    const scopes = [
      'forge:receipt:write',
      'forge:promotion:decide',
      'forge:change:read',
      'forge:queue:read',
    ].join(' ')

    const verificationResponse = await run(
      requestJson('/api/forge/verification-receipts', {
        json: {
          schema: 'openagents.forge.verification.receipt.v0.1',
          tenant_ref: 'tenant.openagents',
          verification_ref: 'verification.forge.6770',
          change_ref: 'change.forge.6770',
          repository_ref: 'repo:OpenAgentsInc/openagents',
          base_ref: 'refs/heads/main',
          base_head: '8e0c9b2eaf84c821caf555cae233a0d27e94d4ab',
          head_ref: 'refs/heads/forge-6770',
          head_head: '9e0c9b2eaf84c821caf555cae233a0d27e94d4ac',
          packfile_ref: 'packfile.forge.6770',
          packfile_sha256: 'sha256:verification',
          executor_identity_ref: 'agent.public.forge',
          command_ref: 'cmd.test',
          command_args: ['bun', 'test'],
          exit_code: 0,
          verdict: 'passed',
          started_at: now,
          completed_at: '2026-06-28T17:02:00.000Z',
          artifact_refs: ['artifact:test-log'],
          log_sha256: 'sha256:log',
          source_refs: ['github:OpenAgentsInc/openagents#6770'],
          redacted: true,
        },
        headers: authHeaders(scopes),
        method: 'POST',
      }),
    )
    expect(verificationResponse.status).toBe(201)

    const promotionResponse = await run(
      requestJson('/api/forge/promotion-decisions', {
        json: {
          schema: 'openagents.forge.promotion.decision.v0.1',
          tenant_ref: 'tenant.openagents',
          promotion_ref: 'promotion.forge.6770',
          queue_ref: 'queue.forge.main',
          change_ref: 'change.forge.6770',
          decision: 'approved',
          base_head: '8e0c9b2eaf84c821caf555cae233a0d27e94d4ab',
          candidate_head: '9e0c9b2eaf84c821caf555cae233a0d27e94d4ac',
          promoted_head: '9e0c9b2eaf84c821caf555cae233a0d27e94d4ac',
          verification_ref: 'verification.forge.6770',
          gate_refs: ['gate.tests'],
          blocker_refs: [],
          decided_by_ref: 'agent.public.forge',
          decided_at: '2026-06-28T17:03:00.000Z',
          source_refs: ['github:OpenAgentsInc/openagents#6770'],
          redacted: true,
        },
        headers: authHeaders(scopes),
        method: 'POST',
      }),
    )
    expect(promotionResponse.status).toBe(201)

    const receiptsResponse = await run(
      new Request(
        'https://openagents.com/api/forge/verification-receipts?tenantRef=tenant.openagents&changeRef=change.forge.6770',
        { headers: authHeaders(scopes) },
      ),
    )
    const receiptsBody = (await receiptsResponse.json()) as {
      verificationReceipts: ReadonlyArray<unknown>
    }
    expect(receiptsBody.verificationReceipts).toHaveLength(1)

    const decisionsResponse = await run(
      new Request(
        'https://openagents.com/api/forge/promotion-decisions?tenantRef=tenant.openagents&changeRef=change.forge.6770',
        { headers: authHeaders(scopes) },
      ),
    )
    const decisionsBody = (await decisionsResponse.json()) as {
      promotionDecisions: ReadonlyArray<unknown>
    }
    expect(decisionsBody.promotionDecisions).toHaveLength(1)
  })

  test('records GitHub mirror receipts as promotion-derived downstream state', async () => {
    const { run } = makeHarness()
    const scopes = [
      'forge:mirror:read',
      'forge:mirror:write',
    ].join(' ')
    const receipt = {
      schema: 'openagents.forge.github_mirror.receipt.v0.1',
      tenant_ref: 'tenant.openagents',
      mirror_ref: 'mirror.github.openagents.main.6796',
      repository_ref: 'repo.openagents.openagents',
      promotion_ref: 'promotion.forge.6796',
      change_ref: 'change.forge.6796',
      source_canonical_ref: 'refs/forge/promoted/openagents/main',
      destination_github_ref: 'refs/heads/main',
      commit_id: '9e0c9b2eaf84c821caf555cae233a0d27e94d4ac',
      status: 'mirrored',
      attempted_at: '2026-06-28T17:04:00.000Z',
      mirrored_at: '2026-06-28T17:04:03.000Z',
      refusal_reason: null,
      error_reason: null,
      retry_count: 0,
      idempotency_key:
        'tenant.openagents:repo.openagents.openagents:promotion.forge.6796:refs/heads/main',
      source_refs: ['github:OpenAgentsInc/openagents#6796'],
      redacted: true,
    }

    const firstResponse = await run(
      requestJson('/api/forge/github-mirror-receipts', {
        json: receipt,
        headers: authHeaders(scopes),
        method: 'POST',
      }),
    )
    expect(firstResponse.status).toBe(201)
    const firstBody = (await firstResponse.json()) as {
      githubMirrorReceipt: { promotion_ref: string; status: string }
    }
    expect(firstBody.githubMirrorReceipt).toMatchObject({
      promotion_ref: 'promotion.forge.6796',
      status: 'mirrored',
    })

    const secondResponse = await run(
      requestJson('/api/forge/github-mirror-receipts', {
        json: { ...receipt, retry_count: 1 },
        headers: authHeaders(scopes),
        method: 'POST',
      }),
    )
    expect(secondResponse.status).toBe(201)

    const listResponse = await run(
      new Request(
        'https://openagents.com/api/forge/github-mirror-receipts?tenantRef=tenant.openagents&changeRef=change.forge.6796',
        { headers: authHeaders(scopes) },
      ),
    )
    const listBody = (await listResponse.json()) as {
      githubMirrorReceipts: ReadonlyArray<{ retry_count: number }>
    }

    expect(listResponse.status).toBe(200)
    expect(listBody.githubMirrorReceipts).toHaveLength(1)
    expect(listBody.githubMirrorReceipts[0]?.retry_count).toBe(1)
  })

  test('imports the public OpenAgents main ref into canonical refs idempotently', async () => {
    const { canonicalStore, run } = makeHarness()
    const headers = authHeaders('forge:admin forge:change:read')
    const json = {
      tenantRef: 'tenant.openagents',
      repositoryRef: 'repo.openagents.openagents',
    }

    const firstResponse = await run(
      requestJson('/api/forge/admin/import-openagents', {
        json,
        headers,
        method: 'POST',
      }),
    )
    const firstBody = (await firstResponse.json()) as {
      changed: boolean
      defaultBranch: { ref_name: string; object_id: string }
      import: { tenantRef: string; repositoryRef: string; defaultBranchRef: string }
    }

    expect(firstResponse.status).toBe(201)
    expect(firstBody.changed).toBe(true)
    expect(firstBody.import).toMatchObject({
      defaultBranchRef: 'refs/heads/main',
      repositoryRef: 'repo.openagents.openagents',
      tenantRef: 'tenant.openagents',
    })
    expect(firstBody.defaultBranch).toMatchObject({
      object_id: githubMainSha,
      ref_name: 'refs/heads/main',
    })

    const secondResponse = await run(
      requestJson('/api/forge/admin/import-openagents', {
        json,
        headers,
        method: 'POST',
      }),
    )
    const secondBody = (await secondResponse.json()) as { changed: boolean }
    expect(secondResponse.status).toBe(200)
    expect(secondBody.changed).toBe(false)

    await expect(
      canonicalStore.listRefs('tenant.openagents', 'repo.openagents.openagents'),
    ).resolves.toHaveLength(1)

    const refsResponse = await run(
      new Request(
        'https://openagents.com/api/forge/refs?tenantRef=tenant.openagents&repositoryRef=repo.openagents.openagents',
        { headers },
      ),
    )
    const refsBody = (await refsResponse.json()) as {
      defaultBranch: { object_id: string; ref_name: string }
      refs: ReadonlyArray<unknown>
      repository: { github: string; defaultBranchRef: string }
    }

    expect(refsResponse.status).toBe(200)
    expect(refsBody.refs).toHaveLength(1)
    expect(refsBody.defaultBranch).toMatchObject({
      object_id: githubMainSha,
      ref_name: 'refs/heads/main',
    })
    expect(refsBody.repository).toMatchObject({
      defaultBranchRef: 'refs/heads/main',
      github: 'OpenAgentsInc/openagents',
    })
  })

  test('fails closed for wrong OpenAgents import targets', async () => {
    const { run } = makeHarness()
    const response = await run(
      requestJson('/api/forge/admin/import-openagents', {
        json: {
          tenantRef: 'tenant.other',
          repositoryRef: 'repo.openagents.openagents',
        },
        headers: authHeaders('forge:admin'),
        method: 'POST',
      }),
    )
    const body = (await response.json()) as { error: string }

    expect(response.status).toBe(403)
    expect(body.error).toBe('forge_openagents_import_target_forbidden')
  })
})
