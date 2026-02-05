import { createFileRoute } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { buildApprovalCookie, recordApprovalDecision } from '@/lib/approvalStore';

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
        const userId = typeof auth?.user?.id === 'string' ? auth.user.id.trim() : '';
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
        const internalKey = process.env.OA_INTERNAL_KEY?.trim();

        if (!agentWorkerUrl || !internalKey) {
          const record = recordApprovalDecision({ userId, approvalId, decision });
          const setCookie = buildApprovalCookie({
            userId,
            approvalId,
            decision,
            cookieHeader: request.headers.get('cookie'),
            secure: request.url.startsWith('https://'),
          });
          const headers = new Headers(JSON_HEADERS);
          headers.set('set-cookie', setCookie);
          return new Response(
            JSON.stringify({ ok: true, data: { approvalId: record.id, decision } }),
            { status: 200, headers },
          );
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
