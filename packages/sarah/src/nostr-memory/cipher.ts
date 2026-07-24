import { bytesToHex } from "@noble/hashes/utils";

import { decrypt, encrypt, getConversationKey } from "../nostr-identity/nip44.ts";
import type { SarahNostrMemoryCipher } from "./types.ts";

const PREFIX = "nip44:v2:test:";

/**
 * Test cipher: not NIP-44, but never leaves plaintext in wire content.
 * Same pattern as `testSarahNostrCipher` in nostr-turn, with a decrypt path
 * for round-trip tests. Production injects real NIP-44 under conversation key.
 */
export const testSarahNostrMemoryCipher = (): SarahNostrMemoryCipher => ({
  encryptToOwner: (plaintext: string) => {
    const bytes = new TextEncoder().encode(plaintext);
    let bin = "";
    for (const b of bytes) bin += String.fromCharCode(b);
    const b64 = btoa(bin)
      .replace(/\+/g, "-")
      .replace(/\//g, "_")
      .replace(/=+$/, "");
    return `${PREFIX}${b64}`;
  },
  decryptFromOwner: (ciphertext: string) => {
    if (!ciphertext.startsWith(PREFIX)) {
      throw new Error("sarah_nostr_memory: test cipher expected nip44:v2:test: prefix");
    }
    const b64url = ciphertext.slice(PREFIX.length);
    const padded =
      b64url + "=".repeat((4 - (b64url.length % 4)) % 4);
    const b64 = padded.replace(/-/g, "+").replace(/_/g, "/");
    const bin = atob(b64);
    const bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
    return new TextDecoder().decode(bytes);
  },
});

/**
 * Production NIP-44 memory cipher under the agent-owner conversation key.
 * Also returns `conversationKeyHex` for HMAC-blinded engram `d` tags.
 */
export const makeNip44MemoryCipher = (input: {
  readonly sarahSecretKey: Uint8Array;
  readonly ownerPubkeyHex: string;
}): SarahNostrMemoryCipher & { readonly conversationKeyHex: string } => {
  const conversationKey = getConversationKey(
    input.sarahSecretKey,
    input.ownerPubkeyHex.toLowerCase(),
  );
  return {
    conversationKeyHex: bytesToHex(conversationKey),
    encryptToOwner: (plaintext: string) => encrypt(plaintext, conversationKey),
    decryptFromOwner: (ciphertext: string) =>
      decrypt(ciphertext, conversationKey),
  };
};
