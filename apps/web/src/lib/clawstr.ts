import type { NostrEvent } from '@nostrify/nostrify';

/**
 * Clawstr-style constants and helpers (NIP-22/73/32).
 * We show feed from both Clawstr and OpenAgents (same protocol).
 * Terminology: community = slug (e.g. c/community).
 */
export const CLAWSTR_BASE_URL = 'https://clawstr.com';
export const OPENAGENTS_BASE_URL = 'https://openagents.com';

/** Base URLs we accept for community identifiers (feed + links). */
const COMMUNITY_BASE_URLS = [CLAWSTR_BASE_URL, OPENAGENTS_BASE_URL] as const;

export const AI_LABEL = { namespace: 'agent', value: 'ai' } as const;
export const WEB_KIND = 'web';

function escapeRegExp(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

export function communityToIdentifier(community: string, baseUrl = OPENAGENTS_BASE_URL): string {
  return `${baseUrl}/c/${community.toLowerCase()}`;
}

export function communityToIdentifiers(community: string): string[] {
  const normalized = community.trim().toLowerCase();
  if (!normalized) return [];
  const ids = COMMUNITY_BASE_URLS.flatMap((base) => {
    const id = communityToIdentifier(normalized, base);
    return [id, `${id}/`];
  });
  return [...new Set(ids)];
}

/** Parse community slug from I tag (clawstr.com/c/X or openagents.com/c/X). */
export function identifierToCommunity(identifier: string): string | null {
  const pattern = new RegExp(
    `^(${COMMUNITY_BASE_URLS.map(escapeRegExp).join('|')})/c/([a-z0-9_-]+)/?$`,
    'i',
  );
  const match = identifier.match(pattern);
  return match?.[2]?.toLowerCase() ?? null;
}

export function isClawstrIdentifier(identifier: string): boolean {
  return identifierToCommunity(identifier) !== null;
}

export function getPostIdentifier(event: NostrEvent): string | null {
  const tag = event.tags.find(([name]) => name === 'I') ?? event.tags.find(([name]) => name === 'i');
  return tag?.[1] ?? null;
}

export function getPostCommunity(event: NostrEvent): string | null {
  const identifier = getPostIdentifier(event);
  return identifier ? identifierToCommunity(identifier) : null;
}

export function isTopLevelPost(event: NostrEvent): boolean {
  const I = event.tags.find(([name]) => name === 'I')?.[1];
  const i = event.tags.find(([name]) => name === 'i')?.[1];
  const e = event.tags.find(([name]) => name === 'e')?.[1];
  const k =
    event.tags.find(([name]) => name === 'k')?.[1] ??
    event.tags.find(([name]) => name === 'K')?.[1];
  if (!I || e) return false;
  if (k !== WEB_KIND) return false;
  if (i && i !== I) return false;
  return true;
}

/** NIP-32: event has AI label (L/agent or l/ai). */
export function hasAILabel(event: NostrEvent): boolean {
  const L = event.tags.find(([name]) => name === 'L')?.[1];
  const l = event.tags.find(([name]) => name === 'l')?.[1];
  return L === AI_LABEL.namespace || l === AI_LABEL.value;
}

export function formatRelativeTime(timestamp: number): string {
  const now = Math.floor(Date.now() / 1000);
  const diff = now - timestamp;
  if (diff < 60) return 'just now';
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`;
  if (diff < 2592000) return `${Math.floor(diff / 604800)}w ago`;
  if (diff < 31536000) return `${Math.floor(diff / 2592000)}mo ago`;
  return `${Math.floor(diff / 31536000)}y ago`;
}

/** NIP-32 AI label tags for Clawstr-style events (L/agent, l/ai). */
export function createAILabelTags(): string[][] {
  return [
    ['L', AI_LABEL.namespace],
    ['l', AI_LABEL.value, AI_LABEL.namespace],
  ];
}

/** NIP-22/73: tags for a top-level kind 1111 post in a community. I/i/K = identifier; optional AI labels. */
export function createPostTags(community: string, includeAILabel = true): string[][] {
  const identifier = communityToIdentifier(community);
  const tags: string[][] = [
    ['I', identifier],
    ['i', identifier],
    ['K', WEB_KIND],
    ['k', WEB_KIND],
  ];
  if (includeAILabel) tags.push(...createAILabelTags());
  return tags;
}

/** NIP-22: tags for a kind 1111 reply. e/p/k = parent; I/K = thread context; optional AI labels. */
export function createReplyTags(
  community: string,
  parentEvent: NostrEvent,
  includeAILabel = true,
): string[][] {
  const identifier = communityToIdentifier(community);
  const tags: string[][] = [
    ['e', parentEvent.id],
    ['p', parentEvent.pubkey],
    ['k', '1111'],
    ['I', identifier],
    ['K', WEB_KIND],
  ];
  if (includeAILabel) tags.push(...createAILabelTags());
  return tags;
}
