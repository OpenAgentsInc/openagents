import { describe, expect, test } from 'vitest'

import { makeInMemoryArtanisLaborUnattendedReceiptStore } from './artanis-labor-receipt-store'
import type {
  ArtanisLaborRequestProposal,
  ArtanisLaborRequesterDeps,
} from './artanis-labor-requester'
import {
  resolveAndPersistArtanisLaborDelivery,
  runAndPersistArtanisLaborRequestTick,
  type ArtanisLaborRequestedOutcome,
} from './artanis-labor-tick-driver'

const proposal = (
  overrides?: Partial<ArtanisLaborRequestProposal>,
): ArtanisLaborRequestProposal => ({
  budgetSats: 2_000,
  deadlineRef: 'deadline.public.artanis_labor.soon',
  objectiveRef: 'objective.public.artanis_labor.fix_test',
  repositoryRefs: ['repo.public.github.OpenAgentsInc.openagents'],
  requiredCapabilityRefs: ['capability.pylon.local_claude_agent'],
  title: 'Artanis labor request: fix test',
  verificationCommandRef: 'command.public.pylon.labor.bun_test',
  ...overrides,
})

const requesterDeps = (
  overrides?: Partial<ArtanisLaborRequesterDeps>,
): ArtanisLaborRequesterDeps => ({
  alreadyReservedThisTickMsat: 0,
  artanisActorRef: 'agent:artanis',
  enabled: true,
  nowIso: '2026-06-20T12:00:00.000Z',
  perTickBudgetMsat: 10_000_000,
  propose: async () => proposal(),
  recordTickReceipt: async () => {},
  reserveEscrow: async () => ({
    ok: true,
    reserveReceiptRef: 'receipt.labor_escrow.reserve.artanis_1',
  }),
  seedBalanceAvailableMsat: 10_000_000,
  submitWorkRequest: async () => ({
    jobEventId: 'a'.repeat(64),
    topicId: 'topic_1',
    workRequestId: 'work_request_1',
  }),
  ...overrides,
})

const driverBase = {
  artanisActorRef: 'agent:artanis',
  tickRef: 'tick.public.artanis.2026-06-20T12:00',
}

describe('artanis labor tick driver: request stage', () => {
  test('a placed tick seals a pending receipt into the store, readable by ref', async () => {
    const store = makeInMemoryArtanisLaborUnattendedReceiptStore()
    const { put, requestOutcome, sealed } = await runAndPersistArtanisLaborRequestTick({
      ...driverBase,
      requesterDeps: requesterDeps(),
      store,
    })

    expect(requestOutcome.kind).toBe('requested')
    expect(sealed.receipt.terminalState).toBe('requested_pending_delivery')
    expect(put.kind).toBe('stored')

    const fetched = await store.get(sealed.receiptRef)
    expect(fetched?.serialized).toBe(sealed.serialized)
  })

  test('a disabled tick still seals a skipped receipt (gate auditability)', async () => {
    const store = makeInMemoryArtanisLaborUnattendedReceiptStore()
    let proposed = false
    const { requestOutcome, sealed } = await runAndPersistArtanisLaborRequestTick({
      ...driverBase,
      requesterDeps: requesterDeps({
        enabled: false,
        propose: async () => {
          proposed = true
          return proposal()
        },
      }),
      store,
    })

    expect(proposed).toBe(false)
    expect(requestOutcome).toEqual({ kind: 'skipped', reason: 'config_disabled' })
    expect(sealed.receipt.terminalState).toBe('skipped_config_disabled')
    expect((await store.list()).map(s => s.receiptRef)).toEqual([sealed.receiptRef])
  })

  test('a refused tick (over budget) seals a refused receipt', async () => {
    const store = makeInMemoryArtanisLaborUnattendedReceiptStore()
    const { requestOutcome, sealed } = await runAndPersistArtanisLaborRequestTick({
      ...driverBase,
      requesterDeps: requesterDeps({ perTickBudgetMsat: 1 }),
      store,
    })

    expect(requestOutcome.kind).toBe('refused')
    expect(sealed.receipt.terminalState).toBe('refused')
    expect(sealed.receipt.workRequestId).toBeNull()
  })

  test('re-running the same tick is idempotent by content address', async () => {
    const store = makeInMemoryArtanisLaborUnattendedReceiptStore()
    const first = await runAndPersistArtanisLaborRequestTick({
      ...driverBase,
      requesterDeps: requesterDeps(),
      store,
    })
    const second = await runAndPersistArtanisLaborRequestTick({
      ...driverBase,
      requesterDeps: requesterDeps(),
      store,
    })

    expect(first.put.kind).toBe('stored')
    expect(second.put.kind).toBe('already_stored')
    expect(second.sealed.receiptRef).toBe(first.sealed.receiptRef)
    expect((await store.list())).toHaveLength(1)
  })
})

