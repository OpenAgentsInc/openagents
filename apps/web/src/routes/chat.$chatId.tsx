import { createFileRoute, redirect } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { useAgent } from 'agents/react';
import { useAgentChat } from '@cloudflare/ai-chat/react';
import { Effect } from 'effect';
import { useMemo, useRef, useState } from 'react';
import { Streamdown } from 'streamdown';
import { TelemetryService } from '../effect/telemetry';
import type { UIMessage } from 'ai';
import type { FormEvent } from 'react';

export const Route = createFileRoute('/chat/$chatId')({
  loader: async ({ context, params }) => {
    const result = await context.effectRuntime.runPromise(
      Effect.gen(function* () {
        const telemetry = yield* TelemetryService;

        const auth = yield* Effect.tryPromise({
          try: () => getAuth(),
          catch: (err) => err,
        }).pipe(Effect.catchAll(() => Effect.succeed(null)));

        const userId = auth?.user?.id ?? null;
        if (!userId) {
          yield* telemetry.withNamespace('route.chat').event('chat.unauth');
          return { kind: 'redirect' as const, to: '/' as const };
        }

        if (params.chatId !== userId) {
          yield* telemetry.withNamespace('route.chat').event('chat.mismatch', {
            userId,
            chatId: params.chatId,
          });
          return { kind: 'redirect' as const, to: '/assistant' as const };
        }

        yield* telemetry.withNamespace('route.chat').event('chat.open', {
          userId,
          chatId: params.chatId,
        });

        return { kind: 'ok' as const, userId };
      }),
    );

    if (result.kind === 'redirect') {
      throw redirect({ to: result.to });
    }

    return { userId: result.userId };
  },
  component: ChatPage,
});

