import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type { AdjutantAssignment } from './adjutant-assignments'
import {
  defaultResearchPolicyModeForAssignment,
  makeAdjutantResearchPolicyService,
} from './adjutant-research-policies'

const assignment = (
  assignmentKind: AdjutantAssignment['assignmentKind'],
): AdjutantAssignment => ({
  agentId: 'agent_adjutant',
  archivedAt: null,
  assignedByUserId: 'github:operator',
  assignmentKind,
  blockedAt: null,
  commitSha: null,
  completedAt: null,
  createdAt: '2026-06-05T00:00:00.000Z',
  currentRunId: null,
  goalId: 'agent_goal_1',
  id: `assignment_${assignmentKind}`,
  objective: 'Build the requested customer Site.',
  projectId: 'project_adjutant',
  siteId: assignmentKind.startsWith('site_') ? 'site_project_1' : null,
  softwareOrderId: 'software_order_1',
  status: 'preflight_pending',
  taskSpecPath: null,
  teamId: 'team_openagents_core',
  updatedAt: '2026-06-05T00:00:00.000Z',
  visibility: 'team',
})

type StoredResearchPolicy = Readonly<{
  actor_user_id: string | null
  archived_at: string | null
  assignment_id: string
  customer_safe_summary: string
  policy_mode:
    | 'research_required'
    | 'research_optional'
    | 'research_not_applicable'
    | 'research_bypassed_by_operator'
  reason: string
  source_authority_ref: string | null
  updated_at: string
}>

class PolicyStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly rows: Array<StoredResearchPolicy>,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    const [assignmentId] = this.values
    const row =
      this.rows.find(
        policy =>
          policy.assignment_id === assignmentId && policy.archived_at === null,
      ) ?? null

    return Promise.resolve((row as T | undefined) ?? null)
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    const [
      assignmentId,
      policyMode,
      reason,
      customerSafeSummary,
      actorUserId,
      sourceAuthorityRef,
      createdAt,
      updatedAt,
    ] = this.values
    const existing = this.rows.find(
      row => row.assignment_id === assignmentId,
    )
    const next: StoredResearchPolicy = {
      actor_user_id: typeof actorUserId === 'string' ? actorUserId : null,
      archived_at: null,
      assignment_id: String(assignmentId),
      customer_safe_summary: String(customerSafeSummary),
      policy_mode: policyMode as StoredResearchPolicy['policy_mode'],
      reason: String(reason),
      source_authority_ref:
        typeof sourceAuthorityRef === 'string' ? sourceAuthorityRef : null,
      updated_at: String(updatedAt ?? createdAt),
    }

    if (existing === undefined) {
      this.rows.push(next)
    } else {
      const index = this.rows.indexOf(existing)
      this.rows[index] = next
    }

    return Promise.resolve({
      meta: {} as D1Meta & Record<string, unknown>,
      results: [],
      success: true,
    })
  }

  all<T = unknown>(): Promise<D1Result<T>> {
    return Promise.reject(new Error(`Unexpected all: ${this.query}`))
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(): Promise<[Array<string>, ...Array<T>] | Array<T>> {
    return Promise.reject(new Error(`Unexpected raw: ${this.query}`))
  }
}

const policyDb = (rows: Array<StoredResearchPolicy>): D1Database => ({
  batch: () => Promise.reject(new Error('batch should not be used')),
  dump: () => Promise.reject(new Error('dump should not be used')),
  exec: () => Promise.reject(new Error('exec should not be used')),
  prepare: query => new PolicyStatement(query, rows),
  withSession: () => {
    throw new Error('withSession should not be used')
  },
})

describe('Adjutant research policies', () => {
  test.each([
    ['site_generation', 'research_required'],
    ['site_adjustment', 'research_optional'],
    ['general_order_fulfillment', 'research_optional'],
    ['site_review', 'research_not_applicable'],
    ['site_deployment', 'research_not_applicable'],
  ] as const)('derives %s default policy', (kind, expected) => {
    expect(defaultResearchPolicyModeForAssignment(assignment(kind))).toBe(
      expected,
    )
  })

  test('reads deterministic default policy without a stored override', async () => {
    const service = makeAdjutantResearchPolicyService(policyDb([]))

    const policy = await Effect.runPromise(
      service.readEffectivePolicy(assignment('site_generation')),
    )

    expect(policy).toMatchObject({
      customerSafeStatus: 'research_required',
      defaultMode: 'research_required',
      effectiveMode: 'research_required',
      reason: null,
      source: 'default_assignment_kind',
    })
  })

  test('stores an operator bypass with bounded customer-safe projection', async () => {
    const rows: Array<StoredResearchPolicy> = []
    const subject = assignment('site_generation')
    const service = makeAdjutantResearchPolicyService(policyDb(rows), {
      nowIso: () => '2026-06-05T01:00:00.000Z',
    })

    const policy = await Effect.runPromise(
      service.setPolicyOverride(subject, {
        actorUserId: 'github:operator',
        assignmentId: subject.id,
        customerSafeSummary:
          'The operator approved this assignment using existing customer-provided context.',
        policyMode: 'research_bypassed_by_operator',
        reason: 'Customer supplied enough public source context in the order.',
        sourceAuthorityRef: 'order:software_order_1',
      }),
    )

    expect(policy).toMatchObject({
      actorUserId: 'github:operator',
      customerSafeStatus: 'research_bypassed',
      defaultMode: 'research_required',
      effectiveMode: 'research_bypassed_by_operator',
      source: 'operator_override',
      sourceAuthorityRef: 'order:software_order_1',
    })
    expect(rows).toHaveLength(1)
  })

  test('rejects empty bypass reason and secret-shaped summaries', async () => {
    const subject = assignment('site_generation')
    const service = makeAdjutantResearchPolicyService(policyDb([]))

    await expect(
      Effect.runPromise(
        service.setPolicyOverride(subject, {
          actorUserId: 'github:operator',
          assignmentId: subject.id,
          customerSafeSummary: 'Approved by operator.',
          policyMode: 'research_bypassed_by_operator',
          reason: '   ',
        }),
      ),
    ).rejects.toMatchObject({ _tag: 'AdjutantResearchPolicyValidationError' })

    await expect(
      Effect.runPromise(
        service.setPolicyOverride(subject, {
          actorUserId: 'github:operator',
          assignmentId: subject.id,
          customerSafeSummary:
            'Do not show bearer token sk-abcdefghijklmnopqrstuvwxyz in public status.',
          policyMode: 'research_bypassed_by_operator',
          reason: 'Unsafe summary.',
        }),
      ),
    ).rejects.toMatchObject({ _tag: 'AdjutantResearchPolicyUnsafePayload' })
  })
})
