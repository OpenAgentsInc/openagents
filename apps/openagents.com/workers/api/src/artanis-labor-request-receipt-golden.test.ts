import { describe, expect, test } from 'vitest'

import {
  buildArtanisLaborUnattendedRequestReceipt,
  deriveArtanisLaborUnattendedRequestReceiptRef,
  parseArtanisLaborUnattendedRequestReceipt,
  serializeArtanisLaborUnattendedRequestReceipt,
  verifyArtanisLaborUnattendedRequestReceipt,
  type ArtanisLaborReceiptTerminalState,
  type ArtanisLaborRequestReceiptInput,
} from './artanis-labor-request-receipt'

// Golden wire-format + content-address regression vectors for the consolidated
// Artanis unattended labor request receipt (#4731, blocker
// artanis_labor_unattended_request_receipts_missing).
//
// Why this exists separately from the behavioural test:
// the existing suite proves the ref is DETERMINISTIC (same input -> same ref)
// and DIVERGENT (different inputs -> different refs), but it recomputes both
// sides of every comparison, so a refactor of `serialize`/`derive` (changed key
// order, spacing, field rename, digest length) would change every persisted
// ref while ALL of those tests still pass. That is the dangerous case for a
// CONTENT-ADDRESSED, DURABLE store: receipts already written to D1 are keyed by
// their ref, and `get`/`list` re-verify the stored bytes still address that ref.
// A silent format change would make every previously-persisted receipt fail its
// tamper-evident read and become un-dereferenceable from the public feed.
//
// These vectors freeze the exact bytes and ref for each terminal state against
// fixed inputs. If you change the wire format on purpose, you must re-bless these
// values AND ship a migration/version bump for already-stored receipts; this test
// makes that decision explicit instead of silent.

const base = {
  artanisActorRef: 'agent:artanis',
  nowIso: '2026-06-20T12:00:00.000Z',
  tickRef: 'tick.public.artanis.2026-06-20T12:00',
}

const requestedOutcome = {
  budgetMsat: 2_000_000,
  kind: 'requested',
  receipt: {
    jobEventId: 'a'.repeat(64),
    topicId: 'topic_1',
    workRequestId: 'work_request_1',
  },
  reserveReceiptRef: 'receipt.labor_escrow.reserve.artanis_1',
} as const

type GoldenVector = Readonly<{
  terminalState: ArtanisLaborReceiptTerminalState
  input: ArtanisLaborRequestReceiptInput
  ref: string
  wire: string
}>

