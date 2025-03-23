import { breezService } from "@/services/breez/breezService"
import { IWalletStore } from "../types"

export async function receivePayment(store: IWalletStore, amount: number, description?: string) {
  if (!breezService.isInitialized()) {
    throw new Error("Wallet not initialized")
  }

  try {
    const bolt11 = await breezService.receivePayment(amount, description)
    store.setError(null)
    return bolt11
  } catch (error) {
    console.error("[WalletStore] Receive payment error:", error)
    store.setError(error instanceof Error ? error.message : "Failed to create invoice")
    throw error
  }
}
