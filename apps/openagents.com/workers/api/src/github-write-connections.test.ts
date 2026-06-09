import { describe, expect, test } from 'vitest'
import { Effect } from 'effect'

import {
  GITHUB_WRITE_REQUIRED_SCOPES,
  GitHubWriteCallbackMismatch,
  GitHubWriteGrantExpired,
  GitHubWriteMissingConnection,
  GitHubWritePermissionFailure,
  type GitHubWriteAuthGrantRecord,
  type GitHubWriteConnectionAttemptRecord,
  type GitHubWriteConnectionRecord,
  type GitHubWriteRepository,
  githubWriteConnectionRef,
  githubWriteSecretKey,
  githubWriteSecretRef,
  hasRequiredGitHubWriteScopes,
  issueGitHubWriteGrant,
  makeGitHubWriteConnectionService,
  makeGitHubWriteRepositoryService,
  recordGitHubWriteConnectionConnected,
  requireGitHubWriteCallbackAccount,
  resolveGitHubWriteGrant,
  startGitHubWriteConnectionAttempt,
} from './github-write-connections'

class MemoryGitHubWriteRepository implements GitHubWriteRepository {
  readonly attempts: Array<GitHubWriteConnectionAttemptRecord> = []
  readonly connections: Array<GitHubWriteConnectionRecord> = []
  readonly grants: Array<GitHubWriteAuthGrantRecord> = []

  createAttempt(
    attempt: GitHubWriteConnectionAttemptRecord,
  ): Promise<GitHubWriteConnectionAttemptRecord> {
    this.attempts.push(attempt)

    return Promise.resolve(attempt)
  }

  findAttemptByState(
    state: string,
  ): Promise<GitHubWriteConnectionAttemptRecord | undefined> {
    return Promise.resolve(
      this.attempts.find(attempt => attempt.state === state),
    )
  }

  markAttemptFailed(
    attempt: GitHubWriteConnectionAttemptRecord,
    status: 'expired' | 'denied' | 'failed',
    reason: string,
    now: string,
  ): Promise<GitHubWriteConnectionAttemptRecord> {
    const updated = {
      ...attempt,
      failedAt: now,
      failureReason: reason,
      status,
      updatedAt: now,
    }
    const index = this.attempts.findIndex(
      candidate => candidate.id === attempt.id,
    )
    this.attempts.splice(index, 1, updated)

    return Promise.resolve(updated)
  }

  recordConnectedAttempt(
    input: Readonly<{
      attempt: GitHubWriteConnectionAttemptRecord
      connection: GitHubWriteConnectionRecord
    }>,
  ): Promise<GitHubWriteConnectionRecord> {
    const attemptIndex = this.attempts.findIndex(
      attempt => attempt.id === input.attempt.id,
    )
    this.attempts.splice(attemptIndex, 1, {
      ...input.attempt,
      completedAt: input.connection.updatedAt,
      status: 'connected',
      updatedAt: input.connection.updatedAt,
    })

    const connectionIndex = this.connections.findIndex(
      connection =>
        connection.userId === input.connection.userId &&
        connection.githubId === input.connection.githubId,
    )

    if (connectionIndex === -1) {
      this.connections.push(input.connection)

      return Promise.resolve(input.connection)
    }

    const updated = {
      ...this.connections[connectionIndex],
      ...input.connection,
      id: this.connections[connectionIndex]?.id ?? input.connection.id,
    }
    this.connections.splice(connectionIndex, 1, updated)

    return Promise.resolve(updated)
  }

  listConnectionsForUser(
    userId: string,
  ): Promise<ReadonlyArray<GitHubWriteConnectionRecord>> {
    return Promise.resolve(
      this.connections.filter(
        connection =>
          connection.userId === userId && connection.deletedAt === null,
      ),
    )
  }

  listPendingAttemptsForUser(
    userId: string,
  ): Promise<ReadonlyArray<GitHubWriteConnectionAttemptRecord>> {
    return Promise.resolve(
      this.attempts.filter(
        attempt => attempt.userId === userId && attempt.status === 'pending',
      ),
    )
  }

  disconnectConnection(): Promise<GitHubWriteConnectionRecord | undefined> {
    return Promise.resolve(undefined)
  }

  findUsableConnectionForUser(
    userId: string,
  ): Promise<GitHubWriteConnectionRecord | undefined> {
    return Promise.resolve(
      this.connections.find(
        connection =>
          connection.userId === userId &&
          connection.status === 'connected' &&
          connection.health === 'healthy' &&
          connection.secretRef !== null &&
          connection.deletedAt === null,
      ),
    )
  }

  createGrant(
    grant: GitHubWriteAuthGrantRecord,
  ): Promise<GitHubWriteAuthGrantRecord> {
    this.grants.push(grant)

    return Promise.resolve(grant)
  }

  findGrantByRef(
    grantRef: string,
  ): Promise<GitHubWriteAuthGrantRecord | undefined> {
    return Promise.resolve(
      this.grants.find(grant => grant.grantRef === grantRef),
    )
  }

  markGrantUsed(
    grant: GitHubWriteAuthGrantRecord,
  ): Promise<GitHubWriteAuthGrantRecord> {
    const index = this.grants.findIndex(candidate => candidate.id === grant.id)
    this.grants.splice(index, 1, grant)

    return Promise.resolve(grant)
  }
}

const sequentialIds = () => {
  const state = { index: 0 }

  return (prefix: string): string => {
    state.index += 1

    return `${prefix}_${state.index}`
  }
}

