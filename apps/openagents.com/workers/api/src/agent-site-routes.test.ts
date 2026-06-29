import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  AGENT_TOKEN_PREFIX,
  type AgentCredentialLookup,
  type AgentRegistrationStore,
  sha256Hex,
} from './agent-registration'
import {
  AgentSitesInternalGateHeader,
  AgentSitesInternalGateValue,
  makeAgentSiteRoutes,
} from './agent-site-routes'
import { isRecord } from './json-boundary'

type Row = Record<string, unknown>

class BuilderApiStore {
  site_builder_saved_versions: Array<Row> = []
  site_builder_artifacts: Array<Row> = []
  site_builder_events: Array<Row> = []
  site_builder_file_snapshots: Array<Row> = []
  site_builder_messages: Array<Row> = []
  site_builder_phase_runs: Array<Row> = []
  site_builder_previews: Array<Row> = []
  site_builder_sessions: Array<Row> = []
  site_events: Array<Row> = []
  site_projects: Array<Row> = []
  site_versions: Array<Row> = []
  software_orders: Array<Row> = []
}

const builderApiTables = [
  'software_orders',
  'site_projects',
  'site_versions',
  'site_events',
  'site_builder_saved_versions',
  'site_builder_sessions',
  'site_builder_messages',
  'site_builder_events',
  'site_builder_phase_runs',
  'site_builder_file_snapshots',
  'site_builder_previews',
  'site_builder_artifacts',
] as const

type BuilderApiTable = (typeof builderApiTables)[number]

const builderApiTableFromQuery = (query: string): BuilderApiTable => {
  const table = builderApiTables.find(name => query.includes(name))

  if (table === undefined) {
    throw new Error(`Unknown builder API table for query: ${query}`)
  }

  return table
}

const activeBuilderRow = (row: Row): boolean => row.archived_at === null

const findBuilderRowByIdempotency = (
  rows: ReadonlyArray<Row>,
  idempotencyKey: string,
): Row | null =>
  rows.find(
    row => row.idempotency_key === idempotencyKey && activeBuilderRow(row),
  ) ?? null

const pushBuilderFileSnapshot = (
  store: BuilderApiStore,
  input: Readonly<{
    contentHash?: string | undefined
    path: string
    previewText?: string | null | undefined
    sequence: number
    sessionId: string
    visibility?: string | undefined
  }>,
) => {
  store.site_builder_file_snapshots.push({
    archived_at: null,
    artifact_ref: null,
    byte_size: input.previewText?.length ?? 0,
    content_hash: input.contentHash ?? `sha256:test-${input.sequence}`,
    created_at: '2026-06-05T23:30:00.000Z',
    id: `site_builder_file_${input.sequence}`,
    idempotency_key: `site-builder-file:${input.sequence}`,
    language: input.path.endsWith('.tsx') ? 'tsx' : null,
    metadata_json: '{}',
    path: input.path,
    preview_text: input.previewText ?? null,
    sequence: input.sequence,
    session_id: input.sessionId,
    source_ref: null,
    updated_at: '2026-06-05T23:30:00.000Z',
    visibility: input.visibility ?? 'customer',
  })
}

const seedSoftwareOrder = (
  store: BuilderApiStore,
  input: Readonly<{
    id?: string | undefined
    request?: string | undefined
    userId?: string | undefined
  }> = {},
) => {
  store.software_orders.push({
    archived_at: null,
    id: input.id ?? 'software_order_1',
    repository_default_branch: null,
    repository_name: null,
    repository_owner: null,
    repository_provider: null,
    request: input.request ?? 'Build a customer-safe product page.',
    user_id: input.userId ?? 'user_owner',
  })
}

const seedSiteProject = (
  store: BuilderApiStore,
  input: Readonly<{
    id?: string | undefined
    ownerUserId?: string | undefined
    slug?: string | undefined
    softwareOrderId?: string | null | undefined
    title?: string | undefined
  }> = {},
) => {
  store.site_projects.push({
    access_mode: 'public',
    active_deployment_id: null,
    active_version_id: null,
    archived_at: null,
    created_at: '2026-06-05T23:00:00.000Z',
    id: input.id ?? 'site_123',
    owner_user_id: input.ownerUserId ?? 'user_owner',
    project_id: null,
    prompt: 'Build a customer-safe product page.',
    slug: input.slug ?? 'customer-site',
    software_order_id: input.softwareOrderId ?? null,
    source_repository_name: null,
    source_repository_owner: null,
    source_repository_provider: null,
    source_repository_ref: null,
    status: 'draft',
    team_id: null,
    title: input.title ?? 'Customer Site',
    updated_at: '2026-06-05T23:00:00.000Z',
    visibility: 'public',
  })
}

class BuilderApiStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: BuilderApiStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    const table = builderApiTableFromQuery(this.query)
    const rows = this.store[table]

    if (
      table === 'site_projects' &&
      this.query.includes('WHERE software_order_id = ?')
    ) {
      return Promise.resolve(
        (rows.find(
          row =>
            row.software_order_id === String(this.values[0]) &&
            activeBuilderRow(row),
        ) ?? null) as T | null,
      )
    }

    if (table === 'site_projects' && this.query.includes('WHERE slug = ?')) {
      return Promise.resolve(
        (rows.find(
          row => row.slug === String(this.values[0]) && activeBuilderRow(row),
        ) ?? null) as T | null,
      )
    }

    if (this.query.includes('WHERE id = ?')) {
      return Promise.resolve(
        (rows.find(
          row => row.id === String(this.values[0]) && activeBuilderRow(row),
        ) ?? null) as T | null,
      )
    }

    if (this.query.includes('WHERE idempotency_key = ?')) {
      return Promise.resolve(
        findBuilderRowByIdempotency(rows, String(this.values[0])) as T | null,
      )
    }

    if (
      this.query.includes('WHERE session_id = ?') &&
      this.query.includes('AND path = ?')
    ) {
      const sessionId = String(this.values[0])
      const path = String(this.values[1])
      const file =
        rows
          .filter(
            row =>
              row.session_id === sessionId &&
              row.path === path &&
              activeBuilderRow(row),
          )
          .sort(
            (left, right) => Number(right.sequence) - Number(left.sequence),
          )[0] ?? null

      return Promise.resolve(file as T | null)
    }

    if (this.query.includes('MAX(sequence)')) {
      const sessionId = String(this.values[0])
      const maxSequence = rows
        .filter(row => row.session_id === sessionId && activeBuilderRow(row))
        .reduce((max, row) => Math.max(max, Number(row.sequence ?? 0)), 0)

      return Promise.resolve({ next_sequence: maxSequence + 1 } as T)
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    const table = builderApiTableFromQuery(this.query)

    if (table === 'site_projects') {
      this.store.site_projects.push({
        access_mode: String(this.values[8]),
        active_deployment_id: null,
        active_version_id: null,
        archived_at: null,
        created_at: String(this.values[16]),
        id: String(this.values[0]),
        owner_user_id: String(this.values[2]),
        project_id: this.values[4] as string | null,
        prompt: String(this.values[7]),
        slug: String(this.values[5]),
        software_order_id: this.values[1] as string | null,
        source_repository_name: this.values[12] as string | null,
        source_repository_owner: this.values[11] as string | null,
        source_repository_provider: this.values[10] as string | null,
        source_repository_ref: this.values[13] as string | null,
        status: 'draft',
        team_id: this.values[3] as string | null,
        title: String(this.values[6]),
        updated_at: String(this.values[17]),
        visibility: String(this.values[9]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (table === 'site_events') {
      this.store.site_events.push({
        actor_run_id: this.values[7] as string | null,
        actor_user_id: this.values[6] as string | null,
        created_at: String(this.values[9]),
        deployment_id: this.values[3] as string | null,
        id: String(this.values[0]),
        payload_json: this.values[8] as string | null,
        site_id: String(this.values[1]),
        summary: String(this.values[5]),
        type: String(this.values[4]),
        version_id: this.values[2] as string | null,
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (table === 'site_versions') {
      this.store.site_versions.push({
        artifact_manifest_r2_key: this.values[5] as string | null,
        build_command: this.values[8] as string | null,
        build_log_r2_key: this.values[6] as string | null,
        build_status: String(this.values[7]),
        created_at: String(this.values[16]),
        created_by_run_id: this.values[15] as string | null,
        created_by_user_id: this.values[14] as string | null,
        d1_binding_name: this.values[11] as string | null,
        id: String(this.values[0]),
        metadata_json: String(this.values[13]),
        r2_binding_name: this.values[12] as string | null,
        rejected_at: this.values[18] as string | null,
        saved_at: this.values[17] as string | null,
        site_id: String(this.values[1]),
        source_archive_r2_key: this.values[4] as string | null,
        source_commit_sha: this.values[3] as string | null,
        source_kind: String(this.values[2]),
        static_assets_manifest_json: String(this.values[10]),
        worker_module_r2_key: this.values[9] as string | null,
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (table === 'site_builder_saved_versions') {
      this.store.site_builder_saved_versions.push({
        archived_at: null,
        artifact_ref: this.values[6] as string | null,
        build_receipt_ref: this.values[7] as string | null,
        created_at: String(this.values[11]),
        id: String(this.values[0]),
        idempotency_key: String(this.values[1]),
        notes: this.values[9] as string | null,
        preview_id: this.values[5] as string | null,
        session_id: String(this.values[2]),
        site_id: String(this.values[3]),
        site_metadata_json: String(this.values[10]),
        site_version_id: String(this.values[4]),
        source_hash: this.values[8] as string | null,
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    const idempotencyKey = String(this.values[1])

    if (
      findBuilderRowByIdempotency(this.store[table], idempotencyKey) !== null
    ) {
      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (table === 'site_builder_sessions') {
      this.store.site_builder_sessions.push({
        active_artifact_id: null,
        active_preview_id: null,
        archived_at: null,
        created_at: String(this.values[14]),
        created_by_actor_ref: String(this.values[7]),
        customer_user_id: this.values[6] as string | null,
        id: String(this.values[0]),
        idempotency_key: idempotencyKey,
        metadata_json: String(this.values[13]),
        order_id: this.values[3] as string | null,
        owner_user_id: String(this.values[5]),
        prompt_summary: String(this.values[9]),
        site_id: this.values[2] as string | null,
        source_revision_id: this.values[11] as string | null,
        source_site_version_id: this.values[10] as string | null,
        status: String(this.values[8]),
        updated_at: String(this.values[15]),
        workroom_id: this.values[4] as string | null,
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (table === 'site_builder_messages') {
      this.store.site_builder_messages.push({
        actor_kind: String(this.values[4]),
        archived_at: null,
        body: String(this.values[6]),
        created_at: String(this.values[8]),
        id: String(this.values[0]),
        idempotency_key: idempotencyKey,
        metadata_json: String(this.values[7]),
        sequence: Number(this.values[3]),
        session_id: String(this.values[2]),
        visibility: String(this.values[5]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (table === 'site_builder_events') {
      this.store.site_builder_events.push({
        archived_at: null,
        created_at: String(this.values[12]),
        event_kind: String(this.values[4]),
        id: String(this.values[0]),
        idempotency_key: idempotencyKey,
        payload_json: String(this.values[11]),
        phase_kind: this.values[5] as string | null,
        sequence: Number(this.values[3]),
        session_id: String(this.values[2]),
        source_ref: this.values[10] as string | null,
        status: String(this.values[7]),
        summary: String(this.values[9]),
        title: String(this.values[8]),
        visibility: String(this.values[6]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (table === 'site_builder_file_snapshots') {
      this.store.site_builder_file_snapshots.push({
        archived_at: null,
        artifact_ref: this.values[9] as string | null,
        byte_size: Number(this.values[7]),
        content_hash: String(this.values[6]),
        created_at: String(this.values[13]),
        id: String(this.values[0]),
        idempotency_key: idempotencyKey,
        language: this.values[5] as string | null,
        metadata_json: String(this.values[12]),
        path: String(this.values[3]),
        preview_text: this.values[10] as string | null,
        sequence: Number(this.values[4]),
        session_id: String(this.values[2]),
        source_ref: this.values[8] as string | null,
        updated_at: String(this.values[14]),
        visibility: String(this.values[11]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (table === 'site_builder_previews') {
      this.store.site_builder_previews.push({
        archived_at: null,
        artifact_ref: this.values[7] as string | null,
        created_at: String(this.values[10]),
        health_ref: this.values[8] as string | null,
        id: String(this.values[0]),
        idempotency_key: idempotencyKey,
        metadata_json: String(this.values[9]),
        preview_kind: String(this.values[3]),
        preview_url: this.values[5] as string | null,
        session_id: String(this.values[2]),
        status: String(this.values[4]),
        updated_at: String(this.values[11]),
        version_ref: this.values[6] as string | null,
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    const table = builderApiTableFromQuery(this.query)
    const sessionId = String(this.values[0])
    const cursor = this.query.includes('sequence >')
      ? Number(this.values[1])
      : 0
    const results = this.store[table]
      .filter(
        row =>
          row.session_id === sessionId &&
          activeBuilderRow(row) &&
          Number(row.sequence ?? 0) > cursor,
      )
      .sort((left, right) => {
        if (table === 'site_builder_file_snapshots') {
          const pathOrder = String(left.path).localeCompare(String(right.path))

          return pathOrder === 0
            ? Number(right.sequence) - Number(left.sequence)
            : pathOrder
        }

        return Number(left.sequence ?? 0) - Number(right.sequence ?? 0)
      })

    return Promise.resolve({
      results: results as ReadonlyArray<T>,
      success: true,
    } as D1Result<T>)
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(options?: {
    columnNames?: boolean
  }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
    return options?.columnNames === true
      ? Promise.resolve([[]])
      : Promise.resolve([])
  }
}

const builderApiDb = (store: BuilderApiStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new BuilderApiStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const memoryArtifacts = (
  objects: Map<string, unknown> = new Map(),
): R2Bucket =>
  ({
    put: async (key: string, value: unknown) => {
      objects.set(key, value)

      return null
    },
  }) as R2Bucket

const executionContext = {
  passThroughOnException: () => undefined,
  waitUntil: () => undefined,
} as never

type TestSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

const session: TestSession = {
  user: {
    email: 'agent-operator@openagents.com',
    userId: 'user_agent_operator',
  },
}

const makeRequest = (
  path: string,
  input: Readonly<{
    bearerToken?: string | undefined
    body?: unknown
    gate?: boolean
    idempotencyKey?: string
    method?: string
  }> = {},
) =>
  new Request(`https://openagents.com${path}`, {
    ...(input.body === undefined ? {} : { body: JSON.stringify(input.body) }),
    headers: {
      ...(input.gate === false
        ? {}
        : { [AgentSitesInternalGateHeader]: AgentSitesInternalGateValue }),
      ...(input.idempotencyKey === undefined
        ? {}
        : { 'idempotency-key': input.idempotencyKey }),
      ...(input.bearerToken === undefined
        ? {}
        : { authorization: `Bearer ${input.bearerToken}` }),
      ...(input.body === undefined
        ? {}
        : { 'content-type': 'application/json' }),
    },
    method: input.method ?? 'POST',
  })

const routeRequest = async (
  request: Request,
  input: Readonly<{
    artifacts?: R2Bucket | undefined
    agentStore?: AgentRegistrationStore | undefined
    db?: D1Database | undefined
    session?: TestSession | undefined
  }> = {},
): Promise<Response> => {
  const agentStore = input.agentStore
  const routes = makeAgentSiteRoutes({
    appendRefreshedSessionCookies: (response, activeSession) => {
      response.headers.set('x-test-session-user-id', activeSession.user.userId)

      return response
    },
    artifactsForEnv: () => input.artifacts,
    ...(agentStore === undefined
      ? {}
      : { agentStoreForEnv: () => agentStore }),
    dbForEnv: () => {
      if (input.db === undefined) {
        throw new Error('D1 test database was not provided.')
      }

      return input.db
    },
    isAdminEmail: email => email === 'agent-operator@openagents.com',
    requireBrowserSession: async () => input.session,
  })
  const routed = routes.routeAgentSiteRequest(
    request,
    {} as never,
    executionContext,
  )

  if (routed === undefined) {
    throw new Error('Expected agent Site route to match.')
  }

  return Effect.runPromise(routed)
}

const agentSiteToken = `${AGENT_TOKEN_PREFIX}site_action_test`

const agentLookup = (
  metadata: Record<string, unknown>,
): AgentCredentialLookup => ({
  credentialId: 'agent_credential_sites',
  profileMetadataJson: JSON.stringify(metadata),
  tokenPrefix: `${AGENT_TOKEN_PREFIX}site`,
  user: {
    avatarUrl: null,
    createdAt: '2026-06-05T00:00:00.000Z',
    displayName: 'Site Action Agent',
    id: 'agent_site_user',
    kind: 'agent',
    primaryEmail: null,
    status: 'active',
    updatedAt: '2026-06-05T00:00:00.000Z',
  },
})

class MemoryAgentSiteStore implements AgentRegistrationStore {
  constructor(private readonly lookup?: AgentCredentialLookup) {}

  createAgentRegistration = async () => {}

  findAgentByTokenHash = async (tokenHash: string) =>
    tokenHash === (await sha256Hex(agentSiteToken))
      ? this.lookup
      : undefined

  touchAgentCredential = async () => {}


  updateAgentDisplayName = async () => 0
}

describe('agent Site routes', () => {
  test('rejects non-public access without the internal preview gate', async () => {
    const response = await routeRequest(
      makeRequest('/api/agent/sites', {
        body: { title: 'Customer Site' },
        gate: false,
        idempotencyKey: 'agent-sites-test-1',
      }),
      { session },
    )

    await expect(response.json()).resolves.toEqual({
      error: 'agent_sites_internal_gate_required',
      message:
        'Agent Sites APIs require the internal preview gate or a registered agent bearer token with a matching agentSiteGrants scope.',
      requiredHeader: AgentSitesInternalGateHeader,
    })
    expect(response.status).toBe(403)
  })

  test('requires a browser session behind the internal gate', async () => {
    const response = await routeRequest(
      makeRequest('/api/agent/sites', {
        body: { title: 'Customer Site' },
        idempotencyKey: 'agent-sites-test-2',
      }),
    )

    await expect(response.json()).resolves.toEqual({ error: 'unauthorized' })
    expect(response.status).toBe(401)
  })

  test('allows registered agent tokens with matching Site action grant', async () => {
    const store = new BuilderApiStore()
    seedSiteProject(store, { id: 'site_123', ownerUserId: 'user_owner' })
    const response = await routeRequest(
      makeRequest('/api/agent/sites/site_123/previews', {
        bearerToken: agentSiteToken,
        body: { description: 'Preview candidate shell.' },
        gate: false,
        idempotencyKey: 'agent-sites-token-preview',
      }),
      {
        agentStore: new MemoryAgentSiteStore(
          agentLookup({
            agentSiteGrants: [
              {
                expiresAt: null,
                scopes: ['sites:preview:request'],
                siteId: 'site_123',
                status: 'active',
              },
            ],
          }),
        ),
        db: builderApiDb(store),
      },
    )
    const payload = await response.json()

    expect(response.status).toBe(202)
    expect(response.headers.get('ratelimit-policy')).toBe('60;w=60')
    expect(response.headers.get('x-openagents-recovery-modes')).toContain(
      'future_l402',
    )
    expect(response.headers.has('x-test-session-user-id')).toBe(false)
    expect(payload).toMatchObject({
      agentSites: {
        action: 'preview_request',
        implementationState: 'preview_queued',
        receipt: {
          actorUserId: 'agent_site_user',
          idempotencyKey: 'agent-sites-token-preview',
          ownerUserId: 'agent_site_user',
          requiredScope: 'sites:preview:request',
          scopeSatisfiedBy: 'registered_agent_token_with_agent_site_grant',
          status: 'queued',
        },
      },
    })
    expect(store.site_builder_previews).toHaveLength(1)
  })

  test('rejects registered agent tokens without the required Site action scope', async () => {
    const response = await routeRequest(
      makeRequest('/api/agent/sites/site_123/versions', {
        bearerToken: agentSiteToken,
        body: { notes: 'Save this candidate.' },
        gate: false,
        idempotencyKey: 'agent-sites-token-save-denied',
      }),
      {
        agentStore: new MemoryAgentSiteStore(
          agentLookup({
            agentSiteGrants: [
              {
                expiresAt: null,
                scopes: ['sites:preview:request'],
                siteId: 'site_123',
                status: 'active',
              },
            ],
          }),
        ),
      },
    )

    expect(response.status).toBe(403)
    await expect(response.json()).resolves.toEqual({
      error: 'agent_sites_scope_required',
      message: 'Agent Sites API requires a registered agent token with scope.',
      requiredScope: 'sites:version:save',
    })
  })

  test('requires idempotency keys for mutating agent Site actions', async () => {
    const response = await routeRequest(
      makeRequest('/api/agent/sites', {
        body: { title: 'Customer Site' },
      }),
      { session },
    )

    expect(response.status).toBe(400)
    expect(response.headers.get('x-test-session-user-id')).toBe(
      'user_agent_operator',
    )
    await expect(response.json()).resolves.toEqual({
      error: 'idempotency_key_required',
      message: 'Mutating agent Site actions require Idempotency-Key.',
    })
  })

  test('returns scoped execution receipts for each agent Site action', async () => {
    const store = new BuilderApiStore()
    seedSiteProject(store, { id: 'site_123' })
    const db = builderApiDb(store)
    const cases = [
      {
        action: 'project_create',
        body: { prompt: 'Build a public order page.', title: 'Order Page' },
        implementationState: 'operator_review_required',
        path: '/api/agent/sites',
        receiptStatus: 'operator_review_required',
        scope: 'sites:project:create',
        status: 202,
      },
      {
        action: 'builder_session_open',
        body: { goal: 'Start from the approved customer brief.' },
        implementationState: 'builder_session_created',
        path: '/api/agent/sites/site_123/builder-sessions',
        receiptStatus: 'created',
        scope: 'sites:builder-session:create',
        status: 201,
      },
      {
        action: 'preview_request',
        body: { description: 'Preview candidate shell.' },
        implementationState: 'preview_queued',
        path: '/api/agent/sites/site_123/previews',
        receiptStatus: 'queued',
        scope: 'sites:preview:request',
        status: 202,
      },
      {
        action: 'save_version',
        body: { notes: 'Candidate version for operator review.' },
        implementationState: 'operator_review_required',
        path: '/api/agent/sites/site_123/versions',
        receiptStatus: 'operator_review_required',
        scope: 'sites:version:save',
        status: 202,
      },
      {
        action: 'deploy_version_request',
        body: { notes: 'Request deploy after owner approval.' },
        implementationState: 'deploy_review_requested',
        path: '/api/agent/sites/site_123/deploy-requests',
        receiptStatus: 'queued',
        scope: 'sites:deploy:request',
        status: 202,
      },
    ] as const

    for (const item of cases) {
      const response = await routeRequest(
        makeRequest(item.path, {
          body: item.body,
          idempotencyKey: `agent-sites-${item.action}`,
        }),
        { db, session },
      )
      const payload = await response.json()

      expect(response.status).toBe(item.status)
      expect(payload).toMatchObject({
        agentSites: {
          action: item.action,
          authority: {
            deploy: 'request_only',
            saveVersion: 'available_with_builder_session_and_artifact_manifest',
          },
          implementationState: item.implementationState,
          projection: {
            deployWillRun: false,
            previewWillRun: item.action === 'preview_request',
            projectWillBeCreated: false,
            versionWillBeSaved: false,
          },
          receipt: {
            actorUserId: 'user_agent_operator',
            idempotencyKey: `agent-sites-${item.action}`,
            ownerUserId: 'user_agent_operator',
            requiredScope: item.scope,
            scopeSatisfiedBy: 'browser_session_plus_internal_preview_gate',
            status: item.receiptStatus,
          },
          requiredScope: item.scope,
        },
      })
    }
  })

  test('keeps deploy requests separate from deploy authority', async () => {
    const store = new BuilderApiStore()
    seedSiteProject(store, { id: 'site_123' })
    const response = await routeRequest(
      makeRequest('/api/agent/sites/site_123/deploy-requests', {
        body: { versionId: 'sitever_123' },
        idempotencyKey: 'agent-sites-deploy-request',
      }),
      { db: builderApiDb(store), session },
    )
    const payload = await response.json()

    expect(isRecord(payload)).toBe(true)
    const agentSites = isRecord(payload) ? payload.agentSites : undefined
    expect(isRecord(agentSites)).toBe(true)

    expect(response.status).toBe(202)
    if (isRecord(agentSites)) {
      expect(agentSites.requiredScope).toBe('sites:deploy:request')
      expect(isRecord(agentSites.authority)).toBe(true)
      expect(isRecord(agentSites.projection)).toBe(true)

      if (isRecord(agentSites.authority)) {
        expect(agentSites.authority.deploy).toBe('request_only')
      }

      if (isRecord(agentSites.projection)) {
        expect(agentSites.projection.deployWillRun).toBe(false)
      }

      expect(agentSites.implementationState).toBe('deploy_review_requested')
      expect(agentSites.deploymentAuthority).toBe('request_only')
    }
    expect(store.site_builder_events).toHaveLength(2)
    expect(store.site_events).toHaveLength(1)
  })

  test('creates real Site projects from scoped order-backed agent requests', async () => {
    const store = new BuilderApiStore()
    seedSoftwareOrder(store, {
      id: 'software_order_agent_site',
      request: 'Build a public product page for agents.',
      userId: 'user_customer',
    })
    const response = await routeRequest(
      makeRequest('/api/agent/sites', {
        body: {
          customerOrderId: 'software_order_agent_site',
          siteSlug: 'agent-product-page',
          title: 'Agent Product Page',
        },
        idempotencyKey: 'agent-sites-project-create-real',
      }),
      { db: builderApiDb(store), session },
    )
    const payload = await response.json()
    const agentSites = isRecord(payload) ? payload.agentSites : undefined

    expect(response.status).toBe(201)
    expect(store.site_projects).toHaveLength(1)
    expect(store.site_events).toHaveLength(1)
    expect(isRecord(agentSites)).toBe(true)
    if (isRecord(agentSites)) {
      expect(agentSites.implementationState).toBe('project_created')
      expect(agentSites.publicUrl).toBe(
        'https://sites.openagents.com/agent-product-page',
      )
    }
  })

  test('saves real Site versions when evidence gates are complete', async () => {
    const store = new BuilderApiStore()
    seedSiteProject(store, { id: 'site_123' })
    const db = builderApiDb(store)
    const created = await routeRequest(
      makeRequest('/api/agent/sites/site_123/builder-sessions', {
        body: { goal: 'Prepare the generated files for saving.' },
        idempotencyKey: 'agent-sites-save-session',
      }),
      { db, session },
    )
    const createdPayload = await created.json()
    const createdAgentSites = isRecord(createdPayload)
      ? createdPayload.agentSites
      : undefined
    const builderSession =
      isRecord(createdAgentSites) &&
      isRecord(createdAgentSites.siteBuilderSession)
        ? createdAgentSites.siteBuilderSession
        : {}
    const artifactObjects = new Map<string, unknown>()
    const response = await routeRequest(
      makeRequest('/api/agent/sites/site_123/versions', {
        body: {
          notes: 'Save the reviewed static build.',
          siteBuilderSessionId: String(builderSession.id),
          staticAssetsManifest: {
            assets: {
              'index.html': {
                contentType: 'text/html',
                r2Key: 'sites/site_123/builds/index.html',
              },
            },
          },
        },
        idempotencyKey: 'agent-sites-save-version-real',
      }),
      { artifacts: memoryArtifacts(artifactObjects), db, session },
    )
    const payload = await response.json()
    const agentSites = isRecord(payload) ? payload.agentSites : undefined

    expect(response.status).toBe(201)
    expect(store.site_versions).toHaveLength(1)
    expect(store.site_builder_saved_versions).toHaveLength(1)
    expect(
      Array.from(artifactObjects.keys()).some(key =>
        key.endsWith('/static-assets-manifest.json'),
      ),
    ).toBe(true)
    expect(isRecord(agentSites)).toBe(true)
    if (isRecord(agentSites)) {
      expect(agentSites.implementationState).toBe('version_saved')
      expect(agentSites.receipt).toMatchObject({ status: 'saved' })
    }
  })

  test('rejects bodies that do not match the typed action schema', async () => {
    const response = await routeRequest(
      makeRequest('/api/agent/sites', {
        body: { prompt: 123 },
        idempotencyKey: 'agent-sites-invalid-body',
      }),
      { session },
    )

    expect(response.status).toBe(400)
    await expect(response.json()).resolves.toEqual({
      error: 'invalid_request_body',
      message: 'Request body does not match the agent Site action schema.',
    })
  })

  test('rejects unsupported methods before accepting skeleton actions', async () => {
    const response = await routeRequest(
      makeRequest('/api/agent/sites', {
        idempotencyKey: 'agent-sites-get',
        method: 'GET',
      }),
      { session },
    )

    expect(response.status).toBe(405)
    expect(response.headers.get('allow')).toBe('POST')
  })

  test('creates and reconnects Site builder sessions', async () => {
    const store = new BuilderApiStore()
    const db = builderApiDb(store)
    const firstResponse = await routeRequest(
      makeRequest('/api/sites/builder-sessions', {
        body: {
          orderId: 'software_order_1',
          promptSummary: 'Build a customer-safe product page.',
          siteId: 'site_project_1',
        },
        gate: false,
        idempotencyKey: 'site-builder-session:order-1',
      }),
      { db, session },
    )
    const secondResponse = await routeRequest(
      makeRequest('/api/sites/builder-sessions', {
        body: {
          promptSummary: 'Different text should not replace the session.',
        },
        gate: false,
        idempotencyKey: 'site-builder-session:order-1',
      }),
      { db, session },
    )
    const first = await firstResponse.json()
    const second = await secondResponse.json()
    const firstSession =
      isRecord(first) && isRecord(first.siteBuilderSession)
        ? first.siteBuilderSession
        : {}
    const secondSession =
      isRecord(second) && isRecord(second.siteBuilderSession)
        ? second.siteBuilderSession
        : {}

    expect(firstResponse.status).toBe(201)
    expect(secondResponse.status).toBe(201)
    expect(firstSession.id).toBe(secondSession.id)
    expect(firstSession.promptSummary).toBe(
      'Build a customer-safe product page.',
    )
    expect(store.site_builder_sessions).toHaveLength(1)
  })

  test('does not leak private builder session existence to another user', async () => {
    const store = new BuilderApiStore()
    const db = builderApiDb(store)
    const ownerResponse = await routeRequest(
      makeRequest('/api/sites/builder-sessions', {
        body: { promptSummary: 'Build a customer-safe product page.' },
        gate: false,
        idempotencyKey: 'site-builder-session:order-1',
      }),
      { db, session },
    )
    const ownerPayload = await ownerResponse.json()
    const ownerSession =
      isRecord(ownerPayload) && isRecord(ownerPayload.siteBuilderSession)
        ? ownerPayload.siteBuilderSession
        : {}
    const otherUser = {
      user: {
        email: 'other@example.com',
        userId: 'user_other',
      },
    } satisfies TestSession
    const response = await routeRequest(
      makeRequest(`/api/sites/builder-sessions/${String(ownerSession.id)}`, {
        gate: false,
        method: 'GET',
      }),
      { db, session: otherUser },
    )

    await expect(response.json()).resolves.toEqual({
      error: 'site_builder_session_not_found',
    })
    expect(response.status).toBe(404)
  })

  test('appends customer feedback messages to builder sessions', async () => {
    const store = new BuilderApiStore()
    const db = builderApiDb(store)
    const created = await routeRequest(
      makeRequest('/api/sites/builder-sessions', {
        body: { promptSummary: 'Build a customer-safe product page.' },
        gate: false,
        idempotencyKey: 'site-builder-session:order-1',
      }),
      { db, session },
    )
    const createdPayload = await created.json()
    const createdSession =
      isRecord(createdPayload) && isRecord(createdPayload.siteBuilderSession)
        ? createdPayload.siteBuilderSession
        : {}
    const response = await routeRequest(
      makeRequest(
        `/api/sites/builder-sessions/${String(createdSession.id)}/messages`,
        {
          body: { body: 'Please make the hero copy clearer.' },
          gate: false,
          idempotencyKey: 'site-builder-message:1',
        },
      ),
      { db, session },
    )
    const payload = await response.json()
    const returnedSession =
      isRecord(payload) && isRecord(payload.siteBuilderSession)
        ? payload.siteBuilderSession
        : {}

    expect(response.status).toBe(201)
    expect(
      Array.isArray(returnedSession.messages)
        ? returnedSession.messages.length
        : 0,
    ).toBe(1)
  })

  test('restricts builder event append to admins', async () => {
    const store = new BuilderApiStore()
    const db = builderApiDb(store)
    const created = await routeRequest(
      makeRequest('/api/sites/builder-sessions', {
        body: { promptSummary: 'Build a customer-safe product page.' },
        gate: false,
        idempotencyKey: 'site-builder-session:order-1',
      }),
      { db, session },
    )
    const createdPayload = await created.json()
    const createdSession =
      isRecord(createdPayload) && isRecord(createdPayload.siteBuilderSession)
        ? createdPayload.siteBuilderSession
        : {}
    const nonAdmin = {
      user: {
        email: 'customer@example.com',
        userId: session.user.userId,
      },
    } satisfies TestSession
    const denied = await routeRequest(
      makeRequest(
        `/api/operator/sites/builder-sessions/${String(createdSession.id)}/events`,
        {
          body: {
            eventKind: 'phase_started',
            phaseKind: 'planning',
            summary: 'Planning started.',
            title: 'Planning',
          },
          gate: false,
          idempotencyKey: 'site-builder-event:denied',
        },
      ),
      { db, session: nonAdmin },
    )
    const accepted = await routeRequest(
      makeRequest(
        `/api/operator/sites/builder-sessions/${String(createdSession.id)}/events`,
        {
          body: {
            eventKind: 'phase_started',
            phaseKind: 'planning',
            summary: 'Planning started.',
            title: 'Planning',
          },
          gate: false,
          idempotencyKey: 'site-builder-event:accepted',
        },
      ),
      { db, session },
    )
    const payload = await accepted.json()
    const operatorSession =
      isRecord(payload) && isRecord(payload.operatorSiteBuilderSession)
        ? payload.operatorSiteBuilderSession
        : {}

    expect(denied.status).toBe(404)
    expect(accepted.status).toBe(201)
    expect(operatorSession.eventCount).toBe(1)
  })

  test('streams builder events with cursor replay and visibility filtering', async () => {
    const store = new BuilderApiStore()
    const db = builderApiDb(store)
    const created = await routeRequest(
      makeRequest('/api/sites/builder-sessions', {
        body: { promptSummary: 'Build a customer-safe product page.' },
        gate: false,
        idempotencyKey: 'site-builder-session:order-1',
      }),
      { db, session },
    )
    const createdPayload = await created.json()
    const createdSession =
      isRecord(createdPayload) && isRecord(createdPayload.siteBuilderSession)
        ? createdPayload.siteBuilderSession
        : {}

    await routeRequest(
      makeRequest(
        `/api/operator/sites/builder-sessions/${String(createdSession.id)}/events`,
        {
          body: {
            eventKind: 'phase_started',
            phaseKind: 'planning',
            summary: 'Planning started.',
            title: 'Planning',
            visibility: 'customer',
          },
          gate: false,
          idempotencyKey: 'site-builder-event:1',
        },
      ),
      { db, session },
    )
    await routeRequest(
      makeRequest(
        `/api/operator/sites/builder-sessions/${String(createdSession.id)}/events`,
        {
          body: {
            eventKind: 'phase_completed',
            phaseKind: 'planning',
            summary: 'Planning completed.',
            title: 'Planning complete',
            visibility: 'operator',
          },
          gate: false,
          idempotencyKey: 'site-builder-event:2',
        },
      ),
      { db, session },
    )

    const nonAdminOwner = {
      user: {
        email: 'customer@example.com',
        userId: session.user.userId,
      },
    } satisfies TestSession
    const customerStream = await routeRequest(
      makeRequest(
        `/api/sites/builder-sessions/${String(createdSession.id)}/events`,
        {
          gate: false,
          method: 'GET',
        },
      ),
      { db, session: nonAdminOwner },
    )
    const customerText = await customerStream.text()
    const operatorStream = await routeRequest(
      makeRequest(
        `/api/sites/builder-sessions/${String(createdSession.id)}/events?cursor=1`,
        {
          gate: false,
          method: 'GET',
        },
      ),
      { db, session },
    )
    const operatorText = await operatorStream.text()

    expect(customerStream.status).toBe(200)
    expect(customerStream.headers.get('content-type')).toContain(
      'text/event-stream',
    )
    expect(customerText).toContain('event: phase_started')
    expect(customerText).not.toContain('phase_completed')
    expect(operatorText).toContain('id: 2')
    expect(operatorText).toContain('event: phase_completed')
    expect(operatorText).not.toContain('id: 1')
  })

  test('lists and reads generated file snapshots with customer-safe visibility', async () => {
    const store = new BuilderApiStore()
    const db = builderApiDb(store)
    const created = await routeRequest(
      makeRequest('/api/sites/builder-sessions', {
        body: { promptSummary: 'Build a customer-safe product page.' },
        gate: false,
        idempotencyKey: 'site-builder-session:files',
      }),
      { db, session },
    )
    const createdPayload = await created.json()
    const createdSession =
      isRecord(createdPayload) && isRecord(createdPayload.siteBuilderSession)
        ? createdPayload.siteBuilderSession
        : {}
    const sessionId = String(createdSession.id)

    pushBuilderFileSnapshot(store, {
      path: 'src/App.tsx',
      previewText: 'export function App() { return <main>First</main> }',
      sequence: 1,
      sessionId,
    })
    pushBuilderFileSnapshot(store, {
      path: 'src/App.tsx',
      previewText: 'export function App() { return <main>Latest</main> }',
      sequence: 3,
      sessionId,
    })
    pushBuilderFileSnapshot(store, {
      path: 'src/private-build-log.txt',
      previewText: 'operator-only runner output',
      sequence: 2,
      sessionId,
      visibility: 'operator',
    })

    const nonAdminOwner = {
      user: {
        email: 'customer@example.com',
        userId: session.user.userId,
      },
    } satisfies TestSession
    const listResponse = await routeRequest(
      makeRequest(`/api/sites/builder-sessions/${sessionId}/files`, {
        gate: false,
        method: 'GET',
      }),
      { db, session: nonAdminOwner },
    )
    const readResponse = await routeRequest(
      makeRequest(
        `/api/sites/builder-sessions/${sessionId}/files/read?path=src%2FApp.tsx`,
        {
          gate: false,
          method: 'GET',
        },
      ),
      { db, session: nonAdminOwner },
    )
    const deniedResponse = await routeRequest(
      makeRequest(
        `/api/sites/builder-sessions/${sessionId}/files/read?path=src%2Fprivate-build-log.txt`,
        {
          gate: false,
          method: 'GET',
        },
      ),
      { db, session: nonAdminOwner },
    )
    const listPayload = await listResponse.json()
    const readPayload = await readResponse.json()
    const files =
      isRecord(listPayload) && Array.isArray(listPayload.files)
        ? listPayload.files
        : []
    const file =
      isRecord(readPayload) && isRecord(readPayload.file)
        ? readPayload.file
        : {}

    expect(listResponse.status).toBe(200)
    expect(files).toHaveLength(1)
    expect(isRecord(files[0]) ? files[0].path : undefined).toBe('src/App.tsx')
    expect(isRecord(files[0]) ? files[0].sequence : undefined).toBe(3)
    expect(readResponse.status).toBe(200)
    expect(file.previewText).toContain('Latest')
    expect(deniedResponse.status).toBe(404)
  })

  test('returns file tree and safe export manifest for generated snapshots', async () => {
    const store = new BuilderApiStore()
    const db = builderApiDb(store)
    const created = await routeRequest(
      makeRequest('/api/sites/builder-sessions', {
        body: { promptSummary: 'Build a customer-safe product page.' },
        gate: false,
        idempotencyKey: 'site-builder-session:file-tree',
      }),
      { db, session },
    )
    const createdPayload = await created.json()
    const createdSession =
      isRecord(createdPayload) && isRecord(createdPayload.siteBuilderSession)
        ? createdPayload.siteBuilderSession
        : {}
    const sessionId = String(createdSession.id)

    pushBuilderFileSnapshot(store, {
      path: 'src/App.tsx',
      previewText: 'export function App() { return <main /> }',
      sequence: 1,
      sessionId,
    })
    pushBuilderFileSnapshot(store, {
      path: 'README.md',
      previewText: '# Site',
      sequence: 2,
      sessionId,
    })

    const treeResponse = await routeRequest(
      makeRequest(`/api/sites/builder-sessions/${sessionId}/files/tree`, {
        gate: false,
        method: 'GET',
      }),
      { db, session },
    )
    const exportResponse = await routeRequest(
      makeRequest(`/api/sites/builder-sessions/${sessionId}/files/export`, {
        gate: false,
        method: 'GET',
      }),
      { db, session },
    )
    const treePayload = await treeResponse.json()
    const exportPayload = await exportResponse.json()
    const fileTree =
      isRecord(treePayload) && Array.isArray(treePayload.fileTree)
        ? treePayload.fileTree
        : []
    const exportFiles =
      isRecord(exportPayload) && Array.isArray(exportPayload.files)
        ? exportPayload.files
        : []

    expect(treeResponse.status).toBe(200)
    expect(fileTree).toHaveLength(2)
    expect(isRecord(fileTree[0]) ? fileTree[0].segments : undefined).toEqual([
      'README.md',
    ])
    expect(exportResponse.status).toBe(200)
    expect(isRecord(exportPayload) ? exportPayload.exportKind : undefined).toBe(
      'customer_safe_preview_manifest',
    )
    expect(
      isRecord(exportPayload)
        ? exportPayload.sourceArchiveAvailable
        : undefined,
    ).toBe(false)
    expect(exportFiles).toHaveLength(2)
    expect(
      isRecord(exportFiles[0]) ? exportFiles[0].previewText : undefined,
    ).toBe('# Site')
  })

  test('does not leak generated files to unrelated users', async () => {
    const store = new BuilderApiStore()
    const db = builderApiDb(store)
    const created = await routeRequest(
      makeRequest('/api/sites/builder-sessions', {
        body: { promptSummary: 'Build a customer-safe product page.' },
        gate: false,
        idempotencyKey: 'site-builder-session:file-leak',
      }),
      { db, session },
    )
    const createdPayload = await created.json()
    const createdSession =
      isRecord(createdPayload) && isRecord(createdPayload.siteBuilderSession)
        ? createdPayload.siteBuilderSession
        : {}
    const sessionId = String(createdSession.id)

    pushBuilderFileSnapshot(store, {
      path: 'src/App.tsx',
      previewText: 'export function App() { return <main /> }',
      sequence: 1,
      sessionId,
    })

    const otherUser = {
      user: {
        email: 'other@example.com',
        userId: 'user_other',
      },
    } satisfies TestSession
    const response = await routeRequest(
      makeRequest(`/api/sites/builder-sessions/${sessionId}/files`, {
        gate: false,
        method: 'GET',
      }),
      { db, session: otherUser },
    )

    await expect(response.json()).resolves.toEqual({
      error: 'site_builder_session_not_found',
    })
    expect(response.status).toBe(404)
  })
})
