import { describe, expect, test } from 'vitest'

import type { PushResponse } from '@openagentsinc/khala-sync'
import type { SyncSql } from '@openagentsinc/khala-sync-server'

import type {
  GitHubWriteConnectionRecord,
  GitHubWriteRepository,
} from '../github-write-connections'
import {
  KHALA_WRITEBACK_DISPATCH_CLIENT_GROUP_ID,
  KHALA_WRITEBACK_LANE,
  buildWritebackRuntimeEvent,
  decodeKhalaAgentComputerWritebackOutcome,
  publishKhalaAgentComputerWriteback,
  readWritebackTargetTurn,
  recordKhalaWritebackRuntimeEvent,
  resolveKhalaWritebackAuthorization,
  validateWritebackOutcomeShape,
  writebackPermissionReasonRef,
  type KhalaAgentComputerWritebackOutcome,
  type WritebackTargetTurn,
} from './khala-agent-computer-writeback'

// --------------------------------------------------------------------------
// Fakes: a tagged-template SQL that answers the bounded turn read by keyword,
// a recording executePush, and a minimal GitHub write repository.
// --------------------------------------------------------------------------

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
  clientId: string
  clientGroupId: string
  event: {
    kind: string
    status?: string
    branch?: string
    branchUrl?: string
    pullRequestUrl?: string
    pullRequestNumber?: number
    reasonRef?: string
    repositoryFullName?: string
    sequence?: number
    threadId?: string
    turnId?: string
    source?: { lane?: string }
    visibility?: string
  }
}

const makeRecordingExecutePush = (
  behavior: (event: RecordedEvent) => 'applied' | 'rejected' = () => 'applied',
) => {
  const recorded: Array<RecordedEvent> = []
  const executePush = (input: {
    readonly userId: string
    readonly request: {
      readonly clientId: string
      readonly clientGroupId: string
      readonly mutations: ReadonlyArray<{ mutationId: number; argsJson: string }>
    }
  }): Promise<PushResponse> => {
    const envelope = input.request.mutations[0]!
    const rec: RecordedEvent = {
      clientGroupId: input.request.clientGroupId,
      clientId: input.request.clientId,
      event: JSON.parse(envelope.argsJson),
      userId: input.userId,
    }
    recorded.push(rec)
    const status = behavior(rec)
    return Promise.resolve({
      lastMutationId: envelope.mutationId,
      protocolVersion: 1,
      results: [
        {
          mutationId: envelope.mutationId,
          status,
          ...(status === 'rejected' ? { errorCode: 'runtime_event_exists' } : {}),
        },
      ],
    } as unknown as PushResponse)
  }
  return { executePush: executePush as never, recorded }
}

const connection = (
  overrides: Partial<GitHubWriteConnectionRecord> = {},
): GitHubWriteConnectionRecord => ({
  connectedAt: '2026-07-06T00:00:00.000Z',
  connectionRef: 'ghw:conn1',
  createdAt: '2026-07-06T00:00:00.000Z',
  deletedAt: null,
  disconnectedAt: null,
  githubId: '14167547',
  githubLogin: 'octocat',
  health: 'healthy',
  id: 'conn1',
  lastStatusAt: '2026-07-06T00:00:00.000Z',
  metadataJson: null,
  scopes: ['repo', 'workflow'],
  secretRef: 'github-write://ghw:conn1',
  status: 'connected',
  updatedAt: '2026-07-06T00:00:00.000Z',
  userId: 'github:14167547',
  ...overrides,
})

const makeGitHubRepo = (
  usable: GitHubWriteConnectionRecord | undefined,
): GitHubWriteRepository =>
  ({
    findUsableConnectionForUser: () => Promise.resolve(usable),
  }) as unknown as GitHubWriteRepository

let uuidCounter = 0
const deterministicUuid = () => `uuid-${(uuidCounter += 1)}`

const baseTurn: WritebackTargetTurn = {
  eventCount: 3,
  ownerUserId: 'github:14167547',
  threadId: 'thread.t1',
  turnId: 'turn.t1',
}

const turnRow: TurnRow = {
  event_count: 3,
  owner_user_id: 'github:14167547',
  thread_id: 'thread.t1',
  turn_id: 'turn.t1',
}

