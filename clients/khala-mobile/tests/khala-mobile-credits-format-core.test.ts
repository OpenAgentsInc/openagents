import { describe, expect, test } from "bun:test"

import {
  formatUsdCents,
  isLowBalance,
  LOW_BALANCE_THRESHOLD_CENTS,
  selectDisplayedBalanceUsdCents,
  signedAmountLabel,
  transactionKindLabel,
} from "../src/sync/khala-mobile-credits-format-core"

describe("formatUsdCents", () => {
  test("formats whole dollars", () => {
    expect(formatUsdCents(1000)).toBe("$10.00")
  })

  test("formats cents", () => {
    expect(formatUsdCents(5)).toBe("$0.05")
  })

  test("formats zero", () => {
    expect(formatUsdCents(0)).toBe("$0.00")
  })

  test("formats negative amounts with a leading sign", () => {
    expect(formatUsdCents(-250)).toBe("-$2.50")
  })
})

describe("isLowBalance", () => {
  test("is true below the threshold", () => {
    expect(isLowBalance(LOW_BALANCE_THRESHOLD_CENTS - 1)).toBe(true)
    expect(isLowBalance(0)).toBe(true)
  })

  test("is false at or above the threshold", () => {
    expect(isLowBalance(LOW_BALANCE_THRESHOLD_CENTS)).toBe(false)
    expect(isLowBalance(1000)).toBe(false)
  })
})

describe("transactionKindLabel", () => {
  test("labels every kind", () => {
    expect(transactionKindLabel("grant")).toBe("Free credit")
    expect(transactionKindLabel("purchase")).toBe("Purchase")
    expect(transactionKindLabel("charge")).toBe("Usage")
    expect(transactionKindLabel("other")).toBe("Other")
  })
})

describe("signedAmountLabel", () => {
  test("grants and purchases show a plus sign", () => {
    expect(signedAmountLabel("grant", 1000)).toBe("+$10.00")
    expect(signedAmountLabel("purchase", 500)).toBe("+$5.00")
  })

  test("charges show a minus sign regardless of the stored sign", () => {
    expect(signedAmountLabel("charge", 25)).toBe("-$0.25")
    expect(signedAmountLabel("charge", -25)).toBe("-$0.25")
  })
})

describe("selectDisplayedBalanceUsdCents (#8505 Part 2)", () => {
  test("prefers the live-synced value when present, even when the REST value also exists", () => {
    expect(
      selectDisplayedBalanceUsdCents({ restBalanceUsdCents: 100, syncedBalanceUsdCents: 500 }),
    ).toBe(500)
  })

  test("prefers the synced value even when it is zero (a real, live zero balance)", () => {
    expect(
      selectDisplayedBalanceUsdCents({ restBalanceUsdCents: 1000, syncedBalanceUsdCents: 0 }),
    ).toBe(0)
  })

  test("falls back to the REST value before the synced entity exists (cold start / not yet backfilled)", () => {
    expect(
      selectDisplayedBalanceUsdCents({ restBalanceUsdCents: 250, syncedBalanceUsdCents: null }),
    ).toBe(250)
  })

  test("is null (renders nothing) when neither source has a value yet", () => {
    expect(
      selectDisplayedBalanceUsdCents({ restBalanceUsdCents: null, syncedBalanceUsdCents: null }),
    ).toBeNull()
  })
})
