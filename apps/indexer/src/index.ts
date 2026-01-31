/**
 * Moltbook indexer: R2 + D1 + KV + Queues + Cron.
 * Served at openagents.com/api/indexer/* (no subdomain).
 */

import type { Env, IndexerJob, MoltbookPost, MoltbookComment } from './types';
import { r2KeyPost, r2KeyCommentsPage, r2KeyQuarantine } from './r2-keys';
import { scanSecrets, redactForD1 } from './secrets';
import { computeSignals } from './signals';
import { fetchMoltbookJson, extractPosts, extractComments } from './moltbook';
import { getAuthorName, getAuthorId, getSubmoltName } from './extract';

const STATE_LAST_NEW_POST_ID = 'state:last_new_seen_post_id';
const STATE_LAST_NEW_CREATED_AT = 'state:last_new_seen_created_at';
const STATE_BACKOFF_UNTIL = 'backoff_until';
const COMMENTS_PAGE_SIZE = 50;

/** D1 accepts only string, number, null. Coerce objects/dates to string. */
function d1Val(v: unknown): string | number | null {
  if (v === null || v === undefined) return null;
  if (typeof v === 'string' || typeof v === 'number') return v;
  if (typeof v === 'boolean') return v ? 1 : 0;
  if (typeof v === 'object') return JSON.stringify(v);
  return String(v);
}

// --- Cron: incremental ingest (Lane A) ---

export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    const url = new URL(request.url);
    let path = url.pathname;
    if (path.startsWith('/api/indexer')) {
      path = path.slice('/api/indexer'.length) || '/';
    }
    if (path === '') path = '/';

    if (request.method === 'OPTIONS') {
      return new Response(null, { status: 204, headers: corsHeaders() });
    }

    if (path === '/health' || path === '/') {
      return jsonResponse({ ok: true, service: 'openagents-moltbook-indexer', path: '/api/indexer' });
    }
    if (path === '/v1/search') {
      return handleSearch(request, env, url);
    }
    if (path === '/v1/metrics/wallet-adoption') {
      return handleWalletAdoption(request, env);
    }
    if (path === '/v1/wallet-interest') {
      return handleWalletInterest(request, env);
    }
    if (path === '/ingest' && request.method === 'POST') {
      return runIngestAndRespond(env);
    }
    if (path === '/ingest/backfill-authors' && request.method === 'POST') {
      return runBackfillAuthorsAndRespond(env);
    }
    if (path === '/ingest/backfill-comments' && request.method === 'POST') {
      return runBackfillCommentsAndRespond(env);
    }

    return jsonResponse({ error: 'not found' }, 404);
  },

  async scheduled(_event: ScheduledEvent, env: Env, _ctx: ExecutionContext): Promise<void> {
    await runIngest(env);
  },

  async queue(batch: MessageBatch<IndexerJob>, env: Env, ctx: ExecutionContext): Promise<void> {
    const backoffUntil = await env.STATE.get(STATE_BACKOFF_UNTIL);
    if (backoffUntil && Date.now() < parseInt(backoffUntil, 10)) {
      batch.retryAll();
      return;
    }

    for (const msg of batch.messages) {
      try {
        const job = msg.body;
        if (job.type === 'FETCH_COMMENTS' || job.type === 'BACKFILL_COMMENTS') {
          await processCommentJob(env, job);
        }
        msg.ack();
      } catch (e) {
        msg.retry();
      }
    }
  },
};

