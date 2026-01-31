/**
 * Fetch Moltbook via OpenAgents proxy. Respect 429 and retry_after_minutes.
 */

import type { Env } from './types';

export interface FetchResult<T> {
  ok: true;
  data: T;
  retryAfterMinutes?: number;
}

export interface FetchError {
  ok: false;
  status: number;
  retryAfterMinutes?: number;
  body?: string;
}

export async function fetchMoltbookJson<T = unknown>(
  env: Env,
  path: string,
  query: Record<string, string> = {}
): Promise<FetchResult<T> | FetchError> {
  const base = env.MOLTBOOK_API_BASE ?? 'https://openagents.com/api/moltbook/api';
  const url = new URL(path.startsWith('http') ? path : `${base.replace(/\/$/, '')}/${path.replace(/^\//, '')}`);
  for (const [k, v] of Object.entries(query)) {
    if (v !== undefined && v !== '') url.searchParams.set(k, v);
  }

  const headers: Record<string, string> = {
    Accept: 'application/json',
  };
  if (env.MOLTBOOK_API_KEY) {
    headers['Authorization'] = `Bearer ${env.MOLTBOOK_API_KEY}`;
  }

  const res = await fetch(url.toString(), { headers });
  const retryHeader = res.headers.get('retry-after') ?? res.headers.get('retry_after_minutes');
  let retryAfterMinutes: number | undefined;
  if (retryHeader) {
    const n = parseInt(retryHeader, 10);
    if (!isNaN(n)) retryAfterMinutes = n;
  }

  if (res.status === 429) {
    return { ok: false, status: 429, retryAfterMinutes, body: await res.text() };
  }
  if (!res.ok) {
    return { ok: false, status: res.status, retryAfterMinutes, body: await res.text() };
  }

  const ct = res.headers.get('content-type') ?? '';
  const body = await res.text();
  if (!ct.includes('application/json') && body.trimStart().startsWith('<')) {
    return { ok: false, status: res.status, body: body.slice(0, 500) };
  }
  let data: T;
  try {
    data = JSON.parse(body) as T;
  } catch {
    return { ok: false, status: res.status, body: body.slice(0, 500) };
  }
  return { ok: true, data, retryAfterMinutes };
}

/** Extract posts array from feed response (array or { posts/data/recentPosts } ). */
export function extractPosts(value: unknown): unknown[] {
  if (Array.isArray(value)) return value;
  if (value && typeof value === 'object') {
    const o = value as Record<string, unknown>;
    for (const key of ['posts', 'data', 'recentPosts', 'recent_posts']) {
      const arr = o[key];
      if (Array.isArray(arr)) return arr;
    }
  }
  return [];
}

/** Extract comments array from comments response (Moltbook may use comments/data or nested). */
export function extractComments(value: unknown): unknown[] {
  if (Array.isArray(value)) return value;
  if (value && typeof value === 'object') {
    const o = value as Record<string, unknown>;
    for (const key of ['comments', 'data']) {
      const v = o[key];
      if (Array.isArray(v)) return v;
      if (v && typeof v === 'object' && Array.isArray((v as Record<string, unknown>)['comments'])) {
        return (v as Record<string, unknown>)['comments'] as unknown[];
      }
    }
  }
  return [];
}
