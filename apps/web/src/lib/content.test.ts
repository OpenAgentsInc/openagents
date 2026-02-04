import { describe, expect, it } from 'vitest';
import { getKbEntries, getKbEntryBySlug } from './content';

describe('content helpers', () => {
  it('loads KB entries', () => {
    const entries = getKbEntries();
    expect(entries.length).toBeGreaterThan(0);
    expect(entries[0]?.slug).toBeTruthy();
  });

  it('finds a KB entry by slug', () => {
    const entry = getKbEntryBySlug('openclaw-wallets');
    expect(entry?.slug).toBe('openclaw-wallets');
    expect(entry?.body.length).toBeGreaterThan(0);
  });
});
