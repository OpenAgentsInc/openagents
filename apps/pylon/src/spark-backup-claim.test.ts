import { describe, expect, it } from "bun:test"
import { createSparkBackupHelper } from "./spark-backup-helper.js"

const fakeModule = (sdk: unknown) =>
  ({
    defaultConfig: () => ({}),
    SdkBuilder: { new: () => ({ withStorage: () => ({ build: async () => sdk }) }) },
  }) as never

describe("spark backup helper: claim pending Lightning HTLCs (#5166)", () => {
  it("claims a pending HTLC by its preimage and reports the post-claim balance", async () => {
    const claimed: string[] = []
    let balance = 0
    const sdk = {
      syncWallet: async () => ({}),
      listPayments: async () => ({
        payments: [
          { method: "lightning", status: "pending", amount: 50000n, details: { type: "lightning", htlcDetails: { status: "waitingForPreimage", preimage: "abc123" } } },
          { method: "lightning", status: "pending", details: { type: "lightning", htlcDetails: { status: "preimageShared" } } },
        ],
      }),
      claimHtlcPayment: async (req: { preimage: string }) => {
        claimed.push(req.preimage)
        balance += 50000
        return { payment: { amount: 50000n } }
      },
      getInfo: async () => ({ balanceSats: balance }),
      disconnect: async () => {},
    }
    const helper = createSparkBackupHelper({ apiKey: "k", mnemonic: "seed words here", loadModule: async () => fakeModule(sdk) })
    const res = await helper("claim")
    expect(res.exitCode).toBe(0)
    const data = JSON.parse(res.stdout)
    expect(claimed).toEqual(["abc123"])
    expect(data.claimed_count).toBe(1)
    expect(data.claimed_sats).toBe(50000)
    expect(data.balance_sats).toBe(50000)
    expect(data.claimable_seen).toBe(1)
  })

  it("claims nothing and does not throw when no HTLC is pending", async () => {
    const sdk = {
      syncWallet: async () => ({}),
      listPayments: async () => ({ payments: [] }),
      claimHtlcPayment: async () => { throw new Error("must not be called") },
      getInfo: async () => ({ balanceSats: 0 }),
      disconnect: async () => {},
    }
    const helper = createSparkBackupHelper({ apiKey: "k", mnemonic: "seed words here", loadModule: async () => fakeModule(sdk) })
    const res = await helper("claim")
    const data = JSON.parse(res.stdout)
    expect(data.claimed_count).toBe(0)
    expect(data.balance_sats).toBe(0)
  })
})
