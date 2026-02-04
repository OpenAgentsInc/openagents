/* @vitest-environment jsdom */
import { afterEach, describe, expect, it } from 'vitest';
import { hasNostrExtension } from './publishKind1111';

describe('publishKind1111 helpers', () => {
  afterEach(() => {
    delete (globalThis as { nostr?: unknown }).nostr;
  });

  it('detects nostr extension', () => {
    expect(hasNostrExtension()).toBe(false);
    (globalThis as { nostr?: unknown }).nostr = {};
    expect(hasNostrExtension()).toBe(true);
  });
});
