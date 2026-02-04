import { describe, expect, it, vi } from 'vitest';
import type { NostrEvent } from '@nostrify/nostrify';
import {
  buildCommunityCounts,
  dedupeEvents,
  fetchDiscoveredCommunities,
  mergeCommunityCounts,
} from './discoveredCommunities';
import { AI_LABEL, WEB_KIND } from './clawstr';
import { queryCachedEvents } from './nostrEventCache';
import { queryWithFallback } from './nostrQuery';

vi.mock('./nostrEventCache', () => ({
  queryCachedEvents: vi.fn(),
}));

vi.mock('./nostrQuery', () => ({
  queryWithFallback: vi.fn(),
}));

const mockQueryCachedEvents = vi.mocked(queryCachedEvents);
const mockQueryWithFallback = vi.mocked(queryWithFallback);

function makeEvent(id: string, tags: string[][]): NostrEvent {
  return {
    id,
    pubkey: 'pub',
    created_at: 0,
    kind: 1111,
    tags,
    content: '',
    sig: 'sig',
  } as NostrEvent;
}

describe('discoveredCommunities helpers', () => {
  it('builds community counts from top-level AI posts', () => {
    const aiTags = [
      ['L', AI_LABEL.namespace],
      ['l', AI_LABEL.value, AI_LABEL.namespace],
    ];
    const events = [
      makeEvent('1', [
        ['I', 'https://openagents.com/c/test'],
        ['K', WEB_KIND],
        ['i', 'https://openagents.com/c/test'],
        ['k', WEB_KIND],
        ...aiTags,
      ]),
      makeEvent('2', [
        ['I', 'https://openagents.com/c/test'],
        ['K', WEB_KIND],
        ['i', 'https://openagents.com/c/test'],
        ['k', WEB_KIND],
      ]),
      makeEvent('3', [
        ['I', 'https://openagents.com/c/clawcloud-api'],
        ['K', WEB_KIND],
        ['i', 'https://openagents.com/c/clawcloud-api'],
        ['k', WEB_KIND],
        ...aiTags,
      ]),
    ];

    const counts = buildCommunityCounts(events, false);
    expect(counts).toEqual([{ slug: 'test', count: 1 }]);
  });

  it('dedupes events by id', () => {
    const events = [makeEvent('1', []), makeEvent('1', []), makeEvent('2', [])];
    expect(dedupeEvents(events).map((e) => e.id)).toEqual(['1', '2']);
  });

  it('merges community counts and caps to limit', () => {
    const merged = mergeCommunityCounts(
      [
        { slug: 'alpha', count: 1 },
        { slug: 'beta', count: 3 },
      ],
      [
        { slug: 'beta', count: 4 },
        { slug: 'gamma', count: 2 },
      ],
      2,
    );
    expect(merged).toEqual([
      { slug: 'beta', count: 4 },
      { slug: 'gamma', count: 2 },
    ]);
  });

  it('fetchDiscoveredCommunities combines cache + live results', async () => {
    const aiTags = [
      ['L', AI_LABEL.namespace],
      ['l', AI_LABEL.value, AI_LABEL.namespace],
    ];
    const cached = [
      makeEvent('1', [
        ['I', 'https://openagents.com/c/test'],
        ['K', WEB_KIND],
        ['i', 'https://openagents.com/c/test'],
        ['k', WEB_KIND],
        ...aiTags,
      ]),
    ];
    const live = [
      makeEvent('2', [
        ['I', 'https://openagents.com/c/second'],
        ['K', WEB_KIND],
        ['i', 'https://openagents.com/c/second'],
        ['k', WEB_KIND],
        ...aiTags,
      ]),
    ];
    mockQueryCachedEvents.mockResolvedValueOnce(cached);
    mockQueryWithFallback.mockResolvedValueOnce(live);

    const result = await fetchDiscoveredCommunities(
      { query: vi.fn() },
      { limit: 10, showAll: false },
    );

    expect(result.data.map((c) => c.slug).sort()).toEqual(['second', 'test']);
    expect(result.meta.cachedCount).toBe(1);
    expect(result.meta.combinedCount).toBe(2);
  });
});
