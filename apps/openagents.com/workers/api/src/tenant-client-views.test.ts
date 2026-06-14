import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  TenantClientWorkroomViewDenied,
  tenantClientWorkroomView,
} from './tenant-client-views'
import { type TenantRef } from './tenant-custom-hostnames'

// Minimal fake D1 covering only the two queries this core issues:
//   1. team_memberships INNER JOIN teams  (readActiveTeamMembershipRole)
//   2. omni_workrooms LEFT JOIN site_projects (readWorkroomWithTenant)

type MembershipRow = Readonly<{
  team_id: string
  user_id: string
  role: 'owner' | 'admin' | 'member' | 'viewer'
}>

type WorkroomFixture = Readonly<{
  id: string
  site_team_id: string | null
  visibility: 'private' | 'customer' | 'team' | 'public'
  data_classification?: string
}>

const baseWorkroomRow = (fixture: WorkroomFixture) => ({
  accepted_outcome_contract_id: null,
  archived_at: null,
  artifact_refs_json: JSON.stringify(['artifact.public.summary']),
  assignment_id: null,
  blocker_refs_json: JSON.stringify([]),
  classification_caveat_ref: 'classification_caveat_reviewed',
  created_at: '2026-06-14T00:00:00.000Z',
  customer_intent_ref: 'intent.customer.summary',
  data_classification: fixture.data_classification ?? 'customer',
  email_refs_json: JSON.stringify([]),
  id: fixture.id,
  idempotency_key: `idem_${fixture.id}`,
  metadata_json: JSON.stringify({}),
  public_receipt_ref: `omni_workroom:order:${fixture.id}`,
  receipt_refs_json: JSON.stringify(['receipt.public.1']),
  site_id: fixture.site_team_id === null ? null : `site_${fixture.id}`,
  site_team_id: fixture.site_team_id,
  software_order_id: `software_order_${fixture.id}`,
  source_refs_json: JSON.stringify(['source.public.1']),
  status: 'active',
  task_packet_ref: null,
  trust_tier: 'verified',
  updated_at: '2026-06-14T01:00:00.000Z',
  visibility: fixture.visibility,
  work_kind: 'coding',
})

class Store {
  memberships: Array<MembershipRow> = []
  workrooms: Array<WorkroomFixture> = []
}

class Statement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: Store,
  ) {}

  bind(...values: ReadonlyArray<unknown>) {
    this.values = values

    return this
  }

  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM team_memberships')) {
      const teamId = String(this.values[0])
      const userId = String(this.values[1])
      const row = this.store.memberships.find(
        m => m.team_id === teamId && m.user_id === userId,
      )

      return Promise.resolve((row ? { role: row.role } : null) as T | null)
    }

    if (this.query.includes('FROM omni_workrooms')) {
      const id = String(this.values[0])
      const fixture = this.store.workrooms.find(w => w.id === id)

      return Promise.resolve(
        (fixture ? baseWorkroomRow(fixture) : null) as T | null,
      )
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }
}

const makeDb = (store: Store): D1Database =>
  ({
    prepare: (query: string) => new Statement(query, store),
  }) as unknown as D1Database

const tenant = (teamId: string): TenantRef => ({
  teamId,
  hostname: `${teamId}.clients.example.com`,
  status: 'active',
})

