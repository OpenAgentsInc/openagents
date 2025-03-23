import Constants from "expo-constants"
import { breezService } from "@/services/breez"
import { SecureStorageService } from "@/services/storage/secureStorage"
import { IWalletStore } from "../types"
import { fetchBalanceInfo } from "./fetchBalanceInfo"

export async function restoreWallet(store: IWalletStore, mnemonic: string) {
  try {
    // First disconnect if we're initialized
    if (breezService.isInitialized()) {
      await breezService.disconnect()
    }

    // Reset the store state
    store.setInitialized(false)
    store.setBalanceSat(0)
    store.setPendingSendSat(0)
    store.setPendingReceiveSat(0)
    store.setTransactions([])
    store.setMnemonic(undefined)  // Changed from null to undefined

    // Validate and save mnemonic to secure storage
    const saved = await SecureStorageService.setMnemonic(mnemonic)
    if (!saved) {
      throw new Error("Failed to save mnemonic")
    }

    // Set mnemonic in store
    store.setMnemonic(mnemonic)

    // Initialize with new mnemonic
    const breezApiKey = Constants.expoConfig?.extra?.BREEZ_API_KEY
    if (!breezApiKey) {
      throw new Error("BREEZ_API_KEY not set")
    }

    // Initialize breez with the new mnemonic
    await breezService.initialize({
      workingDir: "",
      apiKey: breezApiKey,
      network: "MAINNET",
      mnemonic: mnemonic,
    })

    store.setInitialized(true)
    store.setError(null)

    // Fetch initial balance
    await fetchBalanceInfo(store)
    return true
  } catch (error) {
    console.error("[WalletStore] Restoration error:", error)
    store.setError(error instanceof Error ? error.message : "Failed to restore wallet")
    return false
  }
}