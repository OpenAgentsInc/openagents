import { afterEach, describe, expect, it, vi } from 'vitest';
import { createApproval, getApproval, resolveApproval } from './approvalStore';

describe('approvalStore', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('creates and resolves approvals', () => {
    if (!globalThis.crypto) {
      // @ts-expect-error test shim
      globalThis.crypto = { randomUUID: () => 'id-123' };
    } else {
      vi.spyOn(globalThis.crypto, 'randomUUID').mockReturnValue('id-123');
    }
    vi.spyOn(Date, 'now').mockReturnValue(123456);

    const record = createApproval({
      userId: 'user-1',
      summary: 'Test',
      toolName: 'tool',
      toolInput: { a: 1 },
    });
    expect(record.id).toBe('id-123');
    expect(record.status).toBe('pending');

    const fetched = getApproval('user-1', 'id-123');
    expect(fetched?.summary).toBe('Test');

    const resolved = resolveApproval({
      userId: 'user-1',
      approvalId: 'id-123',
      decision: 'approved',
    });
    expect(resolved?.status).toBe('approved');
    expect(resolved?.resolvedAtMs).toBe(123456);
  });
});
