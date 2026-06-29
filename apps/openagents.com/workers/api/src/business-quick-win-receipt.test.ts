import { describe, expect, test } from 'vitest'

import {
  assertFirstPaidQuickWinReceipt,
  buildBusinessQuickWinReceipt,
  BusinessQuickWinReceiptInvariantError,
  publicBusinessQuickWinReceiptProjection,
  REQUIRED_PAID_QUICK_WIN_STATES,
  type BusinessQuickWinReceiptInput,
} from './business-quick-win-receipt'

const intakeOnly: BusinessQuickWinReceiptInput = {
  signupId: 'business_signup_abc123',
  offeringPromiseId: 'business.coding_quick_win.v1',
  quickWinSummary: 'Fix the failing checkout test suite with passing tests.',
}

const fullyPaid: BusinessQuickWinReceiptInput = {
  ...intakeOnly,
  quickWinScopedRef: 'spec:quick-win-001',
  deliveredEvidenceRef: 'https://github.com/example/pr/42',
  outcomeAcceptedRef: 'acceptance:acc-001',
  buyerPaidRef: 'payment:pay-001',
  providerSettledRef: 'settlement:set-001',
}

describe('buildBusinessQuickWinReceipt', () => {
  test('names every lifecycle state exactly once in order', () => {
    const receipt = buildBusinessQuickWinReceipt(intakeOnly)
    expect(receipt.lines.map(line => line.stateId)).toEqual([
      'intake_recorded',
      'quick_win_scoped',
      'delivered_with_evidence',
      'outcome_accepted',
      'buyer_paid',
      'provider_settled',
    ])
  })

  test('intake-only receipt evidences only intake_recorded and is not paid', () => {
    const receipt = buildBusinessQuickWinReceipt(intakeOnly)
    expect(receipt.evidencedStateCount).toBe(1)
    expect(receipt.paidQuickWin).toBe(false)
    expect(receipt.unevidencedStateIds).toEqual([
      'quick_win_scoped',
      'delivered_with_evidence',
      'outcome_accepted',
      'buyer_paid',
      'provider_settled',
    ])
    const intakeLine = receipt.lines.find(
      line => line.stateId === 'intake_recorded',
    )
    expect(intakeLine?.evidenceState).toBe('evidenced')
    expect(intakeLine?.evidenceRef).toBe('business_signup_abc123')
  })

  test('flags settlement-implying states', () => {
    const receipt = buildBusinessQuickWinReceipt(intakeOnly)
    const implying = receipt.lines
      .filter(line => line.impliesSettlement)
      .map(line => line.stateId)
    expect(implying).toEqual(['buyer_paid', 'provider_settled'])
  })

  test('fully delivered, paid, settled receipt evidences all states', () => {
    const receipt = buildBusinessQuickWinReceipt(fullyPaid)
    expect(receipt.evidencedStateCount).toBe(6)
    expect(receipt.unevidencedStateIds).toEqual([])
    expect(receipt.paidQuickWin).toBe(true)
  })

  test('is deterministic for identical input', () => {
    expect(buildBusinessQuickWinReceipt(fullyPaid)).toEqual(
      buildBusinessQuickWinReceipt(fullyPaid),
    )
  })

  test('uses default operator-assisted caveat when none supplied', () => {
    const receipt = buildBusinessQuickWinReceipt(intakeOnly)
    expect(receipt.publicCaveatRef).toBe(
      'caveat.business_quick_win.operator_assisted_not_self_serve',
    )
  })

  test('rejects a paid state that skips an unevidenced prerequisite', () => {
    expect(() =>
      buildBusinessQuickWinReceipt({
        ...intakeOnly,
        // buyer_paid without delivery/acceptance is dishonest.
        buyerPaidRef: 'payment:pay-001',
      }),
    ).toThrow(BusinessQuickWinReceiptInvariantError)
  })

  test('requires a non-empty signupId', () => {
    expect(() =>
      buildBusinessQuickWinReceipt({ ...intakeOnly, signupId: '   ' }),
    ).toThrow(BusinessQuickWinReceiptInvariantError)
  })

  test('requires a non-empty offeringPromiseId and quickWinSummary', () => {
    expect(() =>
      buildBusinessQuickWinReceipt({ ...intakeOnly, offeringPromiseId: '' }),
    ).toThrow(BusinessQuickWinReceiptInvariantError)
    expect(() =>
      buildBusinessQuickWinReceipt({ ...intakeOnly, quickWinSummary: '' }),
    ).toThrow(BusinessQuickWinReceiptInvariantError)
  })

  test('treats blank evidence refs as not yet evidenced', () => {
    const receipt = buildBusinessQuickWinReceipt({
      ...intakeOnly,
      quickWinScopedRef: '   ',
    })
    expect(receipt.evidencedStateCount).toBe(1)
  })
})

