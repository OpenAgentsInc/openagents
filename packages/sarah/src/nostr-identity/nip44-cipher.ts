/**
 * SarahNostrCipher backed by real NIP-44 v2 (owner conversation key).
 */
import { hexToBytes } from "@noble/hashes/utils";

import { decrypt, encrypt, getConversationKey } from "./nip44.ts";
import type { SarahNostrCipher } from "../nostr-turn/types.ts";

/**
 * Build a NIP-44 cipher from Sarah's secret key and the owner public key.
 * Secret key bytes stay in the caller/signer closure — pass only for encrypt.
 */
export const makeNip44OwnerCipher = (input: {
  readonly sarahSecretKey: Uint8Array;
  readonly ownerPubkeyHex: string;
}): SarahNostrCipher & {
  readonly decryptFromSarah: (ciphertext: string) => string;
} => {
  const conversationKey = getConversationKey(
    input.sarahSecretKey,
    input.ownerPubkeyHex.toLowerCase(),
  );
  return {
    encryptToOwner: (plaintext: string) => encrypt(plaintext, conversationKey),
    decryptFromSarah: (ciphertext: string) => decrypt(ciphertext, conversationKey),
  };
};

/** Parse 64-hex secret into bytes. */
export const secretKeyFromHex = (hex: string): Uint8Array =>
  hexToBytes(hex.trim().toLowerCase());
