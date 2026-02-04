/* @vitest-environment jsdom */
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  pickReadRelays,
  recordRelayClose,
  recordRelayError,
  recordRelayOpen,
} from './relayHealth';

function loadState(): Record<string, unknown> {
  const raw = localStorage.getItem('openagents-relay-health-v1');
  return raw ? (JSON.parse(raw) as Record<string, unknown>) : {};
}

describe('relayHealth', () => {
  afterEach(() => {
    vi.restoreAllMocks();
    localStorage.clear();
  });

  it('records open/error/close events', () => {
    localStorage.clear();
    const now = 1_700_000_000_000;
    vi.spyOn(Date, 'now').mockReturnValue(now);

    recordRelayOpen('wss://relay.one', 120);
    recordRelayError('wss://relay.one');
    recordRelayClose('wss://relay.one');

    const state = loadState();
    const entry = state['wss://relay.one'] as {
      openCount: number;
      errorCount: number;
      closeCount: number;
      avgOpenMs?: number;
      lastOpenAt?: number;
      lastErrorAt?: number;
    };
    expect(entry.openCount).toBe(1);
    expect(entry.errorCount).toBe(1);
    expect(entry.closeCount).toBe(1);
    expect(entry.avgOpenMs).toBe(120);
    expect(entry.lastOpenAt).toBe(now);
    expect(entry.lastErrorAt).toBe(now);
  });

  it('prefers highest scoring relays', () => {
    localStorage.clear();
    recordRelayOpen('wss://relay.good', 40);
    recordRelayOpen('wss://relay.good', 40);
    recordRelayError('wss://relay.bad');
    recordRelayError('wss://relay.bad');
    recordRelayClose('wss://relay.bad');

    const picked = pickReadRelays([
      'wss://relay.good',
      'wss://relay.bad',
      'wss://relay.ok',
    ]);
    expect(picked[0]).toBe('wss://relay.good');
    expect(picked.length).toBe(2);
  });
});
