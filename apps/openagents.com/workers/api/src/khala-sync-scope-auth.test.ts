// KS-7.1 (#8305) Worker scope-auth wiring tests: the REAL
// `makeKhalaSyncScopeReadResolver` over a fake D1 (answering the exact
// membership/ownership queries the production capabilities issue) and a
// fake Postgres client for the fleet scope-owner lookup. Proves the full
// auth matrix at the Worker seam — self/foreign personal, public, live
// team member vs non-member (and revocation-by-row-removal), agent_run and
// thread ownership incl. team runs and the autopilot-thread mapping,
// fleet_run owners, unknown taxonomy, and fail-closed unavailability.

import { describe, expect, test } from 'vitest'

import {
  agentRunScope,
  fleetRunScope,
  personalScope,
  publicScope,
  SyncScope,
  teamScope,
  threadScope,
} from '@openagentsinc/khala-sync'
import type { SyncSql } from '@openagentsinc/khala-sync-server'

import { makeKhalaSyncScopeReadResolver } from './khala-sync-scope-auth'

const scopeOf = (raw: string): SyncScope => SyncScope.make(raw)

const USER = 'user-a'
const OTHER = 'user-b'
const TEAM = 'team-1'

// ---------------------------------------------------------------------------
// Fake D1: answers the three production queries from in-memory state.
// ---------------------------------------------------------------------------

type FakeD1State = Readonly<{
  /** Active memberships as `${teamId}:${userId}` → role. */
  memberships: Map<string, string>
  /** agent_runs rows by id. */
  runs: Map<string, { id: string; team_id: string | null; user_id: string }>
  /** team_chat_messages autopilot_thread_id → agent_run_id. */
  autopilotThreads: Map<string, string>
}>

const fakeD1 = (state: FakeD1State): D1Database =>
  ({
    prepare: (sql: string) => ({
      bind: (...bindings: Array<unknown>) => ({
        first: async <T>(): Promise<T | null> => {
          if (sql.includes('FROM team_memberships')) {
            const [teamId, userId] = bindings as [string, string]
            const role = state.memberships.get(`${teamId}:${userId}`)
            return (role === undefined ? null : { role }) as T | null
          }
          if (sql.includes('FROM agent_runs')) {
            const [runId] = bindings as [string]
            return (state.runs.get(runId) ?? null) as T | null
          }
          if (sql.includes('FROM team_chat_messages')) {
            const [threadId] = bindings as [string]
            const runId = state.autopilotThreads.get(threadId)
            return (
              runId === undefined ? null : { agent_run_id: runId }
            ) as T | null
          }
          throw new Error(`fake D1 has no route for query: ${sql.slice(0, 80)}`)
        },
      }),
    }),
  }) as unknown as D1Database

const throwingD1 = (): D1Database =>
  ({
    prepare: () => ({
      bind: () => ({
        first: async () => {
          throw new Error('D1 unavailable')
        },
      }),
    }),
  }) as unknown as D1Database

// ---------------------------------------------------------------------------
// Fake Postgres client for khala_sync_scope_owners
// ---------------------------------------------------------------------------

const fleetOwnerClient = (owners: Readonly<Record<string, string>>) => {
  let ended = 0
  const sql = ((
    strings: TemplateStringsArray,
    ...values: ReadonlyArray<unknown>
  ) => {
    const text = strings.join('$')
    if (text.includes('khala_sync_scope_owners')) {
      const owner = owners[String(values[0])]
      return Promise.resolve(
        owner === undefined ? [] : [{ owner_user_id: owner }],
      )
    }
    return Promise.reject(new Error('unexpected query'))
  }) as unknown as SyncSql
  return {
    client: {
      end: () => {
        ended += 1
        return Promise.resolve()
      },
      sql,
    },
    endedCount: () => ended,
  }
}

const BINDING = { connectionString: 'postgresql://fake/khala' }

const makeResolver = (
  input: Readonly<{
    state?: Partial<FakeD1State>
    db?: D1Database
    fleetOwners?: Readonly<Record<string, string>>
    binding?: { connectionString: string } | undefined
  }> = {},
) => {
  const fleet = fleetOwnerClient(input.fleetOwners ?? {})
  const resolver = makeKhalaSyncScopeReadResolver({
    binding: 'binding' in input ? input.binding : BINDING,
    db:
      input.db ??
      fakeD1({
        autopilotThreads: new Map(),
        memberships: new Map(),
        runs: new Map(),
        ...input.state,
      }),
    makeSqlClient: async () => fleet.client,
  })
  return { fleet, resolver }
}

