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
      },
    },
  },
});
