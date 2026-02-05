import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  buildApprovalCookie,
  createApproval,
  getApproval,
  getApprovalFromCookie,
  recordApprovalDecision,
  resolveApproval,
} from './approvalStore';

describe('approvalStore', () => {
  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it('creates and resolves approvals', () => {
    const uuid = '123e4567-e89b-12d3-a456-426614174000';
    vi.stubGlobal('crypto', { randomUUID: () => uuid });
    vi.spyOn(Date, 'now').mockReturnValue(123456);

    const record = createApproval({
      userId: 'user-1',
      summary: 'Test',
      toolName: 'tool',
      toolInput: { a: 1 },
    });
    expect(record.id).toBe(uuid);
    expect(record.status).toBe('pending');

    const fetched = getApproval('user-1', uuid);
    expect(fetched?.summary).toBe('Test');

    const resolved = resolveApproval({
      userId: 'user-1',
      approvalId: uuid,
      decision: 'approved',
    });
    expect(resolved?.status).toBe('approved');
    expect(resolved?.resolvedAtMs).toBe(123456);
  });

  it('persists approval decisions in cookies', () => {
    vi.spyOn(Date, 'now').mockReturnValue(456789);
    recordApprovalDecision({
      userId: 'user-2',
      approvalId: 'approval-1',
      decision: 'approved',
    });

    const setCookie = buildApprovalCookie({
      userId: 'user-2',
      approvalId: 'approval-1',
      decision: 'approved',
      cookieHeader: null,
      secure: false,
    });
    const cookieHeader = setCookie.split(';')[0] ?? '';
    const stored = getApprovalFromCookie({
      userId: 'user-2',
      approvalId: 'approval-1',
      cookieHeader,
    });

    expect(stored?.status).toBe('approved');
    expect(stored?.resolvedAtMs).toBe(456789);
  });
});
