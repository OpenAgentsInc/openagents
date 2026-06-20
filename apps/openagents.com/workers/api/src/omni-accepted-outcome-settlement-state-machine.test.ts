import { describe, expect, test } from 'vitest'

import type { OmniAcceptedOutcomeEconomicsRecord } from './omni-accepted-outcome-economics'
import {
  advanceOmniAcceptedOutcomeSettlementMachine,
  createOmniAcceptedOutcomeSettlementMachine,
  isOmniSettlementMachineComplete,
  OMNI_SETTLEMENT_STATE_ORDER,
  OmniSettlementStateMachineTransitionError,
  OmniSettlementStateMachineValidationError,
  publicOmniAcceptedOutcomeSettlementMachineProjection,
  type OmniAcceptedOutcomeSettlementMachine,
  type OmniSettlementStateId,
} from './omni-accepted-outcome-settlement-state-machine'

const baseRecord: OmniAcceptedOutcomeEconomicsRecord = {
  acceptedOutcomeContractId: 'omni_accepted_outcome_contract_1',
  acceptedValueCents: 5000,
  archivedAt: null,
  artifactCostCents: 100,
  buyerPriceAsset: 'usd',
  buyerPriceCents: 5000,
  createdAt: '2026-06-20T00:00:00.000Z',
  creditsCharged: 0,
  fundingMode: 'credit_funded',
  grossMarginCents: 4400,
  id: 'omni_outcome_economics_1',
  idempotencyKey: 'idem-1',
  internalCaveatRef: null,
  metadata: {},
  noSettlementImplication: true,
  providerCostCents: 300,
  publicCaveatRef: 'caveat.no_settlement',
  retryCostCents: 0,
  reviewCostCents: 100,
  reviewMinutes: 5,
  runnerCostCents: 100,
  satsCharged: 0,
  totalCostCents: 600,
  updatedAt: '2026-06-20T00:00:00.000Z',
  workKind: 'coding',
  workroomId: 'omni_workroom_coding_1',
}

const advanceThrough = (
  record: OmniAcceptedOutcomeEconomicsRecord,
  states: ReadonlyArray<OmniSettlementStateId>,
  options: Readonly<{ dispatchArmed?: boolean }> = {},
): OmniAcceptedOutcomeSettlementMachine =>
  states.reduce(
    (machine, stateId, index) =>
      advanceOmniAcceptedOutcomeSettlementMachine(machine, record, stateId, {
        evidenceRef: `evidence.${stateId}`,
        recordedAt: `2026-06-20T00:0${index}:00.000Z`,
      }),
    createOmniAcceptedOutcomeSettlementMachine(record, options),
  )

describe('createOmniAcceptedOutcomeSettlementMachine', () => {
  test('starts INERT, unstarted, and disclaiming settlement', () => {
    const machine = createOmniAcceptedOutcomeSettlementMachine(baseRecord)
    expect(machine.dispatchArmed).toBe(false)
    expect(machine.state).toBe(null)
    expect(machine.transitions).toEqual([])
    expect(machine.noSettlementImplication).toBe(true)
    expect(machine.economicsId).toBe('omni_outcome_economics_1')
  })

  test('exposes exactly the eight ordered lifecycle states', () => {
    expect(OMNI_SETTLEMENT_STATE_ORDER).toEqual([
      'authorized',
      'paid',
      'accepted',
      'pending_payout',
      'dispatched',
      'confirmed',
      'reconciled',
      'margin',
    ])
  })
})