const prOutcome: KhalaAgentComputerWritebackOutcome = {
  branch: 'agent/task-1',
  branchUrl: 'https://github.com/octocat/repo/tree/agent/task-1',
  changedFileCount: 4,
  pullRequestNumber: 42,
  pullRequestUrl: 'https://github.com/octocat/repo/pull/42',
  repositoryFullName: 'octocat/repo',
  status: 'pull_request_opened',
}

const branchOnlyOutcome: KhalaAgentComputerWritebackOutcome = {
  branch: 'agent/task-1',
  branchUrl: 'https://github.com/octocat/repo/tree/agent/task-1',
  changedFileCount: 2,
  repositoryFullName: 'octocat/repo',
  status: 'branch_pushed',
}

const baseDeps = (
  turns: ReadonlyArray<TurnRow>,
  push: ReturnType<typeof makeRecordingExecutePush>,
  usable: GitHubWriteConnectionRecord | undefined = connection(),
) => ({
  executePush: push.executePush,
  githubWriteRepository: makeGitHubRepo(usable),
  now: () => '2026-07-06T00:00:00.000Z',
  sql: makeFakeSql(turns),
  uuid: deterministicUuid,
})

// --------------------------------------------------------------------------

describe('resolveKhalaWritebackAuthorization', () => {
  test('authorized when a usable connection carries repo + workflow scopes', async () => {
    const decision = await resolveKhalaWritebackAuthorization(
      makeGitHubRepo(connection()),
      'github:14167547',
    )
    expect(decision).toEqual({
      authorized: true,
      connectionRef: 'ghw:conn1',
      scopes: ['repo', 'workflow'],
      source: 'github_write_connection',
    })
  })

  test('seam alignment: authorized via a usable brokerable IDENTITY when no write-connection row exists (#8477)', async () => {
    const decision = await resolveKhalaWritebackAuthorization(
      makeGitHubRepo(undefined),
      'github:300914913',
      { hasUsableIdentityAuthorization: () => Promise.resolve(true) },
    )
    expect(decision).toEqual({
      authorized: true,
      connectionRef: 'github-identity:github:300914913',
      scopes: ['repo', 'workflow'],
      source: 'github_identity',
    })
  })

  test('write-connection takes precedence over identity (no identity check needed)', async () => {
    let identityChecked = false
    const decision = await resolveKhalaWritebackAuthorization(
      makeGitHubRepo(connection()),
      'github:14167547',
      {
        hasUsableIdentityAuthorization: () => {
          identityChecked = true
          return Promise.resolve(true)
        },
      },
    )
    expect(decision.authorized).toBe(true)
    if (decision.authorized) expect(decision.source).toBe('github_write_connection')
    expect(identityChecked).toBe(false)
  })

  test('blocked when neither a write-connection NOR a usable identity exists', async () => {
    const decision = await resolveKhalaWritebackAuthorization(
      makeGitHubRepo(undefined),
      'github:300914913',
      { hasUsableIdentityAuthorization: () => Promise.resolve(false) },
    )
    expect(decision.authorized).toBe(false)
    if (!decision.authorized) {
      expect(decision.reason).toBe('github_write_connection_required')
    }
  })

  test('identity-authority errors fail closed (treated as not usable)', async () => {
    const decision = await resolveKhalaWritebackAuthorization(
      makeGitHubRepo(undefined),
      'github:300914913',
      { hasUsableIdentityAuthorization: () => Promise.reject(new Error('kv down')) },
    )
    expect(decision.authorized).toBe(false)
  })

  test('blocked (connection_required) when the user has no usable connection', async () => {
    const decision = await resolveKhalaWritebackAuthorization(
      makeGitHubRepo(undefined),
      'github:14167547',
    )
    expect(decision.authorized).toBe(false)
    if (!decision.authorized) {
      expect(decision.reason).toBe('github_write_connection_required')
      expect(decision.message.length).toBeGreaterThan(0)
    }
  })

  test('blocked (permission_missing) when scopes lack repo/workflow', async () => {
    const decision = await resolveKhalaWritebackAuthorization(
      makeGitHubRepo(connection({ scopes: ['read:user'] })),
      'github:14167547',
    )
    expect(decision.authorized).toBe(false)
    if (!decision.authorized) {
      expect(decision.reason).toBe('github_write_permission_missing')
    }
  })

  test('blocked (connection_unusable) when a secret ref is absent', async () => {
    const decision = await resolveKhalaWritebackAuthorization(
      makeGitHubRepo(connection({ secretRef: null })),
      'github:14167547',
    )
    expect(decision.authorized).toBe(false)
    if (!decision.authorized) {
      expect(decision.reason).toBe('github_write_connection_unusable')
    }
  })
})

