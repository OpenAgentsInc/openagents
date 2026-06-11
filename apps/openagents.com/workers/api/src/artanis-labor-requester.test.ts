import { describe, expect, test } from 'vitest'

import {
  handleArtanisLaborResultDelivery,
  runArtanisLaborRequestTick,
  type ArtanisLaborRequestProposal,
} from './artanis-labor-requester'

const proposal = (overrides?: Partial<ArtanisLaborRequestProposal>) => ({
  budgetSats: 2_000,
  deadlineRef: 'deadline.public.artanis_labor.soon',
  objectiveRef: 'objective.public.artanis_labor.fix_test',
  repositoryRefs: ['repo.public.github.OpenAgentsInc.openagents'],
  requiredCapabilityRefs: ['capability.pylon.local_claude_agent'],
  title: 'Artanis labor request: fix test',
  verificationCommandRef: 'command.public.pylon.labor.bun_test',
  ...overrides,
})

describe('artanis labor request tick', () => {
  test('default disabled gate does not propose or spend', async () => {
    let proposed = false
    const result = await runArtanisLaborRequestTick({
      alreadyReservedThisTickMsat: 0,
      artanisActorRef: 'agent:artanis',
      enabled: false,
      nowIso: '2026-06-10T23:45:00.000Z',
      perTickBudgetMsat: 10_000_000,
      propose: async () => {
        proposed = true
        return proposal()
      },
      recordTickReceipt: async () => {},
      reserveEscrow: async () => ({ ok: true, reserveReceiptRef: 'receipt.reserve' }),
      seedBalanceAvailableMsat: 10_000_000,
      submitWorkRequest: async () => ({
        jobEventId: 'a'.repeat(64),
        topicId: 'topic_1',
        workRequestId: 'work_request_1',
      }),
    })

    expect(result).toEqual({ kind: 'skipped', reason: 'config_disabled' })
    expect(proposed).toBe(false)
  })

  test('proposes within budget, submits through the work-request path, and reserves escrow', async () => {
    const receipts: unknown[] = []
    const submitted: ArtanisLaborRequestProposal[] = []
    const reserved: unknown[] = []
    const result = await runArtanisLaborRequestTick({
      alreadyReservedThisTickMsat: 1_000_000,
      artanisActorRef: 'agent:artanis',
      enabled: true,
      nowIso: '2026-06-10T23:45:00.000Z',
      perTickBudgetMsat: 5_000_000,
      propose: async () => proposal(),
      recordTickReceipt: async input => {
        receipts.push(input)
      },
      reserveEscrow: async input => {
        reserved.push(input)
        return { ok: true, reserveReceiptRef: 'receipt.labor_escrow.reserve.artanis_1' }
      },
      seedBalanceAvailableMsat: 10_000_000,
      submitWorkRequest: async input => {
        submitted.push(input)
        return {
          jobEventId: 'a'.repeat(64),
          topicId: 'topic_1',
          workRequestId: 'work_request_1',
        }
      },
    })

    expect(result).toEqual({
      budgetMsat: 2_000_000,
      kind: 'requested',
      receipt: {
        jobEventId: 'a'.repeat(64),
        topicId: 'topic_1',
        workRequestId: 'work_request_1',
      },
      reserveReceiptRef: 'receipt.labor_escrow.reserve.artanis_1',
    })
    expect(submitted).toHaveLength(1)
    expect(reserved).toEqual([
      {
        amountMsat: 2_000_000,
        jobEventId: 'a'.repeat(64),
        requesterActorRef: 'agent:artanis',
        workRequestId: 'work_request_1',
      },
    ])
    expect(receipts).toMatchObject([
      {
        kind: 'request_labor_proposed',
        refs: expect.arrayContaining([
          'work_request.public.work_request_1',
          'receipt.labor_escrow.reserve.artanis_1',
        ]),
      },
    ])
  })

  test('schema-invalid and budget-over proposals are typed refusals with tick receipts', async () => {
    const schemaReceipts: unknown[] = []
    await expect(
      runArtanisLaborRequestTick({
        alreadyReservedThisTickMsat: 0,
        artanisActorRef: 'agent:artanis',
        enabled: true,
        nowIso: '2026-06-10T23:45:00.000Z',
        perTickBudgetMsat: 5_000_000,
        propose: async () => proposal({ budgetSats: 0 }),
        recordTickReceipt: async input => {
          schemaReceipts.push(input)
        },
        reserveEscrow: async () => ({ ok: true, reserveReceiptRef: 'receipt.reserve' }),
        seedBalanceAvailableMsat: 10_000_000,
        submitWorkRequest: async () => ({
          jobEventId: 'a'.repeat(64),
          topicId: 'topic_1',
          workRequestId: 'work_request_1',
        }),
      }),
    ).resolves.toMatchObject({
      kind: 'refused',
      reason: 'schema_invalid',
    })
    expect(schemaReceipts).toMatchObject([
      {
        kind: 'request_labor_refused',
        refs: ['refusal.artanis_labor_request.schema_invalid'],
      },
    ])

    await expect(
      runArtanisLaborRequestTick({
        alreadyReservedThisTickMsat: 4_000_000,
        artanisActorRef: 'agent:artanis',
        enabled: true,
        nowIso: '2026-06-10T23:45:00.000Z',
        perTickBudgetMsat: 5_000_000,
        propose: async () => proposal({ budgetSats: 2_000 }),
        recordTickReceipt: async () => {},
        reserveEscrow: async () => ({ ok: true, reserveReceiptRef: 'receipt.reserve' }),
        seedBalanceAvailableMsat: 10_000_000,
        submitWorkRequest: async () => {
          throw new Error('should not submit over budget')
        },
      }),
    ).resolves.toMatchObject({
      kind: 'refused',
      reason: 'per_tick_labor_budget_exceeded',
    })
  })

  test('escrow reserve refusal is returned as a typed refusal', async () => {
    await expect(
      runArtanisLaborRequestTick({
        alreadyReservedThisTickMsat: 0,
        artanisActorRef: 'agent:artanis',
        enabled: true,
        nowIso: '2026-06-10T23:45:00.000Z',
        perTickBudgetMsat: 5_000_000,
        propose: async () => proposal(),
        recordTickReceipt: async () => {},
        reserveEscrow: async () => ({ ok: false, reason: 'insufficient_available_balance' }),
        seedBalanceAvailableMsat: 10_000_000,
        submitWorkRequest: async () => ({
          jobEventId: 'a'.repeat(64),
          topicId: 'topic_1',
          workRequestId: 'work_request_1',
        }),
      }),
    ).resolves.toMatchObject({
      kind: 'refused',
      reason: 'insufficient_available_balance',
      refusalRef: 'refusal.artanis_labor_request.insufficient_available_balance',
    })
  })
})

