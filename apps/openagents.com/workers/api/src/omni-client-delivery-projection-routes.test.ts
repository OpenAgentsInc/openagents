import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OMNI_BUSINESS_OBJECT_WRITE_FIXTURE,
  OMNI_SOURCE_AUTHORITY_BINDING_FIXTURE,
} from './omni-source-authorized-business-objects'
import { OmniWorkroomRecord } from './omni-workrooms'
import {
  OMNI_CLIENT_DELIVERY_PROJECTION_BLOCKERS,
  OMNI_CLIENT_DELIVERY_PROJECTION_PROMISE_ID,
  OmniClientDeliveryProjectionEndpoint,
  type OmniClientDeliveryProjectionDeps,
  emptyOmniClientDeliveryProjectionStore,
  handleOmniClientDeliveryProjectionApi,
  isOmniClientDeliveryProjectionEnabled,
  makeInMemoryOmniClientDeliveryProjectionStore,
} from './omni-client-delivery-projection-routes'

const nowIso = '2026-06-19T05:30:00.000Z'

const workroomFixture: OmniWorkroomRecord = {
  acceptedOutcomeContractId: null,
  archivedAt: null,
  artifactRefs: [],
  assignmentId: null,
  blockerRefs: [],
  classificationCaveatRef: 'classification_caveat_unreviewed',
  createdAt: '2026-06-19T05:00:00.000Z',
  customerIntentRef: 'customer_intent.acme_delivery',
  dataClassification: 'customer',
  emailRefs: [],
  id: 'workroom.acme_delivery',
  idempotencyKey: 'idem.acme_delivery',
  metadata: {},
  publicReceiptRef: 'omni_workroom:order:idem',
  receiptRefs: [],
  siteId: null,
  softwareOrderId: 'software_order.acme',
  sourceRefs: [],
  status: 'active',
  taskPacketRef: null,
  trustTier: 'unverified',
  updatedAt: '2026-06-19T05:00:00.000Z',
  visibility: 'customer',
  workKind: 'business',
}

const workroom = (): OmniWorkroomRecord =>
  S.decodeUnknownSync(OmniWorkroomRecord)(workroomFixture)

const armedStore = () =>
  makeInMemoryOmniClientDeliveryProjectionStore([
    {
      bindings: [OMNI_SOURCE_AUTHORITY_BINDING_FIXTURE],
      workroom: workroom(),
      writes: [OMNI_BUSINESS_OBJECT_WRITE_FIXTURE],
    },
  ])

const armedReadyStore = () =>
  makeInMemoryOmniClientDeliveryProjectionStore([
    {
      bindings: [OMNI_SOURCE_AUTHORITY_BINDING_FIXTURE],
      config: {
        closeoutReceiptRef: 'closeout_receipt.acme',
        integrationEnabled: true,
        ownerSignOffRef: 'owner_sign_off.acme',
      },
      workroom: workroom(),
      writes: [OMNI_BUSINESS_OBJECT_WRITE_FIXTURE],
    },
  ])

const deps = (
  override: Partial<OmniClientDeliveryProjectionDeps> = {},
): OmniClientDeliveryProjectionDeps => ({
  enabled: false,
  nowIso: () => nowIso,
  ...override,
})

const get = (url = 'https://example.com' + OmniClientDeliveryProjectionEndpoint) =>
  new Request(url, { method: 'GET' })