const GOLDEN_VECTORS: ReadonlyArray<GoldenVector> = [
  {
    terminalState: 'skipped_config_disabled',
    input: { ...base, requestOutcome: { kind: 'skipped', reason: 'config_disabled' } },
    ref: 'receipt.artanis_labor.unattended_request.be462eda9412ec1f',
    wire: '{"artanisActorRef":"agent:artanis","budgetMsat":null,"issuedAtIso":"2026-06-20T12:00:00.000Z","lifecycleRefs":["stage.artanis_labor_request.skipped.config_disabled"],"schema":"artanis.labor.unattended_request_receipt.v1","terminalState":"skipped_config_disabled","tickRef":"tick.public.artanis.2026-06-20T12:00","workRequestId":null}',
  },
  {
    terminalState: 'refused',
    input: {
      ...base,
      requestOutcome: {
        kind: 'refused',
        reason: 'per_tick_labor_budget_exceeded',
        refusalRef: 'refusal.artanis_labor_request.per_tick_labor_budget_exceeded',
      },
    },
    ref: 'receipt.artanis_labor.unattended_request.cea28fbd76304adf',
    wire: '{"artanisActorRef":"agent:artanis","budgetMsat":null,"issuedAtIso":"2026-06-20T12:00:00.000Z","lifecycleRefs":["stage.artanis_labor_request.refused","refusal.artanis_labor_request.per_tick_labor_budget_exceeded"],"schema":"artanis.labor.unattended_request_receipt.v1","terminalState":"refused","tickRef":"tick.public.artanis.2026-06-20T12:00","workRequestId":null}',
  },
  {
    terminalState: 'requested_pending_delivery',
    input: { ...base, requestOutcome: requestedOutcome },
    ref: 'receipt.artanis_labor.unattended_request.6e32a6efd0a8a496',
    wire: '{"artanisActorRef":"agent:artanis","budgetMsat":2000000,"issuedAtIso":"2026-06-20T12:00:00.000Z","lifecycleRefs":["stage.artanis_labor_request.proposed","work_request.public.work_request_1","nostr.event.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","receipt.labor_escrow.reserve.artanis_1"],"schema":"artanis.labor.unattended_request_receipt.v1","terminalState":"requested_pending_delivery","tickRef":"tick.public.artanis.2026-06-20T12:00","workRequestId":"work_request_1"}',
  },
  {
    terminalState: 'accepted_released',
    input: {
      ...base,
      acceptanceOutcome: {
        kind: 'accepted',
        releaseReceiptRef: 'receipt.labor_escrow.release.artanis_1',
      },
      requestOutcome: requestedOutcome,
    },
    ref: 'receipt.artanis_labor.unattended_request.c3d9b5e2ee285648',
    wire: '{"artanisActorRef":"agent:artanis","budgetMsat":2000000,"issuedAtIso":"2026-06-20T12:00:00.000Z","lifecycleRefs":["stage.artanis_labor_request.proposed","work_request.public.work_request_1","nostr.event.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","receipt.labor_escrow.reserve.artanis_1","stage.artanis_labor_request.accepted","receipt.labor_escrow.release.artanis_1"],"schema":"artanis.labor.unattended_request_receipt.v1","terminalState":"accepted_released","tickRef":"tick.public.artanis.2026-06-20T12:00","workRequestId":"work_request_1"}',
  },
  {
    terminalState: 'rejected_refunded',
    input: {
      ...base,
      acceptanceOutcome: {
        kind: 'rejected_refunded',
        reasonRef: 'verifier.public.artanis_labor.bun_test.failed',
        refundReceiptRef: 'receipt.labor_escrow.refund.artanis_1',
      },
      requestOutcome: requestedOutcome,
    },
    ref: 'receipt.artanis_labor.unattended_request.c989dfb529a88912',
    wire: '{"artanisActorRef":"agent:artanis","budgetMsat":2000000,"issuedAtIso":"2026-06-20T12:00:00.000Z","lifecycleRefs":["stage.artanis_labor_request.proposed","work_request.public.work_request_1","nostr.event.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","receipt.labor_escrow.reserve.artanis_1","stage.artanis_labor_request.rejected_refunded","verifier.public.artanis_labor.bun_test.failed","receipt.labor_escrow.refund.artanis_1"],"schema":"artanis.labor.unattended_request_receipt.v1","terminalState":"rejected_refunded","tickRef":"tick.public.artanis.2026-06-20T12:00","workRequestId":"work_request_1"}',
  },
]

describe('artanis unattended labor receipt golden wire format', () => {
  for (const vector of GOLDEN_VECTORS) {
    test(`${vector.terminalState}: serialized bytes are frozen`, () => {
      const receipt = buildArtanisLaborUnattendedRequestReceipt(vector.input)
      expect(receipt.terminalState).toBe(vector.terminalState)
      expect(serializeArtanisLaborUnattendedRequestReceipt(receipt)).toBe(vector.wire)
    })

    test(`${vector.terminalState}: content-addressed ref is frozen`, () => {
      const receipt = buildArtanisLaborUnattendedRequestReceipt(vector.input)
      expect(deriveArtanisLaborUnattendedRequestReceiptRef(receipt)).toBe(vector.ref)
    })

    test(`${vector.terminalState}: frozen bytes still verify under the frozen ref`, () => {
      // This is the durable-store contract: a receipt persisted under its ref in
      // an earlier release must still parse, re-derive its ref, and verify today.
      const receipt = verifyArtanisLaborUnattendedRequestReceipt(vector.wire, vector.ref)
      expect(receipt.terminalState).toBe(vector.terminalState)
      expect(parseArtanisLaborUnattendedRequestReceipt(vector.wire)).toEqual(receipt)
    })
  }

  test('every terminal state has exactly one golden vector with a distinct ref', () => {
    const states = GOLDEN_VECTORS.map(v => v.terminalState)
    expect(new Set(states).size).toBe(states.length)
    const refs = GOLDEN_VECTORS.map(v => v.ref)
    expect(new Set(refs).size).toBe(refs.length)
    // Pin the full terminal-state coverage so a new state cannot be added to the
    // receipt union without also being blessed here.
    expect([...states].sort()).toEqual(
      [
        'accepted_released',
        'refused',
        'rejected_refunded',
        'requested_pending_delivery',
        'skipped_config_disabled',
      ].sort(),
    )
  })
})