describe('artanis validator-gated acceptance', () => {
  const delivery = {
    acceptanceEventRef: 'nostr.event.' + 'b'.repeat(64),
    providerActorRef: 'agent:provider',
    resultRef: 'result.public.artanis_labor.delivery_1',
    verificationCommandRef: 'command.public.pylon.labor.bun_test',
    workRequestId: 'work_request_1',
  }

  test('passing validator verdict releases escrow and records an acceptance receipt', async () => {
    const receipts: unknown[] = []
    const releases: unknown[] = []
    const result = await handleArtanisLaborResultDelivery(delivery, {
      recordTickReceipt: async input => {
        receipts.push(input)
      },
      refundEscrow: async () => {
        throw new Error('should not refund passing verdict')
      },
      releaseEscrow: async input => {
        releases.push(input)
        return { ok: true, releaseReceiptRef: 'receipt.labor_escrow.release.artanis_1' }
      },
      validateResult: async () => ({
        passed: true,
        verifierRef: 'verifier.public.artanis_labor.bun_test.passed',
      }),
    })

    expect(result).toEqual({
      kind: 'accepted',
      releaseReceiptRef: 'receipt.labor_escrow.release.artanis_1',
    })
    expect(releases).toEqual([
      {
        acceptanceEventRef: delivery.acceptanceEventRef,
        providerActorRef: 'agent:provider',
        workRequestId: 'work_request_1',
      },
    ])
    expect(receipts).toMatchObject([
      {
        kind: 'request_labor_accepted',
        refs: expect.arrayContaining([
          'result.public.artanis_labor.delivery_1',
          'verifier.public.artanis_labor.bun_test.passed',
          'receipt.labor_escrow.release.artanis_1',
        ]),
      },
    ])
  })

  test('failing validator verdict refunds escrow and records a rejection receipt', async () => {
    const refunds: unknown[] = []
    const result = await handleArtanisLaborResultDelivery(delivery, {
      recordTickReceipt: async () => {},
      refundEscrow: async input => {
        refunds.push(input)
        return { ok: true, refundReceiptRef: 'receipt.labor_escrow.refund.artanis_1' }
      },
      releaseEscrow: async () => {
        throw new Error('should not release failing verdict')
      },
      validateResult: async () => ({
        passed: false,
        reasonRef: 'verifier.public.artanis_labor.bun_test.failed',
      }),
    })

    expect(result).toEqual({
      kind: 'rejected_refunded',
      reasonRef: 'verifier.public.artanis_labor.bun_test.failed',
      refundReceiptRef: 'receipt.labor_escrow.refund.artanis_1',
    })
    expect(refunds).toEqual([
      {
        reasonRef: 'verifier.public.artanis_labor.bun_test.failed',
        workRequestId: 'work_request_1',
      },
    ])
  })
})
