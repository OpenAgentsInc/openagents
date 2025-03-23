import { getPublicKey, nip19 } from "nostr-tools"
import { sha256 } from "@noble/hashes/sha256"
import { bytesToHex } from "@noble/hashes/utils"
import { NostrKeys } from "./nostr.types"

/**
 * This class let us interact with the Nostr network.
 *
 * It's primarily based on our arclib library: https://github.com/OpenAgentsInc/arclib
 */
export class Nostr {
  constructor() {
    console.log("Nostr class initialized.")
  }

  /**
   * Derives Nostr keys from a mnemonic by hashing it to create a private key
   */
  async deriveNostrKeys(mnemonic: string): Promise<NostrKeys> {
    // Hash the mnemonic to get a 32-byte private key
    const hash = sha256(mnemonic)
    const privateKey = bytesToHex(hash)

    // Get public key using nostr-tools
    const publicKey = getPublicKey(privateKey)

    // Convert to bech32 format
    const npub = nip19.npubEncode(publicKey)
    const nsec = nip19.nsecEncode(privateKey)

    return {
      privateKey,
      publicKey,
      npub,
      nsec
    }
  }
}

// Singleton instance of the API for convenience
export const nostr = new Nostr()
