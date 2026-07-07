import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type { PushResponse } from '@openagentsinc/khala-sync'
import {
  makeMutatorRegistry,
  runtimeMutators,
  type MutatorRegistry,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import type {
  AgentCredentialLookup,
  AgentRegistrationRecord,
  AgentRegistrationStore,
} from '../agent-registration'
import { sha256Hex } from '../agent-registration'
import type {
  GitHubWriteConnectionRecord,
  GitHubWriteRepository,
} from '../github-write-connections'
import type { KhalaSyncHyperdriveBinding } from '../khala-sync-push-routes'
import { publishKhalaAgentComputerWriteback } from './khala-agent-computer-writeback'
import {
  KHALA_AGENT_COMPUTER_WRITEBACK_INGEST_PATH,
  KHALA_AGENT_COMPUTER_WRITEBACK_SCHEMA_VERSION,
  makeKhalaAgentComputerWritebackRoutes,
  type KhalaAgentComputerWritebackDependencies,
} from './khala-agent-computer-writeback-routes'

// --------------------------------------------------------------------------
// Fakes
// --------------------------------------------------------------------------

const nowIso = '2026-07-06T12:00:00.000Z'
const agentToken = 'oa_agent_khala_agent_computer_writeback_test'
const ownerUserId = 'github:14167547'

type TestEnv = { readonly tag: 'test-env' }
const env: TestEnv = { tag: 'test-env' }

class MemoryAgentStore implements AgentRegistrationStore {
  constructor(
    private readonly tokenHash: string,
    private readonly openauthUserId: string | null = null,
  ) {}

  createAgentRegistration(_record: AgentRegistrationRecord): Promise<void> {
    return Promise.resolve()
  }

  findAgentByTokenHash(
    tokenHash: string,
    _now: string,
  ): Promise<AgentCredentialLookup | undefined> {
    if (tokenHash !== this.tokenHash) return Promise.resolve(undefined)
    return Promise.resolve({
      credentialId: 'credential-agent-computer-writeback-1',
      openauthUserId: this.openauthUserId,
      profileMetadataJson: '{}',
      tokenPrefix: 'oa_agent_khal',
      user: {
        avatarUrl: null,
        createdAt: nowIso,
        displayName: 'Agent Computer Executor',
        id: 'agent-computer-executor-1',
        kind: 'agent',
        primaryEmail: null,
        status: 'active',
        updatedAt: nowIso,
      },
    })
  }

  touchAgentCredential(): Promise<void> {
    return Promise.resolve()
  }

  updateAgentDisplayName(): Promise<number> {
    return Promise.resolve(0)
  }
}

type TurnRow = {
  turn_id: string
  thread_id: string
  owner_user_id: string
  event_count: number
}

const makeFakeSql = (turns: ReadonlyArray<TurnRow>): SyncSql => {
  const sql = (strings: TemplateStringsArray, ...values: Array<unknown>) => {
    const text = strings.join(' ')
    if (text.includes('FROM khala_sync_runtime_turns')) {
      const turnId = values[0] as string
      return Promise.resolve(turns.filter(row => row.turn_id === turnId))
    }
    throw new Error(`unexpected SQL in fake: ${text}`)
  }
  return sql as unknown as SyncSql
}

type RecordedEvent = {
  userId: string
  event: { kind: string; status?: string; reasonRef?: string }
}

const makeRecordingExecutePush = () => {
  const recorded: Array<RecordedEvent> = []
  const executePush = (input: {
    readonly userId: string
    readonly request: {
      readonly mutations: ReadonlyArray<{
        mutationId: number
        argsJson: string
      }>
    }
  }): Promise<PushResponse> => {
    const envelope = input.request.mutations[0]!
    recorded.push({ event: JSON.parse(envelope.argsJson), userId: input.userId })
    return Promise.resolve({
      lastMutationId: envelope.mutationId,
      protocolVersion: 1,
      results: [{ mutationId: envelope.mutationId, status: 'applied' }],
    } as unknown as PushResponse)
  }
  return { executePush: executePush as never, recorded }
}

const usableConnection = (): GitHubWriteConnectionRecord => ({
  connectedAt: nowIso,
  connectionRef: 'ghw:conn1',
  createdAt: nowIso,
  deletedAt: null,
  disconnectedAt: null,
  githubId: '14167547',
  githubLogin: 'octocat',
  health: 'healthy',
  id: 'conn1',
  lastStatusAt: nowIso,
  metadataJson: null,
  scopes: ['repo', 'workflow'],
  secretRef: 'github-write://ghw:conn1',
  status: 'connected',
  updatedAt: nowIso,
  userId: ownerUserId,
})

const makeGitHubRepo = (
  usable: GitHubWriteConnectionRecord | undefined,
): GitHubWriteRepository =>
  ({
    findUsableConnectionForUser: () => Promise.resolve(usable),
  }) as unknown as GitHubWriteRepository

const registry: MutatorRegistry = makeMutatorRegistry([...runtimeMutators])

const binding: KhalaSyncHyperdriveBinding = {
  connectionString: 'postgres://fake/khala',
}

const makeDeps = (
  overrides: Partial<KhalaAgentComputerWritebackDependencies<TestEnv>> = {},
  options: Readonly<{
    tokenHash: string
    openauthUserId?: string | null
    turns?: ReadonlyArray<TurnRow>
    connection?: GitHubWriteConnectionRecord | undefined
    executePush?: never
  }>,
): KhalaAgentComputerWritebackDependencies<TestEnv> => {
  const turns = options.turns ?? [
    {
      event_count: 3,
      owner_user_id: ownerUserId,
      thread_id: 'thread-1',
      turn_id: 'turn-1',
    },
  ]
  return {
    agentStore: () =>
      new MemoryAgentStore(options.tokenHash, options.openauthUserId ?? null),
    binding: () => binding,
    githubWriteRepository: () =>
      makeGitHubRepo(
        options.connection === undefined && !('connection' in options)
          ? usableConnection()
          : options.connection,
      ),
    makeSqlClient: () =>
      Promise.resolve({ end: () => Promise.resolve(), sql: makeFakeSql(turns) }),
    // Thread a fake executePush into the real recorder so we exercise the true
    // route -> publishKhalaAgentComputerWriteback path without a database.
    publish: (deps, input) =>
      publishKhalaAgentComputerWriteback(
        { ...deps, executePush: options.executePush ?? undefined },
        input,
      ),
    registry,
    ...overrides,
  }
}

const request = (
  body: unknown,
  init: Readonly<{ method?: string; token?: string | null }> = {},
): Request => {
  const headers = new Headers({ 'content-type': 'application/json' })
  if (init.token !== null) {
    headers.set('authorization', `Bearer ${init.token ?? agentToken}`)
  }
  return new Request(
    `https://openagents.com${KHALA_AGENT_COMPUTER_WRITEBACK_INGEST_PATH}`,
    {
      ...(body === undefined ? {} : { body: JSON.stringify(body) }),
      headers,
      method: init.method ?? 'POST',
    },
  )
}

const validBody = (
  outcome: Record<string, unknown> = {
    branch: 'pylon/assignment-1',
    branchUrl: 'https://github.com/octocat/repo/tree/pylon/assignment-1',
    changedFileCount: 2,
    pullRequestNumber: 7,
    pullRequestUrl: 'https://github.com/octocat/repo/pull/7',
    repositoryFullName: 'octocat/repo',
    status: 'pull_request_opened',
  },
) => ({
  outcome,
  ownerUserId,
  schemaVersion: KHALA_AGENT_COMPUTER_WRITEBACK_SCHEMA_VERSION,
  turnId: 'turn-1',
})

const run = async (
  deps: KhalaAgentComputerWritebackDependencies<TestEnv>,
  req: Request,
): Promise<Response> => {
  const routes = makeKhalaAgentComputerWritebackRoutes(deps)
  return Effect.runPromise(
    routes.handleKhalaAgentComputerWritebackIngestApi(req, env),
  )
}

// --------------------------------------------------------------------------
// Tests
// --------------------------------------------------------------------------

describe('khala agent computer writeback route', () => {
  test('records a pull-request writeback under the turn owner', async () => {
    const { executePush, recorded } = makeRecordingExecutePush()
    const tokenHash = await sha256Hex(agentToken)
    const deps = makeDeps({}, { executePush: executePush as never, tokenHash })
    const response = await run(deps, request(validBody()))
    const json = (await response.json()) as Record<string, unknown>
    expect(response.status).toBe(200)
    expect(json.ok).toBe(true)
    expect(json.decision).toBe('recorded')
    expect(json.status).toBe('pull_request_opened')
    expect(json.threadId).toBe('thread-1')
    expect(recorded).toHaveLength(1)
    expect(recorded[0]!.userId).toBe(ownerUserId)
    expect(recorded[0]!.event.kind).toBe('writeback.recorded')
    expect(recorded[0]!.event.status).toBe('pull_request_opened')
  })

  test('missing bearer token is unauthorized', async () => {
    const tokenHash = await sha256Hex(agentToken)
    const deps = makeDeps({}, { tokenHash })
    const response = await run(deps, request(validBody(), { token: null }))
    expect(response.status).toBe(401)
  })

  test('non-POST is method not allowed', async () => {
    const tokenHash = await sha256Hex(agentToken)
    const deps = makeDeps({}, { tokenHash })
    const response = await run(
      deps,
      request(undefined, { method: 'GET' }),
    )
    expect(response.status).toBe(405)
  })

  test('a linked agent cannot post for a different owner', async () => {
    const tokenHash = await sha256Hex(agentToken)
    const deps = makeDeps(
      {},
      { openauthUserId: 'github:99999999', tokenHash },
    )
    const response = await run(deps, request(validBody()))
    const json = (await response.json()) as Record<string, unknown>
    expect(response.status).toBe(403)
    expect(json.error).toBe('khala_agent_computer_writeback_forbidden')
  })

  test('an invalid outcome shape is a 400 validation error', async () => {
    const tokenHash = await sha256Hex(agentToken)
    const deps = makeDeps({}, { tokenHash })
    // pull_request_opened without a PR url is rejected by the outcome schema/gate.
    const response = await run(
      deps,
      request(
        validBody({
          branch: 'pylon/assignment-1',
          branchUrl: 'https://github.com/octocat/repo/tree/pylon/assignment-1',
          repositoryFullName: 'octocat/repo',
          status: 'pull_request_opened',
        }),
      ),
    )
    const json = (await response.json()) as Record<string, unknown>
    expect(response.status).toBe(400)
    expect(json.decision).toBe('outcome_invalid')
  })

  test('a missing GitHub write connection records an honest failed event', async () => {
    const { executePush, recorded } = makeRecordingExecutePush()
    const tokenHash = await sha256Hex(agentToken)
    const deps = makeDeps(
      {},
      { connection: undefined, executePush: executePush as never, tokenHash },
    )
    const response = await run(deps, request(validBody()))
    const json = (await response.json()) as Record<string, unknown>
    expect(response.status).toBe(200)
    expect(json.ok).toBe(false)
    expect(json.decision).toBe('permission_blocked')
    expect(json.reason).toBe('github_write_connection_required')
    // The blocked success is recorded as an honest failed writeback event.
    expect(recorded).toHaveLength(1)
    expect(recorded[0]!.event.status).toBe('failed')
    expect(recorded[0]!.event.reasonRef).toBe(
      'writeback.permission.github_write_connection_required',
    )
  })

  test('an unknown turn is a 404', async () => {
    const tokenHash = await sha256Hex(agentToken)
    const deps = makeDeps({}, { tokenHash, turns: [] })
    const response = await run(deps, request(validBody()))
    const json = (await response.json()) as Record<string, unknown>
    expect(response.status).toBe(404)
    expect(json.decision).toBe('turn_not_found')
  })

  test('an unconfigured KHALA_SYNC_DB binding is a 503', async () => {
    const tokenHash = await sha256Hex(agentToken)
    const deps = makeDeps({ binding: () => undefined }, { tokenHash })
    const response = await run(deps, request(validBody()))
    expect(response.status).toBe(503)
  })
})