function ChatPage() {
  const { chatId } = Route.useParams();

  const agent = useAgent({
    agent: 'chat',
    name: chatId,
  });

  const chat = useAgentChat({
    agent,
    resume: true,
    // SSR: `useAgent` (PartySocket) uses a dummy host when `window` is undefined.
    // Avoid fetching `/get-messages` against that host during SSR; the client will
    // fetch messages from the real origin after hydration.
    ...(typeof window === 'undefined' ? { getInitialMessages: null } : {}),
  });
  const { clearHistory } = chat;

  const [input, setInput] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);
  const [isExportingBlueprint, setIsExportingBlueprint] = useState(false);
  const isStreaming = chat.status === 'streaming';
  const isBusy = chat.status === 'submitted' || chat.status === 'streaming';

  const messages = chat.messages as ReadonlyArray<UIMessage>;

  const rendered = useMemo(() => {
    return messages
      .map((msg) => {
      const parts: ReadonlyArray<unknown> = Array.isArray((msg as any).parts) ? (msg as any).parts : [];

      const text = parts
        .filter(
          (p): p is { type: 'text'; text: string } =>
            Boolean(p) &&
            typeof p === 'object' &&
            (p as any).type === 'text' &&
            typeof (p as any).text === 'string',
        )
        .map((p) => p.text)
        .join('');

      return {
        id: msg.id,
        role: msg.role,
        text,
      };
      })
      .filter((m) => m.role === 'user' || m.text.trim().length > 0);
  }, [messages]);

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    const text = input.trim();
    if (!text || isBusy) return;

    setInput('');
    void chat.sendMessage({ text }).catch(() => {
      // Best effort: restore input if send fails.
      setInput(text);
    });
    // Keep cursor in input after send (next tick so React has committed)
    setTimeout(() => inputRef.current?.focus(), 0);
  };

  const onExportBlueprint = async () => {
    if (isExportingBlueprint) return;
    setIsExportingBlueprint(true);
    try {
      const response = await fetch(`/agents/chat/${chatId}/blueprint`);
      if (!response.ok) {
        throw new Error(`Export failed (${response.status})`);
      }
      const blueprint: unknown = await response.json();

      const blob = new Blob([JSON.stringify(blueprint, null, 2)], {
        type: 'application/json',
      });
      const url = URL.createObjectURL(blob);
      const link = document.createElement('a');
      link.href = url;
      link.download = `autopilot-blueprint-${chatId}.json`;
      document.body.appendChild(link);
      link.click();
      link.remove();
      URL.revokeObjectURL(url);
    } catch (err) {
      console.error(err);
      window.alert('Failed to export Blueprint JSON.');
    } finally {
      setIsExportingBlueprint(false);
    }
  };

  return (
    <div className="fixed inset-0 flex flex-col h-screen overflow-hidden bg-bg-primary text-text-primary font-mono">
      {/* Header - overseer-style */}
      <header className="flex items-center h-12 px-4 gap-3 border-b border-border-dark bg-bg-secondary shrink-0">
        <span className="text-accent font-mono font-bold text-base tracking-[0.12em] leading-none uppercase">
          OpenAgents
        </span>
        <div className="h-6 w-px bg-border-dark/70" aria-hidden="true" />
        <span className="text-xs text-text-dim uppercase tracking-wider">Autopilot</span>
      </header>

      {/* Main chat area */}
      <main className="flex-1 flex flex-col min-h-0 mx-auto w-full max-w-4xl p-4">
        <div className="flex-1 flex flex-col overflow-hidden rounded-lg border border-border-dark bg-surface-primary">
          <div className="flex-1 overflow-y-auto p-4 overseer-scroll">
            <div className="flex flex-col gap-3">
              {rendered.map((m) => (
                <div
                  key={m.id}
                  className={[
                    'max-w-[90%] rounded border px-3 py-2 text-sm leading-relaxed font-mono',
                    m.role === 'user'
                      ? 'self-end bg-accent-subtle text-text-primary border-accent-muted'
                      : 'self-start bg-surface-secondary text-text-primary border-border-dark',
                  ].join(' ')}
                >
                  {m.text ? (
                    m.role === 'user' ? (
                      <div className="whitespace-pre-wrap">{m.text}</div>
                    ) : (
                      <Streamdown mode={isStreaming ? 'streaming' : 'static'} isAnimating={isStreaming}>
                        {m.text}
                      </Streamdown>
                    )
                  ) : null}
                </div>
              ))}
            </div>
          </div>

          <form
            onSubmit={onSubmit}
            className="flex items-center gap-2 border-t border-border-dark p-3 bg-bg-secondary"
          >
            <input
              ref={inputRef}
              autoFocus
              value={input}
              onChange={(e) => setInput(e.target.value)}
              placeholder="Message Autopilot…"
              className="h-9 flex-1 rounded border border-border-dark bg-surface-primary px-3 text-sm text-text-primary placeholder:text-text-dim outline-none focus:border-border-focus focus:ring-1 focus:ring-border-focus font-mono"
            />
            {isBusy ? (
              <button
                type="button"
                onClick={() => void chat.stop()}
                className="inline-flex h-9 items-center justify-center rounded px-3 text-sm font-medium bg-surface-primary text-text-primary border border-border-dark hover:bg-surface-secondary hover:border-border-hover focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus font-mono"
              >
                Stop
              </button>
            ) : (
              <button
                type="submit"
                className="inline-flex h-9 items-center justify-center rounded px-3 text-sm font-medium bg-accent text-bg-primary border border-accent hover:bg-accent-muted hover:border-accent-muted focus:outline-none focus-visible:ring-2 focus-visible:ring-accent font-mono"
              >
                Send
              </button>
            )}
          </form>
        </div>
      </main>

      {/* Control panel - bottom right */}
      <div className="absolute bottom-4 right-4 flex flex-col gap-1 text-right">
        <button
          type="button"
          onClick={() => void onExportBlueprint()}
          disabled={isExportingBlueprint}
          className="text-xs font-mono text-text-muted hover:text-text-primary disabled:opacity-50 disabled:cursor-not-allowed focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus focus-visible:ring-offset-2 focus-visible:ring-offset-bg-primary rounded px-2 py-1"
        >
          {isExportingBlueprint ? 'Exporting…' : 'Export Blueprint JSON'}
        </button>
        <button
          type="button"
          onClick={() => clearHistory()}
          disabled={isBusy}
          className="text-xs font-mono text-text-muted hover:text-text-primary disabled:opacity-50 disabled:cursor-not-allowed focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus focus-visible:ring-offset-2 focus-visible:ring-offset-bg-primary rounded px-2 py-1"
        >
          Delete all messages
        </button>
      </div>
    </div>
  );
}

// (Tool parts are intentionally not rendered in the MVP UI.)
