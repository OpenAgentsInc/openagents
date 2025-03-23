import * as bip39 from "bip39"
import * as SecureStore from "expo-secure-store"

const MNEMONIC_KEY = "onyx_mnemonic_v1"

export class SecureStorageService {
  static async getMnemonic(): Promise<string | null> {
    try {
      return await SecureStore.getItemAsync(MNEMONIC_KEY)
    } catch (error) {
      console.error("Error getting mnemonic from secure storage:", error)
      return null
    }
  }

  static async setMnemonic(mnemonic: string): Promise<boolean> {
    try {
      if (!bip39.validateMnemonic(mnemonic)) {
        throw new Error("Invalid mnemonic")
      }
      await SecureStore.setItemAsync(MNEMONIC_KEY, mnemonic)
      return true
    } catch (error) {
      console.error("Error saving mnemonic to secure storage:", error)
      return false
    }
  }

  static async generateMnemonic(): Promise<string> {
    const mnemonic = bip39.generateMnemonic()
    await SecureStore.setItemAsync(MNEMONIC_KEY, mnemonic)
    return mnemonic
  }

  static async deleteMnemonic(): Promise<void> {
    try {
      await SecureStore.deleteItemAsync(MNEMONIC_KEY)
    } catch (error) {
      console.error("Error deleting mnemonic from secure storage:", error)
    }
  }
}