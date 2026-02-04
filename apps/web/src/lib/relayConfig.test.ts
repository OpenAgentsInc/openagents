/* @vitest-environment jsdom */
import { describe, expect, it, vi } from 'vitest';
import {
  DEFAULT_RELAY_METADATA,
  RELAY_STORAGE_KEY,
  buildRelayMetadataFromUrls,
  getStoredRelayMetadata,
  getStoredRelays,
  normalizeRelayUrl,
  setStoredRelayMetadata,
} from './relayConfig';

describe('relayConfig', () => {
  it('normalizes relay URLs', () => {
    expect(normalizeRelayUrl('wss://relay.example.com')).toBe('wss://relay.example.com');
    expect(normalizeRelayUrl('ws://relay.example.com')).toBe('wss://relay.example.com');
    expect(normalizeRelayUrl('https://relay.example.com')).toBe('wss://relay.example.com');
    expect(normalizeRelayUrl('http://relay.example.com')).toBe('wss://relay.example.com');
    expect(normalizeRelayUrl('relay.example.com')).toBe('wss://relay.example.com');
    expect(normalizeRelayUrl('ftp://relay.example.com')).toBeNull();
  });

  it('builds relay metadata with normalized entries', () => {
    vi.spyOn(Date, 'now').mockReturnValue(1_700_000_000_000);
    const metadata = buildRelayMetadataFromUrls([
      'relay.example.com',
      'https://relay.example.com',
      'wss://relay.two',
    ]);
    expect(metadata.relays.map((entry) => entry.url)).toEqual([
      'wss://relay.example.com',
      'wss://relay.two',
    ]);
    expect(metadata.updatedAt).toBe(1_700_000_000);
  });

  it('returns defaults when no local storage is set', () => {
    localStorage.clear();
    const metadata = getStoredRelayMetadata();
    expect(metadata.relays.length).toBe(DEFAULT_RELAY_METADATA.relays.length);
  });

  it('reads stored relay metadata from localStorage', () => {
    const payload = {
      relays: [{ url: 'relay.one', read: true, write: false }],
      updatedAt: 123,
    };
    localStorage.setItem(RELAY_STORAGE_KEY, JSON.stringify(payload));
    const metadata = getStoredRelayMetadata();
    expect(metadata.relays[0]?.url).toBe('wss://relay.one');
    expect(metadata.relays[0]?.read).toBe(true);
    expect(metadata.relays[0]?.write).toBe(false);
    expect(metadata.updatedAt).toBe(123);
  });

  it('accepts legacy relay array storage', () => {
    localStorage.setItem('clawstr-relays', JSON.stringify(['relay.legacy', 'wss://relay.two']));
    const metadata = getStoredRelayMetadata();
    expect(metadata.relays.map((entry) => entry.url)).toEqual([
      'wss://relay.legacy',
      'wss://relay.two',
    ]);
  });

  it('stores sanitized relay metadata', () => {
    localStorage.clear();
    setStoredRelayMetadata({
      relays: [
        { url: 'relay.one', read: false, write: false },
        { url: 'https://relay.two', read: true, write: false },
      ],
      updatedAt: 10,
    });
    const stored = getStoredRelayMetadata();
    expect(stored.relays.length).toBe(2);
    expect(stored.relays[0]?.url).toBe('wss://relay.one');
    expect(stored.relays[0]?.read).toBe(true);
    expect(stored.relays[0]?.write).toBe(true);
    expect(stored.updatedAt).toBe(10);
  });

  it('returns read relays from stored metadata', () => {
    localStorage.clear();
    setStoredRelayMetadata({
      relays: [
        { url: 'relay.read', read: true, write: false },
        { url: 'relay.write', read: false, write: true },
      ],
      updatedAt: 1,
    });
    const relays = getStoredRelays();
    expect(relays).toEqual(['wss://relay.read']);
  });
});
