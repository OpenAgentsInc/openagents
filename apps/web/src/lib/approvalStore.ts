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
