/**
 * Derived signals for posts/comments: wallet, openclaw, npub, lud16, github, etc.
 */

export type SignalName =
  | 'mentions_openclaw'
  | 'mentions_wallet'
  | 'mentions_invoice'
  | 'mentions_lightning'
  | 'mentions_zap'
  | 'mentions_sats'
  | 'has_lud16'
  | 'has_npub'
  | 'links_github'
  | 'language_guess';

const LUD16 = /[a-zA-Z0-9._+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}/;
const NPUB = /npub1[a-zA-Z0-9]{58}/;
const GITHUB = /github\.com\/[a-zA-Z0-9_.-]+\/[a-zA-Z0-9_.-]+/;

export interface SignalRow {
  object_type: 'post' | 'comment' | 'author';
  object_id: string;
  signal: SignalName;
  value: string | null;
  created_at: string;
}

export function computeSignals(
  objectType: 'post' | 'comment' | 'author',
  objectId: string,
  content: string | null | undefined,
  now: string
): SignalRow[] {
  const text = (content ?? '').toLowerCase();
  const rows: SignalRow[] = [];

  if (/openclaw/.test(text)) rows.push({ object_type: objectType, object_id: objectId, signal: 'mentions_openclaw', value: '1', created_at: now });
  if (/\bwallet\b/.test(text)) rows.push({ object_type: objectType, object_id: objectId, signal: 'mentions_wallet', value: '1', created_at: now });
  if (/\binvoice\b/.test(text)) rows.push({ object_type: objectType, object_id: objectId, signal: 'mentions_invoice', value: '1', created_at: now });
  if (/\blightning\b/.test(text)) rows.push({ object_type: objectType, object_id: objectId, signal: 'mentions_lightning', value: '1', created_at: now });
  if (/\bzap\b/.test(text)) rows.push({ object_type: objectType, object_id: objectId, signal: 'mentions_zap', value: '1', created_at: now });
  if (/\bsats?\b/.test(text)) rows.push({ object_type: objectType, object_id: objectId, signal: 'mentions_sats', value: '1', created_at: now });

  const fullContent = content ?? '';
  if (LUD16.test(fullContent)) rows.push({ object_type: objectType, object_id: objectId, signal: 'has_lud16', value: '1', created_at: now });
  if (NPUB.test(fullContent)) rows.push({ object_type: objectType, object_id: objectId, signal: 'has_npub', value: '1', created_at: now });
  if (GITHUB.test(fullContent)) rows.push({ object_type: objectType, object_id: objectId, signal: 'links_github', value: '1', created_at: now });

  return rows;
}
