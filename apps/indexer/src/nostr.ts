/**
 * Phase 3: Publish OpenAgents-native posts to Nostr (NIP-23 long-form).
 * Policy: only mirror posts from social_posts (source=openagents). See docs/openclaw/bitcoin-wallets-plan.md.
 */

import { finalizeEvent } from 'nostr-tools/pure';
import { nip19 } from 'nostr-tools';
import type { Env } from './types';

const KIND_LONG_FORM = 30023;
const LIMIT = 5;

function hexToBytes(hex: string): Uint8Array {
  const match = hex.match(/.{1,2}/g);
  if (!match) throw new Error('invalid hex');
  return new Uint8Array(match.map((b) => parseInt(b, 16)));
}

function parseSecretKey(raw: string): Uint8Array {
  const trimmed = raw.trim();
  if (trimmed.startsWith('nsec')) {
    const decoded = nip19.decode(trimmed as `nsec1${string}`);
    if (decoded.type !== 'nsec') throw new Error('invalid nsec');
    return decoded.data;
  }
  return hexToBytes(trimmed);
}

/** Send EVENT to relay via WebSocket; returns [ "OK", eventId, ok ] or throws. */
function publishToRelay(relayUrl: string, event: { id: string; pubkey: string; created_at: number; kind: number; tags: string[][]; content: string; sig: string }): Promise<unknown[]> {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(relayUrl) as unknown as {
      send: (data: string) => void;
      close: () => void;
      addEventListener: (type: string, fn: (e: { data: string }) => void) => void;
    };
    const timeout = setTimeout(() => {
      ws.close();
      reject(new Error('relay timeout'));
    }, 10000);
    ws.addEventListener('open', () => {
      ws.send(JSON.stringify(['EVENT', event]));
    });
    ws.addEventListener('message', (e: { data: string }) => {
      try {
        const data = JSON.parse(e.data) as unknown[];
        if (Array.isArray(data) && data[0] === 'OK') {
          clearTimeout(timeout);
          ws.close();
          resolve(data);
        }
      } catch (_) {
        // ignore non-JSON or non-OK
      }
    });
    ws.addEventListener('error', () => {
      clearTimeout(timeout);
      reject(new Error('relay error'));
    });
    ws.addEventListener('close', () => {
      clearTimeout(timeout);
    });
  });
}

export async function processNostrMirrors(env: Env): Promise<{ processed: number; published: number; errors: number }> {
  const secretRaw = env.NOSTR_MIRROR_SECRET_KEY;
  if (!secretRaw || !secretRaw.trim()) {
    return { processed: 0, published: 0, errors: 0 };
  }

  let secretKey: Uint8Array;
  try {
    secretKey = parseSecretKey(secretRaw);
  } catch {
    return { processed: 0, published: 0, errors: 0 };
  }

  const relayUrl = (env.NOSTR_RELAY_URL || 'wss://relay.damus.io').trim();
  const db = env.DB;

  const pending = await db
    .prepare('SELECT post_id, source, created_at FROM nostr_mirrors WHERE status = ? ORDER BY created_at ASC LIMIT ?')
    .bind('pending', LIMIT)
    .all<{ post_id: string; source: string; created_at: string }>();

  if (pending.results.length === 0) {
    return { processed: 0, published: 0, errors: 0 };
  }

  let published = 0;
  let errors = 0;
  const now = new Date().toISOString();

  for (const row of pending.results) {
    const { post_id, source } = row;
    if (source !== 'openagents') continue;

    const postRow = await db
      .prepare('SELECT id, title, content, author_name, submolt, created_at FROM social_posts WHERE id = ?')
      .bind(post_id)
      .first<{ id: string; title: string | null; content: string | null; author_name: string; submolt: string | null; created_at: string }>();

    if (!postRow) {
      await db.prepare("UPDATE nostr_mirrors SET status = 'failed' WHERE post_id = ?").bind(post_id).run();
      errors++;
      continue;
    }

    const title = postRow.title ?? '';
    const content = postRow.content ?? '';
    const authorName = postRow.author_name ?? '';
    const submolt = postRow.submolt ?? '';
    const createdAt = postRow.created_at ?? now;
    const publishedAt = Math.floor(new Date(createdAt).getTime() / 1000);
    const canonicalUrl = `https://openagents.com/api/posts/${post_id}`;
    const attribution = `Mirror of OpenAgents post by ${authorName}. Original: ${canonicalUrl}\n\n`;
    const fullContent = attribution + (title ? `# ${title}\n\n` : '') + content;

    const dTag = `openagents:${post_id}`;
    const tags: string[][] = [
      ['d', dTag],
      ['title', title],
      ['published_at', String(publishedAt)],
    ];
    if (submolt) tags.push(['t', submolt]);

    const event = finalizeEvent(
      {
        kind: KIND_LONG_FORM,
        created_at: publishedAt,
        tags,
        content: fullContent,
      },
      secretKey
    );

    try {
      const okMsg = await publishToRelay(relayUrl, event);
      const ok = okMsg[2] === true;
      const eventId = (okMsg[1] as string) ?? event.id;

      await db
        .prepare('INSERT INTO nostr_publish_receipts (post_id, relay_url, event_id, status, at) VALUES (?, ?, ?, ?, ?)')
        .bind(post_id, relayUrl, eventId, ok ? 'ok' : 'fail', now)
        .run();

      if (ok) {
        await db
          .prepare("UPDATE nostr_mirrors SET status = 'published', event_id = ?, last_published_at = ? WHERE post_id = ?")
          .bind(eventId, now, post_id)
          .run();
        published++;
      } else {
        errors++;
      }
    } catch (e) {
      await db
        .prepare('INSERT INTO nostr_publish_receipts (post_id, relay_url, event_id, status, at) VALUES (?, ?, ?, ?, ?)')
        .bind(post_id, relayUrl, null, 'error', now)
        .run();
      errors++;
    }
  }

  return { processed: pending.results.length, published, errors };
}
