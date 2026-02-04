/* @vitest-environment jsdom */
import { describe, expect, it, vi } from 'vitest';
import { posthogCapture, posthogIdentify } from './posthog';

describe('posthog helpers', () => {
  it('no-ops when posthog is unavailable', () => {
    // Should not throw
    posthogCapture('event');
    posthogIdentify('id');
  });

  it('calls posthog with page context', () => {
    const capture = vi.fn();
    const identify = vi.fn();
    // @ts-expect-error test shim
    window.posthog = { capture, identify };
    history.replaceState({}, '', '/test?x=1');

    posthogCapture('evt', { a: 1 });
    posthogIdentify('user', { b: 2 });

    expect(capture).toHaveBeenCalledWith('evt', {
      path: '/test',
      search: '?x=1',
      a: 1,
    });
    expect(identify).toHaveBeenCalledWith('user', {
      path: '/test',
      search: '?x=1',
      b: 2,
    });
  });
});
