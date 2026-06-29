import { describe, expect, test } from "bun:test"
import { projectHostInventoryFixture } from "../src/inventory"
import { createOperatorSnapshot, formatOperatorSnapshotText } from "../src/operator"
import { assertPublicProjectionSafe } from "../src/state"
import { sparkPrimaryWalletBalanceFromStatus, type WalletStatusProjection } from "../src/wallet"

const inventory = projectHostInventoryFixture({
  platform: "darwin",
  arch: "arm64",
  cpuCores: 10,
  cpuModel: "Apple M2 Pro",
  totalMemoryBytes: 32 * 1024 * 1024 * 1024,
  freeMemoryBytes: 10 * 1024 * 1024 * 1024,
  homeFreeBytes: 80 * 1024 * 1024 * 1024,
  networkInterfaceCount: 5,
  externalNetworkInterfaceCount: 2,
  opencodeInstalled: true,
  appleFmReady: true,
  now: "2026-06-09T00:00:00.000Z",
})

const wallet: WalletStatusProjection = {
  schema: "openagents.pylon.wallet_status.v0.3",
  configured: true,
  daemonOnline: true,
  balanceSats: 50_000,
  receiveReady: true,
  sendReady: true,
  readiness: "send-ready",
  blockerRefs: [],
  payoutTargetRefs: ["payout.bolt12.publicref"],
  settlementRefs: [],
  unifiedBalance: sparkPrimaryWalletBalanceFromStatus(
    {
      balanceSats: 21,
      sendReady: true,
    },
    {
      detectedBalanceSats: 50_000,
      claimableHtlcSats: null,
      nextActionRefs: [],
      state: "credited",
    },
  ),
}

describe("Pylon operator snapshot", () => {
  test("builds safe operate, wallet, inspect, and recovery state without raw wallet material", () => {
    const snapshot = createOperatorSnapshot({
      inventory,
      wallet,
      recentJobRefs: ["assignment.public.job1"],
      marketActivityRefs: ["market.activity.public1"],
      receiptRefs: ["assignment.closeout.public1"],
    })
    const text = formatOperatorSnapshotText(snapshot)

    expect(snapshot.desiredMode).toBe("automated")
    expect(snapshot.earningsState).toBe("pending-settlement")
    expect(snapshot.wallet.balanceKnown).toBe(true)
    expect(snapshot.wallet.unifiedBalance.totalVisibleSats).toBe(50_000)
    expect(snapshot.inspect.eligibleInventoryCount).toBe(1)
    expect(snapshot.recovery.operatorOptInRequired).toBe(true)
    expect(snapshot.recovery.sandboxProfileRequired).toBe(true)
    expect(snapshot.recovery.budgetRequired).toBe(true)
    expect(snapshot.recovery.noWalletSecretEvidenceRequired).toBe(true)
    expect(text).toContain("Operate: automated")
    expect(text).toContain("Agent balance: 50000 sats")
    expect(text).not.toContain("Total visible:")
    expect(text).toContain("Recovery: opt-in gates")
    expect(JSON.stringify(snapshot)).not.toContain("lnbc")
    expect(JSON.stringify(snapshot)).not.toContain("mnemonic")
    assertPublicProjectionSafe(snapshot)
  })

  test("keeps local-agent self-steering blocked behind explicit operator gates", () => {
    const snapshot = createOperatorSnapshot({ inventory, wallet: { ...wallet, receiveReady: false, balanceSats: null } })

    expect(snapshot.recovery.headlessCommandRefs).toContain("command.pylon.status_json")
    expect(snapshot.recovery.headlessCommandRefs).toContain("command.pylon.assignment_poll")
    expect(snapshot.wallet.balanceKnown).toBe(false)
    expect(snapshot.wallet.balanceSats).toBeNull()
  })

  test("rejects unsafe operator snapshot payloads", () => {
    expect(() =>
      createOperatorSnapshot({
        inventory,
        wallet: {
          ...wallet,
          payoutTargetRefs: ["lnbc10n1rawinvoice"],
        },
      }),
    ).toThrow()
  })
})