describe('advanceOmniAcceptedOutcomeSettlementMachine', () => {
  test('records all eight distinct evidence states in order', () => {
    const machine = advanceThrough(baseRecord, OMNI_SETTLEMENT_STATE_ORDER)
    expect(machine.transitions.map(t => t.stateId)).toEqual([
      ...OMNI_SETTLEMENT_STATE_ORDER,
    ])
    expect(isOmniSettlementMachineComplete(machine)).toBe(true)
    expect(machine.state).toBe('margin')
    // Eight DISTINCT states; none collapsed.
    expect(new Set(machine.transitions.map(t => t.stateId)).size).toBe(8)
  })

  test('is idempotent: re-recording the current state is a no-op', () => {
    const machine = advanceThrough(baseRecord, ['authorized'])
    const again = advanceOmniAcceptedOutcomeSettlementMachine(
      machine,
      baseRecord,
      'authorized',
      { evidenceRef: 'evidence.retry', recordedAt: '2026-06-20T01:00:00.000Z' },
    )
    expect(again).toBe(machine)
    expect(again.transitions).toHaveLength(1)
  })

  test('rejects skipping a state (gap-free / monotonic)', () => {
    expect(() => advanceThrough(baseRecord, ['authorized', 'accepted'])).toThrow(
      OmniSettlementStateMachineTransitionError,
    )
  })

  test('rejects moving backward', () => {
    const machine = advanceThrough(baseRecord, ['authorized', 'paid'])
    expect(() =>
      advanceOmniAcceptedOutcomeSettlementMachine(
        machine,
        baseRecord,
        'authorized',
        {
          evidenceRef: 'evidence.back',
          recordedAt: '2026-06-20T02:00:00.000Z',
        },
      ),
    ).toThrow(OmniSettlementStateMachineTransitionError)
  })

  test('first transition must be authorized', () => {
    expect(() => advanceThrough(baseRecord, ['paid'])).toThrow(
      OmniSettlementStateMachineTransitionError,
    )
  })

  test('INERT machine records dispatched/confirmed as intent_only, no money moved', () => {
    const machine = advanceThrough(baseRecord, OMNI_SETTLEMENT_STATE_ORDER)
    const dispatched = machine.transitions.find(t => t.stateId === 'dispatched')
    const confirmed = machine.transitions.find(t => t.stateId === 'confirmed')
    expect(dispatched?.evidenceKind).toBe('intent_only')
    expect(dispatched?.movedMoney).toBe(false)
    expect(confirmed?.evidenceKind).toBe('intent_only')
    expect(confirmed?.movedMoney).toBe(false)
    // No money moved -> the machine still disclaims settlement implication.
    expect(machine.noSettlementImplication).toBe(true)
  })

  test('armed machine records externally_confirmed money movement and drops the disclaimer', () => {
    const machine = advanceThrough(baseRecord, OMNI_SETTLEMENT_STATE_ORDER, {
      dispatchArmed: true,
    })
    const dispatched = machine.transitions.find(t => t.stateId === 'dispatched')
    expect(dispatched?.evidenceKind).toBe('externally_confirmed')
    expect(dispatched?.movedMoney).toBe(true)
    expect(machine.noSettlementImplication).toBe(false)
  })

  test('non-money-movement states carry accounting/derived evidence', () => {
    const machine = advanceThrough(baseRecord, OMNI_SETTLEMENT_STATE_ORDER)
    const byState = new Map(machine.transitions.map(t => [t.stateId, t]))
    expect(byState.get('authorized')?.evidenceKind).toBe('accounting_recorded')
    expect(byState.get('paid')?.evidenceKind).toBe('accounting_recorded')
    expect(byState.get('accepted')?.evidenceKind).toBe('accounting_recorded')
    expect(byState.get('pending_payout')?.evidenceKind).toBe('derived')
    expect(byState.get('reconciled')?.evidenceKind).toBe('derived')
    expect(byState.get('margin')?.evidenceKind).toBe('derived')
  })

  test('records the correct monetary figure per state', () => {
    const machine = advanceThrough(baseRecord, OMNI_SETTLEMENT_STATE_ORDER)
    const byState = new Map(machine.transitions.map(t => [t.stateId, t]))
    expect(byState.get('authorized')?.amountCents).toBe(5000)
    expect(byState.get('paid')?.amountCents).toBe(5000)
    expect(byState.get('accepted')?.amountCents).toBe(5000)
    // distributable margin = max(0, gross) = 4400
    expect(byState.get('pending_payout')?.amountCents).toBe(4400)
    expect(byState.get('margin')?.amountCents).toBe(4400)
  })

  test('margin may be negative on a loss but payout states never are', () => {
    const lossRecord: OmniAcceptedOutcomeEconomicsRecord = {
      ...baseRecord,
      acceptedValueCents: 100,
      grossMarginCents: -500,
      totalCostCents: 600,
    }
    const machine = advanceThrough(lossRecord, OMNI_SETTLEMENT_STATE_ORDER)
    const byState = new Map(machine.transitions.map(t => [t.stateId, t]))
    expect(byState.get('margin')?.amountCents).toBe(-500)
    // distributable = max(0, -500) = 0; never a negative owed balance.
    expect(byState.get('pending_payout')?.amountCents).toBe(0)
    expect(byState.get('dispatched')?.amountCents).toBe(0)
  })

  test('rejects unsafe evidence refs', () => {
    const machine = createOmniAcceptedOutcomeSettlementMachine(baseRecord)
    expect(() =>
      advanceOmniAcceptedOutcomeSettlementMachine(
        machine,
        baseRecord,
        'authorized',
        {
          evidenceRef: 'lnbc1pdeadbeef',
          recordedAt: '2026-06-20T00:00:00.000Z',
        },
      ),
    ).toThrow(OmniSettlementStateMachineValidationError)
  })

  test('rejects a record id mismatch', () => {
    const machine = createOmniAcceptedOutcomeSettlementMachine(baseRecord)
    expect(() =>
      advanceOmniAcceptedOutcomeSettlementMachine(
        machine,
        { ...baseRecord, id: 'other_outcome' },
        'authorized',
        { evidenceRef: 'evidence.x', recordedAt: '2026-06-20T00:00:00.000Z' },
      ),
    ).toThrow(OmniSettlementStateMachineValidationError)
  })
})

describe('publicOmniAcceptedOutcomeSettlementMachineProjection', () => {
  test('drops monetary figures and refs but keeps honest evidence labels', () => {
    const machine = advanceThrough(baseRecord, OMNI_SETTLEMENT_STATE_ORDER)
    const projection =
      publicOmniAcceptedOutcomeSettlementMachineProjection(machine)
    expect(projection.complete).toBe(true)
    expect(projection.state).toBe('margin')
    expect(projection.transitions).toHaveLength(8)
    for (const transition of projection.transitions) {
      expect(transition).not.toHaveProperty('amountCents')
      expect(transition).not.toHaveProperty('evidenceRef')
      expect(transition).toHaveProperty('evidenceKind')
      expect(transition).toHaveProperty('movedMoney')
    }
  })

  test('an incomplete machine projects complete = false', () => {
    const machine = advanceThrough(baseRecord, ['authorized', 'paid'])
    const projection =
      publicOmniAcceptedOutcomeSettlementMachineProjection(machine)
    expect(projection.complete).toBe(false)
    expect(projection.state).toBe('paid')
  })
})
