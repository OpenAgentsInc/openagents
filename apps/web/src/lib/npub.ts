import { decode, npubEncode } from 'nostr-tools/nip19';

/**
 * Decode NIP-19 npub to hex pubkey. Returns null if invalid.
 */
export function npubDecodeToHex(npub: string): string | null {
  try {
    const decoded = decode(npub);
    if (decoded.type === 'npub') return decoded.data;
    return null;
  } catch {
    return null;
  }
}

/**
 * Encode hex pubkey to NIP-19 npub for profile URLs.
 */
export function pubkeyToNpub(pubkey: string): string {
  return npubEncode(pubkey);
}
