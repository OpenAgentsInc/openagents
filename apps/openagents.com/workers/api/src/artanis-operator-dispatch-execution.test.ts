import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  makeArtanisDispatchExecution,
  readEffectiveArtanisPylonDispatchApprovalForOwner,
} from './artanis-operator-dispatch-execution'
import { ARTANIS_OWNER_OPENAUTH_USER_ID } from './artanis-owner-authority'
import type { ArtanisDispatchPlanInput } from './artanis-operator-tools'
import type {
  PylonApiAssignmentRecord,
  PylonApiRegistrationRecord,
  PylonApiStore,
} from './pylon-api'

const nowIso = '2026-06-26T12:00:00.000Z'

// A minimal eligible owner-linked Codex Pylon registration (active, fresh
// heartbeat, codex capability, one available slot). Mirrors the delegation
// test fixture.
const registration = (
  overrides: Partial<PylonApiRegistrationRecord> = {},
): PylonApiRegistrationRecord => ({
  capabilityRefs: ['capability.pylon.local_codex'],
  clientProtocolVersion: '0.3.0',
  clientVersion: '0.3.0',
  createdAt: nowIso,
  displayName: 'Linked Codex Pylon',
  id: 'pylon_api_registration_1',
  latestCapacityRefs: [
    'capacity.coding.codex.ready=1',
    'capacity.coding.codex.available=1',
  ],
  latestHeartbeatAt: nowIso,
  latestHeartbeatStatus: 'online',
  latestHealthRefs: ['health.public.pylon_cli.ok'],
  latestLoadRefs: ['load.coding.codex.busy=0', 'load.coding.codex.queued=0'],
  latestResourceMode: 'background_20',
  ownerAgentCredentialId: 'agent_credential_owner',
  ownerAgentTokenPrefix: 'oa_agent_owner',
  ownerAgentUserId: 'agent_owner',
  providerMarketRelayRefs: [],
  providerNip90LaneRefs: [],
  providerNostrNpub: null,
  providerNostrPubkey: null,
  publicProjectionJson: '{}',
  pylonRef: 'pylon.owner.codex',
  resourceMode: 'background_20',
  status: 'active',
  updatedAt: nowIso,
  walletReady: true,
  walletRef: null,
  ...overrides,
})

const makeStore = (input: {
  registrations: ReadonlyArray<PylonApiRegistrationRecord>
}): { store: PylonApiStore; created: Array<PylonApiAssignmentRecord> } => {
  const created: Array<PylonApiAssignmentRecord> = []
  const store: PylonApiStore = {
    createAssignment: async record => {
      created.push(record)
      return { idempotent: false, record }
    },
    createEvent: async () => {
      throw new Error('not used')
    },
    listAssignmentsForPylon: async () => [],
    listEventsForAssignment: async () => [],
    listEventsForPylon: async () => [],
    listProviderJobLifecycleForPylons: async () => [],
    listRegistrations: async () => input.registrations,
    listRegistrationsForOwnerAgentUserIds: async ownerAgentUserIds =>
      input.registrations.filter(item =>
        ownerAgentUserIds.includes(item.ownerAgentUserId),
      ),
    readAssignment: async () => undefined,
    readAssignmentByIdempotencyKeyHash: async () => undefined,
    readEventByIdempotencyKeyHash: async () => undefined,
    readRegistration: async pylonRef =>
      input.registrations.find(item => item.pylonRef === pylonRef),
    updateAssignment: async record => record,
    updateAssignmentIfState: async () => undefined,
    upsertProviderJobLifecycle: async record => record,
    upsertRegistration: async record => record,
  }
  return { created, store }
}

const plan: ArtanisDispatchPlanInput = {
  branch: 'main',
  filePaths: [],
  issue: 6320,
  objective: 'Burn down public issue work per the roadmap.',
  prompt: 'Implement public issue #6320. Burn down public issue work.',
  verify: undefined,
}

let idCounter = 0
const makeDeps = (overrides: {
  registrations?: ReadonlyArray<PylonApiRegistrationRecord>
  linkedAgentUserIds?: ReadonlyArray<string>
  approved?: boolean
}) => {
  const { created, store } = makeStore({
    registrations: overrides.registrations ?? [registration()],
  })
  return {
    created,
    deps: {
      listLinkedAgentUserIds: async () =>
        overrides.linkedAgentUserIds ?? ['agent_owner'],
      makeId: () => `id${(idCounter += 1)}`,
      nowIso: () => nowIso,
      ownerOpenAuthUserId: 'user_owner',
      pylonStore: store,
      readEffectivePylonDispatchApproval: async () =>
        overrides.approved ?? false,
    },
  }
}

