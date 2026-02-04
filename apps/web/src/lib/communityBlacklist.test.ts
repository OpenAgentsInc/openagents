import { describe, expect, it } from 'vitest';
import { COMMUNITY_BLACKLIST, isCommunityBlacklisted } from './communityBlacklist';

describe('communityBlacklist', () => {
  it('matches blacklisted slugs', () => {
    for (const slug of COMMUNITY_BLACKLIST) {
      expect(isCommunityBlacklisted(slug)).toBe(true);
      expect(isCommunityBlacklisted(slug.toUpperCase())).toBe(true);
    }
  });

  it('returns false for allowed slugs', () => {
    expect(isCommunityBlacklisted('openagents')).toBe(false);
  });
});
