/**
 * Hardcoded blacklist of community slugs (e.g. from discovery, feeds, links).
 * Blacklisted communities are excluded from discovery and show a blocked state on direct URL.
 */
export const COMMUNITY_BLACKLIST: readonly string[] = ['clawcloud-api'];

const blacklistSet = new Set(
  COMMUNITY_BLACKLIST.map((s) => s.trim().toLowerCase()).filter(Boolean),
);

export function isCommunityBlacklisted(slug: string): boolean {
  return blacklistSet.has(slug.trim().toLowerCase());
}
