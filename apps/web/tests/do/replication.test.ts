import { env } from 'cloudflare:test';
import { describe, expect, it } from 'vitest';

import type { WorkerEnv } from '../../src/effuse-host/env';

declare module 'cloudflare:test' {
  interface ProvidedEnv extends WorkerEnv {}
}

const getUserSpaceStub = (userSpaceId: string) => {
  if (!env.UserSpaceDO) throw new Error('UserSpaceDO binding missing');
  const id = env.UserSpaceDO.idFromName(userSpaceId);
  return env.UserSpaceDO.get(id);
};

const timeout = (ms: number) =>
  new Promise<never>((_resolve, reject) => setTimeout(() => reject(new Error(`timeout after ${ms}ms`)), ms));

describe('UserSpaceDO Convex replication (non-blocking)', () => {
  it('schedules replication via waitUntil (does not block responses) and is idempotent by eventId', async () => {
    const userSpaceId = `userspace-repl-${Date.now()}`;
    const stub = getUserSpaceStub(userSpaceId);

    // Ensure the DO sees a convex url so it attempts replication.
    (process as any).env ??= {};
    process.env.VITE_CONVEX_URL = process.env.VITE_CONVEX_URL || 'https://test.convex.cloud';

    const originalFetch = globalThis.fetch;
    let convexFetchCount = 0;
    let resolveConvexFetch: ((response: Response) => void) | null = null;
    const convexFetchPromise = new Promise<Response>((resolve) => {
      resolveConvexFetch = resolve;
    });

    globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
      const urlString =
        typeof input === 'string'
          ? input
          : input instanceof URL
            ? input.toString()
            : input.url;
      const url = new URL(urlString);

      if (url.hostname.endsWith('.convex.cloud')) {
        convexFetchCount += 1;
        // Hold the request open until the test explicitly resolves it, to prove
        // DO waitUntil doesn't block the primary response.
        void init;
        return convexFetchPromise;
      }

      throw new Error(`External fetch blocked in tests: ${url.toString()}`);
    }) as any;

    try {
      const eventId = `evt-${crypto.randomUUID()}`;

      const applyEvent = async () => {
        return stub.fetch(
          new Request('http://example.com/api/user-space/events', {
            method: 'POST',
            headers: {
              'content-type': 'application/json',
              'x-user-id': userSpaceId,
              authorization: 'Bearer token-1',
            },
            body: JSON.stringify({
              eventId,
              kind: 'replication.test',
              json: JSON.stringify({ hello: 'world' }),
            }),
          }),
        );
      };

      // First apply inserts and kicks off replication. The response must resolve
      // even though the Convex fetch promise is still pending.
      const response1 = await Promise.race([applyEvent(), timeout(300)]);
      expect(response1.status).toBe(200);
      const json1 = (await response1.json()) as any;
      expect(json1.ok).toBe(true);
      expect(json1.inserted).toBe(true);
      const countAfterFirst = convexFetchCount;
      expect(countAfterFirst).toBeGreaterThan(0);

      // Second apply is a duplicate by eventId: must not trigger replication again.
      const response2 = await Promise.race([applyEvent(), timeout(300)]);
      const json2 = (await response2.json()) as any;
      expect(json2.ok).toBe(true);
      expect(json2.inserted).toBe(false);
      expect(convexFetchCount).toBe(countAfterFirst);

      // Unblock the pending Convex request so waitUntil work can complete.
      resolveConvexFetch?.(
        new Response(JSON.stringify({ status: 'ok' }), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        }),
      );
      await new Promise((r) => setTimeout(r, 0));
    } finally {
      globalThis.fetch = originalFetch;
    }
  });
});
