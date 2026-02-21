import { Effect } from "effect"
import { describe, expect, it } from "@effect/vitest"

import {
  buildWalletExecutionReceipt,
  canonicalWalletExecutionReceiptHash,
} from "../src/receipt/canonical.js"

describe("wallet execution receipt canonicalization", () => {
  it.effect("produces stable canonical hash for identical payment facts", () =>
    Effect.sync(() => {
      const input = {
        requestId: "req-123",
        walletId: "wallet-ep212",
        host: "SATS4AI.COM",
        paymentId: "pay-123",
        invoiceHash: "ABCDEF1234",
        quotedAmountMsats: 45_000,
        settledAmountMsats: 45_000,
        preimageHex: "A".repeat(64),
        paidAtMs: 1_777_000_000_000,
      }

      const first = buildWalletExecutionReceipt(input)
      const second = buildWalletExecutionReceipt({
        ...input,
        host: "sats4ai.com",
        invoiceHash: "abcdef1234",
        preimageHex: "a".repeat(64),
      })

      expect(first.canonicalJsonSha256).toBe(second.canonicalJsonSha256)
      expect(first.receiptId).toBe(second.receiptId)
      expect(first.host).toBe("sats4ai.com")
      expect(first.invoiceHash).toBe("abcdef1234")
      expect(first.preimageSha256).toMatch(/^[0-9a-f]{64}$/)
      expect(first.canonicalJsonSha256).toMatch(/^[0-9a-f]{64}$/)
      expect(first.receiptId).toMatch(/^lwr_[0-9a-f]{24}$/)
    }),
  )

  it.effect("hash changes when settled amount changes", () =>
    Effect.sync(() => {
      const baseline = canonicalWalletExecutionReceiptHash({
        requestId: "req-200",
        walletId: "wallet-ep212",
        host: "sats4ai.com",
        paymentId: "pay-200",
        invoiceHash: "hash-200",
        quotedAmountMsats: 50_000,
        settledAmountMsats: 50_000,
        preimageHex: "b".repeat(64),
        paidAtMs: 1_777_000_000_100,
      })

      const changed = canonicalWalletExecutionReceiptHash({
        requestId: "req-200",
        walletId: "wallet-ep212",
        host: "sats4ai.com",
        paymentId: "pay-200",
        invoiceHash: "hash-200",
        quotedAmountMsats: 50_000,
        settledAmountMsats: 49_000,
        preimageHex: "b".repeat(64),
        paidAtMs: 1_777_000_000_100,
      })

      expect(changed).not.toBe(baseline)
    }),
  )
})

