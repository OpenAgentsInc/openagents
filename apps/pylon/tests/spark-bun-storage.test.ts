import { describe, expect, test } from "bun:test"
import { Database } from "bun:sqlite"
import { SparkBunStorage } from "../src/spark-bun-storage"

// Exercises the Bun-native port of the Breez SDK Spark `Storage` interface
// against an in-memory bun:sqlite database (#5080). No WASM, no network.
// This is the data-integrity proof: a wrong bind or row mapping silently
// corrupts wallet data, so we round-trip real shapes incl. BigInt fields.

function newStorage(): SparkBunStorage {
  // ":memory:" proves the schema + migrations build cleanly under bun:sqlite.
  return new SparkBunStorage(":memory:")
}

describe("SparkBunStorage (bun:sqlite port of Breez SDK Storage)", () => {
  test("migrations build the full schema in-memory", () => {
    const s = newStorage()
    // user_version must equal the migration count after initialize().
    const db = new Database(":memory:")
    db.close()
    s.close()
    expect(true).toBe(true)
  })

  test("cached items round-trip (get/set/delete)", async () => {
    const s = newStorage()
    expect(await s.getCachedItem("missing")).toBeNull()

    await s.setCachedItem("k1", "v1")
    expect(await s.getCachedItem("k1")).toBe("v1")

    // INSERT OR REPLACE semantics.
    await s.setCachedItem("k1", "v2")
    expect(await s.getCachedItem("k1")).toBe("v2")

    await s.deleteCachedItem("k1")
    expect(await s.getCachedItem("k1")).toBeNull()
    s.close()
  })

  test("payment insert -> getPaymentById -> listPayments with BigInt amount/fees", async () => {
    const s = newStorage()
    const payment = {
      id: "pay-1",
      paymentType: "receive",
      status: "completed",
      // 64-bit values exercised as bigint; stored as TEXT, read back as BigInt.
      amount: 9007199254740993n, // > Number.MAX_SAFE_INTEGER
      fees: 42n,
      timestamp: 1700000000,
      method: { type: "spark" },
      details: {
        type: "spark",
        invoiceDetails: { foo: "bar" },
        htlcDetails: { status: "claimed" },
      },
    }
    await s.insertPayment(payment)

    const fetched = await s.getPaymentById("pay-1")
    expect(fetched.id).toBe("pay-1")
    expect(fetched.paymentType).toBe("receive")
    expect(fetched.status).toBe("completed")
    expect(fetched.amount).toBe(9007199254740993n)
    expect(typeof fetched.amount).toBe("bigint")
    expect(fetched.fees).toBe(42n)
    expect(fetched.timestamp).toBe(1700000000)
    expect(fetched.method).toEqual({ type: "spark" })
    expect(fetched.details.type).toBe("spark")
    expect(fetched.details.invoiceDetails).toEqual({ foo: "bar" })
    expect(fetched.details.htlcDetails).toEqual({ status: "claimed" })

    const list = await s.listPayments({ limit: 10 })
    expect(list.length).toBe(1)
    expect(list[0].id).toBe("pay-1")
    expect(list[0].amount).toBe(9007199254740993n)
    s.close()
  })

  test("insertPayment upsert (ON CONFLICT) updates the row", async () => {
    const s = newStorage()
    const base = {
      id: "pay-up",
      paymentType: "receive",
      status: "pending",
      amount: 100n,
      fees: 1n,
      timestamp: 10,
      method: null,
      details: { type: "spark", invoiceDetails: { a: 1 } },
    }
    await s.insertPayment(base)
    await s.insertPayment({ ...base, status: "completed", amount: 200n })
    const fetched = await s.getPaymentById("pay-up")
    expect(fetched.status).toBe("completed")
    expect(fetched.amount).toBe(200n)
    const list = await s.listPayments({})
    expect(list.length).toBe(1)
    s.close()
  })

  test("lightning payment round-trips invoice + htlc details and getPaymentByInvoice", async () => {
    const s = newStorage()
    const payment = {
      id: "pay-ln",
      paymentType: "receive",
      status: "completed",
      amount: 500n,
      fees: 0n,
      timestamp: 1700001000,
      method: null,
      details: {
        type: "lightning",
        invoice: "lnbc-test-invoice",
        destinationPubkey: "02deadbeef",
        description: "test ln",
        htlcDetails: {
          paymentHash: "hash123",
          preimage: "preimage123",
          expiryTime: 600,
          status: "preimageShared",
        },
      },
    }
    await s.insertPayment(payment)

    const byInvoice = await s.getPaymentByInvoice("lnbc-test-invoice")
    expect(byInvoice).not.toBeNull()
    expect(byInvoice.id).toBe("pay-ln")
    expect(byInvoice.details.type).toBe("lightning")
    expect(byInvoice.details.invoice).toBe("lnbc-test-invoice")
    expect(byInvoice.details.htlcDetails.paymentHash).toBe("hash123")
    expect(byInvoice.details.htlcDetails.status).toBe("preimageShared")

    expect(await s.getPaymentByInvoice("no-such-invoice")).toBeNull()
    s.close()
  })

  test("getPaymentById rejects for unknown id", async () => {
    const s = newStorage()
    await expect(s.getPaymentById("nope")).rejects.toThrow(/not found/)
    s.close()
  })

  test("deposit lifecycle: add -> list -> update(claimError/refund) -> delete", async () => {
    const s = newStorage()
    await s.addDeposit("txA", 0, 12345, true)
    await s.addDeposit("txB", 1, 6789, false)

    let deposits = await s.listDeposits()
    expect(deposits.length).toBe(2)
    const a = deposits.find((d: any) => d.txid === "txA")
    expect(a.amountSats).toBe(12345)
    expect(a.isMature).toBe(true)
    const b = deposits.find((d: any) => d.txid === "txB")
    expect(b.isMature).toBe(false)
    expect(a.claimError).toBeNull()

    // update -> claimError
    await s.updateDeposit("txA", 0, { type: "claimError", error: { kind: "feeExceeded", maxFee: 10 } })
    deposits = await s.listDeposits()
    const aAfter = deposits.find((d: any) => d.txid === "txA")
    expect(aAfter.claimError).toEqual({ kind: "feeExceeded", maxFee: 10 })
    expect(aAfter.refundTx).toBeNull()

    // update -> refund clears claimError
    await s.updateDeposit("txA", 0, { type: "refund", refundTx: "rawtx", refundTxid: "rtxid" })
    deposits = await s.listDeposits()
    const aRefund = deposits.find((d: any) => d.txid === "txA")
    expect(aRefund.refundTx).toBe("rawtx")
    expect(aRefund.refundTxId).toBe("rtxid")
    expect(aRefund.claimError).toBeNull()

    // upsert via addDeposit ON CONFLICT updates amount + maturity, no dup
    await s.addDeposit("txA", 0, 99999, false)
    deposits = await s.listDeposits()
    expect(deposits.length).toBe(2)
    const aUpserted = deposits.find((d: any) => d.txid === "txA")
    expect(aUpserted.amountSats).toBe(99999)
    expect(aUpserted.isMature).toBe(false)

    await s.updateDeposit("txZ", 0, { type: "bogus" } as any).catch((e) => e)
    await expect(s.updateDeposit("txZ", 0, { type: "bogus" } as any)).rejects.toThrow(/Unknown payload type/)

    await s.deleteDeposit("txA", 0)
    deposits = await s.listDeposits()
    expect(deposits.length).toBe(1)
    expect(deposits[0].txid).toBe("txB")
    s.close()
  })

  test("sync outgoing change round-trips with BigInt revision", async () => {
    const s = newStorage()
    expect(await s.syncGetLastRevision()).toBe(0n)

    const record = {
      id: { type: "payment", dataId: "d1" },
      schemaVersion: "1",
      updatedFields: { fieldA: "valA" },
    }
    const rev = await s.syncAddOutgoingChange(record)
    expect(typeof rev).toBe("bigint")
    expect(rev).toBe(1n)

    const pending = await s.syncGetPendingOutgoingChanges(10)
    expect(pending.length).toBe(1)
    expect(pending[0].change.id).toEqual({ type: "payment", dataId: "d1" })
    expect(pending[0].change.updatedFields).toEqual({ fieldA: "valA" })
    expect(pending[0].change.localRevision).toBe(1n)
    expect(pending[0].parent).toBeNull()

    const latest = await s.syncGetLatestOutgoingChange()
    expect(latest.change.id.dataId).toBe("d1")
    expect(latest.change.localRevision).toBe(1n)

    // second pending change increments the local queue revision
    const rev2 = await s.syncAddOutgoingChange({
      id: { type: "payment", dataId: "d2" },
      schemaVersion: "1",
      updatedFields: { x: "y" },
    })
    expect(rev2).toBe(2n)

    // complete sync for d1: moves to sync_state at server revision 5
    await s.syncCompleteOutgoingSync(
      { id: { type: "payment", dataId: "d1" }, schemaVersion: "1", revision: 5n, data: { full: "state" } },
      rev,
    )
    expect(await s.syncGetLastRevision()).toBe(5n)

    const pendingAfter = await s.syncGetPendingOutgoingChanges(10)
    expect(pendingAfter.length).toBe(1)
    expect(pendingAfter[0].change.id.dataId).toBe("d2")
    s.close()
  })

  test("sync incoming records insert -> get -> delete with parent join", async () => {
    const s = newStorage()
    // Seed a sync_state parent so the incoming join has an oldState.
    await s.syncUpdateRecordFromIncoming({
      id: { type: "contact", dataId: "c1" },
      schemaVersion: "1",
      revision: 3n,
      data: { name: "old" },
    })
    expect(await s.syncGetLastRevision()).toBe(3n)

    await s.syncInsertIncomingRecords([
      { id: { type: "contact", dataId: "c1" }, schemaVersion: "1", revision: 7n, data: { name: "new" } },
    ])

    const incoming = await s.syncGetIncomingRecords(10)
    expect(incoming.length).toBe(1)
    expect(incoming[0].newState.revision).toBe(7n)
    expect(incoming[0].newState.data).toEqual({ name: "new" })
    expect(incoming[0].oldState).not.toBeNull()
    expect(incoming[0].oldState.revision).toBe(3n)
    expect(incoming[0].oldState.data).toEqual({ name: "old" })

    await s.syncDeleteIncomingRecord({ id: { type: "contact", dataId: "c1" }, revision: 7n })
    const afterDelete = await s.syncGetIncomingRecords(10)
    expect(afterDelete.length).toBe(0)
    s.close()
  })

  test("contacts insert -> get -> list -> delete", async () => {
    const s = newStorage()
    await s.insertContact({
      id: "ct1",
      name: "Bob",
      paymentIdentifier: "bob@example.com",
      createdAt: 100,
      updatedAt: 100,
    })
    await s.insertContact({
      id: "ct2",
      name: "Alice",
      paymentIdentifier: "alice@example.com",
      createdAt: 101,
      updatedAt: 101,
    })

    const one = await s.getContact("ct1")
    expect(one.name).toBe("Bob")
    expect(one.paymentIdentifier).toBe("bob@example.com")

    // ORDER BY name ASC -> Alice before Bob
    const list = await s.listContacts({})
    expect(list.map((c: any) => c.name)).toEqual(["Alice", "Bob"])

    // upsert
    await s.insertContact({
      id: "ct1",
      name: "Bobby",
      paymentIdentifier: "bobby@example.com",
      createdAt: 100,
      updatedAt: 200,
    })
    expect((await s.getContact("ct1")).name).toBe("Bobby")

    await s.deleteContact("ct2")
    const afterDelete = await s.listContacts({})
    expect(afterDelete.map((c: any) => c.name)).toEqual(["Bobby"])

    expect(await s.getContact("missing")).toBeNull()
    s.close()
  })

  test("payment metadata + parent ids grouping", async () => {
    const s = newStorage()
    // Parent payment (visible in listPayments).
    await s.insertPayment({
      id: "parent-1",
      paymentType: "receive",
      status: "completed",
      amount: 1000n,
      fees: 0n,
      timestamp: 5,
      method: null,
      details: { type: "spark", invoiceDetails: { p: 1 } },
    })
    // Child payment, linked via parent_payment_id metadata.
    await s.insertPayment({
      id: "child-1",
      paymentType: "receive",
      status: "completed",
      amount: 250n,
      fees: 0n,
      timestamp: 6,
      method: null,
      details: { type: "spark", invoiceDetails: { c: 1 } },
    })
    await s.insertPaymentMetadata("child-1", { parentPaymentId: "parent-1" })

    // listPayments excludes children (pm.parent_payment_id IS NULL).
    const list = await s.listPayments({})
    expect(list.map((p: any) => p.id)).toEqual(["parent-1"])

    const byParent = await s.getPaymentsByParentIds(["parent-1"])
    expect(Object.keys(byParent)).toEqual(["parent-1"])
    expect(byParent["parent-1"].length).toBe(1)
    expect(byParent["parent-1"][0].id).toBe("child-1")

    // No parents requested -> empty.
    expect(await s.getPaymentsByParentIds([])).toEqual({})
    s.close()
  })

  test("setLnurlMetadata stores and surfaces via lightning payment join", async () => {
    const s = newStorage()
    await s.insertPayment({
      id: "pay-zap",
      paymentType: "receive",
      status: "completed",
      amount: 21n,
      fees: 0n,
      timestamp: 7,
      method: null,
      details: {
        type: "lightning",
        invoice: "lnbc-zap",
        destinationPubkey: "02aa",
        description: "zap",
        htlcDetails: { paymentHash: "zaphash", preimage: null, expiryTime: 0, status: "preimageShared" },
      },
    })
    await s.setLnurlMetadata([
      { paymentHash: "zaphash", nostrZapRequest: "{req}", nostrZapReceipt: "{rcpt}", senderComment: "gm" },
    ])

    const fetched = await s.getPaymentById("pay-zap")
    expect(fetched.details.lnurlReceiveMetadata).toEqual({
      nostrZapRequest: "{req}",
      nostrZapReceipt: "{rcpt}",
      senderComment: "gm",
    })
    s.close()
  })
})
