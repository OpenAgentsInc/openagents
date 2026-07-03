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
  makeD1ForgeGitHubMirrorStore,
  type ForgeGitHubMirrorStore,
} from './forge-github-mirror-store'
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
const gitAccessTokensMigration = readFileSync(
  new URL('../migrations/0253_forge_tenant_git_access_tokens.sql', import.meta.url),
  'utf8',
)
const controlPlaneReceiptsMigration = readFileSync(
  new URL('../migrations/0254_forge_control_plane_receipts.sql', import.meta.url),
  'utf8',
)
const promotionDecisionGateResultsMigration = readFileSync(
  new URL(
    '../migrations/0259_forge_promotion_decision_gate_results.sql',
    import.meta.url,
  ),
  'utf8',
)
const githubMirrorReceiptsMigration = readFileSync(
  new URL(
    '../migrations/0260_forge_github_mirror_receipts.sql',
    import.meta.url,
  ),
  'utf8',
)
const canonicalGitMigration = readFileSync(
  new URL('../migrations/0255_forge_git_canonical_store.sql', import.meta.url),
  'utf8',
)
const agentDefinitionRunsMigration = readFileSync(
  new URL('../migrations/0280_agent_definition_runs.sql', import.meta.url),
  'utf8',
)
const agentDefinitionRunBudgetCreditsMigration = readFileSync(
  new URL(
    '../migrations/0282_agent_definition_run_budget_credits.sql',
    import.meta.url,
  ),
  'utf8',
)
const agentDefinitionForgeGitTokensMigration = readFileSync(
  new URL(
    '../migrations/0284_agent_definition_forge_git_tokens.sql',
    import.meta.url,
  ),
  'utf8',
)

