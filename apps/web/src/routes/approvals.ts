import { createFileRoute } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { resolveApiBase, resolveInternalKey } from '@/lib/openclawApi';
import { buildApprovalCookie, recordApprovalDecision } from '@/lib/approvalStore';
import { extractAgentKey } from '@/lib/openclawAuth';

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
        const rawUserId = auth?.user?.id;
        const userId = typeof rawUserId === 'string' ? rawUserId.trim() : '';
        const agentKey = extractAgentKey(request.headers);
        if (!userId && !agentKey) {
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

        let principalId = userId;
        if (!principalId && agentKey) {
          let apiBase = '';
          try {
            apiBase = resolveApiBase();
          } catch (error) {
            return json(500, {
              ok: false,
              error: error instanceof Error ? error.message : 'OpenClaw API base not configured',
            });
          }
          const principalResponse = await fetch(`${apiBase}/openclaw/principal`, {
            method: 'GET',
            headers: {
              'content-type': 'application/json',
              'X-OA-Agent-Key': agentKey,
            },
            signal: request.signal,
          });
          if (!principalResponse.ok) {
            const message = await principalResponse.text().catch(() => '');
            return json(principalResponse.status || 500, {
              ok: false,
              error: message || `OpenClaw principal failed (${principalResponse.status})`,
            });
          }
          const principalPayload = (await principalResponse
            .json()
            .catch(() => null)) as
            | { ok?: boolean; data?: { tenant_id?: string | null }; error?: string | null }
            | null;
          if (!principalPayload?.ok) {
            return json(502, {
              ok: false,
              error: principalPayload?.error ?? 'OpenClaw principal failed',
            });
          }
          const principalTenant = principalPayload.data?.tenant_id ?? '';
          if (!principalTenant || typeof principalTenant !== 'string') {
            return json(502, { ok: false, error: 'OpenClaw principal missing tenant id' });
          }
          principalId = principalTenant;
        }

        const threadIdRaw = (body.threadId ?? '').trim();
        const threadId = threadIdRaw ? `${principalId}:${threadIdRaw}` : `user:${principalId}`;

        const agentWorkerUrl = resolveAgentWorkerUrl();
        let internalKey = '';
        if (agentWorkerUrl) {
          try {
            internalKey = resolveInternalKey();
          } catch {
            internalKey = '';
          }
        }

        if (!agentWorkerUrl || !internalKey) {
          const record = recordApprovalDecision({ userId: principalId, approvalId, decision });
          const setCookie = buildApprovalCookie({
            userId: principalId,
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
        headers.set('X-OA-User-Id', principalId);
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
