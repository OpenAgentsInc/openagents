import { describe, expect, test } from 'vitest'

import type { SyncSql } from '@openagentsinc/khala-sync-server'

import {
  fleetRunRefFromAssignment,
  projectFleetAssignmentTransition,
} from './khala-sync-fleet-projection'
import type { PylonApiAssignmentRecord } from './pylon-api'

const FORBIDDEN = /token|apiKey|authorization|\/Users\//i

const assignment = (
  overrides: Partial<PylonApiAssignmentRecord> = {},
): PylonApiAssignmentRecord => ({
  acceptanceCriteriaRefs: [],
  acceptedWorkRefs: [],
  artifactRefs: [],
  assignmentRef: 'assignment.public.fleet.1',
  closeoutRefs: [],
  codingAssignment: {
    fleetRunRef: 'fleet-run.pylon.supervisor.abc123',
    issueRef: '#8302',
    // Private material that must NEVER reach a post-image:
    codex: { authorization: 'Bearer super-secret' },
    workspace: { path: '/Users/alice/work/openagents' },
  },
  createdAt: '2026-07-04T15:00:00.000Z',
  id: 'a-1',
  idempotencyKeyHash: 'hash-1',
  jobKind: 'codex_agent_task',
  leaseExpiresAt: '2026-07-04T16:00:00.000Z',
  ownerAgentUserId: 'agent-owner-1',
  proofRefs: [],
  publicProjectionJson: '{}',
  pylonRef: 'pylon.test.one',
  rejectionRefs: [],
  resultExpectationRefs: [],
  state: 'offered',
  taskRefs: [],
  updatedAt: '2026-07-04T15:20:11.412Z',
  ...overrides,
})

/**
 * A fake transaction-mode SQL client that answers the projector's
 * statements (scope-owner claim, scope-version allocation, changelog
 * append) and records every bound parameter value for redaction checks.
 */
const makeFakeSqlClient = () => {
  const boundValues: Array<unknown> = []
  const statements: Array<string> = []
  let ended = false
  const tx = (
    strings: TemplateStringsArray,
    ...values: ReadonlyArray<unknown>
  ): Promise<Array<Record<string, unknown>>> => {
    const text = strings.join('$')
    statements.push(text)
    boundValues.push(...values)
    if (text.includes('khala_sync_scope_owners') && text.includes('INSERT')) {
      return Promise.resolve([{ owner_user_id: values[1] as string }])
    }
    if (text.includes('khala_sync_scopes')) {
      return Promise.resolve([{ last_version: 1 }])
    }
    if (text.includes('khala_sync_changelog')) {
      return Promise.resolve([{ committed_at: '2026-07-04T15:20:12.000Z' }])
    }
    return Promise.resolve([])
  }
  const sql = Object.assign(tx, {
    begin: <A>(fn: (t: typeof tx) => Promise<A>): Promise<A> => fn(tx),
  }) as unknown as SyncSql
  return {
    boundValues,
    client: {
      end: () => {
        ended = true
        return Promise.resolve()
      },
      sql,
    },
    isEnded: () => ended,
    statements,
  }
}

describe('fleetRunRefFromAssignment', () => {
  test('extracts a bounded public-safe ref', () => {
    expect(fleetRunRefFromAssignment(assignment())).toBe(
      'fleet-run.pylon.supervisor.abc123',
    )
  })

  test('missing / non-string / unsafe refs behave as absent', () => {
    expect(
      fleetRunRefFromAssignment(assignment({ codingAssignment: null })),
    ).toBeNull()
    expect(
      fleetRunRefFromAssignment(
        assignment({ codingAssignment: { fleetRunRef: 42 } }),
      ),
    ).toBeNull()
    expect(
      fleetRunRefFromAssignment(
        assignment({
          codingAssignment: { fleetRunRef: '/Users/alice/run' },
        }),
      ),
    ).toBeNull()
    expect(
      fleetRunRefFromAssignment(
        assignment({
          codingAssignment: { fleetRunRef: 'alice@example.com' },
        }),
      ),
    ).toBeNull()
  })
})

describe('projectFleetAssignmentTransition', () => {
  test('skips assignments without a fleet run ref (no client created)', async () => {
    let factoryCalls = 0
    const outcome = await projectFleetAssignmentTransition(
      {
        binding: { connectionString: 'postgres://x' },
        makeSqlClient: () => {
          factoryCalls += 1
          throw new Error('must not be called')
        },
      },
      {
        assignment: assignment({ codingAssignment: null }),
        nowIso: '2026-07-04T15:20:11.412Z',
      },
    )
    expect(outcome).toEqual({ outcome: 'skipped_no_fleet_run_ref' })
    expect(factoryCalls).toBe(0)
  })

  test('skips when the KHALA_SYNC_DB binding is absent', async () => {
    const outcome = await projectFleetAssignmentTransition(
      {
        binding: undefined,
        makeSqlClient: () => {
          throw new Error('must not be called')
        },
      },
      { assignment: assignment(), nowIso: '2026-07-04T15:20:11.412Z' },
    )
    expect(outcome).toEqual({ outcome: 'skipped_no_binding' })
  })

  test('projects a redacted fleet_assignment post-image and closes the client', async () => {
    const fake = makeFakeSqlClient()
    const outcome = await projectFleetAssignmentTransition(
      {
        binding: { connectionString: 'postgres://x' },
        makeSqlClient: () => Promise.resolve(fake.client),
      },
      { assignment: assignment(), nowIso: '2026-07-04T15:20:11.412Z' },
    )
    expect(outcome).toEqual({
      outcome: 'projected',
      runId: 'fleet-run.pylon.supervisor.abc123',
    })
    expect(fake.isEnded()).toBe(true)

    // REDACTION PROPERTY: nothing that reached the database carries the
    // coding-assignment payload's secrets or paths.
    const serialized = JSON.stringify(fake.boundValues)
    expect(serialized).not.toMatch(FORBIDDEN)
    expect(serialized).not.toContain('super-secret')
    expect(serialized).toContain('assignment.public.fleet.1')
    expect(serialized).toContain('#8302')
    // The post-image landed in the changelog append.
    expect(
      fake.statements.some(text => text.includes('khala_sync_changelog')),
    ).toBe(true)
  })

  test('a failing database is fail-soft: typed outcome, log called, no throw', async () => {
    const logged: Array<{ event: string; reason: string }> = []
    const outcome = await projectFleetAssignmentTransition(
      {
        binding: { connectionString: 'postgres://x' },
        log: (event, fields) => {
          logged.push({ event, reason: fields.reason })
        },
        makeSqlClient: () =>
          Promise.resolve({
            end: () => Promise.resolve(),
            sql: Object.assign(
              () => Promise.reject(new Error('boom')),
              {
                begin: () => Promise.reject(new Error('connection refused')),
              },
            ) as unknown as SyncSql,
          }),
      },
      { assignment: assignment(), nowIso: '2026-07-04T15:20:11.412Z' },
    )
    expect(outcome.outcome).toBe('failed')
    expect(logged).toEqual([
      {
        event: 'khala_sync_fleet_projection_failed',
        reason: 'projection_failed',
      },
    ])
  })

  test('a throwing client FACTORY is also fail-soft', async () => {
    const outcome = await projectFleetAssignmentTransition(
      {
        binding: { connectionString: 'postgres://x' },
        makeSqlClient: () => Promise.reject(new Error('driver import failed')),
      },
      { assignment: assignment(), nowIso: '2026-07-04T15:20:11.412Z' },
    )
    expect(outcome.outcome).toBe('failed')
  })
})
