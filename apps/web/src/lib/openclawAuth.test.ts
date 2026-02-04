import { describe, expect, it } from 'vitest';
import { extractAgentKey } from './openclawAuth';

describe('openclawAuth', () => {
  it('reads agent key from X-OA-Agent-Key header', () => {
    const headers = new Headers({ 'X-OA-Agent-Key': '  oak_live_test  ' });
    expect(extractAgentKey(headers)).toBe('oak_live_test');
  });

  it('reads agent key from Authorization: Agent', () => {
    const headers = new Headers({ Authorization: 'Agent oak_live_auth' });
    expect(extractAgentKey(headers)).toBe('oak_live_auth');
  });

  it('is case-insensitive for Authorization: agent', () => {
    const headers = new Headers({ Authorization: 'agent oak_live_lower' });
    expect(extractAgentKey(headers)).toBe('oak_live_lower');
  });

  it('returns null when no agent key is present', () => {
    const headers = new Headers({ Authorization: 'Bearer nope' });
    expect(extractAgentKey(headers)).toBeNull();
  });
});
