import { readFileSync } from 'node:fs'
import { DatabaseSync } from 'node:sqlite'

import { describe, expect, test } from 'vitest'

import {
  makeD1ForgeCoordinationStore,
  type ForgeCoordinationStore,
} from './forge-coordination-store'

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

  async batch(
    statements: ReadonlyArray<SqliteD1Statement>,
  ): Promise<Array<{ success: true }>> {
    this.db.exec('BEGIN')
    try {
      for (const statement of statements) {
        await statement.run()
      }
      this.db.exec('COMMIT')
    } catch (error) {
      this.db.exec('ROLLBACK')
      throw error
    }
    return statements.map(() => ({ success: true as const }))
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

const makeStore = (): ForgeCoordinationStore => {
  const db = new DatabaseSync(':memory:')
  db.exec('PRAGMA foreign_keys = ON')
  db.exec(coordinationMigration)
  db.exec(controlPlaneReceiptsMigration)
  return makeD1ForgeCoordinationStore(new SqliteD1(db) as unknown as D1Database)
}

const now = '2026-06-28T16:00:00.000Z'

describe('forge coordination D1 store', () => {
  test('persists issue, change, and NIP-34 status rows', async () => {
    const store = makeStore()
    const issue = await store.upsertIssue({
      tenantRef: 'tenant.openagents',
      issueRef: 'issue.forge.6746',
      githubIssueNumber: 6746,
      title: 'D1 coordination schema',
      state: 'open',
      priorityRef: 'prio:0-pr-burndown',
      sourceRefs: ['github:OpenAgentsInc/openagents#6746'],
      nowIso: now,
    })
    expect(issue.github_issue_number).toBe(6746)

    const change = await store.upsertChange({
      tenantRef: 'tenant.openagents',
      prRef: 'change.forge.6746',
      issueRef: issue.issue_ref,
      changeRef: 'change.forge.6746',
      state: 'ready',
      baseHead: '8e0c9b2eaf84c821caf555cae233a0d27e94d4ab',
      patchHead: '9e0c9b2eaf84c821caf555cae233a0d27e94d4ac',
      verificationRef: 'verification.forge.6746',
      blockerRefs: [],
      sourceRefs: ['github:OpenAgentsInc/openagents#6746'],
      nowIso: now,
    })
    expect(change.issue_ref).toBe(issue.issue_ref)
    expect(change.state).toBe('ready')

    const status = await store.recordStatus({
      tenantRef: 'tenant.openagents',
      statusRef: 'status.forge.6746.open',
      subjectRef: change.change_ref,
      state: 'open',
      actorRef: 'agent.public.forge',
      sourceRefs: ['github:OpenAgentsInc/openagents#6746'],
      createdAt: now,
    })
    expect(status.nip34_kind).toBe(1630)

    await expect(store.listIssues('tenant.openagents', 10)).resolves.toEqual([
      issue,
    ])
    await expect(
      store.listChanges('tenant.openagents', 10, issue.issue_ref),
    ).resolves.toEqual([change])
    await expect(
      store.listStatuses('tenant.openagents', 10, change.change_ref),
    ).resolves.toEqual([status])
  })

  test('admits only one active dispatch lease per work ref and reopens after expiry', async () => {
    const store = makeStore()
    await store.upsertIssue({
      tenantRef: 'tenant.openagents',
      issueRef: 'issue.forge.6746',
      githubIssueNumber: 6746,
      title: 'D1 coordination schema',
      state: 'open',
      sourceRefs: [],
      nowIso: now,
    })

    const first = await store.acquireDispatchLease({
      tenantRef: 'tenant.openagents',
      leaseRef: 'lease.forge.first',
      workRef: 'issue.forge.6746',
      ownerAgentRef: 'agent.public.first',
      acquiredAt: now,
      expiresAt: '2026-06-28T16:05:00.000Z',
      sourceRefs: [],
    })
    expect(first.acquired).toBe(true)

    const blocked = await store.acquireDispatchLease({
      tenantRef: 'tenant.openagents',
      leaseRef: 'lease.forge.second',
      workRef: 'issue.forge.6746',
      ownerAgentRef: 'agent.public.second',
      acquiredAt: '2026-06-28T16:01:00.000Z',
      expiresAt: '2026-06-28T16:10:00.000Z',
      sourceRefs: [],
    })
    expect(blocked.acquired).toBe(false)
    if (blocked.acquired) {
      throw new Error('second lease should not acquire while first lease is active')
    }
    expect(blocked.activeLease?.lease_ref).toBe('lease.forge.first')

    const reopened = await store.acquireDispatchLease({
      tenantRef: 'tenant.openagents',
      leaseRef: 'lease.forge.third',
      workRef: 'issue.forge.6746',
      ownerAgentRef: 'agent.public.third',
      acquiredAt: '2026-06-28T16:06:00.000Z',
      expiresAt: '2026-06-28T16:11:00.000Z',
      sourceRefs: [],
    })
    expect(reopened.acquired).toBe(true)
    if (!reopened.acquired) {
      throw new Error('expired lease should allow a new active lease')
    }
    expect(reopened.lease.lease_ref).toBe('lease.forge.third')
  })

  test('records the latest virtual merge queue ledger snapshot', async () => {
    const store = makeStore()
    await store.recordMergeQueueLedger({
      tenantRef: 'tenant.openagents',
      queueRef: 'queue.forge.first',
      baseHead: '8e0c9b2eaf84c821caf555cae233a0d27e94d4ab',
      actualHead: '8e0c9b2eaf84c821caf555cae233a0d27e94d4ab',
      virtualHead: '9e0c9b2eaf84c821caf555cae233a0d27e94d4ac',
      state: 'projected',
      ready: [{ changeRef: 'change.forge.6746' }],
      blocked: [],
      sourceRefs: ['github:OpenAgentsInc/openagents#6746'],
      nowIso: now,
    })
    const latest = await store.readLatestMergeQueueLedger('tenant.openagents')
    expect(latest?.queue_ref).toBe('queue.forge.first')
    await expect(store.listMergeQueueLedgers('tenant.openagents', 10)).resolves.toEqual([
      latest,
    ])
    expect(JSON.parse(latest?.ready_json ?? '[]')).toEqual([
      { changeRef: 'change.forge.6746' },
    ])
  })

  test('records redacted verification and promotion receipts through shared schemas', async () => {
    const store = makeStore()
    const verificationReceipt = await store.recordVerificationReceipt(
      {
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
        completed_at: '2026-06-28T16:02:00.000Z',
        artifact_refs: ['artifact:test-log'],
        log_sha256: 'sha256:log',
        source_refs: ['github:OpenAgentsInc/openagents#6770'],
        redacted: true,
      },
      now,
    )
    expect(verificationReceipt.command_args).toEqual(['bun', 'test'])
    expect(verificationReceipt.redacted).toBe(true)
    await expect(
      store.listVerificationReceipts('tenant.openagents', 10, 'change.forge.6770'),
    ).resolves.toEqual([verificationReceipt])

    const promotionDecision = await store.recordPromotionDecisionReceipt(
      {
        schema: 'openagents.forge.promotion.decision.v0.1',
        tenant_ref: 'tenant.openagents',
        promotion_ref: 'promotion.forge.6770',
        queue_ref: 'queue.forge.main',
        change_ref: 'change.forge.6770',
        decision: 'approved',
        base_head: '8e0c9b2eaf84c821caf555cae233a0d27e94d4ab',
        candidate_head: '9e0c9b2eaf84c821caf555cae233a0d27e94d4ac',
        promoted_head: '9e0c9b2eaf84c821caf555cae233a0d27e94d4ac',
        verification_ref: verificationReceipt.verification_ref,
        gate_refs: ['gate.tests'],
        blocker_refs: [],
        decided_by_ref: 'agent.public.forge',
        decided_at: '2026-06-28T16:03:00.000Z',
        source_refs: ['github:OpenAgentsInc/openagents#6770'],
        redacted: true,
      },
      now,
    )
    expect(promotionDecision.decision).toBe('approved')
    await expect(
      store.listPromotionDecisionReceipts('tenant.openagents', 10, 'change.forge.6770'),
    ).resolves.toEqual([promotionDecision])
  })
})
