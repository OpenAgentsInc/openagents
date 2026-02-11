import { Effect } from "effect"
import { describe, expect, it } from "@effect/vitest"

import {
  decodeLndBalanceSummary,
  decodeLndBalanceSummarySync,
  decodeLndChannelSummary,
  decodeLndChannelSummarySync,
  decodeLndNodeInfo,
  decodeLndNodeInfoSync,
  decodeLndNodeSnapshot,
  decodeLndNodeSnapshotSync,
  decodeLndSyncState,
  decodeLndSyncStateSync,
  encodeLndBalanceSummary,
  encodeLndBalanceSummarySync,
  encodeLndChannelSummary,
  encodeLndChannelSummarySync,
  encodeLndNodeInfo,
  encodeLndNodeInfoSync,
  encodeLndNodeSnapshot,
  encodeLndNodeSnapshotSync,
  encodeLndSyncState,
  encodeLndSyncStateSync,
  type LndBalanceSummary,
  type LndChannelSummary,
  type LndNodeInfo,
} from "../src/contracts/lnd.js"
import {
  decodeLndInvoiceCreateRequest,
  decodeLndInvoiceCreateRequestSync,
  decodeLndInvoiceListResult,
  decodeLndInvoiceListResultSync,
  decodeLndInvoiceLookupRequest,
  decodeLndInvoiceLookupRequestSync,
  decodeLndInvoiceRecord,
  decodeLndInvoiceRecordSync,
  decodeLndPaymentListResult,
  decodeLndPaymentListResultSync,
  decodeLndPaymentRecord,
  decodeLndPaymentRecordSync,
  decodeLndPaymentSendRequest,
  decodeLndPaymentSendRequestSync,
  decodeLndPaymentTrackRequest,
  decodeLndPaymentTrackRequestSync,
  decodeLndRpcRequest,
  decodeLndRpcRequestSync,
  decodeLndRpcResponse,
  decodeLndRpcResponseSync,
  encodeLndInvoiceCreateRequest,
  encodeLndInvoiceCreateRequestSync,
  encodeLndInvoiceListResult,
  encodeLndInvoiceListResultSync,
  encodeLndInvoiceLookupRequest,
  encodeLndInvoiceLookupRequestSync,
  encodeLndInvoiceRecord,
  encodeLndInvoiceRecordSync,
  encodeLndPaymentListResult,
  encodeLndPaymentListResultSync,
  encodeLndPaymentRecord,
  encodeLndPaymentRecordSync,
  encodeLndPaymentSendRequest,
  encodeLndPaymentSendRequestSync,
  encodeLndPaymentTrackRequest,
  encodeLndPaymentTrackRequestSync,
  encodeLndRpcRequest,
  encodeLndRpcRequestSync,
  encodeLndRpcResponse,
  encodeLndRpcResponseSync,
} from "../src/contracts/rpc.js"

const sync: ReturnType<typeof decodeLndSyncStateSync> = {
  syncedToChain: true,
  blockHeight: 1,
  blockHash: "hash_1",
}

const nodeInfo: LndNodeInfo = {
  nodePubkey: "pub_1",
  alias: "alias_1",
  network: "regtest",
  walletState: "locked",
  sync,
  updatedAtMs: 1,
}

const balances: LndBalanceSummary = {
  confirmedSat: 1,
  unconfirmedSat: 2,
  channelLocalSat: 3,
  channelRemoteSat: 4,
  pendingOpenSat: 5,
  updatedAtMs: 6,
}

const channels: LndChannelSummary = {
  openChannels: 1,
  activeChannels: 1,
  inactiveChannels: 0,
  pendingChannels: 0,
  updatedAtMs: 7,
}

const snapshot = {
  info: nodeInfo,
  balances,
  channels,
}

const rpcRequest = {
  method: "POST" as const,
  path: "/v1/invoices",
  query: { test: "1" },
  body: { amountSat: 10 },
}

const rpcResponse = {
  status: 200,
  headers: { "content-type": "application/json" },
  body: { ok: true },
}

const invoiceCreate = {
  amountSat: 10,
  memo: "memo",
  expirySeconds: 60,
}

const invoiceLookup = {
  paymentRequest: "ln_invoice_1",
}

const invoiceRecord = {
  paymentRequest: "ln_invoice_1",
  rHash: "hash_invoice_1",
  amountSat: 10,
  settled: false,
  createdAtMs: 10,
}

