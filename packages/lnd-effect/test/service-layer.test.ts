import { Effect } from "effect"
import { describe, expect, it } from "@effect/vitest"

import { makeLndDeterministicLayer } from "../src/adapters/lndNodeDeterministic.js"
import { decodeLndNodeInfo } from "../src/contracts/lnd.js"
import { LndInvoiceService } from "../src/services/lndInvoiceService.js"
import { LndNodeService } from "../src/services/lndNodeService.js"
import { LndPaymentService } from "../src/services/lndPaymentService.js"
import { LndTransportService } from "../src/services/lndTransportService.js"
import { LndWalletService } from "../src/services/lndWalletService.js"

describe("lnd-effect service boundaries", () => {
  it.effect("provides deterministic node/wallet/invoice/payment services", () =>
    Effect.gen(function* () {
      const node = yield* LndNodeService
      const wallet = yield* LndWalletService
      const invoices = yield* LndInvoiceService
      const payments = yield* LndPaymentService
      const transport = yield* LndTransportService

      const nodeInfo = yield* node.getNodeInfo()
      expect(nodeInfo.network).toBe("regtest")
      expect(nodeInfo.walletState).toBe("locked")

      const decodedNode = yield* decodeLndNodeInfo(nodeInfo)
      expect(decodedNode.alias).toBe("openagents-local")

      const walletState = yield* wallet.getWalletState()
      expect(walletState).toBe("locked")

      const invoice = yield* invoices.createInvoice({ amountSat: 42 })
      expect(invoice.invoice.startsWith("lnbcrt42")).toBe(true)
      expect(invoice.amountSat).toBe(42)

      const payment = yield* payments.trackPayment("payment_hash_demo")
      expect(payment.status).toBe("succeeded")
      expect(payment.preimageHex?.length).toBe(64)

      const response = yield* transport.send({
        method: "GET",
        path: "/v1/getinfo",
      })
      expect(response.status).toBe(200)
      expect(response.body).toMatchObject({
        ok: true,
        path: "/v1/getinfo",
        method: "GET",
      })
    }).pipe(Effect.provide(makeLndDeterministicLayer())),
  )

  it.effect("allows overriding deterministic defaults for tests", () =>
    Effect.gen(function* () {
      const node = yield* LndNodeService
      const wallet = yield* LndWalletService

      const nodeInfo = yield* node.getNodeInfo()
      const walletState = yield* wallet.getWalletState()

      expect(nodeInfo.alias).toBe("custom-local")
      expect(walletState).toBe("unlocked")
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
        }),
      ),
    ),
  )
})
