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

const state = vi.hoisted(() => ({
  mode: 'no-refresh' as 'no-refresh' | 'refresh',
}));

vi.mock('@workos/authkit-session', async (importOriginal) => {
  const actual = await importOriginal<any>();

  return {
    ...actual,
    createAuthService: () => ({
      withAuth: async (_request: Request) => {
        if (state.mode === 'refresh') {
          return {
            auth: {
              user: { id: 'user-1', email: 'user@example.com', firstName: 'U', lastName: 'S' },
              sessionId: 'sess-1',
              accessToken: 'token-1',
            },
            refreshedSessionData: 'refreshed-session-data',
          };
        }
        return { auth: { user: null }, refreshedSessionData: undefined };
      },
      saveSession: async (_auth: unknown, _data: string) => ({
        headers: { 'Set-Cookie': 'workos-session=refreshed; Path=/; HttpOnly' },
      }),
    }),
  };
});

vi.mock('../../src/auth/workosAuth', async () => {
  const { Effect } = await import('effect');
  return {
    sendMagicAuthCode: vi.fn((_email: string) => Effect.void),
    verifyMagicAuthCode: vi.fn((_input: any) =>
      Effect.succeed({ userId: 'user-verified', setCookieHeader: 'oa-session=ok; Path=/; HttpOnly' }),
    ),
    clearSessionCookie: vi.fn(() => Effect.succeed({ setCookieHeader: 'oa-session=; Path=/; Max-Age=0' })),
  };
});

const workosAuth = await import('../../src/auth/workosAuth');
const { sendMagicAuthCode, verifyMagicAuthCode, clearSessionCookie } = workosAuth as any;

const { default: worker } = await import('../../src/effuse-host/worker');

describe('apps/web worker auth', () => {
  it('GET /api/auth/session works without external network', async () => {
    state.mode = 'no-refresh';
    const request = new Request('http://example.com/api/auth/session');
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);

    expect(response.status).toBe(200);
    expect(response.headers.get('set-cookie')).toBeNull();
    expect(await response.json()).toEqual({
      ok: true,
      userId: null,
      sessionId: null,
      token: null,
      user: null,
    });
  });

  it('persists refreshed session Set-Cookie header (stubbed)', async () => {
    state.mode = 'refresh';
    const request = new Request('http://example.com/api/auth/session');
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);

    expect(response.status).toBe(200);
    expect(response.headers.get('set-cookie') ?? '').toContain('workos-session=refreshed');

    const json = (await response.json()) as any;
    expect(json.ok).toBe(true);
    expect(json.userId).toBe('user-1');
    expect(json.sessionId).toBe('sess-1');
    expect(json.token).toBe('token-1');
  });

  it('stubs magic code endpoints (no WorkOS network)', async () => {
    sendMagicAuthCode.mockClear();
    verifyMagicAuthCode.mockClear();
    clearSessionCookie.mockClear();

    {
      const request = new Request('http://example.com/api/auth/start', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ email: '  TEST@Example.com  ' }),
      });
      const ctx = createExecutionContext();
      const response = await worker.fetch(request, env, ctx);
      await waitOnExecutionContext(ctx);

      expect(response.status).toBe(200);
      expect(await response.json()).toEqual({ ok: true });
      expect(sendMagicAuthCode).toHaveBeenCalledTimes(1);
      expect(sendMagicAuthCode).toHaveBeenLastCalledWith('test@example.com');
    }

    {
      const request = new Request('http://example.com/api/auth/verify', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ email: ' TEST@Example.com ', code: ' 12 34 56 ' }),
      });
      const ctx = createExecutionContext();
      const response = await worker.fetch(request, env, ctx);
      await waitOnExecutionContext(ctx);

      expect(response.status).toBe(200);
      expect(response.headers.get('set-cookie')).toBe('oa-session=ok; Path=/; HttpOnly');
      expect(await response.json()).toEqual({ ok: true, userId: 'user-verified' });

      expect(verifyMagicAuthCode).toHaveBeenCalledTimes(1);
      const arg = verifyMagicAuthCode.mock.calls[0]?.[0] as any;
      expect(arg.email).toBe('test@example.com');
      expect(arg.code).toBe('123456');
    }

    {
      const request = new Request('http://example.com/api/auth/signout', { method: 'POST' });
      const ctx = createExecutionContext();
      const response = await worker.fetch(request, env, ctx);
      await waitOnExecutionContext(ctx);

      expect(response.status).toBe(200);
      const setCookie = response.headers.get('set-cookie') ?? '';
      expect(setCookie).toContain('oa-session=; Path=/; Max-Age=0');
      expect(setCookie).toContain('oa-e2e=');
      expect(await response.json()).toEqual({ ok: true });
      expect(clearSessionCookie).toHaveBeenCalledTimes(1);
    }
  });
});
