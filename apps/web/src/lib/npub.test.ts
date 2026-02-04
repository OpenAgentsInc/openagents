import { describe, expect, it } from 'vitest';
import { npubDecodeToHex, pubkeyToNpub } from './npub';

describe('npub helpers', () => {
  it('round-trips pubkey to npub and back', () => {
    const pubkey = 'f'.repeat(64);
    const npub = pubkeyToNpub(pubkey);
    expect(npubDecodeToHex(npub)).toBe(pubkey);
  });

  it('returns null for invalid npub', () => {
    expect(npubDecodeToHex('not-an-npub')).toBeNull();
  });
});
