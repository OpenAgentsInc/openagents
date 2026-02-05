import { useMemo, useRef, useState } from 'react';
import { createFileRoute } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { useAction, useQuery } from 'convex/react';
import { api } from '../../../convex/_generated/api';
import { OpenClawSetupCards } from '@/components/openclaw/openclaw-setup-cards';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Textarea } from '@/components/ui/textarea';
import { extractAgentKey } from '@/lib/openclawAuth';
import { cn } from '@/lib/utils';
import { resolveApiBase, resolveInternalKey } from '@/lib/openclawApi';
import { consumeOpenClawStream } from '@/lib/openclawStream';

const DEFAULT_AGENT_ID = 'main';
const QUICK_START_MESSAGE =
  'Hello OpenClaw. Give me a quick intro and suggest a first task to try.';

export const Route = createFileRoute('/_app/openclaw/chat')({
  component: OpenClawChatPage,
  server: {
    handlers: {
      POST: async ({ request }) => {
        const body = (await request.json().catch(() => null)) as
          | { input?: unknown; sessionKey?: unknown; agentId?: unknown }
          | null;
        const input = typeof body?.input === 'string' ? body.input.trim() : '';
        if (!input) {
          return new Response(JSON.stringify({ ok: false, error: 'missing input' }), {
            status: 400,
            headers: { 'content-type': 'application/json' },
          });
        }

        const auth = await getAuth().catch(() => null);
        const rawUserId = auth?.user?.id;
        const userId = typeof rawUserId === 'string' ? rawUserId.trim() : '';
        const agentKey = extractAgentKey(request.headers);
        if (!userId && !agentKey) {
          return new Response(JSON.stringify({ ok: false, error: 'not authenticated' }), {
            status: 401,
            headers: { 'content-type': 'application/json' },
          });
        }

        let apiBase = '';
        try {
          apiBase = resolveApiBase();
        } catch (error) {
          return new Response(
            JSON.stringify({
              ok: false,
              error:
                error instanceof Error
                  ? error.message
                  : 'OpenClaw API base not configured',
            }),
            { status: 500, headers: { 'content-type': 'application/json' } },
          );
        }

        let tenantKey = userId;
        if (!tenantKey && agentKey) {
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
            return new Response(
              JSON.stringify({
                ok: false,
                error: message || `OpenClaw principal failed (${principalResponse.status})`,
              }),
              { status: principalResponse.status || 500, headers: { 'content-type': 'application/json' } },
            );
          }
          const principalPayload = (await principalResponse
            .json()
            .catch(() => null)) as
            | { ok?: boolean; data?: { tenant_id?: string | null }; error?: string | null }
            | null;
          if (!principalPayload?.ok) {
            return new Response(
              JSON.stringify({
                ok: false,
                error: principalPayload?.error ?? 'OpenClaw principal failed',
              }),
              { status: 502, headers: { 'content-type': 'application/json' } },
            );
          }
          const principalTenant = principalPayload.data?.tenant_id ?? '';
          if (!principalTenant || typeof principalTenant !== 'string') {
            return new Response(
              JSON.stringify({ ok: false, error: 'OpenClaw principal missing tenant id' }),
              { status: 502, headers: { 'content-type': 'application/json' } },
            );
          }
          tenantKey = principalTenant;
        }

        let internalKey = '';
        if (userId) {
          try {
            internalKey = resolveInternalKey();
          } catch (error) {
            return new Response(
              JSON.stringify({
                ok: false,
                error: error instanceof Error ? error.message : 'OA_INTERNAL_KEY not configured',
              }),
              { status: 500, headers: { 'content-type': 'application/json' } },
            );
          }
        }

        const sessionKey = typeof body?.sessionKey === 'string' ? body.sessionKey.trim() : '';
        const agentId = typeof body?.agentId === 'string' ? body.agentId.trim() : '';

        const payload = {
          model: `openclaw:${agentId || DEFAULT_AGENT_ID}`,
          input,
          stream: true,
          user: tenantKey,
        };

        const headers = new Headers();
        headers.set('content-type', 'application/json');
        headers.set('accept', 'text/event-stream');
        if (userId) {
          headers.set('X-OA-Internal-Key', internalKey);
          headers.set('X-OA-User-Id', userId);
        } else if (agentKey) {
          headers.set('X-OA-Agent-Key', agentKey);
        }
        if (sessionKey) headers.set('x-openclaw-session-key', sessionKey);
        if (agentId) headers.set('x-openclaw-agent-id', agentId);

        const response = await fetch(`${apiBase}/openclaw/chat`, {
          method: 'POST',
          headers,
          body: JSON.stringify(payload),
          signal: request.signal,
        });

        if (!response.ok || !response.body) {
          const message = await response.text().catch(() => '');
          return new Response(
            JSON.stringify({ ok: false, error: message || 'OpenClaw chat failed' }),
            {
              status: response.status || 500,
              headers: { 'content-type': 'application/json' },
            },
          );
        }

        return new Response(response.body, {
          status: response.status,
          headers: {
            'content-type': 'text/event-stream; charset=utf-8',
            'cache-control': 'no-cache',
            connection: 'keep-alive',
          },
        });
      },
    },
  },
});

