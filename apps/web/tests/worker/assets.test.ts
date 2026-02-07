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

describe('apps/web worker assets', () => {
  it('serves /effuse-client.css', async () => {
    const request = new Request('http://example.com/effuse-client.css');
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);

    expect(response.status).toBe(200);
    expect(response.headers.get('content-type') ?? '').toContain('text/css');
    expect((await response.text()).length).toBeGreaterThan(0);
  });

  it('serves /effuse-client.js', async () => {
    const request = new Request('http://example.com/effuse-client.js');
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);

    expect(response.status).toBe(200);
    expect(response.headers.get('content-type') ?? '').toContain('javascript');
    expect((await response.text()).length).toBeGreaterThan(0);
  });

  it('does not fall through to SSR for missing asset paths', async () => {
    const request = new Request('http://example.com/this-asset-does-not-exist.js');
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);

    expect(response.status).toBe(404);
    const body = await response.text();
    expect(body).not.toContain('<!doctype html>');
    expect(body).not.toContain('data-effuse-shell');
  });
});