describe('makeArtanisDispatchExecution (#6366 live seam)', () => {
  test('isOwnerApproved reflects the persisted approval reader', async () => {
    const approved = makeArtanisDispatchExecution(
      makeDeps({ approved: true }).deps,
    )
    const denied = makeArtanisDispatchExecution(
      makeDeps({ approved: false }).deps,
    )
    expect(await Effect.runPromise(approved.isOwnerApproved())).toBe(true)
    expect(await Effect.runPromise(denied.isOwnerApproved())).toBe(false)
  })

  test('createCodexAssignment creates an own-capacity, no-spend assignment on the owner Pylon', async () => {
    const { created, deps } = makeDeps({ approved: true })
    const execution = makeArtanisDispatchExecution(deps)

    const result = await Effect.runPromise(execution.createCodexAssignment(plan))
    expect(result.kind).toBe('created')
    if (result.kind !== 'created') return
    expect(result.pylonRef).toBe('pylon.owner.codex')
    expect(result.assignmentRef).toContain('assignment.public.khala_coding.')
    expect(result.durableRequestId).not.toBeNull()

    // The created record is owner-scoped and codex-typed. The no-spend
    // (unpaid_smoke / own_capacity) payment mode is enforced by the underlying
    // coding-delegation request and verified in that module's tests; here we
    // confirm the assignment was created against the owner's own Pylon.
    expect(created).toHaveLength(1)
    expect(created[0]?.jobKind).toBe('codex_agent_task')
    expect(created[0]?.ownerAgentUserId).toBe('agent_owner')
    expect(created[0]?.pylonRef).toBe('pylon.owner.codex')
  })

  test('rejects with no_linked_agents when the owner has no linked agents', async () => {
    const { deps } = makeDeps({ approved: true, linkedAgentUserIds: [] })
    const execution = makeArtanisDispatchExecution(deps)
    const result = await Effect.runPromise(execution.createCodexAssignment(plan))
    expect(result).toEqual({ kind: 'rejected', reason: 'no_linked_agents' })
  })

  test('rejects with no_eligible_linked_pylon when no owner Pylon is eligible', async () => {
    const { created, deps } = makeDeps({ approved: true, registrations: [] })
    const execution = makeArtanisDispatchExecution(deps)
    const result = await Effect.runPromise(execution.createCodexAssignment(plan))
    expect(result).toEqual({
      kind: 'rejected',
      reason: 'no_eligible_linked_pylon',
    })
    // No assignment is created when there is no eligible capacity.
    expect(created).toHaveLength(0)
  })

  test('does not select another owner account Pylon (own-capacity only)', async () => {
    const { created, deps } = makeDeps({
      approved: true,
      // A live, eligible Pylon owned by a DIFFERENT account.
      registrations: [registration({ ownerAgentUserId: 'agent_other' })],
    })
    const execution = makeArtanisDispatchExecution(deps)
    const result = await Effect.runPromise(execution.createCodexAssignment(plan))
    // Owner only links agent_owner, so the other account's Pylon is invisible.
    expect(result).toEqual({
      kind: 'rejected',
      reason: 'no_eligible_linked_pylon',
    })
    expect(created).toHaveLength(0)
  })
})


// A D1 stub whose `prepare().all()` returns no rows, so the armed-gate read
// resolves to "not approved" (false). Used to prove the owner-promotion path
// short-circuits BEFORE this query (it throws if the query is reached for the
// promoted owner).
const emptyGatesDb = (onQuery?: () => void): D1Database =>
  ({
    prepare: () => {
      onQuery?.()
      return {
        all: async () => ({ results: [] }),
        bind() {
          return this
        },
      }
    },
  }) as unknown as D1Database

describe('readEffectiveArtanisPylonDispatchApprovalForOwner (owner promotion)', () => {
  test('owner-promoted Artanis carries a STANDING approval without an armed gate', async () => {
    let queried = false
    const approved = await readEffectiveArtanisPylonDispatchApprovalForOwner(
      emptyGatesDb(() => {
        queried = true
      }),
      nowIso,
      ARTANIS_OWNER_OPENAUTH_USER_ID,
    )
    expect(approved).toBe(true)
    // The standing promotion short-circuits before the D1 gate query.
    expect(queried).toBe(false)
  })

  test('a non-promoted owner falls back to the armed D1 gate (none -> false)', async () => {
    let queried = false
    const approved = await readEffectiveArtanisPylonDispatchApprovalForOwner(
      emptyGatesDb(() => {
        queried = true
      }),
      nowIso,
      'user_some_other_owner',
    )
    expect(approved).toBe(false)
    // The armed-gate path WAS consulted for a non-promoted owner.
    expect(queried).toBe(true)
  })
})

describe('owner-promoted Artanis dispatch EXECUTES end-to-end (gated tool)', () => {
  test('isOwnerApproved true (standing) + eligible Pylon -> created assignment', async () => {
    const { created, store } = makeStore({ registrations: [registration()] })
    const execution = makeArtanisDispatchExecution({
      listLinkedAgentUserIds: async () => ['agent_owner'],
      makeId: () => `idp${(idCounter += 1)}`,
      nowIso: () => nowIso,
      ownerOpenAuthUserId: ARTANIS_OWNER_OPENAUTH_USER_ID,
      pylonStore: store,
      // Standing owner approval (what the live wiring resolves for owner-Artanis).
      readEffectivePylonDispatchApproval: () =>
        readEffectiveArtanisPylonDispatchApprovalForOwner(
          emptyGatesDb(),
          nowIso,
          ARTANIS_OWNER_OPENAUTH_USER_ID,
        ),
    })

    expect(await Effect.runPromise(execution.isOwnerApproved())).toBe(true)

    const result = await Effect.runPromise(execution.createCodexAssignment(plan))
    expect(result.kind).toBe('created')
    if (result.kind !== 'created') return
    expect(result.pylonRef).toBe('pylon.owner.codex')
    expect(result.assignmentRef).toContain('assignment.public.khala_coding.')
    expect(created).toHaveLength(1)
    expect(created[0]?.jobKind).toBe('codex_agent_task')
  })
})
