import { describe, expect, test } from 'vitest'

import {
  FailedLoadWorkroomSurface,
  RequestedLoadWorkroomLifecycle,
  RequestedLoadWorkroomSurface,
  SelectedWorkroomTab,
  SubmittedWorkroomLifecycleDecision,
  SucceededLoadWorkroomLifecycle,
  SucceededLoadWorkroomSurface,
  SucceededWorkroomLifecycleDecision,
  init,
  lifecycleDecisionRequestInfo,
  surfaceRequestInfo,
  update,
} from './workroom'
import type { OmniWorkroomSurfaceResponse } from './workroom'

const WORKROOM_ID = 'omni-workroom-001'

const loadedSurfaceResponse: OmniWorkroomSurfaceResponse = {
  evidenceBundles: [
    {
      artifactRefs: ['artifact:site-build'],
      id: 'bundle-1',
      publicReceiptRef: 'receipt:public:1',
      status: 'ready',
      workKind: 'software_delivery',
    },
  ],
  generatedAt: '2026-06-14T00:00:00.000Z',
  lifecycleDecisions: [],
  surface: 'customer',
  workroom: {
    artifactRefs: ['artifact:site-build'],
    blockerRefs: [],
    customerIntentRef: 'intent:landing-page',
    dataClassification: 'customer_business',
    publicReceiptRef: 'receipt:public:1',
    receiptRefs: ['receipt:public:1'],
    siteId: 'site-123',
    softwareOrderId: 'order-42',
    sourceRefs: ['source:brief'],
    status: 'delivered',
    trustTier: 'standard',
    workKind: 'software_delivery',
  },
}

