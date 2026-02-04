/* @vitest-environment jsdom */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { getQueryClient } from './queryClient';

const QUERY_CLIENT_KEY = '__OA_QUERY_CLIENT__';
const PERSIST_SETUP_KEY = '__OA_QUERY_CLIENT_PERSIST__';
const PERSIST_KEY = 'clawstr-query-cache-v1';

function resetClient() {
  delete (globalThis as Record<string, unknown>)[QUERY_CLIENT_KEY];
  delete (globalThis as Record<string, unknown>)[PERSIST_SETUP_KEY];
}

describe('queryClient persistence', () => {
  beforeEach(() => {
    localStorage.clear();
    resetClient();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    resetClient();
    localStorage.clear();
  });

  it('returns a singleton client', () => {
    const a = getQueryClient();
    const b = getQueryClient();
    expect(a).toBe(b);
  });

  it('persists allowed clawstr queries and serializes Maps', () => {
    const client = getQueryClient();
    client.setQueryData(['clawstr', 'feed'], new Map([['a', 1]]));
    client.setQueryData(['clawstr', 'posts'], 'denylisted');

    vi.advanceTimersByTime(1100);

    const raw = localStorage.getItem(PERSIST_KEY);
    expect(raw).not.toBeNull();
    const parsed = JSON.parse(raw ?? '{}') as { entries?: Array<{ key: unknown[]; data: unknown }> };
    const entries = parsed.entries ?? [];
    expect(entries.length).toBe(1);
    expect(entries[0]?.key).toEqual(['clawstr', 'feed']);
    expect(entries[0]?.data).toEqual({
      __type: 'map',
      value: [['a', 1]],
    });
  });

  it('restores persisted queries on startup', () => {
    const now = Date.now();
    localStorage.setItem(
      PERSIST_KEY,
      JSON.stringify({
        timestamp: now,
        entries: [
          {
            key: ['clawstr', 'feed'],
            data: { __type: 'map', value: [['x', 2]] },
            updatedAt: now,
          },
        ],
      }),
    );

    const client = getQueryClient();
    const data = client.getQueryData(['clawstr', 'feed']) as Map<string, number> | undefined;
    expect(data).toBeInstanceOf(Map);
    expect(data?.get('x')).toBe(2);
  });
});