const invoiceList = {
  invoices: [invoiceRecord],
  nextOffset: 1,
}

const paymentSend = {
  paymentRequest: "ln_invoice_1",
  feeLimitSat: 2,
  timeoutSeconds: 30,
}

const paymentTrack = {
  paymentHash: "hash_payment_1",
}

const paymentRecord = {
  paymentHash: "hash_payment_1",
  paymentPreimageHex: "ab".repeat(32),
  amountSat: 10,
  feeSat: 1,
  status: "succeeded" as const,
  createdAtMs: 20,
  updatedAtMs: 21,
}

const paymentList = {
  payments: [paymentRecord],
  nextOffset: 2,
}

describe("lnd contracts", () => {
  it.effect("decodes and encodes node contracts", () =>
    Effect.gen(function* () {
      expect((yield* decodeLndSyncState(sync)).blockHash).toBe("hash_1")
      expect((yield* decodeLndNodeInfo(nodeInfo)).alias).toBe("alias_1")
      expect((yield* decodeLndBalanceSummary(balances)).confirmedSat).toBe(1)
      expect((yield* decodeLndChannelSummary(channels)).openChannels).toBe(1)
      expect((yield* decodeLndNodeSnapshot(snapshot)).info.alias).toBe("alias_1")

      expect((yield* encodeLndSyncState(sync)).blockHeight).toBe(1)
      expect((yield* encodeLndNodeInfo(nodeInfo)).nodePubkey).toBe("pub_1")
      expect((yield* encodeLndBalanceSummary(balances)).pendingOpenSat).toBe(5)
      expect((yield* encodeLndChannelSummary(channels)).activeChannels).toBe(1)
      expect((yield* encodeLndNodeSnapshot(snapshot)).channels.updatedAtMs).toBe(7)

      expect(decodeLndSyncStateSync(sync).blockHash).toBe("hash_1")
      expect(decodeLndNodeInfoSync(nodeInfo).alias).toBe("alias_1")
      expect(decodeLndBalanceSummarySync(balances).confirmedSat).toBe(1)
      expect(decodeLndChannelSummarySync(channels).openChannels).toBe(1)
      expect(decodeLndNodeSnapshotSync(snapshot).info.nodePubkey).toBe("pub_1")

      expect(encodeLndSyncStateSync(sync).blockHeight).toBe(1)
      expect(encodeLndNodeInfoSync(nodeInfo).nodePubkey).toBe("pub_1")
      expect(encodeLndBalanceSummarySync(balances).pendingOpenSat).toBe(5)
      expect(encodeLndChannelSummarySync(channels).activeChannels).toBe(1)
      expect(encodeLndNodeSnapshotSync(snapshot).channels.updatedAtMs).toBe(7)
    }),
  )

  it.effect("decodes and encodes rpc contracts", () =>
    Effect.gen(function* () {
      expect((yield* decodeLndRpcRequest(rpcRequest)).path).toBe("/v1/invoices")
      expect((yield* decodeLndRpcResponse(rpcResponse)).status).toBe(200)
      expect((yield* decodeLndInvoiceCreateRequest(invoiceCreate)).amountSat).toBe(10)
      expect((yield* decodeLndInvoiceLookupRequest(invoiceLookup)).paymentRequest).toBe("ln_invoice_1")
      expect((yield* decodeLndInvoiceRecord(invoiceRecord)).rHash).toBe("hash_invoice_1")
      expect((yield* decodeLndInvoiceListResult(invoiceList)).invoices.length).toBe(1)
      expect((yield* decodeLndPaymentSendRequest(paymentSend)).feeLimitSat).toBe(2)
      expect((yield* decodeLndPaymentTrackRequest(paymentTrack)).paymentHash).toBe("hash_payment_1")
      expect((yield* decodeLndPaymentRecord(paymentRecord)).status).toBe("succeeded")
      expect((yield* decodeLndPaymentListResult(paymentList)).payments.length).toBe(1)

      expect((yield* encodeLndRpcRequest(rpcRequest)).method).toBe("POST")
      expect((yield* encodeLndRpcResponse(rpcResponse)).status).toBe(200)
      expect((yield* encodeLndInvoiceCreateRequest(invoiceCreate)).amountSat).toBe(10)
      expect((yield* encodeLndInvoiceLookupRequest(invoiceLookup)).paymentRequest).toBe("ln_invoice_1")
      expect((yield* encodeLndInvoiceRecord(invoiceRecord)).rHash).toBe("hash_invoice_1")
      expect((yield* encodeLndInvoiceListResult(invoiceList)).invoices.length).toBe(1)
      expect((yield* encodeLndPaymentSendRequest(paymentSend)).feeLimitSat).toBe(2)
      expect((yield* encodeLndPaymentTrackRequest(paymentTrack)).paymentHash).toBe("hash_payment_1")
      expect((yield* encodeLndPaymentRecord(paymentRecord)).status).toBe("succeeded")
      expect((yield* encodeLndPaymentListResult(paymentList)).payments.length).toBe(1)

      expect(decodeLndRpcRequestSync(rpcRequest).path).toBe("/v1/invoices")
      expect(decodeLndRpcResponseSync(rpcResponse).status).toBe(200)
      expect(decodeLndInvoiceCreateRequestSync(invoiceCreate).amountSat).toBe(10)
      expect(decodeLndInvoiceLookupRequestSync(invoiceLookup).paymentRequest).toBe("ln_invoice_1")
      expect(decodeLndInvoiceRecordSync(invoiceRecord).rHash).toBe("hash_invoice_1")
      expect(decodeLndInvoiceListResultSync(invoiceList).invoices.length).toBe(1)
      expect(decodeLndPaymentSendRequestSync(paymentSend).feeLimitSat).toBe(2)
      expect(decodeLndPaymentTrackRequestSync(paymentTrack).paymentHash).toBe("hash_payment_1")
      expect(decodeLndPaymentRecordSync(paymentRecord).status).toBe("succeeded")
      expect(decodeLndPaymentListResultSync(paymentList).payments.length).toBe(1)

      expect(encodeLndRpcRequestSync(rpcRequest).method).toBe("POST")
      expect(encodeLndRpcResponseSync(rpcResponse).status).toBe(200)
      expect(encodeLndInvoiceCreateRequestSync(invoiceCreate).amountSat).toBe(10)
      expect(encodeLndInvoiceLookupRequestSync(invoiceLookup).paymentRequest).toBe("ln_invoice_1")
      expect(encodeLndInvoiceRecordSync(invoiceRecord).rHash).toBe("hash_invoice_1")
      expect(encodeLndInvoiceListResultSync(invoiceList).invoices.length).toBe(1)
      expect(encodeLndPaymentSendRequestSync(paymentSend).feeLimitSat).toBe(2)
      expect(encodeLndPaymentTrackRequestSync(paymentTrack).paymentHash).toBe("hash_payment_1")
      expect(encodeLndPaymentRecordSync(paymentRecord).status).toBe("succeeded")
      expect(encodeLndPaymentListResultSync(paymentList).payments.length).toBe(1)
    }),
  )

  it.effect("returns typed deterministic decode failures", () =>
    Effect.gen(function* () {
      const badNode = yield* Effect.either(
        decodeLndNodeInfo({
          ...nodeInfo,
          walletState: "oops",
        }),
      )
      expect(badNode._tag).toBe("Left")
      if (badNode._tag === "Left") {
        expect(badNode.left._tag).toBe("LndContractDecodeError")
        if (badNode.left._tag === "LndContractDecodeError") {
          expect(badNode.left.contract).toBe("LndNodeInfo")
        }
      }

      const badPayment = yield* Effect.either(
        decodeLndPaymentRecord({
          ...paymentRecord,
          amountSat: -1,
        }),
      )
      expect(badPayment._tag).toBe("Left")
      if (badPayment._tag === "Left") {
        expect(badPayment.left._tag).toBe("LndContractDecodeError")
        if (badPayment.left._tag === "LndContractDecodeError") {
          expect(badPayment.left.contract).toBe("LndPaymentRecord")
        }
      }

      expect(() =>
        decodeLndSyncStateSync({
          ...sync,
          blockHeight: -1,
        }),
      ).toThrowError(
        expect.objectContaining({
          _tag: "LndContractDecodeError",
          contract: "LndSyncState",
        }),
      )

      expect(() =>
        decodeLndRpcRequestSync({
          ...rpcRequest,
          method: "FETCH",
        }),
      ).toThrowError(
        expect.objectContaining({
          _tag: "LndContractDecodeError",
          contract: "LndRpcRequest",
        }),
      )
    }),
  )
})
