import type { PylonHostInventoryProjection } from "./inventory.js"
import { assertPublicProjectionSafe } from "./state.js"
import type { UnifiedWalletBalanceProjection, WalletStatusProjection } from "./wallet.js"

export type OperatorMode = "automated" | "inspect" | "recovery"

export type PylonOperatorSnapshot = {
  schema: "openagents.pylon.operator_snapshot.v0.3"
  desiredMode: OperatorMode
  intakeState: "automatic" | "paused" | "blocked"
  earningsState: "no-spend" | "pending-settlement" | "blocked"
  recentJobRefs: string[]
  marketActivityRefs: string[]
  receiptRefs: string[]
  blockerRefs: string[]
  wallet: {
    readiness: string
    networkRef: "network.lightning.regtest_or_mainnet_unknown"
    balanceKnown: boolean
    balanceSats: number | null
    payoutTargetRefs: string[]
    settlementRefs: string[]
    blockerRefs: string[]
    unifiedBalance: UnifiedWalletBalanceProjection
  }
  inspect: {
    inventoryFreshness: string
    eligibleInventoryCount: number
    backendRefs: string[]
    resourceMode: string
    blockerRefs: string[]
  }
  recovery: {
    headlessCommandRefs: string[]
    operatorOptInRequired: boolean
    sandboxProfileRequired: boolean
    budgetRequired: boolean
    noWalletSecretEvidenceRequired: boolean
  }
}

export function createOperatorSnapshot(input: {
  inventory: PylonHostInventoryProjection
  wallet: WalletStatusProjection
  recentJobRefs?: string[]
  marketActivityRefs?: string[]
  receiptRefs?: string[]
  desiredMode?: OperatorMode
}) {
  const blockerRefs = [...new Set([...input.inventory.blockerRefs, ...input.wallet.blockerRefs])]
  const snapshot: PylonOperatorSnapshot = {
    schema: "openagents.pylon.operator_snapshot.v0.3",
    desiredMode: input.desiredMode ?? "automated",
    intakeState: blockerRefs.length > 0 ? "blocked" : "automatic",
    earningsState: input.wallet.sendReady ? "pending-settlement" : input.wallet.receiveReady ? "no-spend" : "blocked",
    recentJobRefs: input.recentJobRefs ?? [],
    marketActivityRefs: input.marketActivityRefs ?? [],
    receiptRefs: input.receiptRefs ?? [],
    blockerRefs,
    wallet: {
      readiness: input.wallet.readiness,
      networkRef: "network.lightning.regtest_or_mainnet_unknown",
      balanceKnown: input.wallet.balanceSats !== null,
      balanceSats: input.wallet.balanceSats,
      payoutTargetRefs: input.wallet.payoutTargetRefs,
      settlementRefs: input.wallet.settlementRefs,
      blockerRefs: input.wallet.blockerRefs,
      unifiedBalance: input.wallet.unifiedBalance,
    },
    inspect: {
      inventoryFreshness: input.inventory.freshness,
      eligibleInventoryCount: input.inventory.eligibleInventoryCount,
      backendRefs: input.inventory.backendHealth.map((backend) => `${backend.backendRef}.${backend.state}`),
      resourceMode: input.inventory.resourceMode,
      blockerRefs: input.inventory.blockerRefs,
    },
    recovery: {
      headlessCommandRefs: [
        "command.pylon.status_json",
        "command.pylon.inventory_json",
        "command.pylon.wallet_status",
        "command.pylon.assignment_poll",
      ],
      operatorOptInRequired: true,
      sandboxProfileRequired: true,
      budgetRequired: true,
      noWalletSecretEvidenceRequired: true,
    },
  }
  assertPublicProjectionSafe(snapshot)
  return snapshot
}

export function formatOperatorSnapshotText(snapshot: PylonOperatorSnapshot) {
  const sparkPrimary = snapshot.wallet.unifiedBalance.primaryRail === "spark"
  const walletBalance = snapshot.wallet.balanceKnown ? `${snapshot.wallet.balanceSats} sats` : "--"
  const totalVisible = snapshot.wallet.unifiedBalance.totalVisibleSats === null
    ? "--"
    : `${snapshot.wallet.unifiedBalance.totalVisibleSats} sats`
  const backendRefs = snapshot.inspect.backendRefs.slice(0, 4).join("\n ")
  const blockers = snapshot.blockerRefs.length > 0 ? snapshot.blockerRefs.slice(0, 4).join("\n ") : "none"

  return [
    `Operate: ${snapshot.desiredMode}`,
    `Intake: ${snapshot.intakeState}`,
    `Earnings: ${snapshot.earningsState}`,
    `Jobs: ${snapshot.recentJobRefs.length}`,
    `Market: ${snapshot.marketActivityRefs.length}`,
    "",
    `Wallet: ${snapshot.wallet.readiness}`,
    sparkPrimary ? `Agent balance: ${walletBalance}` : `Balance: ${walletBalance}`,
    ...(sparkPrimary ? [] : [`Total visible: ${totalVisible}`]),
    `Receipts: ${snapshot.receiptRefs.length}`,
    "",
    `Inspect: ${snapshot.inspect.inventoryFreshness}`,
    `Eligible: ${snapshot.inspect.eligibleInventoryCount}`,
    backendRefs ? ` ${backendRefs}` : " backends: none",
    "",
    `Recovery: opt-in gates`,
    ` ${snapshot.recovery.headlessCommandRefs.join("\n ")}`,
    "",
    `Blockers: ${blockers}`,
  ].join("\n")
}
