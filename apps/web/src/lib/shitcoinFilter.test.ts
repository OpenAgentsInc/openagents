import { describe, expect, it } from 'vitest';
import { filterPostsWithShitcoin, hasShitcoinTicker } from './shitcoinFilter';

describe('shitcoinFilter', () => {
  it('detects ticker-style patterns', () => {
    expect(hasShitcoinTicker('Buy $MOLTEN now')).toBe(true);
    expect(hasShitcoinTicker('This is $BTC')).toBe(true);
    expect(hasShitcoinTicker('no ticker here')).toBe(false);
  });

  it('ignores invalid ticker lengths', () => {
    expect(hasShitcoinTicker('too short $AA')).toBe(false);
    expect(hasShitcoinTicker('too long $TOOLONGGG')).toBe(false);
  });

  it('filters posts that contain tickers', () => {
    const posts = [
      { id: 1, content: 'hello' },
      { id: 2, content: 'buy $PEPE' },
      { id: 3, content: null },
    ];
    const filtered = filterPostsWithShitcoin(posts);
    expect(filtered.map((post) => post.id)).toEqual([1, 3]);
  });
});
