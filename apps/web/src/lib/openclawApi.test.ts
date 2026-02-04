import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { deleteOpenclawInstance, resolveApiBase, resolveInternalKey } from './openclawApi';

const ORIGINAL_ENV = { ...process.env };

describe('openclawApi', () => {
  beforeEach(() => {
    process.env = { ...ORIGINAL_ENV };
  });

  afterEach(() => {
    process.env = { ...ORIGINAL_ENV };
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  describe('resolveApiBase', () => {
    it('normalizes a full https base', () => {
      process.env.OPENCLAW_API_BASE = 'https://example.com/api/';
      expect(resolveApiBase()).toBe('https://example.com/api');
    });

    it('adds https and /api when missing', () => {
      process.env.OPENCLAW_API_BASE = 'example.com';
      expect(resolveApiBase()).toBe('https://example.com/api');
    });

    it('adds https when /api is already present', () => {
      process.env.OPENCLAW_API_BASE = 'example.com/api';
      expect(resolveApiBase()).toBe('https://example.com/api');
    });

    it('throws when no base is configured', () => {
      delete process.env.OPENCLAW_API_BASE;
      delete process.env.OPENAGENTS_API_URL;
      delete process.env.PUBLIC_API_URL;
      expect(() => resolveApiBase()).toThrow('OpenClaw API base not configured');
    });
  });

  describe('resolveInternalKey', () => {
    it('prefers OA_INTERNAL_KEY and trims', () => {
      process.env.OA_INTERNAL_KEY = '  secret  ';
      expect(resolveInternalKey()).toBe('secret');
    });

    it('throws when missing', () => {
      delete process.env.OA_INTERNAL_KEY;
      delete process.env.OPENAGENTS_INTERNAL_KEY;
      expect(() => resolveInternalKey()).toThrow('OA_INTERNAL_KEY not configured');
    });
  });

  describe('deleteOpenclawInstance', () => {
    it('issues a DELETE request with internal headers', async () => {
      const fetchMock = vi.fn(() => {
        return new Response(JSON.stringify({ ok: true, data: { deleted: true } }), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        });
      });
      vi.stubGlobal('fetch', fetchMock);

      const result = await deleteOpenclawInstance({
        apiBase: 'https://api.example.com',
        internalKey: 'internal-key',
        userId: 'user-123',
      });

      if (!result) {
        throw new Error('Expected deleteOpenclawInstance to return a result');
      }
      expect(result.deleted).toBe(true);
      expect(fetchMock).toHaveBeenCalledTimes(1);
      const calls = fetchMock.mock.calls as unknown as Array<
        [RequestInfo | URL, RequestInit | undefined]
      >;
      const [url, init] = calls[0] ?? [];
      expect(String(url)).toBe('https://api.example.com/openclaw/instance');
      expect(init?.method).toBe('DELETE');
      const headers = new Headers(init?.headers as HeadersInit);
      expect(headers.get('X-OA-Internal-Key')).toBe('internal-key');
      expect(headers.get('X-OA-User-Id')).toBe('user-123');
    });
  });

  describe('openclawRequest error handling', () => {
    it('throws when API returns ok:false', async () => {
      const fetchMock = vi.fn(() => {
        return new Response(JSON.stringify({ ok: false, error: 'nope' }), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        });
      });
      vi.stubGlobal('fetch', fetchMock);

      await expect(
        deleteOpenclawInstance({
          apiBase: 'https://api.example.com',
          internalKey: 'internal-key',
          userId: 'user-123',
        }),
      ).rejects.toThrow('nope');
    });

    it('throws when API response is non-200', async () => {
      const fetchMock = vi.fn(() => {
        return new Response(JSON.stringify({ ok: true, data: null, error: 'nope' }), {
          status: 500,
          headers: { 'content-type': 'application/json' },
        });
      });
      vi.stubGlobal('fetch', fetchMock);

      await expect(
        deleteOpenclawInstance({
          apiBase: 'https://api.example.com',
          internalKey: 'internal-key',
          userId: 'user-123',
        }),
      ).rejects.toThrow('nope');
    });
  });
});
