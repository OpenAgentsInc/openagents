import { afterEach, describe, expect, test } from "bun:test"
import { assertPublicProjectionSafe } from "../src/state"
import {
  __resetWarmSparkSessionsForTest,
  closeWarmSparkSession,
  createSparkBackupHelper,
  createSparkBackupSendTransfer,
  syncWarmSparkSession,
} from "../src/spark-backup-helper"

const TEST_MNEMONIC =
  "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"

// A build/sync-counting fake Breez SDK Spark module exposing the SdkBuilder path
// (the Bun-preferred construction the warm session uses). Every `build()`
// increments `builds`; every `syncWallet`/`disconnect` is counted too, so the
// tests can PROVE the warm session builds once and is never disconnected per op.
function countingSparkModule() {
  const counts = { builds: 0, syncs: 0, disconnects: 0, sends: 0 }
  const makeSdk = () => ({
    getInfo: async () => ({ balanceSats: 4242 }),
    receivePayment: async (r: { paymentMethod: { type: string } }) => {
      if (r.paymentMethod.type !== "sparkAddress") throw new Error("only sparkAddress receive")
      return { paymentRequest: "sp1pgssyfakesparkaddress" }
    },
    listPayments: async () => ({ payments: [] }),
    listUnclaimedDeposits: async () => ({ deposits: [] }),
    syncWallet: async () => {
      counts.syncs += 1
      return undefined
    },
    prepareSendPayment: async (request: { paymentRequest: string; amount?: bigint }) => ({
      prepared: true,
      paymentMethod: { type: "bolt11Invoice" },
      paymentRequest: request.paymentRequest,
    }),
    sendPayment: async () => {
      counts.sends += 1
      return {
        payment: { id: "spark-payment-1", amount: 4242n, fees: 3n, status: "complete" },
      }
    },
    disconnect: () => {
      counts.disconnects += 1
    },
  })
  const mod = {
    counts,
    defaultConfig: (network: string) => ({ network, apiKey: undefined as string | undefined }),
    SdkBuilder: {
      new: (_config: Record<string, unknown>, _seed: unknown) => {
        const builder = {
          withStorage: (_storage: unknown) => builder,
          build: async () => {
            counts.builds += 1
            return makeSdk()
          },
        }
        return builder
      },
    },
  }
  return mod
}