const makeStores = (): Readonly<{
  canonicalStore: ForgeGitCanonicalStore
  coordinationStore: ForgeCoordinationStore
  mirrorStore: ForgeGitHubMirrorStore
}> => {
	  const db = new DatabaseSync(':memory:')
	  db.exec('PRAGMA foreign_keys = ON')
	  db.exec(coordinationMigration)
	  db.exec(gitAccessTokensMigration)
	  db.exec(controlPlaneReceiptsMigration)
	  db.exec(promotionDecisionGateResultsMigration)
	  db.exec(githubMirrorReceiptsMigration)
	  db.exec(canonicalGitMigration)
	  db.exec(agentDefinitionRunsMigration)
	  db.exec(agentDefinitionRunBudgetCreditsMigration)
	  db.exec(agentDefinitionForgeGitTokensMigration)
  const d1 = new SqliteD1(db) as unknown as D1Database
  return {
    canonicalStore: makeD1ForgeGitCanonicalStore(d1),
    coordinationStore: makeD1ForgeCoordinationStore(d1),
    mirrorStore: makeD1ForgeGitHubMirrorStore(d1),
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
const headA = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
const headB = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'
const headC = 'cccccccccccccccccccccccccccccccccccccccc'
const headD = 'dddddddddddddddddddddddddddddddddddddddd'
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

const makeMirrorGitHubFetch = (
  input: Readonly<{ currentSha?: string; updateStatus?: number }> = {},
) => {
  const calls: Array<Readonly<{ body: string | null; method: string; url: string }>> =
    []
  const fetchMock = (async (url: RequestInfo | URL, init?: RequestInit) => {
    const target = String(url)
    const method = init?.method ?? 'GET'
    calls.push({
      body: typeof init?.body === 'string' ? init.body : null,
      method,
      url: target,
    })

    if (method === 'PATCH') {
      return Response.json(
        {
          ref: 'refs/heads/main',
          object: { sha: headB, type: 'commit' },
        },
        { status: input.updateStatus ?? 200 },
      )
    }

    return Response.json({
      ref: 'refs/heads/main',
      object: {
        sha: input.currentSha ?? headA,
        type: 'commit',
        url: `https://api.github.com/repos/OpenAgentsInc/openagents/git/commits/${input.currentSha ?? headA}`,
      },
    })
  }) as typeof fetch

  return { calls, fetchMock }
}

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

const makeHarness = (
  input: Readonly<{
    fetch?: typeof fetch
    mirrorGitHubToken?: string | undefined
  }> = {},
) => {
  const { canonicalStore, coordinationStore, mirrorStore } = makeStores()
  const routes = makeForgeControlPlaneRoutes({
    authorizeControlPlaneBearer: (request, _env, scope) =>
      authorizeForgeControlPlaneBearer(request, controlPlaneToken, scope),
    fetch: input.fetch ?? makeGitHubFetch(),
    makeCanonicalStore: () => canonicalStore,
    makeGitHubMirrorStore: () => mirrorStore,
    makeStore: () => coordinationStore,
    mirrorGitHubToken: () =>
      Object.prototype.hasOwnProperty.call(input, 'mirrorGitHubToken')
        ? input.mirrorGitHubToken
        : 'github-mirror-token',
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

  return { canonicalStore, mirrorStore, run, store: coordinationStore }
}

const createForgeWork = async (
  run: (request: Request) => Promise<Response>,
  issueNumber: number,
  scopes: string,
): Promise<void> => {
  const response = await run(
    requestJson('/api/forge/work-records', {
      json: {
        tenantRef: 'tenant.openagents',
        issueRef: `issue.forge.${issueNumber}`,
        githubIssueNumber: issueNumber,
        title: `Forge issue ${issueNumber}`,
        state: 'open',
        sourceRefs: [`github:OpenAgentsInc/openagents#${issueNumber}`],
      },
      headers: authHeaders(scopes),
      method: 'POST',
    }),
  )
  expect(response.status).toBe(201)
}

const createForgeChange = async (
  run: (request: Request) => Promise<Response>,
  input: Readonly<{
    baseHead: string
    blockerRefs?: ReadonlyArray<string>
    issueNumber: number
    patchHead: string
    scopes: string
    verificationRef: string
  }>,
): Promise<void> => {
  const response = await run(
    requestJson('/api/forge/changes', {
      json: {
        tenantRef: 'tenant.openagents',
        prRef: `pr.forge.${input.issueNumber}`,
        issueRef: `issue.forge.${input.issueNumber}`,
        changeRef: `change.forge.${input.issueNumber}`,
        state: 'ready',
        baseHead: input.baseHead,
        patchHead: input.patchHead,
        verificationRef: input.verificationRef,
        blockerRefs: input.blockerRefs ?? [],
        sourceRefs: [`github:OpenAgentsInc/openagents#${input.issueNumber}`],
      },
      headers: authHeaders(input.scopes),
      method: 'POST',
    }),
  )
  expect(response.status).toBe(201)
}

const createPassingVerification = async (
  run: (request: Request) => Promise<Response>,
  input: Readonly<{
    baseHead: string
    changeRef: string
    headHead: string
    scopes: string
    verificationRef: string
  }>,
): Promise<void> => {
  const response = await run(
    requestJson('/api/forge/verification-receipts', {
      json: {
        schema: 'openagents.forge.verification.receipt.v0.1',
        tenant_ref: 'tenant.openagents',
        verification_ref: input.verificationRef,
        change_ref: input.changeRef,
        repository_ref: 'repo.openagents.openagents',
        base_ref: 'refs/heads/main',
        base_head: input.baseHead,
        head_ref: `refs/heads/${input.changeRef}`,
        head_head: input.headHead,
        packfile_ref: `packfile.${input.changeRef}`,
        packfile_sha256: `sha256:${input.verificationRef}`,
        executor_identity_ref: 'agent.public.forge',
        command_ref: `command.${input.verificationRef}`,
        command_args: ['bun', 'test'],
        exit_code: 0,
        verdict: 'passed',
        started_at: now,
        completed_at: '2026-06-28T17:02:00.000Z',
        artifact_refs: ['artifact:test-log'],
        log_sha256: `sha256:log.${input.verificationRef}`,
        source_refs: ['github:OpenAgentsInc/openagents#6794'],
        redacted: true,
      },
      headers: authHeaders(input.scopes),
      method: 'POST',
    }),
  )
  expect(response.status).toBe(201)
}

const createPromotionDecision = async (
  run: (request: Request) => Promise<Response>,
  input: Readonly<{
    baseHead: string
    decision?: 'approved' | 'blocked'
    headHead: string
    issueNumber: number
    scopes: string
    verificationRef?: string | null
  }>,
): Promise<void> => {
  const response = await run(
    requestJson('/api/forge/promotion-decisions', {
      json: {
        schema: 'openagents.forge.promotion.decision.v0.1',
        tenant_ref: 'tenant.openagents',
        promotion_ref: `promotion.forge.${input.issueNumber}`,
        queue_ref: 'queue.forge.openagents.main',
        queue_position: 0,
        change_ref: `change.forge.${input.issueNumber}`,
        decision: input.decision ?? 'approved',
        target_ref: 'refs/heads/main',
        base_head: input.baseHead,
        candidate_head: input.headHead,
        promoted_head:
          (input.decision ?? 'approved') === 'approved' ? input.headHead : null,
        verification_ref: Object.prototype.hasOwnProperty.call(
          input,
          'verificationRef',
        )
          ? input.verificationRef
          : `verification.forge.${input.issueNumber}`,
        gate_refs: ['gate.tests'],
        gate_results: [
          {
            gate_ref: 'gate.tests',
            verdict: (input.decision ?? 'approved') === 'approved' ? 'passed' : 'blocked',
            evidence_refs: [`verification.forge.${input.issueNumber}`],
            blocker_refs:
              (input.decision ?? 'approved') === 'approved'
                ? []
                : ['blocker.forge.test'],
            decided_at: '2026-06-28T17:03:00.000Z',
          },
        ],
        blocker_refs:
          (input.decision ?? 'approved') === 'approved'
            ? []
            : ['blocker.forge.test'],
        decided_by_ref: 'agent.public.forge',
        decided_at: '2026-06-28T17:03:00.000Z',
        source_refs: [`github:OpenAgentsInc/openagents#${input.issueNumber}`],
        redacted: true,
      },
      headers: authHeaders(input.scopes),
      method: 'POST',
    }),
  )
  expect(response.status).toBe(201)
}

const importCanonicalMain = async (
  canonicalStore: ForgeGitCanonicalStore,
  objectId: string,
): Promise<void> => {
  await canonicalStore.importExternalRef({
    changeRef: `change.forge.promoted.${objectId.slice(0, 12)}`,
    objectFormat: 'sha1',
    objectId,
    packfileRef: `packfile.forge.promoted.${objectId.slice(0, 12)}`,
    receivePackRef: `receive-pack.forge.promoted.${objectId.slice(0, 12)}`,
    refName: 'refs/heads/main',
    repositoryRef: 'repo.openagents.openagents',
    sourceDigestSha256: objectId.padEnd(64, '0').slice(0, 64),
    sourceRefs: ['github:OpenAgentsInc/openagents#6796'],
    tenantRef: 'tenant.openagents',
    nowIso: now,
  })
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
          queue_position: 0,
          change_ref: 'change.forge.6770',
          decision: 'approved',
          target_ref: 'refs/heads/main',
          base_head: '8e0c9b2eaf84c821caf555cae233a0d27e94d4ab',
          candidate_head: '9e0c9b2eaf84c821caf555cae233a0d27e94d4ac',
          promoted_head: '9e0c9b2eaf84c821caf555cae233a0d27e94d4ac',
          verification_ref: 'verification.forge.6770',
          gate_refs: ['gate.tests'],
          gate_results: [
            {
              gate_ref: 'gate.tests',
              verdict: 'passed',
              evidence_refs: ['verification.forge.6770'],
              blocker_refs: [],
              decided_at: '2026-06-28T17:03:00.000Z',
            },
          ],
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

  test('mirrors only a Forge-approved canonical promotion to GitHub and reruns idempotently', async () => {
    const github = makeMirrorGitHubFetch({ currentSha: headA })
    const { canonicalStore, run } = makeHarness({ fetch: github.fetchMock })
    const scopes = [
      'forge:work:write',
      'forge:change:write',
      'forge:receipt:write',
      'forge:promotion:decide',
      'forge:mirror:read',
      'forge:mirror:write',
    ].join(' ')

    await createForgeWork(run, 6796, scopes)
    await createForgeChange(run, {
      baseHead: headA,
      issueNumber: 6796,
      patchHead: headB,
      scopes,
      verificationRef: 'verification.forge.6796',
    })
    await createPassingVerification(run, {
      baseHead: headA,
      changeRef: 'change.forge.6796',
      headHead: headB,
      scopes,
      verificationRef: 'verification.forge.6796',
    })
    await createPromotionDecision(run, {
      baseHead: headA,
      headHead: headB,
      issueNumber: 6796,
      scopes,
    })
    await importCanonicalMain(canonicalStore, headB)

    const response = await run(
      requestJson('/api/forge/github-mirror/run', {
        json: {
          tenantRef: 'tenant.openagents',
          promotionRef: 'promotion.forge.6796',
        },
        headers: authHeaders(scopes),
        method: 'POST',
      }),
    )
    const body = (await response.json()) as {
      attention: { state: string }
      mirrorReceipts: ReadonlyArray<{
        attempt_count: number
        commit_id: string
        destination_github_ref: string
        status: string
      }>
      mirroredCount: number
    }

    expect(response.status).toBe(200)
    expect(body.mirroredCount).toBe(1)
    expect(body.attention.state).toBe('clear')
    expect(body.mirrorReceipts[0]).toMatchObject({
      attempt_count: 1,
      commit_id: headB,
      destination_github_ref: 'refs/heads/main',
      status: 'mirrored',
    })
    expect(github.calls.map(call => call.method)).toEqual(['GET', 'PATCH'])
    expect(github.calls.find(call => call.method === 'PATCH')?.body).toBe(
      JSON.stringify({ force: false, sha: headB }),
    )

    const rerunResponse = await run(
      requestJson('/api/forge/github-mirror/run', {
        json: {
          tenantRef: 'tenant.openagents',
          promotionRef: 'promotion.forge.6796',
        },
        headers: authHeaders(scopes),
        method: 'POST',
      }),
    )
    const rerunBody = (await rerunResponse.json()) as {
      mirrorReceipts: ReadonlyArray<{ attempt_count: number; status: string }>
    }

    expect(rerunResponse.status).toBe(200)
    expect(rerunBody.mirrorReceipts[0]).toMatchObject({
      attempt_count: 1,
      status: 'mirrored',
    })
    expect(github.calls.map(call => call.method)).toEqual(['GET', 'PATCH'])

    const listResponse = await run(
      new Request(
        'https://openagents.com/api/forge/github-mirror?tenantRef=tenant.openagents&promotionRef=promotion.forge.6796',
        { headers: authHeaders(scopes) },
      ),
    )
    const listBody = (await listResponse.json()) as {
      mirrorReceipts: ReadonlyArray<unknown>
    }
    expect(listResponse.status).toBe(200)
    expect(listBody.mirrorReceipts).toHaveLength(1)
  })

  test('refuses non-promoted changes without touching GitHub', async () => {
    const github = makeMirrorGitHubFetch()
    const { run } = makeHarness({ fetch: github.fetchMock })
    const scopes = [
      'forge:promotion:decide',
      'forge:mirror:write',
    ].join(' ')

    await createPromotionDecision(run, {
      baseHead: headA,
      decision: 'blocked',
      headHead: headB,
      issueNumber: 6796,
      scopes,
      verificationRef: null,
    })

    const response = await run(
      requestJson('/api/forge/github-mirror/run', {
        json: {
          tenantRef: 'tenant.openagents',
          promotionRef: 'promotion.forge.6796',
        },
        headers: authHeaders(scopes),
        method: 'POST',
      }),
    )
    const body = (await response.json()) as {
      attention: { reasonRefs: ReadonlyArray<string>; state: string }
      mirrorReceipts: ReadonlyArray<{ refusal_reason: string; status: string }>
      refusedCount: number
    }

    expect(response.status).toBe(200)
    expect(body.refusedCount).toBe(1)
    expect(body.attention.state).toBe('needs_attention')
    expect(body.attention.reasonRefs).toContain(
      'forge_github_mirror_requires_approved_promotion',
    )
    expect(body.mirrorReceipts[0]).toMatchObject({
      refusal_reason: 'forge_github_mirror_requires_approved_promotion',
      status: 'refused',
    })
    expect(github.calls).toHaveLength(0)
  })

  test('records failed mirror attempts without advancing GitHub state', async () => {
    const github = makeMirrorGitHubFetch({
      currentSha: headA,
      updateStatus: 422,
    })
    const { canonicalStore, run } = makeHarness({ fetch: github.fetchMock })
    const scopes = [
      'forge:work:write',
      'forge:change:write',
      'forge:receipt:write',
      'forge:promotion:decide',
      'forge:mirror:write',
    ].join(' ')

    await createForgeWork(run, 6796, scopes)
    await createForgeChange(run, {
      baseHead: headA,
      issueNumber: 6796,
      patchHead: headB,
      scopes,
      verificationRef: 'verification.forge.6796',
    })
    await createPassingVerification(run, {
      baseHead: headA,
      changeRef: 'change.forge.6796',
      headHead: headB,
      scopes,
      verificationRef: 'verification.forge.6796',
    })
    await createPromotionDecision(run, {
      baseHead: headA,
      headHead: headB,
      issueNumber: 6796,
      scopes,
    })
    await importCanonicalMain(canonicalStore, headB)

    const response = await run(
      requestJson('/api/forge/github-mirror/run', {
        json: {
          tenantRef: 'tenant.openagents',
          promotionRef: 'promotion.forge.6796',
        },
        headers: authHeaders(scopes),
        method: 'POST',
      }),
    )
    const body = (await response.json()) as {
      attention: { reasonRefs: ReadonlyArray<string>; state: string }
      failedCount: number
      mirrorReceipts: ReadonlyArray<{ error_reason: string; status: string }>
    }

    expect(response.status).toBe(200)
    expect(body.failedCount).toBe(1)
    expect(body.attention.state).toBe('needs_attention')
    expect(body.attention.reasonRefs).toContain(
      'forge_github_mirror_ref_update_http_422',
    )
    expect(body.mirrorReceipts[0]).toMatchObject({
      error_reason: 'forge_github_mirror_ref_update_http_422',
      status: 'failed',
    })
    expect(github.calls.map(call => call.method)).toEqual(['GET', 'PATCH'])
  })

  test('derives nextActualPromotion from live Forge rows and serializes concurrent changes', async () => {
    const { run, store } = makeHarness()
    const scopes = [
      'forge:work:write',
      'forge:change:write',
      'forge:receipt:write',
      'forge:queue:read',
      'forge:queue:write',
    ].join(' ')

    await createForgeWork(run, 6794, scopes)
    await createForgeWork(run, 6795, scopes)
    await createForgeChange(run, {
      baseHead: headA,
      issueNumber: 6794,
      patchHead: headB,
      scopes,
      verificationRef: 'verification.forge.6794',
    })
    await createForgeChange(run, {
      baseHead: headB,
      issueNumber: 6795,
      patchHead: headC,
      scopes,
      verificationRef: 'verification.forge.6795',
    })
    await createPassingVerification(run, {
      baseHead: headA,
      changeRef: 'change.forge.6794',
      headHead: headB,
      scopes,
      verificationRef: 'verification.forge.6794',
    })
    await createPassingVerification(run, {
      baseHead: headB,
      changeRef: 'change.forge.6795',
      headHead: headC,
      scopes,
      verificationRef: 'verification.forge.6795',
    })

    const deriveResponse = await run(
      requestJson('/api/forge/queue/derive', {
        json: {
          tenantRef: 'tenant.openagents',
          queueRef: 'queue.forge.openagents.main',
          actualHead: headA,
          sourceRefs: ['github:OpenAgentsInc/openagents#6794'],
        },
        headers: authHeaders(scopes),
        method: 'POST',
      }),
    )
    const deriveBody = (await deriveResponse.json()) as {
      derived: {
        nextActualPromotion: { candidateRef: string; waitsForActualHead: string | null }
        ready: ReadonlyArray<{ candidateRef: string; waitsForActualHead: string | null }>
        virtualHead: string
      }
      queueSnapshot: { next_promotion_ref: string | null; virtual_head: string }
    }

    expect(deriveResponse.status).toBe(201)
    expect(deriveBody.derived.ready.map(entry => entry.candidateRef)).toEqual([
      'change.forge.6794',
      'change.forge.6795',
    ])
    expect(deriveBody.derived.nextActualPromotion).toMatchObject({
      candidateRef: 'change.forge.6794',
      waitsForActualHead: null,
    })
    expect(deriveBody.derived.ready[1]).toMatchObject({
      candidateRef: 'change.forge.6795',
      waitsForActualHead: headB,
    })
    expect(deriveBody.queueSnapshot.next_promotion_ref).toContain(
      'promotion.forge.next_actual.',
    )
    expect(deriveBody.queueSnapshot.virtual_head).toBe(headC)

    await expect(store.readLatestMergeQueueLedger('tenant.openagents')).resolves.toMatchObject({
      next_promotion_ref: deriveBody.queueSnapshot.next_promotion_ref,
      virtual_head: headC,
    })
  })

  test('blocks stale-base and deletion-poisoned changes before promotion', async () => {
    const { run } = makeHarness()
    const scopes = [
      'forge:work:write',
      'forge:change:write',
      'forge:receipt:write',
      'forge:queue:read',
    ].join(' ')

    await createForgeWork(run, 6794, scopes)
    await createForgeWork(run, 6796, scopes)
    await createForgeChange(run, {
      baseHead: headD,
      issueNumber: 6794,
      patchHead: headB,
      scopes,
      verificationRef: 'verification.forge.6794',
    })
    await createForgeChange(run, {
      baseHead: headA,
      blockerRefs: ['blocker.forge.deletion_poison.protected_path_deleted'],
      issueNumber: 6796,
      patchHead: headC,
      scopes,
      verificationRef: 'verification.forge.6796',
    })
    await createPassingVerification(run, {
      baseHead: headD,
      changeRef: 'change.forge.6794',
      headHead: headB,
      scopes,
      verificationRef: 'verification.forge.6794',
    })
    await createPassingVerification(run, {
      baseHead: headA,
      changeRef: 'change.forge.6796',
      headHead: headC,
      scopes,
      verificationRef: 'verification.forge.6796',
    })

    const response = await run(
      new Request(
        `https://openagents.com/api/forge/queue?tenantRef=tenant.openagents&actualHead=${headA}`,
        { headers: authHeaders(scopes) },
      ),
    )
    const body = (await response.json()) as {
      derived: {
        blocked: ReadonlyArray<{ blockedReasonRef: string; candidateRef: string }>
        nextActualPromotion: null
      }
    }

    expect(response.status).toBe(200)
    expect(body.derived.nextActualPromotion).toBeNull()
    expect(body.derived.blocked).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          blockedReasonRef: 'virtual_merge_queue.blocked.stale_base',
          candidateRef: 'change.forge.6794',
        }),
        expect.objectContaining({
          blockedReasonRef: 'virtual_merge_queue.blocked.protected_path_deleted',
          candidateRef: 'change.forge.6796',
        }),
      ]),
    )
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
