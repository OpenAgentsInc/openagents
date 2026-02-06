import { createFileRoute, redirect } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { useAgent } from 'agents/react';
import { useAgentChat } from '@cloudflare/ai-chat/react';
import { Effect } from 'effect';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Streamdown } from 'streamdown';
import { DotsGridBackground, whitePreset } from '@openagentsinc/hud/react';
import { KranoxFrame } from '../components/KranoxFrame';
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

function sanitizeBlueprintForDisplay(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(sanitizeBlueprintForDisplay);
  }

  if (value && typeof value === 'object') {
    const record = value as Record<string, unknown>;
    const out: Record<string, unknown> = {};
    for (const [key, child] of Object.entries(record)) {
      // UI: avoid the word "address" (this is a handle/nickname field).
      const safeKey = key === 'addressAs' ? 'handle' : key;
      out[safeKey] = sanitizeBlueprintForDisplay(child);
    }
    return out;
  }

  return value;
}

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
  const didMountRef = useRef(false);
  const [isExportingBlueprint, setIsExportingBlueprint] = useState(false);
  const [isResettingAgent, setIsResettingAgent] = useState(false);
  const [blueprint, setBlueprint] = useState<unknown | null>(null);
  const [blueprintError, setBlueprintError] = useState<string | null>(null);
  const [blueprintLoading, setBlueprintLoading] = useState(false);
  const [blueprintUpdatedAt, setBlueprintUpdatedAt] = useState<number | null>(null);
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

  const fetchBlueprint = useCallback(
    async (options?: { silent?: boolean }) => {
      const silent = options?.silent ?? false;
      if (!silent) setBlueprintLoading(true);
      setBlueprintError(null);
      try {
        const response = await fetch(`/agents/chat/${chatId}/blueprint`, {
          cache: 'no-store',
        });
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`);
        }
        const json: unknown = await response.json();
        setBlueprint(json);
        setBlueprintUpdatedAt(Date.now());
      } catch (err) {
        console.error(err);
        setBlueprintError(err instanceof Error ? err.message : 'Failed to load Blueprint.');
      } finally {
        if (!silent) setBlueprintLoading(false);
      }
    },
    [chatId],
  );

  useEffect(() => {
    void fetchBlueprint();
  }, [fetchBlueprint]);

  useEffect(() => {
    if (!isBusy) return;
    const interval = window.setInterval(() => {
      void fetchBlueprint({ silent: true });
    }, 1000);
    return () => window.clearInterval(interval);
  }, [fetchBlueprint, isBusy]);

  useEffect(() => {
    if (!didMountRef.current) {
      didMountRef.current = true;
      return;
    }
    if (!isBusy) {
      void fetchBlueprint({ silent: true });
    }
  }, [fetchBlueprint, isBusy]);

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
      const blueprintJson: unknown = await response.json();

      const blob = new Blob([JSON.stringify(blueprintJson, null, 2)], {
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

  const onResetAgent = async () => {
    if (isResettingAgent || isBusy) return;
    const confirmed = window.confirm(
      'Reset agent?\n\nThis will clear messages and reset your Blueprint to defaults.',
    );
    if (!confirmed) return;

    setIsResettingAgent(true);
    setBlueprintError(null);
    try {
      const response = await fetch(`/agents/chat/${chatId}/reset-agent`, {
        method: 'POST',
      });
      if (!response.ok) {
        throw new Error(`Reset failed (HTTP ${response.status})`);
      }
      setInput('');
      clearHistory();
      await fetchBlueprint();
    } catch (err) {
      console.error(err);
      window.alert('Failed to reset agent.');
    } finally {
      setIsResettingAgent(false);
    }
  };

  const blueprintText = useMemo(() => {
    if (!blueprint) return null;
    try {
      return JSON.stringify(sanitizeBlueprintForDisplay(blueprint), null, 2);
    } catch {
      return null;
    }
  }, [blueprint]);

  return (
    <div className="fixed inset-0 overflow-hidden text-text-primary font-mono">
      {/* Arwes-style ambient background (HUD). */}
      <div
        className="absolute inset-0"
        style={{
          backgroundColor: whitePreset.backgroundColor,
          backgroundImage: [
            // Soft top glow + vignette to frame the UI.
            `radial-gradient(120% 85% at 50% 0%, rgba(255,255,255,0.08) 0%, rgba(255,255,255,0) 55%)`,
            `radial-gradient(ellipse 100% 100% at 50% 50%, transparent 12%, rgba(0,0,0,0.55) 60%, rgba(0,0,0,0.88) 100%)`,
            whitePreset.backgroundImage,
          ].join(', '),
        }}
      >
        <DotsGridBackground
          distance={whitePreset.distance}
          dotsColor="hsla(0, 0%, 100%, 0.035)"
          lineColor="hsla(0, 0%, 100%, 0.03)"
          dotsSettings={{ type: 'circle', size: 2 }}
        />
      </div>

      <div className="relative z-10 flex flex-col h-screen overflow-hidden">
      {/* Header - overseer-style */}
      <header className="flex items-center h-12 px-4 gap-3 border-b border-border-dark bg-bg-secondary shrink-0 shadow-[0_1px_0_rgba(255,255,255,0.06)]">
        <span className="text-accent font-mono font-bold text-base tracking-[0.12em] leading-none uppercase drop-shadow-[0_0_16px_rgba(255,255,255,0.18)]">
          OpenAgents
        </span>
        <div className="h-6 w-px bg-border-dark/70" aria-hidden="true" />
        <span className="text-xs text-text-dim uppercase tracking-wider">Autopilot</span>
      </header>

      {/* Main area */}
      <main className="flex-1 min-h-0 w-full flex overflow-hidden">
        {/* Blueprint sidebar */}
        <aside className="hidden lg:flex lg:w-[360px] shrink-0 border-r border-border-dark bg-bg-secondary shadow-[0_0_0_1px_rgba(255,255,255,0.04)]">
          <div className="flex flex-col h-full min-h-0 w-full">
            <div className="flex items-center justify-between h-11 px-3 border-b border-border-dark">
              <div className="text-xs text-text-dim uppercase tracking-wider">Blueprint</div>
              <div className="flex items-center gap-2">
                {blueprintUpdatedAt ? (
                  <div className="text-[10px] text-text-dim">
                    {new Date(blueprintUpdatedAt).toLocaleTimeString()}
                  </div>
                ) : null}
                <button
                  type="button"
                  onClick={() => void fetchBlueprint()}
                  disabled={blueprintLoading}
                  className="text-[10px] font-mono text-text-muted hover:text-text-primary disabled:opacity-50 disabled:cursor-not-allowed focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus rounded px-2 py-1"
                >
                  {blueprintLoading ? 'Syncing…' : 'Refresh'}
                </button>
              </div>
            </div>
            <div className="flex-1 min-h-0 overflow-y-auto p-3 overseer-scroll">
              {blueprintError ? (
                <div className="text-xs text-red-400">Blueprint error: {blueprintError}</div>
              ) : blueprintText ? (
                <pre className="text-[11px] leading-4 whitespace-pre-wrap break-words text-text-primary">
                  {blueprintText}
                </pre>
              ) : (
                <div className="text-xs text-text-dim">(no blueprint)</div>
              )}
            </div>
          </div>
        </aside>

        {/* Chat */}
        <section className="flex-1 min-h-0 flex flex-col p-4">
          <div className="flex-1 flex flex-col min-h-0 mx-auto w-full max-w-4xl">
            <KranoxFrame className="flex-1 min-h-0">
              <div className="flex h-full min-h-0 flex-col px-4 py-4 sm:px-6 lg:px-8">
                <div className="flex-1 overflow-y-auto overseer-scroll pr-1">
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
                            <Streamdown
                              mode={isStreaming ? 'streaming' : 'static'}
                              isAnimating={isStreaming}
                            >
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
                  className="mt-3 flex items-center gap-2 rounded border border-border-dark bg-bg-secondary p-3 shadow-[0_0_0_1px_rgba(255,255,255,0.04)_inset]"
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
            </KranoxFrame>
          </div>
        </section>
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
          Clear messages
        </button>
        <button
          type="button"
          onClick={() => void onResetAgent()}
          disabled={isBusy || isResettingAgent}
          className="text-xs font-mono text-text-muted hover:text-text-primary disabled:opacity-50 disabled:cursor-not-allowed focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus focus-visible:ring-offset-2 focus-visible:ring-offset-bg-primary rounded px-2 py-1"
        >
          {isResettingAgent ? 'Resetting...' : 'Reset agent'}
        </button>
      </div>
      </div>
    </div>
  );
}

// (Tool parts are intentionally not rendered in the MVP UI.)
