import { describe, expect, it, vi } from 'vitest';

vi.mock('convex/browser', () => ({
  ConvexHttpClient: class ConvexHttpClient {
    url: string;
    constructor(url: string) {
      this.url = url;
    }
  },
}));

const CLIENT_KEY = '__OA_CONVEX_HTTP_CLIENT__';

function resetClient() {
  delete (globalThis as Record<string, unknown>)[CLIENT_KEY];
}

describe('convexHttpClient', () => {
  it('throws when VITE_CONVEX_URL is missing', async () => {
    resetClient();
    (import.meta as { env?: Record<string, string> }).env = {};
    const { getConvexHttpClient } = await import('./convexHttpClient');
    expect(() => getConvexHttpClient()).toThrow('VITE_CONVEX_URL is required');
  });

  it('returns a singleton client', async () => {
    resetClient();
    (import.meta as { env?: Record<string, string> }).env = { VITE_CONVEX_URL: 'https://convex.dev' };
    const { getConvexHttpClient } = await import('./convexHttpClient');
    const first = getConvexHttpClient() as { url: string };
    const second = getConvexHttpClient() as { url: string };
    expect(first).toBe(second);
    expect(first.url).toBe('https://convex.dev');
  });
});
