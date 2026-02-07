import {
  env,
  createExecutionContext,
  waitOnExecutionContext,
} from 'cloudflare:test';
import { describe, expect, it, vi } from 'vitest';

import type { WorkerEnv } from '../../src/effuse-host/env';

declare module 'cloudflare:test' {
  interface ProvidedEnv extends WorkerEnv {}
}

vi.mock('../../src/effuse-app/routes', async () => {
  const { Effect } = await import('effect');
  const { RouteOutcome, html } = await import('@openagentsinc/effuse');

  const matchExact =
    (pathname: string) =>
    (url: URL) => {
      if (url.pathname !== pathname) return null;
      return { pathname, params: {}, search: url.searchParams };
    };

  const slowLoader = () =>
    Effect.async((resume) => {
      const id = setTimeout(() => {
        resume(Effect.succeed(RouteOutcome.ok({})));
      }, 1_000);
      return Effect.sync(() => clearTimeout(id));
    });

  const hugeText = 'a'.repeat(1_600_000);

  const appRoutes = [
    {
      id: '/__test/slow',
      match: matchExact('/__test/slow'),
      loader: slowLoader,
      view: () => Effect.succeed(html`<div>slow</div>`),
    },
    {
      id: '/__test/huge',
      match: matchExact('/__test/huge'),
      loader: () => Effect.succeed(RouteOutcome.ok({})),
      view: () => Effect.succeed(html`<div>${hugeText}</div>`),
      head: () => Effect.succeed({ title: 'Huge' }),
    },
    {
      id: '/__test/hints',
      match: matchExact('/__test/hints'),
      loader: () =>
        Effect.succeed(
          RouteOutcome.ok(
            { ok: true },
            {
              headers: [['x-test', '1']],
              cookies: [{ _tag: 'Set', name: 'a', value: 'b', attributes: 'Path=/' }],
              cache: { mode: 'cache-first', ttlMs: 60_000 },
              dehydrate: { payload: '</script><script>window.__pwned=1</script>' },
            },
          ),
        ),
      view: () => Effect.succeed(html`<div>ok</div>`),
      head: () => Effect.succeed({ title: 'Hints' }),
    },
  ] as const;

  return { appRoutes };
});

const { default: worker } = await import('../../src/effuse-host/worker');

describe('apps/web worker SSR', () => {
  it('respects request aborts', async () => {
    const controller = new AbortController();
    const request = new Request('http://example.com/__test/slow', {
      signal: controller.signal,
    });
    const ctx = createExecutionContext();

    const responsePromise = worker.fetch(request, env, ctx);
    setTimeout(() => controller.abort(), 10);

    const response = await responsePromise;
    await waitOnExecutionContext(ctx);

    expect(response.status).toBe(499);
    expect(response.headers.get('cache-control')).toBe('no-store');
  });

  it('enforces max HTML byte cap', async () => {
    const request = new Request('http://example.com/__test/huge');
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);

    expect(response.status).toBe(500);
    expect(response.headers.get('cache-control')).toBe('no-store');
    expect(await response.text()).toContain('SSR output too large');
  });

  it('applies RouteOkHints (headers, cookies, cache-control, dehydrate)', async () => {
    const request = new Request('http://example.com/__test/hints');
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);

    expect(response.status).toBe(200);
    expect(response.headers.get('x-test')).toBe('1');
    expect(response.headers.get('cache-control')).toBe('no-store');
    expect(response.headers.get('set-cookie') ?? '').toContain('a=b');

    const body = await response.text();
    const start = '<script id="effuse-dehydrate" type="application/json">';
    const end = '</script>';
    const startIndex = body.indexOf(start);
    expect(startIndex).toBeGreaterThanOrEqual(0);
    const endIndex = body.indexOf(end, startIndex + start.length);
    expect(endIndex).toBeGreaterThan(startIndex);

    const jsonText = body.slice(startIndex + start.length, endIndex);

    // escapeJsonForHtmlScript replaces `<` with `\\u003c` to prevent `</script>` breaks.
    expect(jsonText).not.toContain('<');

    const parsed = JSON.parse(jsonText) as Record<string, unknown>;
    expect(Object.keys(parsed)).toEqual(['/__test/hints']);
    expect((parsed['/__test/hints'] as any)?.payload).toBe('</script><script>window.__pwned=1</script>');
  });
});

