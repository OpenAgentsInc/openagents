import { createFileRoute } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { resolveInternalKey } from '@/lib/openclawApi';
import { resolveApproval } from '@/lib/approvalStore';

type ApprovalDecision = 'approved' | 'rejected';

type ApprovalRespondBody = {
  approvalId?: string;
  decision?: ApprovalDecision;
  threadId?: string;
};

type ApiEnvelope<T> = {
  ok: boolean;
  data?: T | null;
  error?: string;
};

const JSON_HEADERS = { 'content-type': 'application/json' };

function json<T>(status: number, payload: ApiEnvelope<T>) {
  return new Response(JSON.stringify(payload), { status, headers: JSON_HEADERS });
}

function resolveAgentWorkerUrl(): string {
  const raw = process.env.AGENT_WORKER_URL ?? '';
  return raw.trim().replace(/\/$/, '');
}

export const Route = createFileRoute('/approvals')({
  server: {
    handlers: {
      POST: async ({ request }) => {
        const auth = await getAuth().catch(() => null);
        const userId = auth?.user?.id ? auth.user.id.trim() : '';
        if (!userId) {
          return json(401, { ok: false, error: 'not authenticated' });
        }

        const body = (await request.json().catch(() => null)) as ApprovalRespondBody | null;
        if (!body) {
          return json(400, { ok: false, error: 'invalid json body' });
        }

        const approvalId = (body.approvalId ?? '').trim();
        if (!approvalId) {
          return json(400, { ok: false, error: 'missing approvalId' });
        }

        const decision = body.decision;
        if (decision !== 'approved' && decision !== 'rejected') {
          return json(400, { ok: false, error: 'invalid decision' });
        }

        const threadIdRaw = (body.threadId ?? '').trim();
        const threadId = threadIdRaw ? `${userId}:${threadIdRaw}` : `user:${userId}`;

        const agentWorkerUrl = resolveAgentWorkerUrl();
        if (!agentWorkerUrl) {
          const record = resolveApproval({ userId, approvalId, decision });
          if (!record) {
            return json(404, { ok: false, error: 'approval not found' });
          }
          return json(200, { ok: true, data: { approvalId: record.id, decision } });
        }

        let internalKey = '';
        try {
          internalKey = resolveInternalKey();
        } catch (error) {
          return json(500, {
            ok: false,
            error: error instanceof Error ? error.message : 'OA_INTERNAL_KEY not configured',
          });
        }

        const headers = new Headers();
        headers.set('content-type', 'application/json');
        headers.set('accept', 'application/json');
        headers.set('X-OA-Internal-Key', internalKey);
        headers.set('X-OA-User-Id', userId);
        headers.set('X-OA-Thread-Id', threadId);

        const upstream = await fetch(`${agentWorkerUrl}/internal/approval/respond`, {
          method: 'POST',
          headers,
          body: JSON.stringify({ approvalId, decision }),
        });

        return new Response(upstream.body, {
          status: upstream.status,
          headers: {
            'content-type': upstream.headers.get('content-type') ?? 'application/json',
          },
        });
      },
    },
  },
});
