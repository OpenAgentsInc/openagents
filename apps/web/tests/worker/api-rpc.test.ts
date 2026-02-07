import {
  env,
  createExecutionContext,
  waitOnExecutionContext,
} from 'cloudflare:test';
import { describe, expect, it } from 'vitest';

import type { WorkerEnv } from '../../src/effuse-host/env';

declare module 'cloudflare:test' {
  interface ProvidedEnv extends WorkerEnv {}
}

const { default: worker } = await import('../../src/effuse-host/worker');

describe('apps/web worker RPC', () => {
  it('/api/rpc works in a Worker runtime, is request-scoped, and is never cached', async () => {
    const ORIGIN = 'http://example.com';

    const originalFetch = globalThis.fetch;
    let sawAgentsFetch = false;

    // Route same-origin fetches back into the in-process Worker to keep the suite hermetic.
    globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
      const inputUrl =
        typeof input === 'string'
          ? input
          : input instanceof URL
            ? input.toString()
            : input.url;
      const url = new URL(inputUrl, ORIGIN);

      if (url.origin !== ORIGIN) {
        throw new Error(`External fetch blocked in tests: ${url.toString()}`);
      }

      const request = input instanceof Request ? input : new Request(url.toString(), init);

      if (url.pathname.startsWith('/agents/')) {
        const cookie = request.headers.get('cookie') ?? '';
        if (!cookie.includes('testcookie=1')) {
          throw new Error(`Expected /agents/* subrequest to forward cookie; got: ${cookie || '(missing)'}`);
        }
        sawAgentsFetch = true;
      }

      const ctx = createExecutionContext();
      const response = await worker.fetch(request, env, ctx);
      const body = await response.arrayBuffer();
      await waitOnExecutionContext(ctx);

      return new Response(body, {
        status: response.status,
        statusText: response.statusText,
        headers: response.headers,
      });
    }) as any;

    try {
      const threadId = `rpc-${Date.now()}`;

      const rpcRequest = {
        _tag: 'Request',
        id: '1',
        tag: 'agent.getToolContracts',
        payload: { chatId: threadId },
        headers: [] as Array<[string, string]>,
      };

      const request = new Request(`${ORIGIN}/api/rpc`, {
        method: 'POST',
        headers: {
          'content-type': 'application/ndjson',
          accept: 'application/ndjson',
          cookie: 'testcookie=1',
        },
        body: `${JSON.stringify(rpcRequest)}\n`,
      });

      const ctx = createExecutionContext();
      const response = await worker.fetch(request, env, ctx);
      const text = await response.text();
      await waitOnExecutionContext(ctx);

      expect(response.status).toBe(200);
      expect(response.headers.get('cache-control')).toBe('no-store');

      const frames = text
        .split('\n')
        .map((line) => line.trim())
        .filter(Boolean)
        .map((line) => JSON.parse(line));

      const exit = frames.find((frame: any) => frame?._tag === 'Exit' && frame.requestId === '1');
      expect(exit).toBeTruthy();
      expect(exit.exit?._tag).toBe('Success');

      const value = exit.exit?.value;
      expect(Array.isArray(value)).toBe(true);
      expect(value.length).toBeGreaterThan(0);

      expect(sawAgentsFetch).toBe(true);
    } finally {
      globalThis.fetch = originalFetch;
    }
  });
});

