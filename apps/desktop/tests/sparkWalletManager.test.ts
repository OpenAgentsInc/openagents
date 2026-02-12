import { Effect } from "effect"
import { describe, expect, it } from "@effect/vitest"

import {
  SparkWalletManagerService,
  projectSparkWalletSnapshotForRenderer,
} from "../src/main/sparkWalletManager"
import { makeSparkWalletHarness } from "./support/sparkWalletHarness"

describe("spark wallet manager", () => {
  it.effect("bootstraps with NIP-06 mnemonic generation and connects sdk", () => {
    const harness = makeSparkWalletHarness()

    return Effect.gen(function* () {
      const manager = yield* SparkWalletManagerService

      yield* manager.bootstrap()
      const snapshot = yield* manager.snapshot()

      expect(snapshot.lifecycle).toBe("connected")
      expect(snapshot.network).toBe("regtest")
      expect(snapshot.mnemonicStored).toBe(true)
      expect(snapshot.apiKeyConfigured).toBe(true)
      expect(snapshot.balanceSats).toBe(1234)
      expect(harness.connectMnemonics.length).toBe(1)
      expect(harness.connectMnemonics[0]?.split(/\s+/g).length).toBeGreaterThanOrEqual(12)
    }).pipe(
      Effect.provide(harness.layer),
      Effect.ensuring(Effect.sync(harness.cleanup)),
    )
  })

  it.effect("pays bolt11 invoice and returns l402-compatible payment proof", () => {
    const harness = makeSparkWalletHarness()

    return Effect.gen(function* () {
      const manager = yield* SparkWalletManagerService
      yield* manager.bootstrap()

      const result = yield* manager.payInvoice({
        invoice: "lnbcrt1sparktestinvoice",
        host: "seller.example.com",
        maxAmountMsats: 10_000,
      })

      expect(result.paymentId).toBe("spark-payment-1")
      expect(result.amountMsats).toBe(2000)
      expect(result.preimageHex).toBe("ef".repeat(32))
      expect(harness.prepareRequests.length).toBe(1)
      expect(harness.prepareRequests[0]?.paymentRequest).toBe("lnbcrt1sparktestinvoice")
      expect(harness.sendRequests.length).toBe(1)

      const snapshot = yield* manager.snapshot()
      expect(snapshot.lastPaymentId).toBe("spark-payment-1")
      expect(snapshot.lastPaymentAtMs).toBe(1_700_000_100_000)
    }).pipe(
      Effect.provide(harness.layer),
      Effect.ensuring(Effect.sync(harness.cleanup)),
    )
  })

  it.effect("projects renderer-safe snapshot without leaking mnemonic", () => {
    const harness = makeSparkWalletHarness()

    return Effect.gen(function* () {
      const manager = yield* SparkWalletManagerService
      yield* manager.bootstrap()
      const snapshot = yield* manager.snapshot()
      const projected = projectSparkWalletSnapshotForRenderer(snapshot)

      const serialized = JSON.stringify(projected)
      const mnemonic = harness.connectMnemonics[0] ?? ""
      const firstWord = mnemonic.split(/\s+/g)[0] ?? ""

      expect(serialized.includes(firstWord)).toBe(false)
      expect(projected.lifecycle).toBe("connected")
      expect(projected.mnemonicStored).toBe(true)
    }).pipe(
      Effect.provide(harness.layer),
      Effect.ensuring(Effect.sync(harness.cleanup)),
    )
  })
})
