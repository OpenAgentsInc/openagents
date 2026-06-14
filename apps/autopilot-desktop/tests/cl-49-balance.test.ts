import { describe, expect, test } from "bun:test"
import { walletSummary } from "../src/ui/cards/balance"
import type { WalletStatusRow } from "../src/shared/rpc"

const base: WalletStatusRow = {
  configured: true,
  daemonOnline: true,
  balanceSats: null,
  receiveReady: false,
  sendReady: false,
  readiness: "ready",
}

describe("CL-49 walletSummary", () => {
  test("numeric balance uses toLocaleString + ' sats'", () => {
    const { value } = walletSummary({ ...base, balanceSats: 1_234_567 })
    // toLocaleString output is locale-dependent in Bun; check it contains the digits and ends with " sats"
    expect(value).toMatch(/1.234.567|1,234,567/)
    expect(value).toEndWith(" sats")
  })

  test("zero balance renders '0 sats'", () => {
    const { value } = walletSummary({ ...base, balanceSats: 0 })
    expect(value).toBe("0 sats")
  })

  test("null balance renders '—'", () => {
    const { value } = walletSummary({ ...base, balanceSats: null })
    expect(value).toBe("—")
  })

  test("daemonOnline true → 'wallet online' in summary", () => {
    const { summary } = walletSummary({ ...base, daemonOnline: true })
    expect(summary).toContain("wallet online")
  })

  test("daemonOnline false → 'wallet offline' in summary", () => {
    const { summary } = walletSummary({ ...base, daemonOnline: false })
    expect(summary).toContain("wallet offline")
  })

  test("readiness string is included in summary", () => {
    const { summary } = walletSummary({ ...base, readiness: "syncing" })
    expect(summary).toContain("syncing")
  })

  test("receiveReady false → no receive indicator in summary", () => {
    const { summary } = walletSummary({ ...base, receiveReady: false })
    expect(summary).not.toContain("receive ✓")
  })

  test("receiveReady true → 'receive ✓' appended to summary", () => {
    const { summary } = walletSummary({ ...base, receiveReady: true })
    expect(summary).toEndWith(" · receive ✓")
  })

  test("full online+receiveReady summary format", () => {
    const { summary } = walletSummary({
      ...base,
      daemonOnline: true,
      readiness: "ready",
      receiveReady: true,
    })
    expect(summary).toBe("wallet online · ready · receive ✓")
  })

  test("offline without receiveReady summary format", () => {
    const { summary } = walletSummary({
      ...base,
      daemonOnline: false,
      readiness: "offline",
      receiveReady: false,
    })
    expect(summary).toBe("wallet offline · offline")
  })
})
