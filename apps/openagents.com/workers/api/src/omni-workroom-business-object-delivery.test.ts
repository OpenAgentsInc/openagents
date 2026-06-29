import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import { OmniWorkroomRecord } from './omni-workrooms'
import {
  OMNI_BUSINESS_OBJECT_WRITE_FIXTURE,
  OMNI_SOURCE_AUTHORITY_BINDING_FIXTURE,
} from './omni-source-authorized-business-objects'
import {
  OmniBusinessObjectDeliveryPlan,
  buildOmniBusinessObjectDeliveryPlan,
  buildOmniWorkroomSourceAuthorityDeliveryPlan,
  extractWorkroomSourceAuthorityInputs,
  resolveOmniBusinessObjectDeliveryGate,
} from './omni-workroom-business-object-delivery'

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

const buildPlan = (
  config?: Parameters<typeof buildOmniBusinessObjectDeliveryPlan>[0]['config'],
) =>
  buildOmniBusinessObjectDeliveryPlan({
    audience: 'operator',
    bindings: [OMNI_SOURCE_AUTHORITY_BINDING_FIXTURE],
    config,
    nowIso,
    workroom: workroom(),
    writes: [OMNI_BUSINESS_OBJECT_WRITE_FIXTURE],
  })

describe('Omni workroom business-object delivery integration', () => {
  test('gate resolves inert_disabled by default', () => {
    expect(resolveOmniBusinessObjectDeliveryGate({})).toBe('inert_disabled')
    expect(
      resolveOmniBusinessObjectDeliveryGate({ integrationEnabled: false }),
    ).toBe('inert_disabled')
  })

  test('gate resolves enabled_blocked without owner sign-off + closeout', () => {
    expect(
      resolveOmniBusinessObjectDeliveryGate({ integrationEnabled: true }),
    ).toBe('enabled_blocked')
    expect(
      resolveOmniBusinessObjectDeliveryGate({
        integrationEnabled: true,
        ownerSignOffRef: 'owner_sign_off.acme',
      }),
    ).toBe('enabled_blocked')
  })

  test('gate resolves enabled_ready only with flag + owner sign-off + closeout', () => {
    expect(
      resolveOmniBusinessObjectDeliveryGate({
        closeoutReceiptRef: 'closeout_receipt.acme',
        integrationEnabled: true,
        ownerSignOffRef: 'owner_sign_off.acme',
      }),
    ).toBe('enabled_ready')
  })

  test('default plan is inert: nothing applyable, no effects applied', () => {
    const plan = buildPlan()

    expect(S.decodeUnknownSync(OmniBusinessObjectDeliveryPlan)(plan)).toEqual(
      plan,
    )
    expect(plan.gateState).toBe('inert_disabled')
    expect(plan.effectsApplied).toBe(false)
    expect(plan.applyableCount).toBe(0)
    expect(plan.proposedCount).toBe(1)
    expect(plan.blockerRefs).toContain(
      'blocker.business_object_delivery.integration_inert_disabled',
    )
    expect(plan.entries[0]?.applyAllowed).toBe(false)
    expect(plan.entries[0]?.reasonRef).toBe(
      'reason.business_object_delivery.held_inert',
    )
  })

  test('enabled_blocked plan stays inert with gate blockers', () => {
    const plan = buildPlan({ integrationEnabled: true })

    expect(plan.gateState).toBe('enabled_blocked')
    expect(plan.effectsApplied).toBe(false)
    expect(plan.applyableCount).toBe(0)
    expect(plan.blockerRefs).toContain(
      'blocker.business_object_delivery.owner_sign_off_missing',
    )
    expect(plan.blockerRefs).toContain(
      'blocker.business_object_delivery.closeout_receipt_missing',
    )
    expect(plan.entries[0]?.applyAllowed).toBe(false)
  })

  test('enabled_ready plan applies approved writes with receipts', () => {
    const plan = buildPlan({
      closeoutReceiptRef: 'closeout_receipt.acme',
      integrationEnabled: true,
      ownerSignOffRef: 'owner_sign_off.acme',
    })

    expect(plan.gateState).toBe('enabled_ready')
    expect(plan.effectsApplied).toBe(true)
    expect(plan.blockerRefs).toEqual([])
    expect(plan.applyableCount).toBe(1)
    expect(plan.entries[0]?.applyAllowed).toBe(true)
    expect(plan.entries[0]?.appliedReceiptRefs).toEqual([
      'closeout_receipt.acme',
      'receipt.business_object_write.contact_updated',
    ])
    expect(plan.entries[0]?.closeoutReceiptRefs).toEqual([
      'closeout.business_object_write.contact_updated',
      'closeout_receipt.acme',
    ])
    expect(plan.entries[0]?.appliedBusinessObject).toMatchObject({
      businessObjectKind: 'contact',
      businessObjectRef: 'business_object.contact.acme_primary',
      operation: 'update',
      sourceRefs: ['source.workroom.chat_extraction_summary'],
      writeRef: 'business_object_write.acme_contact_1',
    })
    expect(plan.entries[0]?.reasonRef).toBe(
      'reason.source_authority.approved_write_applyable',
    )
  })

  test('extracts source-authority inputs from live workroom metadata', () => {
    const inputs = extractWorkroomSourceAuthorityInputs({
      ...workroom(),
      metadata: {
        sourceAuthority: {
          bindings: [OMNI_SOURCE_AUTHORITY_BINDING_FIXTURE],
          writes: [OMNI_BUSINESS_OBJECT_WRITE_FIXTURE],
        },
      },
    })

    expect(inputs.bindings).toHaveLength(1)
    expect(inputs.config).toBeUndefined()
    expect(inputs.writes).toHaveLength(1)
    expect(inputs.bindings[0]?.id).toBe(
      OMNI_SOURCE_AUTHORITY_BINDING_FIXTURE.id,
    )
  })

  test('extracts empty inputs when no source-authority metadata block exists', () => {
    expect(extractWorkroomSourceAuthorityInputs(workroom())).toMatchObject({
      bindings: [],
      writes: [],
    })
  })

  test('skips malformed metadata entries without throwing', () => {
    const inputs = extractWorkroomSourceAuthorityInputs({
      ...workroom(),
      metadata: {
        sourceAuthority: {
          bindings: [{ not: 'a binding' }, OMNI_SOURCE_AUTHORITY_BINDING_FIXTURE],
          writes: 'not-an-array',
        },
      },
    })

    expect(inputs.bindings).toHaveLength(1)
    expect(inputs.writes).toEqual([])
  })

  test('record-level builder is inert by default from live metadata', () => {
    const plan = buildOmniWorkroomSourceAuthorityDeliveryPlan({
      audience: 'operator',
      nowIso,
      workroom: {
        ...workroom(),
        metadata: {
          sourceAuthority: {
            bindings: [OMNI_SOURCE_AUTHORITY_BINDING_FIXTURE],
            writes: [OMNI_BUSINESS_OBJECT_WRITE_FIXTURE],
          },
        },
      },
    })

    expect(S.decodeUnknownSync(OmniBusinessObjectDeliveryPlan)(plan)).toEqual(
      plan,
    )
    expect(plan.gateState).toBe('inert_disabled')
    expect(plan.effectsApplied).toBe(false)
    expect(plan.applyableCount).toBe(0)
    expect(plan.proposedCount).toBe(1)
    expect(plan.blockerRefs).toContain(
      'blocker.business_object_delivery.integration_inert_disabled',
    )
  })

  test('a write naming an unknown binding is held with binding_not_found', () => {
    const plan = buildOmniBusinessObjectDeliveryPlan({
      audience: 'operator',
      bindings: [],
      config: {
        closeoutReceiptRef: 'closeout_receipt.acme',
        integrationEnabled: true,
        ownerSignOffRef: 'owner_sign_off.acme',
      },
      nowIso,
      workroom: workroom(),
      writes: [OMNI_BUSINESS_OBJECT_WRITE_FIXTURE],
    })

    expect(plan.applyableCount).toBe(0)
    expect(plan.entries[0]?.blockerRefs).toContain(
      'blocker.business_object_delivery.binding_not_found',
    )
  })

  test('source-less writes are denied instead of applied', () => {
    const plan = buildOmniBusinessObjectDeliveryPlan({
      audience: 'operator',
      bindings: [OMNI_SOURCE_AUTHORITY_BINDING_FIXTURE],
      config: {
        closeoutReceiptRef: 'closeout_receipt.acme',
        integrationEnabled: true,
        ownerSignOffRef: 'owner_sign_off.acme',
      },
      nowIso,
      workroom: workroom(),
      writes: [{ ...OMNI_BUSINESS_OBJECT_WRITE_FIXTURE, sourceRefs: [] }],
    })

    expect(plan.effectsApplied).toBe(false)
    expect(plan.applyableCount).toBe(0)
    expect(plan.entries[0]?.applyAllowed).toBe(false)
    expect(plan.entries[0]?.blockerRefs).toContain(
      'blocker.business_object_delivery.write_unsafe',
    )
    expect(plan.entries[0]?.appliedBusinessObject).toBeUndefined()
  })

  test('record-level builder applies approved metadata writes when owner-gated config is present', () => {
    const plan = buildOmniWorkroomSourceAuthorityDeliveryPlan({
      audience: 'operator',
      nowIso,
      workroom: {
        ...workroom(),
        metadata: {
          sourceAuthority: {
            bindings: [OMNI_SOURCE_AUTHORITY_BINDING_FIXTURE],
            config: {
              closeoutReceiptRef: 'closeout_receipt.acme',
              integrationEnabled: true,
              ownerSignOffRef: 'owner_sign_off.acme',
            },
            writes: [OMNI_BUSINESS_OBJECT_WRITE_FIXTURE],
          },
        },
      },
    })

    expect(plan.gateState).toBe('enabled_ready')
    expect(plan.effectsApplied).toBe(true)
    expect(plan.applyableCount).toBe(1)
    expect(plan.entries[0]?.closeoutReceiptRefs).toContain(
      'closeout_receipt.acme',
    )
  })
})
