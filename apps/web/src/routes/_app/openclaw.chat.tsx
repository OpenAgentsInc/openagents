import { useMemo, useRef, useState } from 'react';
import { createFileRoute } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { useAction, useQuery } from 'convex/react';
import { api } from '../../../convex/_generated/api';
import { OpenClawSetupCards } from '@/components/openclaw/openclaw-setup-cards';
import { Button } from '@/components/ui/button';
import { Textarea } from '@/components/ui/textarea';
import { cn } from '@/lib/utils';
import { resolveApiBase, resolveInternalKey } from '@/lib/openclawApi';
import { consumeOpenClawStream } from '@/lib/openclawStream';

const DEFAULT_AGENT_ID = 'main';

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
        const userId = auth?.user?.id;
        if (!userId) {
          return new Response(JSON.stringify({ ok: false, error: 'not authenticated' }), {
            status: 401,
            headers: { 'content-type': 'application/json' },
          });
        }

        let internalKey = '';
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

        const sessionKey = typeof body?.sessionKey === 'string' ? body.sessionKey.trim() : '';
        const agentId = typeof body?.agentId === 'string' ? body.agentId.trim() : '';

        const payload = {
          model: `openclaw:${agentId || DEFAULT_AGENT_ID}`,
          input,
          stream: true,
          user: userId,
        };

        const headers = new Headers();
        headers.set('content-type', 'application/json');
        headers.set('accept', 'text/event-stream');
        headers.set('X-OA-Internal-Key', internalKey);
        headers.set('X-OA-User-Id', userId);
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
  const abortRef = useRef<AbortController | null>(null);

  const instanceQuery = useQuery(api.openclaw.getInstanceForCurrentUser);
  const createInstance = useAction(api.openclawApi.createInstance);
  const deleteInstance = useAction(api.openclawApi.deleteInstance);
  const instance = instanceQuery ?? instanceOverride;

  const canSend =
    status !== 'streaming' &&
    input.trim().length > 0 &&
    instanceBusy !== 'deleting';

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

  const ensureInstanceReady = async (): Promise<boolean> => {
    if (instance?.status === 'ready') return true;
    setInstanceError(null);
    setInstanceBusy('creating');
    try {
      const created = await createInstance();
      setInstanceOverride(created ?? null);
      if (created?.status === 'ready') {
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
    const confirmed = globalThis.confirm?.(
      'Delete your OpenClaw instance? This stops the gateway, removes the record, and clears secrets. You can re-provision later.',
    );
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

  const handleSend = async () => {
    if (!canSend) return;
    setError(null);

    const ready = await ensureInstanceReady();
    if (!ready) return;

    const trimmed = input.trim();
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

  const handleSubmit = (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    void handleSend();
  };

  const handleAbort = () => {
    abortRef.current?.abort();
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
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
              onClick={() => void ensureInstanceReady()}
              disabled={instanceBusy === 'creating'}
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
