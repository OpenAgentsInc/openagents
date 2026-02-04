import { afterEach, describe, expect, it, vi } from 'vitest';
import type { NostrEvent } from '@nostrify/nostrify';
import {
  AI_LABEL,
  WEB_KIND,
  communityToIdentifier,
  communityToIdentifiers,
  createAILabelTags,
  createPostTags,
  createReplyTags,
  formatCount,
  formatRelativeTime,
  formatSats,
  getPostCommunity,
  getPostIdentifier,
  hasAILabel,
  identifierToCommunity,
  identifierToSubclaw,
  isClawstrIdentifier,
  isTopLevelPost,
  subclawToIdentifier,
} from './clawstr';

function eventWithTags(tags: string[][]): NostrEvent {
  return {
    id: 'evt',
    pubkey: 'pub',
    created_at: 0,
    kind: 1111,
    tags,
    content: '',
    sig: 'sig',
  } as NostrEvent;
}

describe('clawstr helpers', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('builds and parses community identifiers', () => {
    const id = communityToIdentifier('Test');
    expect(id).toBe('https://openagents.com/c/test');
    expect(identifierToCommunity(id)).toBe('test');
    expect(subclawToIdentifier('Demo')).toBe('https://openagents.com/c/demo');
    expect(identifierToSubclaw('https://clawstr.com/c/demo/')).toBe('demo');
    expect(isClawstrIdentifier('https://openagents.com/c/demo')).toBe(true);
    expect(isClawstrIdentifier('https://example.com/c/demo')).toBe(false);
  });

  it('returns all accepted identifiers for a community', () => {
    const ids = communityToIdentifiers('Community');
    expect(ids).toEqual([
      'https://clawstr.com/c/community',
      'https://clawstr.com/c/community/',
      'https://openagents.com/c/community',
      'https://openagents.com/c/community/',
    ]);
  });

  it('extracts identifiers and communities from events', () => {
    const evt = eventWithTags([
      ['I', 'https://openagents.com/c/test'],
      ['K', WEB_KIND],
      ['i', 'https://openagents.com/c/test'],
      ['k', WEB_KIND],
    ]);
    expect(getPostIdentifier(evt)).toBe('https://openagents.com/c/test');
    expect(getPostCommunity(evt)).toBe('test');
    expect(isTopLevelPost(evt)).toBe(true);
  });

  it('detects AI labels', () => {
    const evt = eventWithTags([
      ['L', AI_LABEL.namespace],
      ['l', AI_LABEL.value, AI_LABEL.namespace],
    ]);
    expect(hasAILabel(evt)).toBe(true);
  });

  it('formats counts and sats', () => {
    expect(formatCount(999)).toBe('999');
    expect(formatCount(1500)).toBe('1.5k');
    expect(formatCount(2_500_000)).toBe('2.5M');
    expect(formatSats(900)).toBe('900');
    expect(formatSats(1500)).toBe('1.5k');
    expect(formatSats(2_500_000)).toBe('2.5M');
  });

  it('formats relative time buckets', () => {
    vi.spyOn(Date, 'now').mockReturnValue(1_000_000 * 1000);
    expect(formatRelativeTime(1_000_000 - 30)).toBe('just now');
    expect(formatRelativeTime(1_000_000 - 90)).toBe('1m ago');
    expect(formatRelativeTime(1_000_000 - 3600)).toBe('1h ago');
  });

  it('creates AI label, post, and reply tags', () => {
    const aiTags = createAILabelTags();
    expect(aiTags).toEqual([
      ['L', AI_LABEL.namespace],
      ['l', AI_LABEL.value, AI_LABEL.namespace],
    ]);

    const postTags = createPostTags('test', false);
    expect(postTags).toEqual([
      ['I', 'https://openagents.com/c/test'],
      ['K', WEB_KIND],
      ['i', 'https://openagents.com/c/test'],
      ['k', WEB_KIND],
    ]);

    const parent = eventWithTags([]);
    const replyTags = createReplyTags('test', parent, false);
    expect(replyTags).toEqual([
      ['e', parent.id, '', parent.pubkey],
      ['p', parent.pubkey],
      ['k', '1111'],
      ['I', 'https://openagents.com/c/test'],
      ['K', WEB_KIND],
    ]);
  });
});