describe('validateWritebackOutcomeShape', () => {
  test('accepts a well-formed pull-request outcome', () => {
    expect(validateWritebackOutcomeShape(prOutcome)).toBeUndefined()
  })

  test('accepts a well-formed branch-only outcome', () => {
    expect(validateWritebackOutcomeShape(branchOnlyOutcome)).toBeUndefined()
  })

  test('rejects a pull_request_opened outcome missing pullRequestUrl', () => {
    const error = validateWritebackOutcomeShape({
      ...prOutcome,
      pullRequestNumber: undefined,
      pullRequestUrl: undefined,
    })
    expect(error?.reason).toBe('writeback_outcome_shape_invalid')
  })

  test('rejects a branch_pushed outcome that carries PR fields', () => {
    const error = validateWritebackOutcomeShape({
      ...branchOnlyOutcome,
      pullRequestUrl: 'https://github.com/octocat/repo/pull/42',
    })
    expect(error?.reason).toBe('writeback_outcome_shape_invalid')
  })

  test('rejects a failed outcome without a reasonRef', () => {
    const error = validateWritebackOutcomeShape({
      branch: 'agent/task-1',
      branchUrl: 'https://github.com/octocat/repo/tree/agent/task-1',
      repositoryFullName: 'octocat/repo',
      status: 'failed',
    })
    expect(error?.reason).toBe('writeback_outcome_shape_invalid')
  })

  test('rejects a success outcome that carries a reasonRef', () => {
    const error = validateWritebackOutcomeShape({
      ...branchOnlyOutcome,
      reasonRef: 'writeback.permission.github_write_permission_missing',
    })
    expect(error?.reason).toBe('writeback_outcome_shape_invalid')
  })
})

describe('readWritebackTargetTurn', () => {
  test('maps the turn row with numeric event_count', async () => {
    const turn = await readWritebackTargetTurn(makeFakeSql([turnRow]), 'turn.t1')
    expect(turn).toEqual(baseTurn)
  })

  test('null when the turn does not exist', async () => {
    const turn = await readWritebackTargetTurn(makeFakeSql([]), 'turn.missing')
    expect(turn).toBeNull()
  })
})

describe('buildWritebackRuntimeEvent', () => {
  test('produces a private, thread-scoped writeback.recorded event', () => {
    uuidCounter = 0
    const event = buildWritebackRuntimeEvent(
      {
        executePush: (() => undefined) as never,
        now: () => '2026-07-06T00:00:00.000Z',
        registry: undefined as never,
        sql: undefined as never,
        uuid: deterministicUuid,
      },
      baseTurn,
      prOutcome,
    )
    expect(event.kind).toBe('writeback.recorded')
    expect(event.visibility).toBe('private')
    expect(event.threadId).toBe('thread.t1')
    expect(event.turnId).toBe('turn.t1')
    expect(event.sequence).toBe(3)
    expect(event.source.lane).toBe(KHALA_WRITEBACK_LANE)
    if (event.kind === 'writeback.recorded') {
      expect(event.status).toBe('pull_request_opened')
      expect(event.branchUrl).toBe(prOutcome.branchUrl)
      expect(event.pullRequestUrl).toBe(prOutcome.pullRequestUrl)
      expect(event.pullRequestNumber).toBe(42)
      expect(event.changedFileCount).toBe(4)
    }
  })

  test('omits PR fields for a branch-only outcome', () => {
    const event = buildWritebackRuntimeEvent(
      {
        executePush: (() => undefined) as never,
        now: () => '2026-07-06T00:00:00.000Z',
        registry: undefined as never,
        sql: undefined as never,
        uuid: deterministicUuid,
      },
      baseTurn,
      branchOnlyOutcome,
    )
    if (event.kind === 'writeback.recorded') {
      expect(event.status).toBe('branch_pushed')
      expect(event.pullRequestUrl).toBeUndefined()
      expect(event.pullRequestNumber).toBeUndefined()
    }
  })
})

