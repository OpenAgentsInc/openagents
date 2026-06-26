import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeOmniWorkroomRoutes } from './omni-workroom-routes'

type RefRow = Readonly<{ archived_at: string | null; id: string }>
type WorkroomRow = Readonly<{
  accepted_outcome_contract_id: string | null
  archived_at: string | null
  artifact_refs_json: string
  assignment_id: string | null
  blocker_refs_json: string
  classification_caveat_ref: string
  created_at: string
  customer_intent_ref: string
  data_classification: string
  email_refs_json: string
  id: string
  idempotency_key: string
  metadata_json: string
  public_receipt_ref: string
  receipt_refs_json: string
  site_id: string | null
  software_order_id: string
  source_refs_json: string
  status: string
  task_packet_ref: string | null
  trust_tier: string
  updated_at: string
  visibility: string
  work_kind: string
}>

class OmniWorkroomStore {
  orders: Array<RefRow> = [
    { archived_at: null, id: 'software_order_1' },
    { archived_at: null, id: 'software_order_business_1' },
  ]
  sites: Array<RefRow> = [{ archived_at: null, id: 'site_project_1' }]
  workrooms: Array<WorkroomRow> = []
}

class OmniWorkroomStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: OmniWorkroomStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM omni_workrooms')) {
      const arg = String(this.values[0])
      const row =
        this.store.workrooms.find(
          item =>
            (this.query.includes('WHERE id =')
              ? item.id === arg
              : item.idempotency_key === arg) && item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM software_orders')) {
      return Promise.resolve(this.findRef(this.store.orders) as T | null)
    }

    if (this.query.includes('FROM site_projects')) {
      return Promise.resolve(this.findRef(this.store.sites) as T | null)
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT OR IGNORE INTO omni_workrooms')) {
      const idempotencyKey = String(this.values[1])

      if (
        this.store.workrooms.every(
          item => item.idempotency_key !== idempotencyKey,
        )
      ) {
        this.store.workrooms.push({
          accepted_outcome_contract_id: this.values[3] as string | null,
          archived_at: null,
          artifact_refs_json: String(this.values[12]),
          assignment_id: this.values[5] as string | null,
          blocker_refs_json: String(this.values[15]),
          classification_caveat_ref: String(this.values[18]),
          created_at: String(this.values[21]),
          customer_intent_ref: String(this.values[9]),
          data_classification: String(this.values[16]),
          email_refs_json: String(this.values[13]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          metadata_json: String(this.values[20]),
          public_receipt_ref: String(this.values[19]),
          receipt_refs_json: String(this.values[14]),
          site_id: this.values[4] as string | null,
          software_order_id: String(this.values[2]),
          source_refs_json: String(this.values[11]),
          status: String(this.values[7]),
          task_packet_ref: this.values[10] as string | null,
          trust_tier: String(this.values[17]),
          updated_at: String(this.values[22]),
          visibility: String(this.values[8]),
          work_kind: String(this.values[6]),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    return Promise.resolve({ results: [] } as unknown as D1Result<T>)
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(options?: {
    columnNames?: boolean
  }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
    return options?.columnNames === true
      ? Promise.resolve([[]])
      : Promise.resolve([])
  }

  private findRef(rows: ReadonlyArray<RefRow>): RefRow | null {
    const id = String(this.values[0])

    return rows.find(item => item.id === id && item.archived_at === null) ?? null
  }
}

const omniWorkroomDb = (store: OmniWorkroomStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new OmniWorkroomStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

type Bindings = Readonly<{ store: OmniWorkroomStore }>

const operatorSession = (
  _request: Request,
  _env: Bindings,
  _ctx: ExecutionContext,
) => Promise.resolve({ user: { userId: 'operator_user_1' } })

const makeRoutes = (
  store: OmniWorkroomStore,
  requireBrowserSession: OmniWorkroomDeps['requireBrowserSession'] = operatorSession,
) =>
  makeOmniWorkroomRoutes<Bindings>({
    db: () => omniWorkroomDb(store),
    nowIso: () => '2026-06-14T12:00:00.000Z',
    requireBrowserSession,
  })

type OmniWorkroomDeps = Parameters<typeof makeOmniWorkroomRoutes<Bindings>>[0]

const ctx = {} as ExecutionContext

const runRequest = (
  routes: ReturnType<typeof makeRoutes>,
  request: Request,
  store: OmniWorkroomStore,
): Promise<Response> => {
  const effect = routes.routeOmniWorkroomRequest(request, { store }, ctx)

  if (effect === undefined) {
    throw new Error(`No route matched ${request.method} ${request.url}`)
  }

  return Effect.runPromise(effect)
}

const createBody = (overrides: Record<string, unknown> = {}) => ({
  customerIntentRef: 'customer_intent_business_1',
  idempotencyKey: 'omni-workroom:business-1',
  softwareOrderId: 'software_order_business_1',
  sourceRefs: ['source_card_business_1'],
  artifactRefs: ['research_brief_business_1'],
  taskPacketRef: 'task_packet_business_1',
  visibility: 'customer',
  workKind: 'business',
  dataClassification: 'customer',
  classificationCaveatRef: 'classification_caveat_reviewed',
  trustTier: 'reviewed',
  ...overrides,
})

const postRequest = (body: Record<string, unknown>): Request =>
  new Request('https://openagents.com/api/omni/workrooms', {
    method: 'POST',
    body: JSON.stringify(body),
    headers: { 'content-type': 'application/json' },
  })

const getRequest = (id: string, surface?: string): Request =>
  new Request(
    `https://openagents.com/api/omni/workrooms/${id}${
      surface === undefined ? '' : `?surface=${surface}`
    }`,
  )

describe('Omni workroom routes', () => {
  test('creates a business workroom and replays idempotently', async () => {
    const store = new OmniWorkroomStore()
    const routes = makeRoutes(store)

    const created = await runRequest(
      routes,
      postRequest(createBody({ id: 'omni_workroom_business_1' })),
      store,
    )
    expect(created.status).toBe(201)
    const createdJson = (await created.json()) as {
      generatedAt: string
      workroom: { surface: string; workroom: Record<string, unknown> }
    }
    expect(createdJson.workroom.surface).toBe('operator')
    expect(createdJson.workroom.workroom).toMatchObject({
      id: 'omni_workroom_business_1',
      softwareOrderId: 'software_order_business_1',
      workKind: 'business',
    })

    const replay = await runRequest(
      routes,
      postRequest(
        createBody({
          id: 'omni_workroom_business_1',
          customerIntentRef: 'customer_intent_changed',
        }),
      ),
      store,
    )
    expect(replay.status).toBe(200)
    expect(store.workrooms).toHaveLength(1)
  })

  test('reads a workroom with audience-scoped projections', async () => {
    const store = new OmniWorkroomStore()
    const routes = makeRoutes(store)

    await runRequest(
      routes,
      postRequest(
        createBody({
          id: 'omni_workroom_business_2',
          idempotencyKey: 'omni-workroom:business-2',
          dataClassification: 'public',
          classificationCaveatRef: 'classification_caveat_reviewed_public',
        }),
      ),
      store,
    )

    const operator = await runRequest(
      routes,
      getRequest('omni_workroom_business_2', 'operator'),
      store,
    )
    expect(operator.status).toBe(200)
    const operatorJson = (await operator.json()) as {
      surface: string
      workroom: { workroom: Record<string, unknown> }
    }
    expect(operatorJson.surface).toBe('operator')
    expect(operatorJson.workroom.workroom).toHaveProperty('sourceRefs')
    expect(operatorJson.workroom.workroom).toHaveProperty('taskPacketRef')
    expect(operatorJson.workroom.workroom).toHaveProperty('id')

    const customer = await runRequest(
      routes,
      getRequest('omni_workroom_business_2', 'customer'),
      store,
    )
    expect(customer.status).toBe(200)
    const customerJson = (await customer.json()) as {
      workroom: { workroom: Record<string, unknown> }
    }
    expect(customerJson.workroom.workroom).not.toHaveProperty('sourceRefs')
    expect(customerJson.workroom.workroom).not.toHaveProperty('taskPacketRef')

    const publicResponse = await runRequest(
      routes,
      getRequest('omni_workroom_business_2', 'public'),
      store,
    )
    expect(publicResponse.status).toBe(200)
    const publicJson = (await publicResponse.json()) as {
      workroom: { workroom: Record<string, unknown> }
    }
    expect(publicJson.workroom.workroom).not.toHaveProperty('customerIntentRef')
    expect(publicJson.workroom.workroom).not.toHaveProperty('artifactRefs')
    expect(publicJson.workroom.workroom).toHaveProperty('publicReceiptRef')
  })

  test('defaults to the public surface when none is provided', async () => {
    const store = new OmniWorkroomStore()
    const routes = makeRoutes(store)

    await runRequest(
      routes,
      postRequest(
        createBody({
          id: 'omni_workroom_business_3',
          idempotencyKey: 'omni-workroom:business-3',
          dataClassification: 'public',
          classificationCaveatRef: 'classification_caveat_reviewed_public',
        }),
      ),
      store,
    )

    const response = await runRequest(
      routes,
      getRequest('omni_workroom_business_3'),
      store,
    )
    const json = (await response.json()) as { surface: string }
    expect(json.surface).toBe('public')
  })

  test('returns 404 for an unknown workroom id', async () => {
    const store = new OmniWorkroomStore()
    const routes = makeRoutes(store)

    const response = await runRequest(
      routes,
      getRequest('omni_workroom_missing', 'public'),
      store,
    )
    expect(response.status).toBe(404)
  })

  test('rejects creation without an operator session', async () => {
    const store = new OmniWorkroomStore()
    const routes = makeRoutes(store, () => Promise.resolve(undefined))

    const response = await runRequest(routes, postRequest(createBody()), store)
    expect(response.status).toBe(401)
    expect(store.workrooms).toHaveLength(0)
  })

  test('requires an operator session for operator and team reads', async () => {
    const store = new OmniWorkroomStore()
    const denyRoutes = makeRoutes(store, () => Promise.resolve(undefined))
    const allowRoutes = makeRoutes(store)

    await runRequest(
      allowRoutes,
      postRequest(
        createBody({
          id: 'omni_workroom_business_4',
          idempotencyKey: 'omni-workroom:business-4',
        }),
      ),
      store,
    )

    const denied = await runRequest(
      denyRoutes,
      getRequest('omni_workroom_business_4', 'operator'),
      store,
    )
    expect(denied.status).toBe(401)
  })

  test('rejects unsafe metadata at the create boundary', async () => {
    const store = new OmniWorkroomStore()
    const routes = makeRoutes(store)

    const response = await runRequest(
      routes,
      postRequest(
        createBody({
          id: 'omni_workroom_business_5',
          idempotencyKey: 'omni-workroom:business-5',
          metadata: { rawEmail: 'someone@example.com' },
        }),
      ),
      store,
    )
    expect(response.status).toBe(400)
    expect(store.workrooms).toHaveLength(0)
  })

  test('reads the INERT source-authority delivery plan for a live workroom', async () => {
    const store = new OmniWorkroomStore()
    const routes = makeRoutes(store)

    await runRequest(
      routes,
      postRequest(
        createBody({
          id: 'omni_workroom_business_sa',
          idempotencyKey: 'omni-workroom:business-sa',
          metadata: {
            sourceAuthority: {
              bindings: [
                {
                  allowedOperations: ['append', 'create', 'update'],
                  allowedSourceKinds: [
                    'approval_decision',
                    'connector_read',
                    'verified_chat_extraction',
                  ],
                  authority: {
                    authorityBoundary: 'contract_projection_only',
                    noBusinessObjectMutationWithoutApproval: true,
                    noConnectorWritebackWithoutApproval: true,
                    noNotificationSend: true,
                    noSettlementImplication: true,
                    noSpendAuthority: true,
                  },
                  businessObjectKinds: ['contact', 'company', 'task'],
                  caveatRefs: ['caveat.source_authority.proposals_until_approved'],
                  createdAtIso: '2026-06-19T05:00:00.000Z',
                  id: 'source_authority_binding.acme_crm_operator',
                  principalKind: 'authorized_user',
                  principalRef: 'principal.workroom_owner',
                  requiresApproval: true,
                  updatedAtIso: '2026-06-19T05:05:00.000Z',
                  workroomRef: 'workroom.acme_delivery',
                },
              ],
              writes: [
                {
                  appliedReceiptRefs: [],
                  approvalRefs: [],
                  authority: {
                    authorityBoundary: 'contract_projection_only',
                    noBusinessObjectMutationWithoutApproval: true,
                    noConnectorWritebackWithoutApproval: true,
                    noNotificationSend: true,
                    noSettlementImplication: true,
                    noSpendAuthority: true,
                  },
                  blockerRefs: [],
                  bindingRef: 'source_authority_binding.acme_crm_operator',
                  businessObjectKind: 'contact',
                  businessObjectRef: 'business_object.contact.acme_primary',
                  caveatRefs: [],
                  closeoutRefs: [],
                  connectorReadReceiptRefs: [],
                  createdAtIso: '2026-06-19T05:10:00.000Z',
                  evidenceRefs: [],
                  id: 'business_object_write.acme_contact_1',
                  operation: 'update',
                  operatorDiagnosticRefs: [],
                  principalKind: 'authorized_user',
                  principalRef: 'principal.workroom_owner',
                  proposedChangeRefs: ['proposed_change.contact.title_updated'],
                  sourceKind: 'verified_chat_extraction',
                  sourceRefs: ['source.workroom.chat_extraction_summary'],
                  state: 'proposed',
                  updatedAtIso: '2026-06-19T05:25:00.000Z',
                  workroomRef: 'workroom.acme_delivery',
                },
              ],
            },
          },
        }),
      ),
      store,
    )

    const response = await runRequest(
      routes,
      new Request(
        'https://openagents.com/api/omni/workrooms/omni_workroom_business_sa/source-authority?surface=operator',
      ),
      store,
    )
    expect(response.status).toBe(200)
    const json = (await response.json()) as {
      sourceAuthorityDelivery: {
        applyableCount: number
        effectsApplied: boolean
        gateState: string
        proposedCount: number
      }
      surface: string
      workroomId: string
    }
    expect(json.surface).toBe('operator')
    expect(json.workroomId).toBe('omni_workroom_business_sa')
    // INERT integration: reachable on the live surface, never applies anything.
    expect(json.sourceAuthorityDelivery.gateState).toBe('inert_disabled')
    expect(json.sourceAuthorityDelivery.effectsApplied).toBe(false)
    expect(json.sourceAuthorityDelivery.applyableCount).toBe(0)
    expect(json.sourceAuthorityDelivery.proposedCount).toBe(1)
  })

  test('requires an operator session for operator source-authority reads', async () => {
    const store = new OmniWorkroomStore()
    const denyRoutes = makeRoutes(store, () => Promise.resolve(undefined))
    const allowRoutes = makeRoutes(store)

    await runRequest(
      allowRoutes,
      postRequest(
        createBody({
          id: 'omni_workroom_business_sa2',
          idempotencyKey: 'omni-workroom:business-sa2',
        }),
      ),
      store,
    )

    const denied = await runRequest(
      denyRoutes,
      new Request(
        'https://openagents.com/api/omni/workrooms/omni_workroom_business_sa2/source-authority?surface=operator',
      ),
      store,
    )
    expect(denied.status).toBe(401)
  })

  test('returns 404 for source-authority on an unknown workroom', async () => {
    const store = new OmniWorkroomStore()
    const routes = makeRoutes(store)

    const response = await runRequest(
      routes,
      new Request(
        'https://openagents.com/api/omni/workrooms/missing_sa/source-authority?surface=public',
      ),
      store,
    )
    expect(response.status).toBe(404)
  })

  test('rejects unsupported HTTP methods', async () => {
    const store = new OmniWorkroomStore()
    const routes = makeRoutes(store)

    const response = await runRequest(
      routes,
      new Request('https://openagents.com/api/omni/workrooms', {
        method: 'DELETE',
      }),
      store,
    )
    expect(response.status).toBe(405)
  })
})