describe('workroom page submodel', () => {
  test('init starts idle on the overview tab', () => {
    const model = init(WORKROOM_ID, 'overview')

    expect(model.workroomId).toBe(WORKROOM_ID)
    expect(model.activeTab).toBe('overview')
    expect(model.surface._tag).toBe('WorkroomSurfaceIdle')
    expect(model.lifecycle._tag).toBe('WorkroomLifecycleIdle')
    expect(model.decisionAct._tag).toBe('WorkroomDecisionActIdle')
  })

  test('SelectedWorkroomTab switches tabs without emitting a command', () => {
    const model = init(WORKROOM_ID, 'overview')

    const [next, cmd] = update(model, SelectedWorkroomTab({ tab: 'approvals' }))

    expect(next.activeTab).toBe('approvals')
    expect(cmd._tag).toBe('None')
  })

  test('RequestedLoadWorkroomSurface moves to loading and emits LoadSurface', () => {
    const model = init(WORKROOM_ID, 'overview')

    const [next, cmd] = update(model, RequestedLoadWorkroomSurface())

    expect(next.surface._tag).toBe('WorkroomSurfaceLoading')
    expect(next.lifecycle._tag).toBe('WorkroomLifecycleLoading')
    expect(cmd).toEqual({ _tag: 'LoadSurface', workroomId: WORKROOM_ID })
  })

  test('SucceededLoadWorkroomSurface loads the projection', () => {
    const model = init(WORKROOM_ID, 'overview')

    const [next, cmd] = update(
      model,
      SucceededLoadWorkroomSurface({ response: loadedSurfaceResponse }),
    )

    expect(next.surface._tag).toBe('WorkroomSurfaceLoaded')
    if (next.surface._tag === 'WorkroomSurfaceLoaded') {
      expect(next.surface.response.workroom.softwareOrderId).toBe('order-42')
    }
    expect(cmd._tag).toBe('None')
  })

  test('SucceededLoadWorkroomLifecycle with empty history loads cleanly', () => {
    const model = init(WORKROOM_ID, 'approvals')

    const [next] = update(
      model,
      SucceededLoadWorkroomLifecycle({
        response: { audience: 'customer', decisions: [] },
      }),
    )

    expect(next.lifecycle._tag).toBe('WorkroomLifecycleLoaded')
    if (next.lifecycle._tag === 'WorkroomLifecycleLoaded') {
      expect(next.lifecycle.response.decisions).toHaveLength(0)
    }
  })

  test('FailedLoadWorkroomSurface records the error', () => {
    const model = init(WORKROOM_ID, 'overview')

    const [next] = update(
      model,
      FailedLoadWorkroomSurface({ error: 'Network error' }),
    )

    expect(next.surface._tag).toBe('WorkroomSurfaceFailed')
    if (next.surface._tag === 'WorkroomSurfaceFailed') {
      expect(next.surface.error).toBe('Network error')
    }
  })

  test('RequestedLoadWorkroomLifecycle emits a LoadLifecycle command', () => {
    const model = init(WORKROOM_ID, 'approvals')

    const [next, cmd] = update(model, RequestedLoadWorkroomLifecycle())

    expect(next.lifecycle._tag).toBe('WorkroomLifecycleLoading')
    expect(cmd).toEqual({ _tag: 'LoadLifecycle', workroomId: WORKROOM_ID })
  })

  test('SubmittedWorkroomLifecycleDecision submits with a stable idempotency key', () => {
    const model = init(WORKROOM_ID, 'approvals')

    const [next, cmd] = update(
      model,
      SubmittedWorkroomLifecycleDecision({
        customerSafeExplanationRef: 'explanation.accept',
        decisionKind: 'accept',
        receiptRef: 'receipt:public:1',
        workKind: 'software_delivery',
      }),
    )

    expect(next.decisionAct._tag).toBe('WorkroomDecisionActSubmitting')
    if (next.decisionAct._tag === 'WorkroomDecisionActSubmitting') {
      expect(next.decisionAct.decisionKind).toBe('accept')
    }
    expect(cmd._tag).toBe('SubmitLifecycleDecision')
    if (cmd._tag === 'SubmitLifecycleDecision') {
      expect(cmd.idempotencyKey).toBe(
        `browser-omni-lifecycle:${WORKROOM_ID}:accept`,
      )
      expect(cmd.decisionKind).toBe('accept')
    }
  })

  test('SucceededWorkroomLifecycleDecision records success and reloads history', () => {
    const model = init(WORKROOM_ID, 'approvals')

    const [next, cmd] = update(
      model,
      SucceededWorkroomLifecycleDecision({
        response: {
          decision: {
            customerSafeExplanationRef: 'explanation.accept',
            decisionKind: 'accept',
            receiptRef: 'receipt:public:1',
            resultingState: 'accepted',
            workKind: 'software_delivery',
            workroomId: WORKROOM_ID,
          },
          directEffectPermitted: false,
        },
      }),
    )

    expect(next.decisionAct._tag).toBe('WorkroomDecisionActSucceeded')
    expect(next.lifecycle._tag).toBe('WorkroomLifecycleLoading')
    expect(cmd).toEqual({ _tag: 'LoadLifecycle', workroomId: WORKROOM_ID })
  })

  test('surfaceRequestInfo targets the customer surface', () => {
    const info = surfaceRequestInfo(WORKROOM_ID)

    expect(info.request).toBe(
      `/api/omni/workrooms/${WORKROOM_ID}?surface=customer`,
    )
    expect(info.init.method).toBeUndefined()
  })

  test('lifecycleDecisionRequestInfo POSTs with an Idempotency-Key header', () => {
    const info = lifecycleDecisionRequestInfo({
      customerSafeExplanationRef: 'explanation.accept',
      decisionKind: 'accept',
      idempotencyKey: 'browser-omni-lifecycle:omni-workroom-001:accept',
      receiptRef: 'receipt:public:1',
      workKind: 'software_delivery',
      workroomId: WORKROOM_ID,
    })

    expect(info.request).toBe(
      `/api/omni/workrooms/${WORKROOM_ID}/lifecycle-decisions`,
    )
    expect(info.init.method).toBe('POST')
    const headers = info.init.headers as Record<string, string>
    expect(headers['idempotency-key']).toBe(
      'browser-omni-lifecycle:omni-workroom-001:accept',
    )
    expect(headers['content-type']).toBe('application/json')
  })
})
