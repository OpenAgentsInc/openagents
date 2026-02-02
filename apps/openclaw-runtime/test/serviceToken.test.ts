import { describe, expect, it } from 'vitest';
import { extractServiceToken, isServiceTokenValid } from '../src/auth/serviceToken';

describe('service token auth', () => {
  it('extracts bearer token from authorization header', () => {
    const headers = new Headers({ Authorization: 'Bearer test-token' });
    expect(extractServiceToken(headers)).toBe('test-token');
  });

  it('extracts token from x-openagents-service-token header', () => {
    const headers = new Headers({ 'X-OpenAgents-Service-Token': 'header-token' });
    expect(extractServiceToken(headers)).toBe('header-token');
  });

  it('prefers authorization header when both are present', () => {
    const headers = new Headers({
      Authorization: 'Bearer auth-token',
      'X-OpenAgents-Service-Token': 'header-token',
    });
    expect(extractServiceToken(headers)).toBe('auth-token');
  });

  it('validates token against expected value', () => {
    const headers = new Headers({ Authorization: 'Bearer match' });
    expect(isServiceTokenValid(headers, 'match')).toBe(true);
    expect(isServiceTokenValid(headers, 'nope')).toBe(false);
  });
});