describe('artanis labor tick driver: delivery resolution stage', () => {
  const requestedOutcome: ArtanisLaborRequestedOutcome = {
    budgetMsat: 2_000_000,
    kind: 'requested',
    receipt: {
      jobEventId: 'a'.repeat(64),
      topicId: 'topic_1',
      workRequestId: 'work_request_1',
    },
    reserveReceiptRef: 'receipt.labor_escrow.reserve.artanis_1',
  }

  const delivery = {
    acceptanceEventRef: 'nostr.event.acceptance_1',
    providerActorRef: 'agent:pylon',
    resultRef: 'result.public.artanis_labor.work_request_1',
    verificationCommandRef: 'command.public.pylon.labor.bun_test',
    workRequestId: 'work_request_1',
  }

  test('a validator-pass delivery seals an accepted_released receipt', async () => {
    const store = makeInMemoryArtanisLaborUnattendedReceiptStore()
    const { acceptanceOutcome, put, sealed } = await resolveAndPersistArtanisLaborDelivery({
      ...driverBase,
      acceptanceDeps: {
        recordTickReceipt: async () => {},
        refundEscrow: async () => ({ ok: true, refundReceiptRef: 'receipt.refund' }),
        releaseEscrow: async () => ({
          ok: true,
          releaseReceiptRef: 'receipt.labor_escrow.release.artanis_1',
        }),
        validateResult: async () => ({ passed: true, verifierRef: 'verifier.public.bun_test' }),
      },
      delivery,
      nowIso: '2026-06-20T12:05:00.000Z',
      requestOutcome: requestedOutcome,
      store,
    })

    expect(acceptanceOutcome.kind).toBe('accepted')
    expect(sealed.receipt.terminalState).toBe('accepted_released')
    expect(put.kind).toBe('stored')
    expect(await store.get(sealed.receiptRef)).toBeDefined()
  })

  test('a validator-fail delivery seals a rejected_refunded receipt', async () => {
    const store = makeInMemoryArtanisLaborUnattendedReceiptStore()
    const { acceptanceOutcome, sealed } = await resolveAndPersistArtanisLaborDelivery({
      ...driverBase,
      acceptanceDeps: {
        recordTickReceipt: async () => {},
        refundEscrow: async () => ({
          ok: true,
          refundReceiptRef: 'receipt.labor_escrow.refund.artanis_1',
        }),
        releaseEscrow: async () => ({ ok: true, releaseReceiptRef: 'receipt.release' }),
        validateResult: async () => ({
          passed: false,
          reasonRef: 'reason.public.artanis_labor.verification_failed',
        }),
      },
      delivery,
      nowIso: '2026-06-20T12:05:00.000Z',
      requestOutcome: requestedOutcome,
      store,
    })

    expect(acceptanceOutcome.kind).toBe('rejected_refunded')
    expect(sealed.receipt.terminalState).toBe('rejected_refunded')
    expect(sealed.receipt.workRequestId).toBe('work_request_1')
  })
})