describe("#5207 warm Spark session", () => {
  afterEach(async () => {
    // Disconnect + clear the process-level registry between tests so each test
    // starts with a fresh build count and no leaked session.
    await closeWarmSparkSession()
    __resetWarmSparkSessionsForTest()
  })

  test("two warm ops build the SDK exactly ONCE (warm reuse)", async () => {
    const mod = countingSparkModule()
    const helper = createSparkBackupHelper({
      apiKey: "k",
      mnemonic: TEST_MNEMONIC,
      warmSession: true,
      loadModule: async () => mod as never,
    })
    const a = await helper("address")
    const b = await helper("status")
    expect(a.exitCode).toBe(0)
    expect(b.exitCode).toBe(0)
    // Built once, reused for the second op; never disconnected per op.
    expect(mod.counts.builds).toBe(1)
    expect(mod.counts.disconnects).toBe(0)
  })

  test("the COLD path (flag off) builds per op and disconnects per op (unchanged)", async () => {
    const mod = countingSparkModule()
    const helper = createSparkBackupHelper({
      apiKey: "k",
      mnemonic: TEST_MNEMONIC,
      warmSession: false,
      loadModule: async () => mod as never,
    })
    await helper("address")
    await helper("status")
    // Cold: a fresh SDK per command, each disconnected in finally.
    expect(mod.counts.builds).toBe(2)
    expect(mod.counts.disconnects).toBe(2)
  })

  test("concurrent get-or-build dedupes to a SINGLE build (no two SDKs for one wallet)", async () => {
    const mod = countingSparkModule()
    const helper = createSparkBackupHelper({
      apiKey: "k",
      mnemonic: TEST_MNEMONIC,
      warmSession: true,
      loadModule: async () => mod as never,
    })
    // Fire several ops at once before any build resolves; the in-flight build
    // promise must be shared so exactly one SDK is constructed.
    const results = await Promise.all([
      helper("address"),
      helper("status"),
      helper("address"),
      helper("status"),
    ])
    for (const r of results) expect(r.exitCode).toBe(0)
    expect(mod.counts.builds).toBe(1)
    expect(mod.counts.disconnects).toBe(0)
  })

  test("background sync calls syncWallet on the warm session and records freshness", async () => {
    const mod = countingSparkModule()
    const config = {
      apiKey: "k",
      mnemonic: TEST_MNEMONIC,
      warmSession: true,
      loadModule: async () => mod as never,
    }
    const first = await syncWarmSparkSession(config)
    expect(first.synced).toBe(true)
    expect(mod.counts.syncs).toBe(1)
    // A second background sync reuses the SAME warm SDK (one build total).
    const second = await syncWarmSparkSession(config)
    expect(second.synced).toBe(true)
    expect(mod.counts.builds).toBe(1)
    expect(mod.counts.syncs).toBe(2)
    expect(mod.counts.disconnects).toBe(0)
  })

  test("benchmark-style: the 2nd warm SEND does NOT rebuild/reconnect, and skips sync after a recent background sync", async () => {
    const mod = countingSparkModule()
    const config = {
      apiKey: "k",
      mnemonic: TEST_MNEMONIC,
      warmSession: true,
      loadModule: async () => mod as never,
    }
    const sendTransfer = createSparkBackupSendTransfer(config)

    // A background sync just ran (the daemon's timer) -> the session is fresh.
    const sync = await syncWarmSparkSession(config)
    expect(sync.synced).toBe(true)
    const buildsAfterSync = mod.counts.builds // === 1
    const syncsAfterBg = mod.counts.syncs // === 1

    // First warm send: reuses the warm SDK (no new build), and because the
    // background sync is within the freshness window, it SKIPS the pre-send sync.
    const send1 = await sendTransfer({
      amountSats: 100,
      destination: "lnbc100n1rawpaymentrequest",
      idempotencyKey: "warm-send-1",
    })
    expect(send1.ok).toBe(true)
    if (!send1.ok) throw new Error("expected send success")
    assertPublicProjectionSafe(send1)

    // Second warm send: still ONE build total, still no per-op disconnect.
    const send2 = await sendTransfer({
      amountSats: 200,
      destination: "lnbc200n1rawpaymentrequest",
      idempotencyKey: "warm-send-2",
    })
    expect(send2.ok).toBe(true)

    // PROOF of warm reuse: exactly one build/connect for the whole sequence and
    // never a per-op disconnect.
    expect(mod.counts.builds).toBe(buildsAfterSync)
    expect(mod.counts.builds).toBe(1)
    expect(mod.counts.disconnects).toBe(0)
    expect(mod.counts.sends).toBe(2)
    // The sends did NOT re-run syncWallet (the recent background sync covered
    // them) — the ~3s pre-send sync is off the critical path.
    expect(mod.counts.syncs).toBe(syncsAfterBg)
    expect(mod.counts.syncs).toBe(1)
    // Public-safe send result (no payment material).
    expect(JSON.stringify(send1)).not.toContain("lnbc100n1rawpaymentrequest")
  })

  test("a warm SEND with no prior background sync force-syncs ONCE before sending (safety preserved)", async () => {
    const mod = countingSparkModule()
    const config = {
      apiKey: "k",
      mnemonic: TEST_MNEMONIC,
      warmSession: true,
      loadModule: async () => mod as never,
    }
    const sendTransfer = createSparkBackupSendTransfer(config)
    // No background sync yet -> the warm session has never synced, so the send
    // path must force a sync once before sending (original safety posture).
    const send = await sendTransfer({
      amountSats: 100,
      destination: "lnbc100n1rawpaymentrequest",
      idempotencyKey: "cold-warm-send-1",
    })
    expect(send.ok).toBe(true)
    expect(mod.counts.builds).toBe(1)
    expect(mod.counts.syncs).toBe(1) // forced once
    expect(mod.counts.disconnects).toBe(0)
  })
})
