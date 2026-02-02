import { describe, expect, it } from 'vitest';
import { err, ok } from '../src/response';

describe('runtime contract response envelope', () => {
  it('wraps successful responses', () => {
    const payload = ok({ status: 'ok' });
    expect(payload).toEqual({ ok: true, data: { status: 'ok' } });
  });

  it('wraps error responses with code and message', () => {
    const payload = err('unauthorized', 'unauthorized');
    expect(payload.ok).toBe(false);
    if (!payload.ok) {
      expect(payload.error.code).toBe('unauthorized');
      expect(payload.error.message).toBe('unauthorized');
    }
  });
});