describe('assertFirstPaidQuickWinReceipt', () => {
  test('passes for a receipt evidenced through buyer_paid', () => {
    const receipt = buildBusinessQuickWinReceipt(fullyPaid)
    expect(() => assertFirstPaidQuickWinReceipt(receipt)).not.toThrow()
  })

  test('passes even when provider_settled is still pending', () => {
    const receipt = buildBusinessQuickWinReceipt({
      ...fullyPaid,
      providerSettledRef: null,
    })
    expect(() => assertFirstPaidQuickWinReceipt(receipt)).not.toThrow()
  })

  test('rejects an intake-only receipt (no real paid quick win yet)', () => {
    const receipt = buildBusinessQuickWinReceipt(intakeOnly)
    expect(() => assertFirstPaidQuickWinReceipt(receipt)).toThrow(
      BusinessQuickWinReceiptInvariantError,
    )
  })

  test('rejects a delivered-but-unpaid receipt', () => {
    const receipt = buildBusinessQuickWinReceipt({
      ...intakeOnly,
      quickWinScopedRef: 'spec:quick-win-001',
      deliveredEvidenceRef: 'https://github.com/example/pr/42',
      outcomeAcceptedRef: 'acceptance:acc-001',
    })
    expect(() => assertFirstPaidQuickWinReceipt(receipt)).toThrow(
      BusinessQuickWinReceiptInvariantError,
    )
  })

  test('required states stop short of provider_settled', () => {
    expect(REQUIRED_PAID_QUICK_WIN_STATES).toEqual([
      'intake_recorded',
      'quick_win_scoped',
      'delivered_with_evidence',
      'outcome_accepted',
      'buyer_paid',
    ])
  })
})

describe('publicBusinessQuickWinReceiptProjection', () => {
  test('keeps lifecycle labels but drops evidence references', () => {
    const projection = publicBusinessQuickWinReceiptProjection(
      buildBusinessQuickWinReceipt(fullyPaid),
    )
    expect(projection.paidQuickWin).toBe(true)
    for (const line of projection.lines) {
      expect(line).not.toHaveProperty('evidenceRef')
    }
    expect(projection).not.toHaveProperty('signupId')
  })
})

import { readFileSync } from 'node:fs'
import { parseBusinessQuickWinReceiptDocument } from './business-quick-win-receipt'

describe('parseBusinessQuickWinReceiptDocument on template', () => {
  const loadTemplate = (): unknown => {
    return JSON.parse(
      readFileSync(
        './src/fixtures/business-coding-quick-win-receipt.template.json',
        'utf8',
      ),
    )
  }

  test('the template parses as a valid receipt and asserts a paid quick win', () => {
    const raw = loadTemplate()
    const receipt = parseBusinessQuickWinReceiptDocument(raw)

    // Check that it passes the exact paid gate
    expect(() => assertFirstPaidQuickWinReceipt(receipt)).not.toThrow()
    expect(receipt.paidQuickWin).toBe(true)

    // Check that it uses placeholder synthetic refs, not real ones
    expect(receipt.signupId).toContain('.example.')
    for (const line of receipt.lines) {
      if (line.evidenceRef) {
        expect(line.evidenceRef).toContain('.example.')
      }
    }
  })

  test('rejects a document with a leaked extra field', () => {
    const raw = loadTemplate() as any
    raw.internalLedgerId = 'leak-123'
    expect(() => parseBusinessQuickWinReceiptDocument(raw)).toThrow()
  })
})
