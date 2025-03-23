import Constants from "expo-constants"
import { breezService } from "@/services/breez"
import { nostr } from "@/services/nostr/nostr"
import { SecureStorageService } from "@/services/storage/secureStorage"
import { log } from "@/utils/log"
import { IWalletStore } from "../types"

export async function setup(store: IWalletStore) {
  try {
    // Get mnemonic from secure storage
    let mnemonic = await SecureStorageService.getMnemonic()
    
    // Generate new mnemonic if none exists
    if (!mnemonic) {
      mnemonic = await SecureStorageService.generateMnemonic()
      
      // Note: generateMnemonic already saves the mnemonic internally
      // No need to call setMnemonic again
    }

    // Set mnemonic in store
    store.setMnemonic(mnemonic)

    // Initialize breez with the mnemonic
    const breezApiKey = Constants.expoConfig?.extra?.BREEZ_API_KEY
    if (!breezApiKey) {
      throw new Error("BREEZ_API_KEY not set")
    }

    await breezService.initialize({
      workingDir: "",
      apiKey: breezApiKey,
      network: "MAINNET",
      mnemonic: mnemonic,
    })

    // Derive and set Nostr keys
    const keys = await nostr.deriveNostrKeys(mnemonic)
    store.setNostrKeys(keys)

    log({
      name: "WalletStore",
      preview: 'Derived Nostr keys',
      value: keys,
      important: true
    })

    // Mark initialization as complete
    store.setInitialized(true)
    store.setError(null)
    
    return true
  } catch (error) {
    console.error("[WalletStore] Setup error:", error)
    store.setError(error instanceof Error ? error.message : "Failed to setup wallet")
    store.setInitialized(false)
    return false
  }
}