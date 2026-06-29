import { describe, expect, test } from 'vitest'

import {
  type GitHubWritebackAuthorityRequest,
  gitHubWritebackArtifactMetadataJson,
  makeGitHubWritebackAuthorityReceipt,
  recordGitHubWritebackExecutorGate,
  resolveGitHubWritebackAuthority,
} from './github-writeback-authority'

class RecordingD1Statement {
  readonly bound: Array<unknown> = []

  constructor(
    private readonly db: RecordingD1Database,
    readonly query: string,
  ) {}

  bind(...values: Array<unknown>): RecordingD1Statement {
    this.bound.push(...values)

    return this
  }

  run(): Promise<void> {
    this.db.runs.push({
      query: this.query,
      values: this.bound,
    })

    return Promise.resolve()
  }
}

class RecordingD1Database {
  readonly runs: Array<Readonly<{ query: string; values: Array<unknown> }>> = []

  prepare(query: string): RecordingD1Statement {
    return new RecordingD1Statement(this, query)
  }
}

const baseRequest: GitHubWritebackAuthorityRequest = {
  approval: {
    approvedAt: '2026-06-05T12:00:00.000Z',
    source: 'customer_action',
  },
  assignmentId: 'adjutant_assignment_1',
  connection: null,
  grant: null,
  operation: 'open_fork_pull_request',
  repository: {
    fullName: 'customer/app',
    isPrivate: false,
  },
  softwareOrderId: 'software_order_1',
  userId: 'user_1',
}

describe('GitHub writeback authority', () => {
  test('blocks any external write before explicit approval', () => {
    const decision = resolveGitHubWritebackAuthority(
      {
        ...baseRequest,
        approval: null,
      },
      '2026-06-05T12:05:00.000Z',
    )

    expect(decision).toMatchObject({
      blockedReason: 'explicit_approval_required',
      decision: 'blocked',
    })
  })

  test('allows public repository fork PRs after approval without customer write grant', () => {
    const decision = resolveGitHubWritebackAuthority(
      baseRequest,
      '2026-06-05T12:05:00.000Z',
    )

    expect(decision).toMatchObject({
      authorityMode: 'openagents_fork',
      connectionRef: null,
      decision: 'allowed',
      grantRef: null,
    })

    expect(gitHubWritebackArtifactMetadataJson(decision)).toContain(
      'openagents_fork',
    )
  })

  test('blocks private repository PRs until source access and grant are present', () => {
    const decision = resolveGitHubWritebackAuthority(
      {
        ...baseRequest,
        repository: {
          fullName: 'customer/private-app',
          isPrivate: true,
        },
      },
      '2026-06-05T12:05:00.000Z',
    )

    expect(decision).toMatchObject({
      blockedReason: 'github_write_connection_required',
      decision: 'blocked',
    })
  })

  test('allows private repository PRs through a fresh customer grant', () => {
    const decision = resolveGitHubWritebackAuthority(
      {
        ...baseRequest,
        connection: {
          connectionRef: 'github-write_customer',
          hasSecretRef: true,
          health: 'healthy',
          scopes: ['repo', 'workflow'],
          status: 'connected',
        },
        grant: {
          connectionRef: 'github-write_customer',
          expiresAt: '2026-06-05T14:00:00.000Z',
          grantRef: 'github-write-grant_1',
          runnerSessionId: 'agent_run_1',
          status: 'issued',
        },
        operation: 'open_pull_request',
        repository: {
          fullName: 'customer/private-app',
          isPrivate: true,
        },
      },
      '2026-06-05T12:05:00.000Z',
    )

    expect(decision).toMatchObject({
      authorityMode: 'customer_grant',
      connectionRef: 'github-write_customer',
      decision: 'allowed',
      grantRef: 'github-write-grant_1',
    })
  })

  test('blocks expired or already-used grants', () => {
    const request: GitHubWritebackAuthorityRequest = {
      ...baseRequest,
      connection: {
        connectionRef: 'github-write_customer',
        hasSecretRef: true,
        health: 'healthy',
        scopes: ['repo', 'workflow'],
        status: 'connected',
      },
      grant: {
        connectionRef: 'github-write_customer',
        expiresAt: '2026-06-05T12:00:00.000Z',
        grantRef: 'github-write-grant_1',
        runnerSessionId: 'agent_run_1',
        status: 'issued',
      },
      operation: 'open_pull_request',
      repository: {
        fullName: 'customer/private-app',
        isPrivate: true,
      },
    }

    const expired = resolveGitHubWritebackAuthority(
      request,
      '2026-06-05T12:05:00.000Z',
    )
    const used = resolveGitHubWritebackAuthority(
      {
        ...request,
        grant: {
          ...request.grant!,
          expiresAt: '2026-06-05T14:00:00.000Z',
          status: 'used',
        },
      },
      '2026-06-05T12:05:00.000Z',
    )

    expect(expired).toMatchObject({
      blockedReason: 'github_write_grant_expired',
      decision: 'blocked',
    })
    expect(used).toMatchObject({
      blockedReason: 'github_write_grant_not_issued',
      decision: 'blocked',
    })
  })

  test('creates a secret-safe durable authority receipt', () => {
    const decision = resolveGitHubWritebackAuthority(
      baseRequest,
      '2026-06-05T12:05:00.000Z',
    )
    const receipt = makeGitHubWritebackAuthorityReceipt(baseRequest, decision, {
      makeBlockedArtifactId: () => 'blocked_fulfillment_artifact_1',
      makeReceiptId: () => 'github_writeback_authority_receipt_1',
      nowIso: () => '2026-06-05T12:06:00.000Z',
    })

    expect(receipt).toMatchObject({
      authorityMode: 'openagents_fork',
      decision: 'allowed',
      grantRef: null,
      repositoryFullName: 'customer/app',
      repositoryPrivate: false,
      requestedOperation: 'open_fork_pull_request',
    })
    expect(receipt.metadataJson).toContain('openagents_fork')
    expect(receipt.metadataJson).not.toContain('gho_')
  })

  test('executor gate records receipt and customer-safe blocked artifact before writeback', async () => {
    const db = new RecordingD1Database()
    const result = await recordGitHubWritebackExecutorGate(
      db as unknown as D1Database,
      {
        authorityRequest: {
          ...baseRequest,
          approval: null,
        },
        recordBlockedArtifact: true,
      },
      {
        makeBlockedArtifactId: () => 'fulfillment_artifact_blocked_1',
        makeReceiptId: () => 'github_writeback_authority_receipt_1',
        nowIso: () => '2026-06-05T12:06:00.000Z',
      },
    )

    expect(result).toMatchObject({
      blockedArtifactId: 'fulfillment_artifact_blocked_1',
      decision: {
        blockedReason: 'explicit_approval_required',
        decision: 'blocked',
      },
      orderStatus: 'needs_customer_input',
      receipt: {
        decision: 'blocked',
        id: 'github_writeback_authority_receipt_1',
      },
    })
    expect(db.runs).toHaveLength(3)
    expect(db.runs[0]?.query).toContain(
      'INSERT INTO order_github_write_authority_receipts',
    )
    expect(db.runs[1]?.query).toContain('UPDATE software_orders')
    expect(db.runs[2]?.query).toContain(
      'INSERT INTO order_fulfillment_artifacts',
    )
    expect(JSON.stringify(db.runs)).not.toContain('gho_')
  })
})
