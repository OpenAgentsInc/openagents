import { describe, expect, test } from 'vitest'

import {
  ArtanisLaborReceiptError,
  buildArtanisLaborUnattendedRequestReceipt,
  deriveArtanisLaborUnattendedRequestReceiptRef,
  serializeArtanisLaborUnattendedRequestReceipt,
} from './artanis-labor-request-receipt'
import type {
  ArtanisLaborAcceptanceOutcome,
  ArtanisLaborRequesterOutcome,
} from './artanis-labor-requester'

const requestedOutcome: ArtanisLaborRequesterOutcome = {
  budgetMsat: 2_000_000,
  kind: 'requested',
  receipt: {
    jobEventId: 'a'.repeat(64),
    topicId: 'topic_1',
    workRequestId: 'work_request_1',
  },
  reserveReceiptRef: 'receipt.labor_escrow.reserve.artanis_1',
}

const base = {
  artanisActorRef: 'agent:artanis',
  nowIso: '2026-06-20T12:00:00.000Z',
  tickRef: 'tick.public.artanis.2026-06-20T12:00',
}

describe('artanis unattended labor request receipt', () => {
  test('skipped tick projects a config-disabled terminal receipt with no spend', () => {
    const receipt = buildArtanisLaborUnattendedRequestReceipt({
      ...base,
      requestOutcome: { kind: 'skipped', reason: 'config_disabled' },
    })
    expect(receipt).toEqual({
      artanisActorRef: base.artanisActorRef,
      tickRef: base.tickRef,
      budgetMsat: null,
      issuedAtIso: base.nowIso,
      lifecycleRefs: ['stage.artanis_labor_request.skipped.config_disabled'],
      schema: 'artanis.labor.unattended_request_receipt.v1',
      terminalState: 'skipped_config_disabled',
      workRequestId: null,
    })
  })

  test('refused tick cites the refusal ref and no work request', () => {
    const receipt = buildArtanisLaborUnattendedRequestReceipt({
      ...base,
      requestOutcome: {
        kind: 'refused',
        reason: 'per_tick_labor_budget_exceeded',
        refusalRef: 'refusal.artanis_labor_request.per_tick_labor_budget_exceeded',
      },
    })
    expect(receipt.terminalState).toBe('refused')
    expect(receipt.workRequestId).toBeNull()
    expect(receipt.lifecycleRefs).toContain(
      'refusal.artanis_labor_request.per_tick_labor_budget_exceeded',
    )
  })

  test('requested-without-delivery is pending and cites the reserve receipt', () => {
    const receipt = buildArtanisLaborUnattendedRequestReceipt({
      ...base,
      requestOutcome: requestedOutcome,
    })
    expect(receipt.terminalState).toBe('requested_pending_delivery')
    expect(receipt.budgetMsat).toBe(2_000_000)
    expect(receipt.workRequestId).toBe('work_request_1')
    expect(receipt.lifecycleRefs).toEqual([
      'stage.artanis_labor_request.proposed',
      'work_request.public.work_request_1',
      `nostr.event.${'a'.repeat(64)}`,
      'receipt.labor_escrow.reserve.artanis_1',
    ])
  })

  test('accepted delivery folds the release receipt into the lifecycle', () => {
    const accepted: ArtanisLaborAcceptanceOutcome = {
      kind: 'accepted',
      releaseReceiptRef: 'receipt.labor_escrow.release.artanis_1',
    }
    const receipt = buildArtanisLaborUnattendedRequestReceipt({
      ...base,
      acceptanceOutcome: accepted,
      requestOutcome: requestedOutcome,
    })
    expect(receipt.terminalState).toBe('accepted_released')
    expect(receipt.lifecycleRefs).toContain('receipt.labor_escrow.release.artanis_1')
    expect(receipt.lifecycleRefs).toContain('stage.artanis_labor_request.accepted')
  })

  test('failing validator verdict folds the refund and reason into the lifecycle', () => {
    const refunded: ArtanisLaborAcceptanceOutcome = {
      kind: 'rejected_refunded',
      reasonRef: 'verifier.public.artanis_labor.bun_test.failed',
      refundReceiptRef: 'receipt.labor_escrow.refund.artanis_1',
    }
    const receipt = buildArtanisLaborUnattendedRequestReceipt({
      ...base,
      acceptanceOutcome: refunded,
      requestOutcome: requestedOutcome,
    })
    expect(receipt.terminalState).toBe('rejected_refunded')
    expect(receipt.lifecycleRefs).toEqual(
      expect.arrayContaining([
        'verifier.public.artanis_labor.bun_test.failed',
        'receipt.labor_escrow.refund.artanis_1',
      ]),
    )
  })

  test('an acceptance outcome without a requested outcome is rejected', () => {
    expect(() =>
      buildArtanisLaborUnattendedRequestReceipt({
        ...base,
        acceptanceOutcome: {
          kind: 'accepted',
          releaseReceiptRef: 'receipt.labor_escrow.release.artanis_1',
        },
        requestOutcome: { kind: 'skipped', reason: 'config_disabled' },
      }),
    ).toThrow(ArtanisLaborReceiptError)
  })

  test('empty tick ref is rejected', () => {
    expect(() =>
      buildArtanisLaborUnattendedRequestReceipt({
        ...base,
        requestOutcome: requestedOutcome,
        tickRef: '   ',
      }),
    ).toThrow(ArtanisLaborReceiptError)
  })

  test('serialization is canonical: key order is fixed regardless of input', () => {
    const receipt = buildArtanisLaborUnattendedRequestReceipt({
      ...base,
      requestOutcome: requestedOutcome,
    })
    expect(serializeArtanisLaborUnattendedRequestReceipt(receipt)).toBe(
      JSON.stringify({
        artanisActorRef: base.artanisActorRef,
        budgetMsat: 2_000_000,
        issuedAtIso: base.nowIso,
        lifecycleRefs: [
          'stage.artanis_labor_request.proposed',
          'work_request.public.work_request_1',
          `nostr.event.${'a'.repeat(64)}`,
          'receipt.labor_escrow.reserve.artanis_1',
        ],
        schema: 'artanis.labor.unattended_request_receipt.v1',
        terminalState: 'requested_pending_delivery',
        tickRef: base.tickRef,
        workRequestId: 'work_request_1',
      }),
    )
  })

  test('the derived ref is content-addressed and stable across builds', () => {
    const first = buildArtanisLaborUnattendedRequestReceipt({
      ...base,
      requestOutcome: requestedOutcome,
    })
    const second = buildArtanisLaborUnattendedRequestReceipt({
      ...base,
      requestOutcome: requestedOutcome,
    })
    const ref = deriveArtanisLaborUnattendedRequestReceiptRef(first)
    expect(ref).toMatch(
      /^receipt\.artanis_labor\.unattended_request\.[a-f0-9]{16}$/,
    )
    expect(deriveArtanisLaborUnattendedRequestReceiptRef(second)).toBe(ref)
  })

  test('distinct terminal states derive distinct refs', () => {
    const pending = deriveArtanisLaborUnattendedRequestReceiptRef(
      buildArtanisLaborUnattendedRequestReceipt({
        ...base,
        requestOutcome: requestedOutcome,
      }),
    )
    const released = deriveArtanisLaborUnattendedRequestReceiptRef(
      buildArtanisLaborUnattendedRequestReceipt({
        ...base,
        acceptanceOutcome: {
          kind: 'accepted',
          releaseReceiptRef: 'receipt.labor_escrow.release.artanis_1',
        },
        requestOutcome: requestedOutcome,
      }),
    )
    expect(pending).not.toBe(released)
  })

  test('a non-public-safe ref anywhere is refused', () => {
    expect(() =>
      buildArtanisLaborUnattendedRequestReceipt({
        ...base,
        requestOutcome: {
          kind: 'refused',
          reason: 'schema_invalid',
          refusalRef: 'refusal.artanis_labor_request.ghp_deadbeefdeadbeef',
        },
      }),
    ).toThrow()
  })
})
