import { describe, expect, test } from 'vitest'

import {
  calculateCodexUsageDebitCents,
  calculateContainerUsageDebitCents,
  formatUsdCents,
  normalizeCouponCode,
  shouldSuspendBillingBalance,
} from './billing'

describe('billing credits', () => {
  test('formats signed USD cent values', () => {
    expect(formatUsdCents(0)).toBe('$0.00')
    expect(formatUsdCents(2500)).toBe('$25.00')
    expect(formatUsdCents(-17)).toBe('-$0.17')
  })

  test('rounds Computer usage to per-second debit cents', () => {
    expect(calculateContainerUsageDebitCents(0)).toBe(0)
    expect(calculateContainerUsageDebitCents(1)).toBe(1)
    expect(calculateContainerUsageDebitCents(60)).toBe(5)
    expect(calculateContainerUsageDebitCents(61)).toBe(6)
  })

  test('rounds Codex token usage to the configured thousand-token rate', () => {
    expect(calculateCodexUsageDebitCents(0)).toBe(0)
    expect(calculateCodexUsageDebitCents(1)).toBe(1)
    expect(calculateCodexUsageDebitCents(1000)).toBe(2)
    expect(calculateCodexUsageDebitCents(1001)).toBe(3)
  })

  test('normalizes coupon codes for stable redemption keys', () => {
    expect(normalizeCouponCode(' openagents trial ')).toBe('OPENAGENTS-TRIAL')
  })

  test('suspends only when the derived balance is zero or negative', () => {
    expect(shouldSuspendBillingBalance(1)).toBe(false)
    expect(shouldSuspendBillingBalance(0)).toBe(true)
    expect(shouldSuspendBillingBalance(-1)).toBe(true)
  })
})
