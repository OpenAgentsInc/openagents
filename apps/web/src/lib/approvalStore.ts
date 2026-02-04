export type ApprovalDecision = 'approved' | 'rejected';

export type ApprovalRecord = {
  id: string;
  userId: string;
  createdAtMs: number;
  status: 'pending' | ApprovalDecision;
  resolvedAtMs?: number;
  summary: string;
  toolName: string;
  toolInput: unknown;
};

type ApprovalCookieEntry = {
  decision: ApprovalDecision;
  resolvedAtMs: number;
};

type ApprovalCookiePayload = {
  userId: string;
  approvals: Record<string, unknown>;
};

export const APPROVAL_COOKIE_NAME = 'oa_approvals';

const approvalsByUser = new Map<string, Map<string, ApprovalRecord>>();

function getUserMap(userId: string): Map<string, ApprovalRecord> {
  const existing = approvalsByUser.get(userId);
  if (existing) return existing;
  const created = new Map<string, ApprovalRecord>();
  approvalsByUser.set(userId, created);
  return created;
}

export function createApproval(params: {
  userId: string;
  summary: string;
  toolName: string;
  toolInput: unknown;
}): ApprovalRecord {
  const id = crypto.randomUUID();
  const record: ApprovalRecord = {
    id,
    userId: params.userId,
    createdAtMs: Date.now(),
    status: 'pending',
    summary: params.summary,
    toolName: params.toolName,
    toolInput: params.toolInput,
  };
  getUserMap(params.userId).set(id, record);
  return record;
}

export function getApproval(userId: string, approvalId: string): ApprovalRecord | null {
  return getUserMap(userId).get(approvalId) ?? null;
}

export function recordApprovalDecision(params: {
  userId: string;
  approvalId: string;
  decision: ApprovalDecision;
  summary?: string;
  toolName?: string;
  toolInput?: unknown;
}): ApprovalRecord {
  const existing = getApproval(params.userId, params.approvalId);
  const now = Date.now();
  if (existing) {
    existing.status = params.decision;
    existing.resolvedAtMs = now;
    return existing;
  }

  const record: ApprovalRecord = {
    id: params.approvalId,
    userId: params.userId,
    createdAtMs: now,
    status: params.decision,
    resolvedAtMs: now,
    summary: params.summary ?? 'Approval decision recorded.',
    toolName: params.toolName ?? 'unknown',
    toolInput: params.toolInput ?? null,
  };
  getUserMap(params.userId).set(params.approvalId, record);
  return record;
}

export function resolveApproval(params: {
  userId: string;
  approvalId: string;
  decision: ApprovalDecision;
}): ApprovalRecord | null {
  const record = getApproval(params.userId, params.approvalId);
  if (!record) return null;
  record.status = params.decision;
  record.resolvedAtMs = Date.now();
  return record;
}

function parseCookieHeader(cookieHeader: string | null): Record<string, string> {
  if (!cookieHeader) return {};
  const result: Record<string, string> = {};
  const parts = cookieHeader.split(';');
  for (const part of parts) {
    const idx = part.indexOf('=');
    if (idx === -1) continue;
    const key = part.slice(0, idx).trim();
    if (!key) continue;
    result[key] = part.slice(idx + 1).trim();
  }
  return result;
}

function decodeApprovalCookie(cookieHeader: string | null): ApprovalCookiePayload | null {
  const cookies = parseCookieHeader(cookieHeader);
  const raw = cookies[APPROVAL_COOKIE_NAME];
  if (!raw) return null;
  let decoded = raw;
  try {
    decoded = decodeURIComponent(raw);
  } catch {
    // ignore decode failures, attempt raw parse
  }
  try {
    const payload = JSON.parse(decoded) as unknown;
    if (!payload || typeof payload !== 'object') return null;
    const record = payload as { userId?: unknown; approvals?: unknown };
    if (typeof record.userId !== 'string') return null;
    if (!record.approvals || typeof record.approvals !== 'object') return null;
    return { userId: record.userId, approvals: record.approvals as Record<string, unknown> };
  } catch {
    return null;
  }
}

export function getApprovalFromCookie(params: {
  userId: string;
  approvalId: string;
  cookieHeader: string | null;
}): ApprovalRecord | null {
  const payload = decodeApprovalCookie(params.cookieHeader);
  if (!payload || payload.userId !== params.userId) return null;
  const entry = payload.approvals[params.approvalId];
  if (!entry || typeof entry !== 'object') return null;
  const typedEntry = entry as { decision?: unknown; resolvedAtMs?: unknown };
  if (typedEntry.decision !== 'approved' && typedEntry.decision !== 'rejected') return null;
  if (typeof typedEntry.resolvedAtMs !== 'number') return null;
  return {
    id: params.approvalId,
    userId: params.userId,
    createdAtMs: typedEntry.resolvedAtMs,
    status: typedEntry.decision,
    resolvedAtMs: typedEntry.resolvedAtMs,
    summary: 'Stored approval decision.',
    toolName: 'unknown',
    toolInput: null,
  };
}

export function buildApprovalCookie(params: {
  userId: string;
  approvalId: string;
  decision: ApprovalDecision;
  cookieHeader: string | null;
  maxAgeSeconds?: number;
  secure?: boolean;
}): string {
  const now = Date.now();
  const existing = decodeApprovalCookie(params.cookieHeader);
  const payload: ApprovalCookiePayload =
    existing && existing.userId === params.userId
      ? existing
      : { userId: params.userId, approvals: {} };
  const approvals = payload.approvals as Record<string, ApprovalCookieEntry>;
  approvals[params.approvalId] = {
    decision: params.decision,
    resolvedAtMs: now,
  };
  const encoded = encodeURIComponent(JSON.stringify(payload));
  const maxAge = params.maxAgeSeconds ?? 60 * 60 * 24 * 7;
  const parts = [
    `${APPROVAL_COOKIE_NAME}=${encoded}`,
    'Path=/',
    `Max-Age=${maxAge}`,
    'SameSite=Lax',
    'HttpOnly',
  ];
  if (params.secure !== false) {
    parts.push('Secure');
  }
  return parts.join('; ');
}