/** Run incremental ingest (same logic as cron). Returns summary for HTTP. */
async function runIngest(env: Env): Promise<{ postsFetched: number; newPosts: number; queuedComments: number; error?: string }> {
  const backoffUntil = await env.STATE.get(STATE_BACKOFF_UNTIL);
  if (backoffUntil) {
    const until = parseInt(backoffUntil, 10);
    if (Date.now() < until) {
      return { postsFetched: 0, newPosts: 0, queuedComments: 0, error: 'in_backoff' };
    }
    await env.STATE.delete(STATE_BACKOFF_UNTIL);
  }

  const result = await fetchMoltbookJson<unknown>(env, 'posts', { sort: 'new', limit: '25' });
  if (!result.ok) {
    if (result.status === 429 && result.retryAfterMinutes) {
      const until = Date.now() + result.retryAfterMinutes * 60 * 1000;
      await env.STATE.put(STATE_BACKOFF_UNTIL, String(until), { expirationTtl: 3600 });
    }
    const errMsg = result.body ? `${result.status}: ${result.body.slice(0, 200)}` : `fetch ${result.status}`;
    return { postsFetched: 0, newPosts: 0, queuedComments: 0, error: errMsg };
  }

  const posts = extractPosts(result.data);
  let newCount = 0;
  let queuedCount = 0;
  const now = new Date().toISOString();

  for (const raw of posts) {
    const post = raw as MoltbookPost;
    const id = post.id ?? (post as Record<string, unknown>).id;
    if (typeof id !== 'string') continue;

    const existing = await env.DB.prepare('SELECT 1 FROM moltbook_posts WHERE id = ?').bind(id).first();
    if (existing) continue;

    newCount++;
    const r2Key = r2KeyPost(new Date(), id);
    await env.R2.put(r2Key, JSON.stringify(post), { httpMetadata: { contentType: 'application/json' } });

    const scanTitle = scanSecrets(post.title);
    const scanContent = scanSecrets(post.content);
    const hasSecrets = scanTitle.hasSecrets || scanContent.hasSecrets;
    if (hasSecrets) {
      const qKey = r2KeyQuarantine(new Date(), 'post', id);
      await env.R2.put(qKey, JSON.stringify(post), { httpMetadata: { contentType: 'application/json' } });
    }

    const content = redactForD1(post.content ?? '', scanContent);
    const title = redactForD1(post.title ?? '', scanTitle);
    const authorName = getAuthorName(post);
    const authorId = getAuthorId(post);
    const submoltName = getSubmoltName(post.submolt);
    await env.DB.prepare(
      `INSERT INTO moltbook_posts (id, created_at, submolt, title, content, url, author_name, author_id, score, comment_count, raw_r2_key, ingested_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
    )
      .bind(
        id,
        d1Val(post.created_at),
        submoltName ?? null,
        title || null,
        content || null,
        d1Val(post.url),
        authorName ?? null,
        authorId ?? null,
        d1Val(post.score),
        d1Val(post.comment_count),
        r2Key,
        now
      )
      .run();

    const signals = computeSignals('post', id, content || title, now);
    for (const row of signals) {
      await env.DB.prepare(
        `INSERT OR IGNORE INTO derived_signals (object_type, object_id, signal, value, created_at) VALUES (?, ?, ?, ?, ?)`
      )
        .bind(row.object_type, row.object_id, row.signal, row.value, row.created_at)
        .run();
    }

    if (authorName) {
      await env.DB.prepare(
        `INSERT INTO moltbook_authors (name, last_seen_at, raw_profile_r2_key) VALUES (?, ?, NULL) ON CONFLICT(name) DO UPDATE SET last_seen_at = excluded.last_seen_at`
      )
        .bind(authorName, now)
        .run();
    }

    const commentCount = typeof post.comment_count === 'number' ? post.comment_count : 0;
    if (commentCount > 0) {
      await env.JOBS.send({ type: 'FETCH_COMMENTS', post_id: id, attempt: 1 });
      queuedCount++;
    }
  }

  if (posts.length > 0) {
    const last = posts[posts.length - 1] as MoltbookPost;
    const lastId = last.id ?? (last as Record<string, unknown>).id;
    const lastCreated = last.created_at ?? (last as Record<string, unknown>).created_at;
    if (typeof lastId === 'string') await env.STATE.put(STATE_LAST_NEW_POST_ID, lastId);
    if (typeof lastCreated === 'string') await env.STATE.put(STATE_LAST_NEW_CREATED_AT, lastCreated);
  }

  return { postsFetched: posts.length, newPosts: newCount, queuedComments: queuedCount };
}

async function runIngestAndRespond(env: Env): Promise<Response> {
  try {
    const summary = await runIngest(env);
    return jsonResponse({ ok: true, data: summary });
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    return jsonResponse({ ok: false, error: msg }, 500);
  }
}

/** Backfill author_name, author_id, submolt from raw R2 post JSON for existing rows. */
async function runBackfillAuthorsAndRespond(env: Env): Promise<Response> {
  try {
    const rows = await env.DB.prepare('SELECT id, raw_r2_key FROM moltbook_posts WHERE raw_r2_key IS NOT NULL').all();
    const list = (rows.results ?? []) as { id: string; raw_r2_key: string }[];
    let updated = 0;
    let authorsTouched = 0;
    const now = new Date().toISOString();
    for (const row of list) {
      try {
        const obj = await env.R2.get(row.raw_r2_key);
        if (!obj) continue;
        const post = (await obj.json()) as MoltbookPost & { author?: { name?: string; id?: string }; submolt?: unknown };
        const authorName = getAuthorName(post);
        const authorId = getAuthorId(post);
        const submoltName = getSubmoltName(post.submolt);
        await env.DB.prepare(
          'UPDATE moltbook_posts SET author_name = ?, author_id = ?, submolt = ? WHERE id = ?'
        )
          .bind(authorName ?? null, authorId ?? null, submoltName ?? null, row.id)
          .run();
        updated++;
        if (authorName) {
          await env.DB.prepare(
            `INSERT INTO moltbook_authors (name, last_seen_at, raw_profile_r2_key) VALUES (?, ?, NULL) ON CONFLICT(name) DO UPDATE SET last_seen_at = excluded.last_seen_at`
          )
            .bind(authorName, now)
            .run();
          authorsTouched++;
        }
      } catch (_) {
        // skip bad R2 object
      }
    }
    return jsonResponse({ ok: true, data: { posts_updated: updated, authors_touched: authorsTouched, total: list.length } });
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    return jsonResponse({ ok: false, error: msg }, 500);
  }
}

/** Fetch comments for all posts with comment_count > 0 (sync, up to 20 posts per request). Requires MOLTBOOK_API_KEY for comments API. */
async function runBackfillCommentsAndRespond(env: Env): Promise<Response> {
  try {
    const rows = await env.DB.prepare(
      'SELECT id FROM moltbook_posts WHERE (comment_count IS NULL OR comment_count > 0) ORDER BY ingested_at DESC LIMIT 20'
    ).all();
    const posts = (rows.results ?? []) as { id: string }[];
    let processed = 0;
    let errors = 0;
    for (const row of posts) {
      try {
        await processCommentJob(env, { type: 'FETCH_COMMENTS', post_id: row.id, attempt: 1 });
        processed++;
      } catch {
        errors++;
      }
    }
    const payload: { processed: number; errors: number; total: number; hint?: string } = { processed, errors, total: posts.length };
    if (errors === posts.length && posts.length > 0) {
      payload.hint = 'Comments API may require MOLTBOOK_API_KEY. Run: npx wrangler secret put MOLTBOOK_API_KEY';
    }
    return jsonResponse({ ok: true, data: payload });
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    return jsonResponse({ ok: false, error: msg }, 500);
  }
}

async function processCommentJob(env: Env, job: IndexerJob): Promise<void> {
  const { post_id } = job;
  // Moltbook GET /posts/{id}/comments returns 405; single-post GET /posts/{id} includes comments (see snapshot_comments.sh).
  const path = `posts/${post_id}`;
  const result = await fetchMoltbookJson<unknown>(env, path, {});
  if (!result.ok) {
    if (result.status === 429 && result.retryAfterMinutes) {
      const until = Date.now() + result.retryAfterMinutes * 60 * 1000;
      await env.STATE.put(STATE_BACKOFF_UNTIL, String(until), { expirationTtl: 3600 });
    }
    // 401/405: Moltbook comments endpoint requires Authorization (MOLTBOOK_API_KEY)
    if (result.status === 401 || result.status === 405) {
      return; // skip; set wrangler secret MOLTBOOK_API_KEY to enable comments
    }
    throw new Error(`comments fetch failed: ${result.status}`);
  }

  const now = new Date();
  const nowStr = now.toISOString();
  const r2Key = r2KeyCommentsPage(now, post_id, 'single');
  await env.R2.put(r2Key, JSON.stringify(result.data), {
    httpMetadata: { contentType: 'application/json' },
  });

  const comments = extractComments(result.data);
  for (const raw of comments) {
    const c = raw as MoltbookComment;
    const id = c.id ?? (c as Record<string, unknown>).id;
    if (typeof id !== 'string') continue;

    const scan = scanSecrets(c.content);
    const content = redactForD1(c.content ?? '', scan);
    if (scan.hasSecrets) {
      const qKey = r2KeyQuarantine(now, 'comment', id);
      await env.R2.put(qKey, JSON.stringify(raw), { httpMetadata: { contentType: 'application/json' } });
    }

    const cAuthorName = getAuthorName(c);
    const cAuthorId = getAuthorId(c);
    await env.DB.prepare(
      `INSERT OR REPLACE INTO moltbook_comments (id, post_id, parent_id, created_at, author_name, author_id, content, score, raw_r2_key, ingested_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
    )
      .bind(
        id,
        post_id,
        d1Val(c.parent_id),
        d1Val(c.created_at),
        cAuthorName ?? null,
        cAuthorId ?? null,
        content || null,
        d1Val(c.score),
        r2Key,
        nowStr
      )
      .run();

    const signals = computeSignals('comment', id, content, nowStr);
    for (const row of signals) {
      await env.DB.prepare(
        `INSERT OR IGNORE INTO derived_signals (object_type, object_id, signal, value, created_at) VALUES (?, ?, ?, ?, ?)`
      )
        .bind(row.object_type, row.object_id, row.signal, row.value, row.created_at)
        .run();
    }

    if (cAuthorName) {
      await env.DB.prepare(
        `INSERT INTO moltbook_authors (name, last_seen_at, raw_profile_r2_key) VALUES (?, ?, NULL) ON CONFLICT(name) DO UPDATE SET last_seen_at = excluded.last_seen_at`
      )
        .bind(cAuthorName, nowStr)
        .run();
    }
  }
}

// --- HTTP API ---

function corsHeaders(): HeadersInit {
  return {
    'Access-Control-Allow-Origin': '*',
    'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
    'Access-Control-Allow-Headers': 'Authorization, Content-Type',
    'Access-Control-Max-Age': '86400',
  };
}

function jsonResponse(data: unknown, status = 200): Response {
  return new Response(JSON.stringify(data), {
    status,
    headers: { 'Content-Type': 'application/json', ...corsHeaders() },
  });
}

/** When INDEXER_AUTH_HEADER is set, /v1/search, /v1/metrics/*, and /v1/wallet-interest require Authorization: Bearer <value>. */
function requireAuth(request: Request, env: Env): boolean {
  const header = env.INDEXER_AUTH_HEADER;
  if (header) {
    const auth = request.headers.get('Authorization');
    return auth === `Bearer ${header}` || auth === header;
  }
  return true; // if no secret set, allow (MVP: lock down in prod)
}

async function handleSearch(request: Request, env: Env, url: URL): Promise<Response> {
  if (!requireAuth(request, env)) {
    return jsonResponse({ ok: false, error: 'unauthorized' }, 401);
  }
  const q = url.searchParams.get('q') ?? '';
  const limit = Math.min(100, parseInt(url.searchParams.get('limit') ?? '25', 10) || 25);
  if (!q.trim()) {
    return jsonResponse({ ok: true, data: { posts: [], comments: [] } });
  }
  const term = `%${q.trim()}%`;
  const posts = await env.DB.prepare(
    `SELECT id, created_at, submolt, title, author_name, comment_count FROM moltbook_posts WHERE title LIKE ? OR content LIKE ? LIMIT ?`
  )
    .bind(term, term, limit)
    .all();
  const comments = await env.DB.prepare(
    `SELECT id, post_id, created_at, author_name, content FROM moltbook_comments WHERE content LIKE ? LIMIT ?`
  )
    .bind(term, limit)
    .all();
  return jsonResponse({
    ok: true,
    data: { posts: posts.results, comments: comments.results },
  });
}

async function handleWalletAdoption(request: Request, env: Env): Promise<Response> {
  if (!requireAuth(request, env)) {
    return jsonResponse({ ok: false, error: 'unauthorized' }, 401);
  }
  const days = Math.min(365, parseInt(new URL(request.url).searchParams.get('days') ?? '30', 10) || 30);
  const since = new Date(Date.now() - days * 24 * 60 * 60 * 1000).toISOString();
  const withLud16 = await env.DB.prepare(
    `SELECT COUNT(DISTINCT object_id) as c FROM derived_signals WHERE signal = 'has_lud16' AND created_at >= ?`
  )
    .bind(since)
    .first();
  const withNpub = await env.DB.prepare(
    `SELECT COUNT(DISTINCT object_id) as c FROM derived_signals WHERE signal = 'has_npub' AND created_at >= ?`
  )
    .bind(since)
    .first();
  const mentions = await env.DB.prepare(
    `SELECT signal, COUNT(*) as cnt FROM derived_signals WHERE signal IN ('mentions_wallet','mentions_lightning','mentions_openclaw') AND created_at >= ? GROUP BY signal`
  )
    .bind(since)
    .all();
  const metrics: Record<string, number> = {};
  for (const row of mentions.results as { signal: string; cnt: number }[]) {
    metrics[row.signal] = row.cnt;
  }
  return jsonResponse({
    ok: true,
    data: {
      days,
      since,
      distinct_objects_with_lud16: (withLud16 as { c: number })?.c ?? 0,
      distinct_objects_with_npub: (withNpub as { c: number })?.c ?? 0,
      signal_counts: metrics,
    },
  });
}

const WALLET_INTEREST_SIGNALS = ['has_lud16', 'has_npub', 'mentions_wallet', 'mentions_lightning', 'mentions_openclaw'];

async function handleWalletInterest(request: Request, env: Env): Promise<Response> {
  if (!requireAuth(request, env)) {
    return jsonResponse({ ok: false, error: 'unauthorized' }, 401);
  }
  const url = new URL(request.url);
  const days = Math.min(365, parseInt(url.searchParams.get('days') ?? '30', 10) || 30);
  const limit = Math.min(100, parseInt(url.searchParams.get('limit') ?? '20', 10) || 20);
  const since = new Date(Date.now() - days * 24 * 60 * 60 * 1000).toISOString();

  const rows = await env.DB.prepare(
    `SELECT object_type, object_id, signal, created_at FROM derived_signals
     WHERE signal IN (?, ?, ?, ?, ?) AND created_at >= ?
     ORDER BY created_at DESC`
  )
    .bind(...WALLET_INTEREST_SIGNALS, since)
    .all();

  const byKey = new Map<string, { signals: string[]; created_at: string }>();
  for (const r of (rows.results ?? []) as { object_type: string; object_id: string; signal: string; created_at: string }[]) {
    const key = `${r.object_type}:${r.object_id}`;
    if (!byKey.has(key)) {
      byKey.set(key, { signals: [], created_at: r.created_at });
    }
    const entry = byKey.get(key)!;
    if (!entry.signals.includes(r.signal)) entry.signals.push(r.signal);
  }

  const ordered = [...byKey.entries()]
    .sort((a, b) => b[1].created_at.localeCompare(a[1].created_at))
    .slice(0, limit);

  const postIds: string[] = [];
  const commentIds: string[] = [];
  for (const [key] of ordered) {
    const [type, id] = key.split(':');
    if (type === 'post') postIds.push(id);
    else if (type === 'comment') commentIds.push(id);
  }

  const posts: { id: string; title: string | null; url: string | null; created_at: string | null; author_name: string | null; signals: string[] }[] = [];
  const comments: { id: string; post_id: string; content_snippet: string; created_at: string | null; author_name: string | null; signals: string[] }[] = [];

  if (postIds.length > 0) {
    const placeholders = postIds.map(() => '?').join(',');
    const postRows = await env.DB.prepare(
      `SELECT id, title, url, created_at, author_name FROM moltbook_posts WHERE id IN (${placeholders})`
    )
      .bind(...postIds)
      .all();
    const postMap = new Map<string, { title: string | null; url: string | null; created_at: string | null; author_name: string | null }>();
    for (const r of (postRows.results ?? []) as { id: string; title: string | null; url: string | null; created_at: string | null; author_name: string | null }[]) {
      postMap.set(r.id, { title: r.title, url: r.url, created_at: r.created_at, author_name: r.author_name });
    }
    for (const [key] of ordered) {
      const [type, id] = key.split(':');
      if (type !== 'post') continue;
      const meta = postMap.get(id);
      const entry = byKey.get(key)!;
      posts.push({
        id,
        title: meta?.title ?? null,
        url: meta?.url ?? null,
        created_at: meta?.created_at ?? null,
        author_name: meta?.author_name ?? null,
        signals: entry.signals,
      });
    }
  }

  if (commentIds.length > 0) {
    const placeholders = commentIds.map(() => '?').join(',');
    const commentRows = await env.DB.prepare(
      `SELECT id, post_id, content, created_at, author_name FROM moltbook_comments WHERE id IN (${placeholders})`
    )
      .bind(...commentIds)
      .all();
    const commentMap = new Map<string, { post_id: string; content: string | null; created_at: string | null; author_name: string | null }>();
    for (const r of (commentRows.results ?? []) as { id: string; post_id: string; content: string | null; created_at: string | null; author_name: string | null }[]) {
      commentMap.set(r.id, { post_id: r.post_id, content: r.content, created_at: r.created_at, author_name: r.author_name });
    }
    for (const [key] of ordered) {
      const [type, id] = key.split(':');
      if (type !== 'comment') continue;
      const meta = commentMap.get(id);
      const snippet = (meta?.content ?? '').slice(0, 200);
      const entry = byKey.get(key)!;
      comments.push({
        id,
        post_id: meta?.post_id ?? '',
        content_snippet: snippet,
        created_at: meta?.created_at ?? null,
        author_name: meta?.author_name ?? null,
        signals: entry.signals,
      });
    }
  }

  return jsonResponse({
    ok: true,
    data: {
      days,
      since,
      posts,
      comments,
    },
  });
}
