import { Effect } from "effect"
import { describe, expect, it } from "@effect/vitest"

import { makeLndDeterministicLayer } from "../src/adapters/lndNodeDeterministic.js"
import { LndInvoiceService } from "../src/services/lndInvoiceService.js"
import { LndNodeService } from "../src/services/lndNodeService.js"
import { LndPaymentService } from "../src/services/lndPaymentService.js"
import { LndTransportService } from "../src/services/lndTransportService.js"
import { LndWalletService } from "../src/services/lndWalletService.js"

describe("lnd-effect service conformance", () => {
  it.effect("simulates node/wallet/invoice/payment lifecycles deterministically", () =>
    Effect.gen(function* () {
      const node = yield* LndNodeService
      const wallet = yield* LndWalletService
      const invoices = yield* LndInvoiceService
      const payments = yield* LndPaymentService
      const transport = yield* LndTransportService

      const info = yield* node.getNodeInfo()
      expect(info.network).toBe("regtest")
      expect(info.walletState).toBe("locked")

      const balances = yield* node.getBalanceSummary()
      expect(balances.confirmedSat).toBe(0)

      const channels = yield* node.getChannelSummary()
      expect(channels.openChannels).toBe(0)

      const snapshot = yield* node.getNodeSnapshot()
      expect(snapshot.info.alias).toBe("openagents-local")

      const walletState = yield* wallet.getWalletState()
      expect(walletState).toBe("locked")

      const created = yield* invoices.createInvoice({
        amountSat: 42,
        memo: "demo",
      })
      expect(created.paymentRequest.startsWith("lnbcrt42")).toBe(true)
      expect(created.settled).toBe(false)

      const fetched = yield* invoices.getInvoice({ paymentRequest: created.paymentRequest })
      expect(fetched?.rHash).toBe(created.rHash)

      const missing = yield* invoices.getInvoice({ paymentRequest: "ln_missing" })
      expect(missing).toBeNull()

      const listed = yield* invoices.listInvoices({ limit: 5, offset: 0 })
      expect(listed.invoices.length).toBe(1)
      expect(listed.invoices[0]?.paymentRequest).toBe(created.paymentRequest)

      const sent = yield* payments.sendPayment({ paymentRequest: created.paymentRequest })
      expect(sent.status).toBe("succeeded")
      expect(sent.paymentPreimageHex?.length).toBe(64)

      const tracked = yield* payments.trackPayment({ paymentHash: sent.paymentHash })
      expect(tracked.paymentHash).toBe(sent.paymentHash)
      expect(tracked.status).toBe("succeeded")

      const inFlight = yield* payments.trackPayment({ paymentHash: "hash_not_found" })
      expect(inFlight.status).toBe("in_flight")

      const paymentList = yield* payments.listPayments({ limit: 10 })
      expect(paymentList.payments.length).toBe(1)
      expect(paymentList.payments[0]?.paymentHash).toBe(sent.paymentHash)

      const response = yield* transport.send({
        method: "GET",
        path: "/v1/getinfo",
      })
      expect(response.status).toBe(200)
      expect(response.body).toMatchObject({
        ok: true,
        path: "/v1/getinfo",
      })
    }).pipe(Effect.provide(makeLndDeterministicLayer())),
  )

  it.effect("supports deterministic seeds and overrides for CI-friendly tests", () =>
    Effect.gen(function* () {
      const node = yield* LndNodeService
      const wallet = yield* LndWalletService
      const invoices = yield* LndInvoiceService
      const payments = yield* LndPaymentService

      const info = yield* node.getNodeInfo()
      expect(info.alias).toBe("custom-local")

      const state = yield* wallet.getWalletState()
      expect(state).toBe("unlocked")

      const invoiceRows = yield* invoices.listInvoices()
      expect(invoiceRows.invoices.length).toBe(1)

      const paymentRows = yield* payments.listPayments()
      expect(paymentRows.payments.length).toBe(1)
      expect(paymentRows.payments[0]?.status).toBe("failed")
    }).pipe(
      Effect.provide(
        makeLndDeterministicLayer({
          walletState: "unlocked",
          nodeInfo: {
            nodePubkey: "02c3f8f2e7f0d5f9fd0ac5f4b6a2706b2d4f0458a5d8f25be2f8fbf39cccf8f5d1",
            alias: "custom-local",
            network: "simnet",
            walletState: "unlocked",
            sync: {
              syncedToChain: true,
              blockHeight: 123,
              blockHash: "0000000000000000000000000000000000000000000000000000000000000123",
            },
            updatedAtMs: 1_700_000_000_123,
          },
          seedInvoices: [
            {
              paymentRequest: "ln_seeded_1",
              rHash: "hash_seeded_1",
              amountSat: 33,
              settled: true,
              createdAtMs: 1_700_000_000_000,
              settledAtMs: 1_700_000_000_010,
            },
          ],
          seedPayments: [
            {
              paymentHash: "payment_seeded_1",
              amountSat: 50,
              feeSat: 1,
              status: "failed",
              failureReason: "seeded_failure",
              createdAtMs: 1_700_000_000_000,
              updatedAtMs: 1_700_000_000_020,
            },
          ],
        }),
      ),
    ),
  )
})
