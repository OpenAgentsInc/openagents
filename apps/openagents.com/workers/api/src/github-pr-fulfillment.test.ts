import { describe, expect, test } from 'vitest'

import {
  recordCustomerGrantPullRequestFulfillment,
  recordPublicForkPullRequestFulfillment,
} from './github-pr-fulfillment'

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

const publicForkInput = {
  approval: {
    approvedAt: '2026-06-05T12:00:00.000Z',
    source: 'customer_action' as const,
  },
  assignmentId: 'adjutant_assignment_1',
  pullRequest: {
    commitSha: 'abc123',
    forkFullName: 'OpenAgentsInc/customer-app',
    prNumber: 7,
    prUrl: 'https://github.com/customer/app/pull/7',
    sourceBranch: 'openagents/software-order-1',
    targetBranch: 'main',
    testsSummary: 'bun test passed',
  },
  repository: {
    fullName: 'customer/app',
    isPrivate: false,
  },
  softwareOrderId: 'software_order_1',
  summary: 'Opened a review-ready pull request for the requested change.',
  title: 'Review PR',
  userId: 'user_1',
}

const customerGrantInput = {
  ...publicForkInput,
  connection: {
    connectionRef: 'github-write_customer',
    hasSecretRef: true,
    health: 'healthy' as const,
    scopes: ['repo', 'workflow'],
    status: 'connected' as const,
  },
  grant: {
    connectionRef: 'github-write_customer',
    expiresAt: '2026-06-05T14:00:00.000Z',
    grantRef: 'github-write-grant_1',
    runnerSessionId: 'agent_run_1',
    status: 'issued' as const,
  },
  repository: {
    fullName: 'customer/private-app',
    isPrivate: true,
  },
}

describe('GitHub PR fulfillment', () => {
  test('records a public fork PR artifact without customer write grant', async () => {
    const db = new RecordingD1Database()
    const result = await recordPublicForkPullRequestFulfillment(
      db as unknown as D1Database,
      publicForkInput,
      {
        makeArtifactId: (() => {
          let index = 0

          return () => {
            index += 1

            return `fulfillment_artifact_${index}`
          }
        })(),
        nowIso: () => '2026-06-05T12:06:00.000Z',
      },
    )

    expect(result).toMatchObject({
      artifactId: 'fulfillment_artifact_1',
      gate: {
        decision: {
          authorityMode: 'openagents_fork',
          decision: 'allowed',
        },
      },
    })
    expect(db.runs).toHaveLength(3)
    expect(db.runs[0]?.query).toContain(
      'INSERT INTO order_github_write_authority_receipts',
    )
    expect(db.runs[1]?.query).toContain(
      'INSERT INTO order_fulfillment_artifacts',
    )
    expect(db.runs[1]?.values).toContain(
      'https://github.com/customer/app/pull/7',
    )
    expect(db.runs[1]?.values).toContain('openagents/software-order-1')
    expect(JSON.stringify(db.runs[1]?.values)).toContain('bun test passed')
    expect(db.runs[2]?.query).toContain('UPDATE software_orders')
    expect(JSON.stringify(db.runs)).not.toContain('gho_')
  })

  test('records a blocked artifact instead of a PR artifact without approval', async () => {
    const db = new RecordingD1Database()
    const result = await recordPublicForkPullRequestFulfillment(
      db as unknown as D1Database,
      {
        ...publicForkInput,
        approval: null,
      },
      {
        makeArtifactId: (() => {
          let index = 0

          return () => {
            index += 1

            return `fulfillment_artifact_${index}`
          }
        })(),
        nowIso: () => '2026-06-05T12:06:00.000Z',
      },
    )

    expect(result).toMatchObject({
      artifactId: null,
      gate: {
        blockedArtifactId: 'fulfillment_artifact_1',
        decision: {
          blockedReason: 'explicit_approval_required',
          decision: 'blocked',
        },
        orderStatus: 'needs_customer_input',
      },
    })
    expect(db.runs).toHaveLength(3)
    expect(db.runs[1]?.query).toContain('UPDATE software_orders')
    expect(db.runs[2]?.query).toContain(
      'INSERT INTO order_fulfillment_artifacts',
    )
    expect(db.runs[2]?.values).toContain('GitHub writeback needs approval')
    expect(db.runs[2]?.values).not.toContain(
      'https://github.com/customer/app/pull/7',
    )
  })

  test('records a private repo PR artifact only with a fresh customer grant', async () => {
    const db = new RecordingD1Database()
    const result = await recordCustomerGrantPullRequestFulfillment(
      db as unknown as D1Database,
      customerGrantInput,
      {
        makeArtifactId: (() => {
          let index = 0

          return () => {
            index += 1

            return `fulfillment_artifact_${index}`
          }
        })(),
        nowIso: () => '2026-06-05T12:06:00.000Z',
      },
    )

    expect(result).toMatchObject({
      artifactId: 'fulfillment_artifact_1',
      gate: {
        decision: {
          authorityMode: 'customer_grant',
          connectionRef: 'github-write_customer',
          decision: 'allowed',
          grantRef: 'github-write-grant_1',
        },
      },
    })
    expect(db.runs).toHaveLength(3)
    expect(db.runs[1]?.query).toContain(
      'INSERT INTO order_fulfillment_artifacts',
    )
    expect(db.runs[1]?.values).toContain('customer/private-app')
    expect(JSON.stringify(db.runs[1]?.values)).toContain('customer_grant')
    expect(db.runs[2]?.query).toContain('UPDATE software_orders')
  })

  test.each([
    {
      name: 'missing connection',
      override: {
        connection: null,
      },
      reason: 'github_write_connection_required',
    },
    {
      name: 'missing scope',
      override: {
        connection: {
          ...customerGrantInput.connection,
          scopes: ['repo'],
        },
      },
      reason: 'github_write_permission_missing',
    },
    {
      name: 'expired grant',
      override: {
        grant: {
          ...customerGrantInput.grant,
          expiresAt: '2026-06-05T12:00:00.000Z',
        },
      },
      reason: 'github_write_grant_expired',
    },
    {
      name: 'used grant',
      override: {
        grant: {
          ...customerGrantInput.grant,
          status: 'used' as const,
        },
      },
      reason: 'github_write_grant_not_issued',
    },
  ])(
    'blocks private repo PR fulfillment for $name',
    async ({ override, reason }) => {
      const db = new RecordingD1Database()
      const result = await recordCustomerGrantPullRequestFulfillment(
        db as unknown as D1Database,
        {
          ...customerGrantInput,
          ...override,
        },
        {
          makeArtifactId: (() => {
            let index = 0

            return () => {
              index += 1

              return `fulfillment_artifact_${index}`
            }
          })(),
          nowIso: () => '2026-06-05T12:06:00.000Z',
        },
      )

      expect(result).toMatchObject({
        artifactId: null,
        gate: {
          blockedArtifactId: 'fulfillment_artifact_1',
          decision: {
            blockedReason: reason,
            decision: 'blocked',
          },
          orderStatus: 'needs_customer_input',
        },
      })
      expect(db.runs).toHaveLength(3)
      expect(db.runs[1]?.query).toContain('UPDATE software_orders')
      expect(db.runs[2]?.query).toContain(
        'INSERT INTO order_fulfillment_artifacts',
      )
      expect(db.runs[2]?.values).not.toContain(
        'https://github.com/customer/app/pull/7',
      )
    },
  )
})