describe('GitHub write connections', () => {
  test('requires repo and workflow scopes for push-capable runs', () => {
    expect(hasRequiredGitHubWriteScopes(['repo'])).toBe(false)
    expect(hasRequiredGitHubWriteScopes(['repo', 'workflow'])).toBe(true)
  })

  test('creates a connection and resolves a one-time runner grant by ref', async () => {
    const repository = new MemoryGitHubWriteRepository()
    const now = () => new Date('2026-06-02T20:00:00.000Z')
    const attempt = await startGitHubWriteConnectionAttempt(
      repository,
      {
        expectedGithubId: '1',
        expectedGithubLogin: 'chris',
        scopes: GITHUB_WRITE_REQUIRED_SCOPES,
        userId: 'github:1',
      },
      { makeId: sequentialIds(), now },
    )
    const connectionRef = githubWriteConnectionRef('github_write_connection_1')
    const secretRef = githubWriteSecretRef(connectionRef)

    const connection = await recordGitHubWriteConnectionConnected(
      repository,
      {
        attempt,
        connectionRef,
        githubId: '1',
        githubLogin: 'chris',
        scopes: GITHUB_WRITE_REQUIRED_SCOPES,
        secretRef,
      },
      { makeId: sequentialIds(), now },
    )
    const grant = await issueGitHubWriteGrant(
      repository,
      {
        requestedAction: 'autopilot_mission',
        runnerSessionId: 'agent_run_1',
        userId: 'github:1',
      },
      { makeId: sequentialIds(), now },
    )

    expect(connection.connectionRef).toBe(connectionRef)
    expect(connection.hasSecretRef).toBe(true)
    expect(githubWriteSecretKey(connectionRef)).toBe(
      `github-write:token:${connectionRef}`,
    )
    expect(grant?.connectionRef).toBe(connectionRef)
    expect(JSON.stringify(grant)).not.toContain('gho_')

    const resolved = await resolveGitHubWriteGrant(
      repository,
      {
        grantRef: grant?.grantRef ?? '',
        runnerSessionId: 'agent_run_1',
      },
      { now },
    )

    expect(resolved).toMatchObject({
      connectionRef,
      githubLogin: 'chris',
      runnerSessionId: 'agent_run_1',
      secretRef,
      status: 'used',
    })
    expect(repository.grants[0]?.status).toBe('used')
  })

  test('callback account mismatch is a typed error', async () => {
    const repository = new MemoryGitHubWriteRepository()
    const attempt = await startGitHubWriteConnectionAttempt(repository, {
      expectedGithubId: '1',
      expectedGithubLogin: 'chris',
      scopes: GITHUB_WRITE_REQUIRED_SCOPES,
      userId: 'github:1',
    })

    expect(() =>
      requireGitHubWriteCallbackAccount(attempt, '2'),
    ).toThrow(GitHubWriteCallbackMismatch)
  })

  test('grant issue rejects missing scopes with a typed permission error', async () => {
    const repository = new MemoryGitHubWriteRepository()
    repository.connections.push({
      connectedAt: '2026-06-02T20:00:00.000Z',
      connectionRef: 'github-write_1',
      createdAt: '2026-06-02T20:00:00.000Z',
      deletedAt: null,
      disconnectedAt: null,
      githubId: '1',
      githubLogin: 'chris',
      health: 'healthy',
      id: 'github_write_connection_1',
      lastStatusAt: '2026-06-02T20:00:00.000Z',
      metadataJson: null,
      scopes: ['repo'],
      secretRef: 'github-write://github-write_1',
      status: 'connected',
      updatedAt: '2026-06-02T20:00:00.000Z',
      userId: 'github:1',
    })

    await expect(
      issueGitHubWriteGrant(repository, {
        userId: 'github:1',
      }),
    ).rejects.toThrow(GitHubWritePermissionFailure)
  })

  test('grant resolve rejects expired grants with a typed error', async () => {
    const repository = new MemoryGitHubWriteRepository()
    repository.grants.push({
      connectionId: 'github_write_connection_1',
      connectionRef: 'github-write_1',
      createdAt: '2026-06-02T20:00:00.000Z',
      expiresAt: '2026-06-02T20:01:00.000Z',
      failedAt: null,
      grantRef: 'github-write-grant_1',
      id: 'github_write_grant_1',
      metadataJson: null,
      requestedAction: null,
      revokedAt: null,
      runnerSessionId: 'agent_run_1',
      secretRef: 'github-write://github-write_1',
      status: 'issued',
      updatedAt: '2026-06-02T20:00:00.000Z',
      usedAt: null,
      userId: 'github:1',
    })

    await expect(
      resolveGitHubWriteGrant(
        repository,
        {
          grantRef: 'github-write-grant_1',
          runnerSessionId: 'agent_run_1',
        },
        {
          now: () => new Date('2026-06-02T20:02:00.000Z'),
        },
      ),
    ).rejects.toThrow(GitHubWriteGrantExpired)
  })

  test('Effect services expose repository and lifecycle operations', async () => {
    const repository = new MemoryGitHubWriteRepository()
    const repositoryService = makeGitHubWriteRepositoryService(repository)
    const lifecycleService = makeGitHubWriteConnectionService({
      repository,
      makeId: sequentialIds(),
      now: () => new Date('2026-06-02T20:00:00.000Z'),
    })
    const attempt = await Effect.runPromise(
      lifecycleService.startConnectionAttempt({
        expectedGithubId: '1',
        expectedGithubLogin: 'chris',
        scopes: GITHUB_WRITE_REQUIRED_SCOPES,
        userId: 'github:1',
      }),
    )
    const foundAttempt = await Effect.runPromise(
      repositoryService.findAttemptByState(attempt.state),
    )

    expect(foundAttempt?.id).toBe(attempt.id)
    await expect(
      Effect.runPromise(
        lifecycleService.issueGrant({
          userId: 'github:1',
        }),
      ),
    ).rejects.toThrow(GitHubWriteMissingConnection)
  })
})