type ChatMessage = {
  id: string;
  role: 'user' | 'assistant';
  content: string;
};

type ApprovalDialogState = {
  title: string;
  description: string;
  confirmLabel: string;
  confirmVariant?: 'default' | 'destructive';
  action: () => Promise<void>;
};

function OpenClawChatPage() {
  const [messages, setMessages] = useState<Array<ChatMessage>>([]);
  const [input, setInput] = useState('');
  const [status, setStatus] = useState<'idle' | 'streaming'>('idle');
  const [error, setError] = useState<string | null>(null);
  const [sessionKey, setSessionKey] = useState('');
  const [instanceBusy, setInstanceBusy] = useState<'creating' | 'deleting' | null>(null);
  const [instanceError, setInstanceError] = useState<string | null>(null);
  const [instanceOverride, setInstanceOverride] = useState<{
    status: string;
    runtime_name?: string | null;
  } | null>(null);
  const [approvalDialog, setApprovalDialog] = useState<ApprovalDialogState | null>(null);
  const [approvalBusy, setApprovalBusy] = useState(false);
  const [approvalError, setApprovalError] = useState<string | null>(null);
  const abortRef = useRef<AbortController | null>(null);

  const instanceQuery = useQuery(api.openclaw.getInstanceForCurrentUser);
  const createInstance = useAction(api.openclawApi.createInstance);
  const deleteInstance = useAction(api.openclawApi.deleteInstance);
  const instance = instanceQuery ?? instanceOverride;

  const canSend =
    status !== 'streaming' &&
    input.trim().length > 0 &&
    instanceBusy !== 'deleting' &&
    instanceBusy !== 'creating' &&
    !approvalDialog;

  const threadTitle = useMemo(() => {
    if (sessionKey.trim()) return `Session: ${sessionKey.trim()}`;
    return 'Session: user-scoped';
  }, [sessionKey]);

  const appendAssistantDelta = (assistantId: string, delta: string) => {
    setMessages((prev) =>
      prev.map((msg) =>
        msg.id === assistantId
          ? { ...msg, content: `${msg.content}${delta}` }
          : msg,
      ),
    );
  };

  const openApprovalDialog = (config: ApprovalDialogState) => {
    setApprovalError(null);
    setApprovalDialog(config);
  };

  const handleApprovalConfirm = async () => {
    if (!approvalDialog) return;
    setApprovalBusy(true);
    setApprovalError(null);
    try {
      await approvalDialog.action();
      setApprovalDialog(null);
    } catch (err) {
      setApprovalError(err instanceof Error ? err.message : 'Approval failed');
    } finally {
      setApprovalBusy(false);
    }
  };

  const provisionInstance = async (): Promise<boolean> => {
    if (instance?.status === 'ready') return true;
    setInstanceError(null);
    setInstanceBusy('creating');
    try {
      const created = await createInstance();
      setInstanceOverride(created);
      if (created.status === 'ready') {
        return true;
      }
      setInstanceError('OpenClaw is still provisioning. Try again in a moment.');
      return false;
    } catch (err) {
      setInstanceError(err instanceof Error ? err.message : 'Failed to provision OpenClaw');
      return false;
    } finally {
      setInstanceBusy(null);
    }
  };

  const handleDelete = async () => {
    if (!instance) return;
    const confirmFn = typeof globalThis.confirm === 'function' ? globalThis.confirm : null;
    const confirmed = confirmFn
      ? confirmFn(
          'Delete your OpenClaw instance? This stops the gateway, removes the record, and clears secrets. You can re-provision later.',
        )
      : true;
    if (!confirmed) return;
    setInstanceError(null);
    setInstanceBusy('deleting');
    try {
      await deleteInstance();
      setInstanceOverride(null);
      setMessages([]);
      setSessionKey('');
    } catch (err) {
      setInstanceError(err instanceof Error ? err.message : 'Failed to delete OpenClaw');
    } finally {
      setInstanceBusy(null);
    }
  };

  const sendMessage = async (trimmed: string) => {
    setError(null);
    setInput('');

    const userMessage: ChatMessage = {
      id: crypto.randomUUID(),
      role: 'user',
      content: trimmed,
    };
    const assistantId = crypto.randomUUID();
    const assistantMessage: ChatMessage = {
      id: assistantId,
      role: 'assistant',
      content: '',
    };

    setMessages((prev) => [...prev, userMessage, assistantMessage]);
    setStatus('streaming');

    const controller = new AbortController();
    abortRef.current = controller;

    try {
      const response = await fetch('/openclaw/chat', {
        method: 'POST',
        headers: {
          'content-type': 'application/json',
          accept: 'text/event-stream',
        },
        body: JSON.stringify({
          input: trimmed,
          sessionKey: sessionKey.trim() || undefined,
        }),
        signal: controller.signal,
      });

      if (!response.ok || !response.body) {
        const message = await response.text().catch(() => '');
        throw new Error(message || `OpenClaw chat failed (${response.status})`);
      }

      await consumeOpenClawStream(response.body, (delta) => {
        appendAssistantDelta(assistantId, delta);
      });
    } catch (err) {
      if (err instanceof Error && err.name === 'AbortError') {
        setError('Streaming aborted.');
      } else {
        setError(err instanceof Error ? err.message : 'OpenClaw chat failed');
      }
    } finally {
      abortRef.current = null;
      setStatus('idle');
    }
  };

  const handleSend = async () => {
    if (!canSend) return;
    const trimmed = input.trim();
    if (!trimmed) return;

    if (instance?.status !== 'ready') {
      openApprovalDialog({
        title: 'Approve OpenClaw provisioning',
        description:
          'Provisioning creates a managed OpenClaw gateway for your account. This may allocate compute resources and start billing.',
        confirmLabel: 'Provision & Send',
        action: async () => {
          const ready = await provisionInstance();
          if (ready) {
            await sendMessage(trimmed);
          }
        },
      });
      return;
    }

    await sendMessage(trimmed);
  };

  const handleQuickStart = () => {
    if (status === 'streaming' || instanceBusy || approvalDialog) return;
    const message = QUICK_START_MESSAGE;
    if (instance?.status !== 'ready') {
      openApprovalDialog({
        title: 'Approve OpenClaw provisioning',
        description:
          'Provisioning creates a managed OpenClaw gateway for your account. This may allocate compute resources and start billing.',
        confirmLabel: 'Provision & Send',
        action: async () => {
          const ready = await provisionInstance();
          if (ready) {
            await sendMessage(message);
          }
        },
      });
      return;
    }
    void sendMessage(message);
  };

  const handleSubmit = (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    void handleSend();
  };

  const handleAbort = () => {
    abortRef.current?.abort();
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <Dialog
        open={!!approvalDialog}
        onOpenChange={(open) => {
          if (!open) {
            setApprovalDialog(null);
            setApprovalError(null);
          }
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{approvalDialog?.title ?? 'Approve action'}</DialogTitle>
            <DialogDescription>
              {approvalDialog?.description ?? 'Confirm this action to proceed.'}
            </DialogDescription>
          </DialogHeader>
          {approvalError && <p className="text-sm text-red-400">{approvalError}</p>}
          <div className="flex flex-wrap justify-end gap-2">
            <Button
              variant="secondary"
              onClick={() => setApprovalDialog(null)}
              disabled={approvalBusy}
            >
              Cancel
            </Button>
            <Button
              variant={approvalDialog?.confirmVariant ?? 'default'}
              onClick={() => void handleApprovalConfirm()}
              disabled={approvalBusy}
            >
              {approvalBusy ? 'Working…' : approvalDialog?.confirmLabel ?? 'Approve'}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
      <div className="flex items-center justify-between border-b border-border px-4 py-3">
        <div>
          <h1 className="text-lg font-semibold text-foreground">OpenClaw Chat</h1>
          <p className="text-xs text-muted-foreground">{threadTitle}</p>
          <p className="text-xs text-muted-foreground">
            Instance: {instance ? instance.status : 'not provisioned'}
            {instance?.runtime_name ? ` · ${instance.runtime_name}` : ''}
          </p>
        </div>
        <div className="flex items-center gap-2">
          {!instance ? (
            <Button
              size="sm"
              onClick={() =>
                openApprovalDialog({
                  title: 'Approve OpenClaw provisioning',
                  description:
                    'Provisioning creates a managed OpenClaw gateway for your account. This may allocate compute resources and start billing.',
                  confirmLabel: 'Provision OpenClaw',
                  action: async () => {
                    await provisionInstance();
                  },
                })
              }
              disabled={instanceBusy === 'creating' || !!approvalDialog}
            >
              {instanceBusy === 'creating' ? 'Provisioning…' : 'Create OpenClaw'}
            </Button>
          ) : (
            <Button
              size="sm"
              variant="destructive"
              onClick={handleDelete}
              disabled={instanceBusy === 'deleting' || status === 'streaming'}
            >
              {instanceBusy === 'deleting' ? 'Deleting…' : 'Delete OpenClaw'}
            </Button>
          )}
          {status === 'streaming' ? (
            <Button size="sm" variant="secondary" onClick={handleAbort}>
              Stop
            </Button>
          ) : null}
        </div>
      </div>

      <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-4 py-4">
        {instanceError ? (
          <div className="rounded-lg border border-red-400/30 bg-red-500/5 p-4 text-sm text-red-400">
            {instanceError}
          </div>
        ) : null}
        {messages.length === 0 ? (
          <div className="rounded-lg border border-dashed border-border p-4 text-sm text-muted-foreground">
            <p className="text-sm text-foreground">Need help getting started?</p>
            <p className="mt-1 text-sm text-muted-foreground">
              Choose a local or managed OpenClaw path. Managed instances provision automatically on
              first send.
            </p>
            <div className="mt-3 flex flex-wrap gap-2">
              <Button
                size="sm"
                onClick={handleQuickStart}
                disabled={status === 'streaming' || instanceBusy !== null || !!approvalDialog}
              >
                {instance?.status === 'ready' ? 'Send intro message' : 'Provision & send intro'}
              </Button>
              <p className="text-xs text-muted-foreground">
                One click provisions (if needed) and sends the first message.
              </p>
            </div>
            <OpenClawSetupCards className="mt-4" showChatCta={false} />
            <p className="mt-4 text-xs text-muted-foreground">
              Messages are stored in OpenClaw sessions (user-scoped by default).
            </p>
          </div>
        ) : null}

        {messages.map((message) => (
          <div
            key={message.id}
            className={cn(
              'max-w-2xl rounded-lg border px-4 py-3 text-sm leading-relaxed shadow-sm',
              message.role === 'user'
                ? 'ml-auto border-primary/30 bg-primary/5 text-foreground'
                : 'border-border bg-card text-foreground',
            )}
          >
            <div className="mb-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              {message.role === 'user' ? 'You' : 'OpenClaw'}
            </div>
            <div className="whitespace-pre-wrap">
              {message.content ||
                (message.role === 'assistant' && status === 'streaming'
                  ? '…'
                  : '')}
            </div>
          </div>
        ))}
      </div>

      <form
        onSubmit={handleSubmit}
        className="border-t border-border bg-background px-4 py-4"
      >
        <div className="flex flex-col gap-3">
          <div className="grid gap-2">
            <label className="text-xs font-semibold text-muted-foreground">
              Optional session key
            </label>
            <input
              value={sessionKey}
              onChange={(event) => setSessionKey(event.target.value)}
              placeholder="main"
              className="h-9 rounded-md border border-input bg-background px-3 text-sm shadow-sm outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring/40"
            />
          </div>
          <Textarea
            value={input}
            onChange={(event) => setInput(event.target.value)}
            rows={3}
            placeholder="Send a message to your OpenClaw gateway..."
            className="min-h-[96px]"
          />
          <div className="flex items-center justify-between">
            <p className="text-xs text-muted-foreground">
              Streaming via OpenResponses (`/api/openclaw/chat`).
            </p>
            <Button type="submit" disabled={!canSend}>
              Send
            </Button>
          </div>
          {error ? <p className="text-xs text-red-400">{error}</p> : null}
        </div>
      </form>
    </div>
  );
}