describe('makeKhalaSyncScopeReadResolver (Worker auth matrix)', () => {
  test('personal: self allowed, foreign denied — D1 never consulted', async () => {
    const { resolver } = makeResolver({ db: throwingD1() })
    expect(await resolver(USER, personalScope(USER))).toEqual({
      kind: 'allowed',
    })
    expect(await resolver(OTHER, personalScope(USER))).toEqual({
      kind: 'denied',
      reason: 'unauthorized_scope',
    })
  })

  test('public: any authenticated user — D1 never consulted', async () => {
    const { resolver } = makeResolver({ db: throwingD1() })
    expect(await resolver(USER, publicScope('tokens-served'))).toEqual({
      kind: 'allowed',
    })
  })

  test('public: an ANONYMOUS caller (userId undefined) is also allowed — D1 never consulted (KS-8.x)', async () => {
    const { resolver } = makeResolver({ db: throwingD1() })
    expect(await resolver(undefined, publicScope('tokens-served'))).toEqual({
      kind: 'allowed',
    })
  })

  test.each([
    ['personal', personalScope(USER)],
    ['team', teamScope(TEAM)],
    ['agent_run', agentRunScope('run-1')],
    ['thread', threadScope('thread-1')],
    ['fleet_run', fleetRunScope('fleet-1')],
  ] as const)(
    'SECURITY: an ANONYMOUS caller is denied %s at the Worker seam — never a grant',
    async (_label, scope) => {
      // D1 throws if ever consulted (personal/team/agent_run/thread run
      // through D1); the fleet-owner Postgres lookup would grant `fleet-1`
      // to USER if it were ever reached with a userId. Either way, the
      // anonymous caller must see a plain denial, never `allowed`.
      const { resolver } = makeResolver({
        db: throwingD1(),
        fleetOwners: { [String(fleetRunScope('fleet-1'))]: USER },
      })
      const decision = await resolver(undefined, scope)
      expect(decision.kind).not.toBe('allowed')
    },
  )

  test('team: LIVE D1 membership grants; non-member and removed-member deny', async () => {
    const memberships = new Map([[`${TEAM}:${USER}`, 'member']])
    const { resolver } = makeResolver({ state: { memberships } })
    expect(await resolver(USER, teamScope(TEAM))).toEqual({ kind: 'allowed' })
    expect(await resolver(OTHER, teamScope(TEAM))).toEqual({
      kind: 'denied',
      reason: 'unauthorized_scope',
    })
    // Revocation: the SAME resolver re-reads live rows — removing the
    // membership flips the next decision to denied (invariant 7).
    memberships.delete(`${TEAM}:${USER}`)
    expect(await resolver(USER, teamScope(TEAM))).toEqual({
      kind: 'denied',
      reason: 'unauthorized_scope',
    })
  })

  test('agent_run: owner allowed; team run readable by an active member; foreign denied', async () => {
    const { resolver } = makeResolver({
      state: {
        memberships: new Map([[`${TEAM}:${OTHER}`, 'member']]),
        runs: new Map([
          ['run-own', { id: 'run-own', team_id: null, user_id: USER }],
          ['run-team', { id: 'run-team', team_id: TEAM, user_id: USER }],
        ]),
      },
    })
    expect(await resolver(USER, agentRunScope('run-own'))).toEqual({
      kind: 'allowed',
    })
    expect(await resolver(OTHER, agentRunScope('run-own'))).toEqual({
      kind: 'denied',
      reason: 'unauthorized_scope',
    })
    // Team run: the member (via membership) AND the owning user read it.
    expect(await resolver(OTHER, agentRunScope('run-team'))).toEqual({
      kind: 'allowed',
    })
    expect(await resolver(USER, agentRunScope('run-team'))).toEqual({
      kind: 'allowed',
    })
    // Unknown run: denied, never a grant.
    expect(await resolver(USER, agentRunScope('run-missing'))).toEqual({
      kind: 'denied',
      reason: 'unauthorized_scope',
    })
  })

  test('thread: resolves through agent_runs directly and via the autopilot-thread mapping', async () => {
    const { resolver } = makeResolver({
      state: {
        autopilotThreads: new Map([['thread-ap', 'run-own']]),
        runs: new Map([
          ['run-own', { id: 'run-own', team_id: null, user_id: USER }],
        ]),
      },
    })
    // Thread id that IS a run id.
    expect(await resolver(USER, threadScope('run-own'))).toEqual({
      kind: 'allowed',
    })
    // Autopilot thread id that maps to the run.
    expect(await resolver(USER, threadScope('thread-ap'))).toEqual({
      kind: 'allowed',
    })
    expect(await resolver(OTHER, threadScope('thread-ap'))).toEqual({
      kind: 'denied',
      reason: 'unauthorized_scope',
    })
    // Unresolvable thread: denied.
    expect(await resolver(USER, threadScope('thread-missing'))).toEqual({
      kind: 'denied',
      reason: 'unauthorized_scope',
    })
  })

  test('thread: owner-private chat/runtime scopes resolve through khala_sync_scope_owners and revoke live', async () => {
    const owners: Record<string, string> = {
      [threadScope('runtime-thread-1')]: USER,
    }
    const scope = threadScope('runtime-thread-1')
    const { fleet, resolver } = makeResolver({
      fleetOwners: owners,
    })
    expect(await resolver(USER, scope)).toEqual({ kind: 'allowed' })
    expect(await resolver(OTHER, scope)).toEqual({
      kind: 'denied',
      reason: 'unauthorized_scope',
    })
    delete owners[scope]
    expect(await resolver(USER, scope)).toEqual({
      kind: 'denied',
      reason: 'unauthorized_scope',
    })
    expect(fleet.endedCount()).toBe(3)
  })

  test('thread: an owner-private Postgres-owned scope resolves even when D1 is UNAVAILABLE (CFG D1 evacuation regression)', async () => {
    // Real production failure (2026-07-06): the Cloudflare D1 bridge 401s
    // account-wide. A brand-new mobile chat thread is owned in
    // khala_sync_scope_owners (Postgres); its bootstrap must still authorize
    // even though the legacy D1 agent-run lookup throws. Before the fix, the
    // D1 read ran first and threw -> `unavailable` -> endless "Thread
    // unavailable" / must_refetch.
    const scope = threadScope('mobile-thread-1')
    const { resolver } = makeResolver({
      db: throwingD1(),
      fleetOwners: { [String(scope)]: USER },
    })
    expect(await resolver(USER, scope)).toEqual({ kind: 'allowed' })
    // A foreign caller on an owner-private thread we can't match in Postgres
    // still falls through to the (throwing) D1 path and fails CLOSED as
    // `unavailable` — never a grant.
    expect((await resolver(OTHER, scope)).kind).not.toBe('allowed')
  })

  test('fleet_run: khala_sync_scope_owners owner allowed, foreign/unowned denied; client always released', async () => {
    const scope = fleetRunScope('fleet-1')
    const { fleet, resolver } = makeResolver({
      fleetOwners: { [scope]: USER },
    })
    expect(await resolver(USER, scope)).toEqual({ kind: 'allowed' })
    expect(await resolver(OTHER, scope)).toEqual({
      kind: 'denied',
      reason: 'unauthorized_scope',
    })
    expect(await resolver(USER, fleetRunScope('fleet-unowned'))).toEqual({
      kind: 'denied',
      reason: 'unauthorized_scope',
    })
    expect(fleet.endedCount()).toBe(3)
  })

  test('fleet_run with the KHALA_SYNC_DB binding absent fails CLOSED as unavailable', async () => {
    const { resolver } = makeResolver({ binding: undefined })
    const decision = await resolver(USER, fleetRunScope('fleet-1'))
    expect(decision.kind).toBe('unavailable')
  })

  test('a throwing D1 fails CLOSED as unavailable for membership/ownership scopes (never a grant)', async () => {
    const { resolver } = makeResolver({ db: throwingD1() })
    for (const scope of [
      teamScope(TEAM),
      agentRunScope('run-1'),
      threadScope('thread-1'),
    ]) {
      const decision = await resolver(USER, scope)
      expect(decision.kind).toBe('unavailable')
    }
  })

  test('unknown taxonomy members are gated CLOSED with unknown_scope', async () => {
    const { resolver } = makeResolver()
    expect(await resolver(USER, scopeOf('scope.workspace.w-1'))).toEqual({
      kind: 'denied',
      reason: 'unknown_scope',
    })
  })
})
