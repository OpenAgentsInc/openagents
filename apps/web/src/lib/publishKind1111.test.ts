/* @vitest-environment jsdom */
import { afterEach, describe, expect, it } from 'vitest';
import { hasNostrExtension } from './publishKind1111';

describe('publishKind1111 helpers', () => {
  afterEach(() => {
    // @ts-expect-error cleanup
    delete (globalThis as { nostr?: unknown }).nostr;
  });

  it('detects nostr extension', () => {
    expect(hasNostrExtension()).toBe(false);
    // @ts-expect-error test shim
    (globalThis as { nostr?: unknown }).nostr = {};
    expect(hasNostrExtension()).toBe(true);
  });
});
