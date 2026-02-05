import { describe, expect, it } from 'vitest';
import { getKbEntries, getKbEntryBySlug } from './content';

describe('content helpers', () => {
  it('loads KB entries', () => {
    const entries = getKbEntries();
    expect(entries.length).toBeGreaterThan(0);
    expect(entries[0]?.slug).toBeTruthy();
  });

  it('finds a KB entry by slug', () => {
    const entry = getKbEntryBySlug('agent-login');
    expect(entry?.slug).toBe('agent-login');
    expect(entry?.body.length).toBeGreaterThan(0);
  });
});