describe('Omni client-delivery projection endpoint (INERT)', () => {
  test('flag parser only arms on truthy values', () => {
    for (const on of ['1', 'true', 'TRUE', 'yes', 'on', ' On ']) {
      expect(isOmniClientDeliveryProjectionEnabled(on)).toBe(true)
    }
    for (const off of [undefined, '', '0', 'false', 'no', 'off', 'maybe']) {
      expect(isOmniClientDeliveryProjectionEnabled(off)).toBe(false)
    }
  })

  test('disabled: inert, empty, no projection, promise stays yellow', async () => {
    const res = handleOmniClientDeliveryProjectionApi(
      get(),
      deps({ enabled: false, store: armedStore() }),
    )
    expect(res.status).toBe(200)
    expect(res.headers.get('cache-control')).toBe('no-store')
    const body = (await res.json()) as Record<string, unknown>
    expect(body.promiseId).toBe(OMNI_CLIENT_DELIVERY_PROJECTION_PROMISE_ID)
    expect(body.promiseState).toBe('yellow')
    expect(body.enabled).toBe(false)
    expect(body.inert).toBe(true)
    expect(body.effectsApplied).toBe(false)
    expect(body.workroomCount).toBe(0)
    expect(body.proposedWriteCount).toBe(0)
    expect(body.applyableWriteCount).toBe(0)
    expect(body.plans).toEqual([])
    expect(body.blockerCleared).toBe(
      OMNI_CLIENT_DELIVERY_PROJECTION_BLOCKERS.cleared,
    )
  })

  test('enabled but no store: still inert/empty', async () => {
    const res = handleOmniClientDeliveryProjectionApi(
      get(),
      deps({ enabled: true }),
    )
    const body = (await res.json()) as Record<string, unknown>
    expect(body.enabled).toBe(true)
    expect(body.inert).toBe(false)
    expect(body.workroomCount).toBe(0)
    expect(body.plans).toEqual([])
  })

  test('enabled with armed store: projects the delivery plan, holding writes until owner-gated config is ready', async () => {
    const res = handleOmniClientDeliveryProjectionApi(
      get(),
      deps({ enabled: true, store: armedStore() }),
    )
    expect(res.status).toBe(200)
    const body = (await res.json()) as Record<string, unknown>
    expect(body.enabled).toBe(true)
    expect(body.inert).toBe(false)
    expect(body.effectsApplied).toBe(false)
    expect(body.applyableWriteCount).toBe(0)
    expect(body.workroomCount).toBe(1)
    expect(body.proposedWriteCount).toBe(1)
    const plans = body.plans as ReadonlyArray<Record<string, unknown>>
    expect(plans).toHaveLength(1)
    expect(plans[0]?.effectsApplied).toBe(false)
    expect(plans[0]?.gateState).toBe('inert_disabled')
    expect(plans[0]?.applyableCount).toBe(0)
    expect(plans[0]?.workroomId).toBe('workroom.acme_delivery')
  })

  test('enabled with owner-gated ready store: applies approved business-object writes with receipts', async () => {
    const res = handleOmniClientDeliveryProjectionApi(
      get(),
      deps({ enabled: true, store: armedReadyStore() }),
    )
    expect(res.status).toBe(200)
    const body = (await res.json()) as Record<string, unknown>
    expect(body.enabled).toBe(true)
    expect(body.inert).toBe(false)
    expect(body.effectsApplied).toBe(true)
    expect(body.writeMutationAllowed).toBe(true)
    expect(body.connectorWriteAllowed).toBe(false)
    expect(body.settlementAllowed).toBe(false)
    expect(body.publicClaimUpgradeAllowed).toBe(false)
    expect(body.applyableWriteCount).toBe(1)
    const plans = body.plans as ReadonlyArray<Record<string, unknown>>
    expect(plans[0]?.effectsApplied).toBe(true)
    expect(plans[0]?.gateState).toBe('enabled_ready')
    expect(plans[0]?.applyableCount).toBe(1)
  })

  test('empty store factory yields no workrooms', () => {
    expect(emptyOmniClientDeliveryProjectionStore.listWorkrooms()).toEqual([])
  })

  test('invalid audience -> 400', async () => {
    const res = handleOmniClientDeliveryProjectionApi(
      get(
        'https://example.com' +
          OmniClientDeliveryProjectionEndpoint +
          '?audience=bogus',
      ),
      deps({ enabled: true, store: armedStore() }),
    )
    expect(res.status).toBe(400)
  })

  test('valid audience query is honored', async () => {
    const res = handleOmniClientDeliveryProjectionApi(
      get(
        'https://example.com' +
          OmniClientDeliveryProjectionEndpoint +
          '?audience=operator',
      ),
      deps({ enabled: true, store: armedStore() }),
    )
    const body = (await res.json()) as Record<string, unknown>
    expect(body.audience).toBe('operator')
  })

  test('non-GET -> 405', () => {
    const res = handleOmniClientDeliveryProjectionApi(
      new Request(
        'https://example.com' + OmniClientDeliveryProjectionEndpoint,
        { method: 'POST' },
      ),
      deps({ enabled: true, store: armedStore() }),
    )
    expect(res.status).toBe(405)
    expect(res.headers.get('allow')).toBe('GET')
  })
})