describe('tenantClientWorkroomView', () => {
  test('authorized client sees the customer projection', async () => {
    const store = new Store()
    store.memberships.push({
      team_id: 'team_a',
      user_id: 'client_1',
      role: 'viewer',
    })
    store.workrooms.push({
      id: 'wr_1',
      site_team_id: 'team_a',
      visibility: 'customer',
    })

    const view = await Effect.runPromise(
      tenantClientWorkroomView(makeDb(store), {
        clientUserId: 'client_1',
        tenant: tenant('team_a'),
        workroomId: 'wr_1',
      }),
    )

    expect(view.surface).toBe('customer')
    expect(view.teamId).toBe('team_a')
    expect(view.workroomId).toBe('wr_1')
    expect(view.projection.surface).toBe('customer')
    // customer surface includes customer-safe refs
    const workroom = view.projection.workroom as Record<string, unknown>
    expect(workroom.customerIntentRef).toBe('intent.customer.summary')
  })

  test('team-visibility workroom is still projected only at customer surface', async () => {
    const store = new Store()
    store.memberships.push({
      team_id: 'team_a',
      user_id: 'client_1',
      role: 'member',
    })
    store.workrooms.push({
      id: 'wr_team',
      site_team_id: 'team_a',
      visibility: 'team',
    })

    const view = await Effect.runPromise(
      tenantClientWorkroomView(makeDb(store), {
        clientUserId: 'client_1',
        tenant: tenant('team_a'),
        workroomId: 'wr_team',
      }),
    )

    expect(view.projection.surface).toBe('customer')
    // the broader team surface exposes acceptedOutcomeContractId / sourceRefs;
    // the customer surface must NOT, even for a team-visibility workroom.
    expect(
      'acceptedOutcomeContractId' in view.projection.workroom,
    ).toBe(false)
    expect('sourceRefs' in view.projection.workroom).toBe(false)
  })

  test('unauthorized client (no membership) is denied', async () => {
    const store = new Store()
    // membership intentionally omitted
    store.workrooms.push({
      id: 'wr_1',
      site_team_id: 'team_a',
      visibility: 'customer',
    })

    const error = await Effect.runPromise(
      tenantClientWorkroomView(makeDb(store), {
        clientUserId: 'stranger',
        tenant: tenant('team_a'),
        workroomId: 'wr_1',
      }).pipe(Effect.flip),
    )

    expect(error).toBeInstanceOf(TenantClientWorkroomViewDenied)
    expect((error as TenantClientWorkroomViewDenied).reason).toBe(
      'not_authorized_for_tenant',
    )
  })

  test('cross-tenant client is denied (workroom belongs to a different team)', async () => {
    const store = new Store()
    // client is a member of team_b, but asks on tenant team_a host for a
    // workroom owned by team_b.
    store.memberships.push({
      team_id: 'team_a',
      user_id: 'client_x',
      role: 'admin',
    })
    store.workrooms.push({
      id: 'wr_other',
      site_team_id: 'team_b',
      visibility: 'customer',
    })

    const error = await Effect.runPromise(
      tenantClientWorkroomView(makeDb(store), {
        clientUserId: 'client_x',
        tenant: tenant('team_a'),
        workroomId: 'wr_other',
      }).pipe(Effect.flip),
    )

    expect((error as TenantClientWorkroomViewDenied).reason).toBe(
      'workroom_not_in_tenant',
    )
  })

  test('site-less workroom cannot be proven in-tenant and is denied', async () => {
    const store = new Store()
    store.memberships.push({
      team_id: 'team_a',
      user_id: 'client_1',
      role: 'owner',
    })
    store.workrooms.push({
      id: 'wr_no_site',
      site_team_id: null,
      visibility: 'customer',
    })

    const error = await Effect.runPromise(
      tenantClientWorkroomView(makeDb(store), {
        clientUserId: 'client_1',
        tenant: tenant('team_a'),
        workroomId: 'wr_no_site',
      }).pipe(Effect.flip),
    )

    expect((error as TenantClientWorkroomViewDenied).reason).toBe(
      'workroom_not_in_tenant',
    )
  })

  test('private-visibility workroom is denied even to an authorized member; private fields never leak', async () => {
    const store = new Store()
    store.memberships.push({
      team_id: 'team_a',
      user_id: 'client_1',
      role: 'admin',
    })
    store.workrooms.push({
      id: 'wr_private',
      site_team_id: 'team_a',
      visibility: 'private',
    })

    const error = await Effect.runPromise(
      tenantClientWorkroomView(makeDb(store), {
        clientUserId: 'client_1',
        tenant: tenant('team_a'),
        workroomId: 'wr_private',
      }).pipe(Effect.flip),
    )

    expect((error as TenantClientWorkroomViewDenied).reason).toBe(
      'workroom_not_client_visible',
    )
  })

  test('missing workroom is denied as not_found', async () => {
    const store = new Store()
    store.memberships.push({
      team_id: 'team_a',
      user_id: 'client_1',
      role: 'member',
    })

    const error = await Effect.runPromise(
      tenantClientWorkroomView(makeDb(store), {
        clientUserId: 'client_1',
        tenant: tenant('team_a'),
        workroomId: 'missing',
      }).pipe(Effect.flip),
    )

    expect((error as TenantClientWorkroomViewDenied).reason).toBe(
      'workroom_not_found',
    )
  })
})