describe('recordKhalaWritebackRuntimeEvent', () => {
  test('records the event as the turn owner via runtime.recordEvent', async () => {
    const push = makeRecordingExecutePush()
    const result = await recordKhalaWritebackRuntimeEvent(
      { executePush: push.executePush, now: () => 'now', sql: undefined as never, uuid: deterministicUuid },
      baseTurn,
      prOutcome,
    )
    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.status).toBe('pull_request_opened')
      expect(result.sequence).toBe(3)
      expect(result.threadId).toBe('thread.t1')
    }
    expect(push.recorded).toHaveLength(1)
    const rec = push.recorded[0]!
    expect(rec.userId).toBe('github:14167547')
    expect(rec.clientGroupId).toBe(KHALA_WRITEBACK_DISPATCH_CLIENT_GROUP_ID)
    expect(rec.clientId).toContain(KHALA_WRITEBACK_DISPATCH_CLIENT_GROUP_ID)
    expect(rec.event.kind).toBe('writeback.recorded')
    expect(rec.event.status).toBe('pull_request_opened')
  })

  test('a rejected record surfaces a typed record_rejected failure', async () => {
    const push = makeRecordingExecutePush(() => 'rejected')
    const result = await recordKhalaWritebackRuntimeEvent(
      { executePush: push.executePush, now: () => 'now', sql: undefined as never, uuid: deterministicUuid },
      baseTurn,
      prOutcome,
    )
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.reason).toBe('record_rejected')
      expect(result.detail).toBe('runtime_event_exists')
    }
  })
})

