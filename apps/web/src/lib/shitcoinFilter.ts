/**
 * Shitcoin filter: hide posts that mention ticker-style shitcoins
 * ($ followed by 3–7 alphanumeric characters). We do not promote or engage
 * with shitcoins. See MOLTBOOK.md "Shitcoin filter" and AGENTS.md.
 */

/** Matches $TICKER where TICKER is 3–7 alphanumeric chars (e.g. $MOLTEN, $PEPE). */
const SHITCOIN_TICKER_REGEX = /\$[A-Za-z0-9]{3,7}\b/g;

/**
 * Returns true if text contains a shitcoin-style ticker ($ + 3–7 alphanumeric).
 */
export function hasShitcoinTicker(text: string): boolean {
  if (!text || typeof text !== 'string') return false;
  SHITCOIN_TICKER_REGEX.lastIndex = 0;
  return SHITCOIN_TICKER_REGEX.test(text);
}

/**
 * Filters out posts whose content contains a shitcoin ticker.
 * Use when displaying feeds so such posts are hidden and never get engagement.
 */
export function filterPostsWithShitcoin<T extends { content?: string | null }>(
  posts: Array<T>,
): Array<T> {
  return posts.filter((p) => !hasShitcoinTicker(p.content ?? ''));
}
