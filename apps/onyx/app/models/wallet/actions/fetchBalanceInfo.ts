import { breezService } from "@/services/breez/breezService"
import { IWalletStore } from "../types"

export async function fetchBalanceInfo(store: IWalletStore) {
  if (!store.isInitialized || !breezService.isInitialized()) {
    return
  }

  try {
    const balance = await breezService.getBalance()
    store.setBalanceSat(balance.balanceSat)
    store.setPendingSendSat(balance.pendingSendSat)
    store.setPendingReceiveSat(balance.pendingReceiveSat)
    store.setError(null)
  } catch (error) {
    console.error("[WalletStore] Balance fetch error:", error)
    store.setError(error instanceof Error ? error.message : "Failed to fetch balance")
  }
}