describe('publishKhalaAgentComputerWriteback', () => {
  test('happy path: authorized user, PR outcome recorded to the thread', async () => {
    const push = makeRecordingExecutePush()
    const result = await publishKhalaAgentComputerWriteback(
      baseDeps([turnRow], push),
      { outcome: prOutcome, turnId: 'turn.t1', userId: 'github:14167547' },
    )
    expect(result).toMatchObject({
      decision: 'recorded',
      ok: true,
      status: 'pull_request_opened',
      threadId: 'thread.t1',
    })
    expect(push.recorded).toHaveLength(1)
    expect(push.recorded[0]!.event.status).toBe('pull_request_opened')
  })

  test('permission-blocked: unauthorized success records a typed failed event', async () => {
    const push = makeRecordingExecutePush()
    const result = await publishKhalaAgentComputerWriteback(
      baseDeps([turnRow], push, connection({ scopes: ['read:user'] })),
      { outcome: prOutcome, turnId: 'turn.t1', userId: 'github:14167547' },
    )
    expect(result.ok).toBe(false)
    if (!result.ok && result.decision === 'permission_blocked') {
      expect(result.reason).toBe('github_write_permission_missing')
      expect(result.recordedEventId).not.toBeNull()
    }
    // A single FAILED event was surfaced to the thread — never a success.
    expect(push.recorded).toHaveLength(1)
    const rec = push.recorded[0]!
    expect(rec.event.status).toBe('failed')
    expect(rec.event.reasonRef).toBe(
      writebackPermissionReasonRef('github_write_permission_missing'),
    )
    expect(rec.event.pullRequestUrl).toBeUndefined()
  })

  test('no usable connection blocks with connection_required', async () => {
    const push = makeRecordingExecutePush()
    const result = await publishKhalaAgentComputerWriteback(
      { ...baseDeps([turnRow], push), githubWriteRepository: makeGitHubRepo(undefined) },
      { outcome: branchOnlyOutcome, turnId: 'turn.t1', userId: 'github:14167547' },
    )
    expect(result.ok).toBe(false)
    if (!result.ok && result.decision === 'permission_blocked') {
      expect(result.reason).toBe('github_write_connection_required')
    }
    expect(push.recorded[0]!.event.status).toBe('failed')
  })

  test('seam alignment: a real pushed branch is RECORDED (not failed) when the user is authorized via IDENTITY only (#8477)', async () => {
    const push = makeRecordingExecutePush()
    const result = await publishKhalaAgentComputerWriteback(
      {
        ...baseDeps([turnRow], push),
        // No explicit write-connection row for this user...
        githubWriteRepository: makeGitHubRepo(undefined),
        // ...but a usable brokerable identity (the push's real credential).
        identityWriteAuthority: {
          hasUsableIdentityAuthorization: () => Promise.resolve(true),
        },
      },
      { outcome: prOutcome, turnId: 'turn.t1', userId: 'github:14167547' },
    )
    expect(result).toMatchObject({
      decision: 'recorded',
      ok: true,
      status: 'pull_request_opened',
    })
    // The success (with its tappable PR link) is recorded — the seam bug where a
    // real branch was recorded as `failed` is fixed.
    expect(push.recorded).toHaveLength(1)
    expect(push.recorded[0]!.event.status).toBe('pull_request_opened')
    expect(push.recorded[0]!.event.pullRequestUrl).toBe(prOutcome.pullRequestUrl)
  })

  test('a failed outcome skips the authorization gate and is recorded as reported', async () => {
    const push = makeRecordingExecutePush()
    const failedOutcome: KhalaAgentComputerWritebackOutcome = {
      branch: 'agent/task-1',
      branchUrl: 'https://github.com/octocat/repo/tree/agent/task-1',
      reasonRef: 'pull_request.branch_update_rejected',
      repositoryFullName: 'octocat/repo',
      status: 'failed',
    }
    // No usable connection at all — but a failed outcome must still surface.
    const result = await publishKhalaAgentComputerWriteback(
      baseDeps([turnRow], push, undefined),
      { outcome: failedOutcome, turnId: 'turn.t1', userId: 'github:14167547' },
    )
    expect(result).toMatchObject({ decision: 'recorded', ok: true, status: 'failed' })
    expect(push.recorded[0]!.event.reasonRef).toBe(
      'pull_request.branch_update_rejected',
    )
  })

  test('unknown turn is a typed turn_not_found with no event recorded', async () => {
    const push = makeRecordingExecutePush()
    const result = await publishKhalaAgentComputerWriteback(
      baseDeps([], push),
      { outcome: prOutcome, turnId: 'turn.missing', userId: 'github:14167547' },
    )
    expect(result).toMatchObject({ decision: 'turn_not_found', ok: false })
    expect(push.recorded).toHaveLength(0)
  })

  test('owner mismatch is rejected and records nothing', async () => {
    const push = makeRecordingExecutePush()
    const result = await publishKhalaAgentComputerWriteback(
      baseDeps([turnRow], push),
      { outcome: prOutcome, turnId: 'turn.t1', userId: 'github:99999999' },
    )
    expect(result).toMatchObject({ decision: 'owner_mismatch', ok: false })
    expect(push.recorded).toHaveLength(0)
  })

  test('an invalid outcome shape is rejected before any turn read', async () => {
    const push = makeRecordingExecutePush()
    const result = await publishKhalaAgentComputerWriteback(
      baseDeps([turnRow], push),
      {
        outcome: { ...branchOnlyOutcome, pullRequestUrl: 'https://x/pull/1' },
        turnId: 'turn.t1',
        userId: 'github:14167547',
      },
    )
    expect(result).toMatchObject({ decision: 'outcome_invalid', ok: false })
    expect(push.recorded).toHaveLength(0)
  })
})

describe('decodeKhalaAgentComputerWritebackOutcome', () => {
  test('decodes a public-safe outcome payload', () => {
    const outcome = decodeKhalaAgentComputerWritebackOutcome({
      branch: 'agent/task-1',
      branchUrl: 'https://github.com/octocat/repo/tree/agent/task-1',
      changedFileCount: 2,
      repositoryFullName: 'octocat/repo',
      status: 'branch_pushed',
    })
    expect(outcome.status).toBe('branch_pushed')
    expect(outcome.repositoryFullName).toBe('octocat/repo')
  })

  test('rejects an unknown status literal', () => {
    expect(() =>
      decodeKhalaAgentComputerWritebackOutcome({
        branch: 'b',
        branchUrl: 'u',
        repositoryFullName: 'octocat/repo',
        status: 'merged',
      }),
    ).toThrow()
  })
})
