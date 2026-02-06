import { createFileRoute, redirect, useRouter } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { useAgent } from 'agents/react';
import { useAgentChat } from '@cloudflare/ai-chat/react';
import { Effect } from 'effect';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Streamdown } from 'streamdown';
import { DotsGridBackground, whitePreset } from '@openagentsinc/hud/react';
import { KranoxFrame } from '../components/KranoxFrame';
import { TelemetryService } from '../effect/telemetry';
import { AgentApiService } from '../effect/agentApi';
import type { UIMessage } from 'ai';
import type { FormEvent } from 'react';

export const Route = createFileRoute('/autopilot')({
  loader: async ({ context }) => {
    const result = await context.effectRuntime.runPromise(
      Effect.gen(function* () {
        const telemetry = yield* TelemetryService;

        const auth = yield* Effect.tryPromise({
          try: () => getAuth(),
          catch: (err) => err,
        }).pipe(Effect.catchAll(() => Effect.succeed(null)));

        const userId = auth?.user?.id ?? null;
        if (!userId) {
          yield* telemetry.withNamespace('route.autopilot').event('autopilot.unauth');
          return { kind: 'redirect' as const, to: '/' as const };
        }

        yield* telemetry.withNamespace('route.autopilot').event('autopilot.open', {
          userId,
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
  const { userId } = Route.useLoaderData();
  const chatId = userId;
  const router = useRouter();
  const runtime = router.options.context.effectRuntime;

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
  const [isEditingBlueprint, setIsEditingBlueprint] = useState(false);
  const [blueprintDraft, setBlueprintDraft] = useState<{
    userHandle: string;
    agentName: string;
    identityVibe: string;
    soulVibe: string;
    soulBoundaries: string;
  } | null>(null);
  const [isSavingBlueprint, setIsSavingBlueprint] = useState(false);
  const [isAtBottom, setIsAtBottom] = useState(true);
  const isStreaming = chat.status === 'streaming';
  const isBusy = chat.status === 'submitted' || chat.status === 'streaming';

  const messages = chat.messages as ReadonlyArray<UIMessage>;
  const scrollRef = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);

  const recomputeIsAtBottom = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;
    const thresholdPx = 96;
    const distanceFromBottom = el.scrollHeight - (el.scrollTop + el.clientHeight);
    setIsAtBottom(distanceFromBottom <= thresholdPx);
  }, []);

  const scrollToBottom = useCallback((behavior: ScrollBehavior = 'auto') => {
    bottomRef.current?.scrollIntoView({ block: 'end', behavior });
  }, []);

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

  const renderedTailKey = useMemo(() => {
    const last = rendered.at(-1);
    if (!last) return '';
    return `${last.id}:${last.text.length}`;
  }, [rendered]);

  const makeDraftFromBlueprint = useCallback((value: unknown) => {
    const b: any = value ?? {};
    const docs = b?.docs ?? {};
    const identity = docs.identity ?? {};
    const user = docs.user ?? {};
    const soul = docs.soul ?? {};

    const boundaries: string = Array.isArray(soul.boundaries)
      ? soul.boundaries
          .map((s: unknown) => (typeof s === 'string' ? s : ''))
          .filter(Boolean)
          .join('\n')
      : '';

    return {
      userHandle: typeof user.addressAs === 'string' ? user.addressAs : '',
      agentName: typeof identity.name === 'string' ? identity.name : '',
      identityVibe: typeof identity.vibe === 'string' ? identity.vibe : '',
      soulVibe: typeof soul.vibe === 'string' ? soul.vibe : '',
      soulBoundaries: boundaries,
    };
  }, []);

  const fetchBlueprint = useCallback(
    async (options?: { silent?: boolean }) => {
      const silent = options?.silent ?? false;
      if (!silent) setBlueprintLoading(true);
      setBlueprintError(null);
      await runtime
        .runPromise(
          Effect.gen(function* () {
            const api = yield* AgentApiService;
            const json = yield* api.getBlueprint(chatId);
            yield* Effect.sync(() => {
              setBlueprint(json);
              setBlueprintUpdatedAt(Date.now());
              if (!isEditingBlueprint) {
                setBlueprintDraft(makeDraftFromBlueprint(json));
              }
            });
          }).pipe(
            Effect.catchAll((err) =>
              Effect.gen(function* () {
                const telemetry = yield* TelemetryService;
                yield* telemetry
                  .withNamespace('ui.blueprint')
                  .log('error', 'blueprint.fetch_failed', {
                    message: err instanceof Error ? err.message : String(err),
                  });
                yield* Effect.sync(() => {
                  setBlueprintError(
                    err instanceof Error ? err.message : 'Failed to load Blueprint.',
                  );
                });
              }),
            ),
            Effect.ensuring(
              Effect.sync(() => {
                if (!silent) setBlueprintLoading(false);
              }),
            ),
          ),
        )
        .catch(() => {
          // Any defects were already handled best-effort above.
        });
    },
    [chatId, runtime, isEditingBlueprint, makeDraftFromBlueprint],
  );

  const fetchChatMessages = useCallback(async (): Promise<Array<UIMessage>> => {
    return runtime
      .runPromise(
        Effect.gen(function* () {
          const api = yield* AgentApiService;
          return yield* api.getMessages(chatId);
        }),
      )
      .catch(() => []);
  }, [chatId, runtime]);

  useEffect(() => {
    void fetchBlueprint();
  }, [fetchBlueprint]);

  useEffect(() => {
    recomputeIsAtBottom();
  }, [recomputeIsAtBottom, renderedTailKey]);

  useEffect(() => {
    if (!isAtBottom) return;
    if (rendered.length === 0) return;
    scrollToBottom(isStreaming ? 'auto' : 'smooth');
  }, [isAtBottom, isStreaming, rendered.length, renderedTailKey, scrollToBottom]);

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
    setTimeout(() => scrollToBottom('auto'), 0);
  };

  const onExportBlueprint = async () => {
    if (isExportingBlueprint) return;
    setIsExportingBlueprint(true);
    try {
      const blueprintJson = await runtime.runPromise(
        Effect.gen(function* () {
          const api = yield* AgentApiService;
          return yield* api.getBlueprint(chatId);
        }),
      );

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
      await runtime
        .runPromise(
          Effect.gen(function* () {
            const telemetry = yield* TelemetryService;
            yield* telemetry.withNamespace('ui.blueprint').log('error', 'blueprint.export_failed', {
              message: err instanceof Error ? err.message : String(err),
            });
          }),
        )
        .catch(() => {});
      window.alert('Failed to export Blueprint JSON.');
    } finally {
      setIsExportingBlueprint(false);
    }
  };

  const onStartEditBlueprint = () => {
    if (!blueprint) return;
    setBlueprintDraft(makeDraftFromBlueprint(blueprint));
    setIsEditingBlueprint(true);
  };

  const onCancelEditBlueprint = () => {
    setBlueprintDraft(blueprint ? makeDraftFromBlueprint(blueprint) : null);
    setIsEditingBlueprint(false);
  };

  const onSaveBlueprint = async () => {
    if (!blueprintDraft || !blueprint || isSavingBlueprint) return;
    setIsSavingBlueprint(true);
    setBlueprintError(null);

    const nowIso = new Date().toISOString();
    const nextBoundaries = blueprintDraft.soulBoundaries
      .split('\n')
      .map((s) => s.trim())
      .filter(Boolean);

    // Clone the export object so we can safely mutate and re-import it.
    const next: any = JSON.parse(JSON.stringify(blueprint));
    next.exportedAt = nowIso;

    // USER (handle only)
    next.docs.user.addressAs = blueprintDraft.userHandle.trim() || next.docs.user.addressAs;
    next.docs.user.updatedAt = nowIso;
    next.docs.user.updatedBy = 'user';
    next.docs.user.version = Number(next.docs.user.version ?? 1) + 1;

    // IDENTITY
    next.docs.identity.name = blueprintDraft.agentName.trim() || next.docs.identity.name;
    next.docs.identity.vibe = blueprintDraft.identityVibe.trim() || next.docs.identity.vibe;
    next.docs.identity.updatedAt = nowIso;
    next.docs.identity.updatedBy = 'user';
    next.docs.identity.version = Number(next.docs.identity.version ?? 1) + 1;

    // SOUL
    next.docs.soul.vibe = blueprintDraft.soulVibe.trim() || next.docs.soul.vibe;
    next.docs.soul.boundaries = nextBoundaries;
    next.docs.soul.updatedAt = nowIso;
    next.docs.soul.updatedBy = 'user';
    next.docs.soul.version = Number(next.docs.soul.version ?? 1) + 1;

    const saved = await runtime
      .runPromise(
        Effect.gen(function* () {
          const telemetry = yield* TelemetryService;
          const api = yield* AgentApiService;
          yield* telemetry.withNamespace('ui.blueprint').event('blueprint.save', {
            changed: [
              'user.handle',
              'identity.name',
              'identity.vibe',
              'soul.vibe',
              'soul.boundaries',
            ],
          });
          yield* api.importBlueprint(chatId, next);
          return true as const;
        }).pipe(
          Effect.catchAll((err) =>
            Effect.gen(function* () {
              const telemetry = yield* TelemetryService;
              yield* telemetry.withNamespace('ui.blueprint').log('error', 'blueprint.save_failed', {
                message: err instanceof Error ? err.message : String(err),
              });
              yield* Effect.sync(() => {
                setBlueprintError(err instanceof Error ? err.message : 'Failed to save Blueprint.');
              });
              return false as const;
            }),
          ),
        ),
      )
      .catch(() => false);

    setIsSavingBlueprint(false);

    if (saved) {
      setIsEditingBlueprint(false);
      await fetchBlueprint();
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
      await runtime.runPromise(
        Effect.gen(function* () {
          const api = yield* AgentApiService;
          yield* api.resetAgent(chatId);
        }),
      );
      setInput('');
      await fetchBlueprint();
      const nextMessages = await fetchChatMessages();
      chat.setMessages(nextMessages);
    } catch (err) {
      await runtime
        .runPromise(
          Effect.gen(function* () {
            const telemetry = yield* TelemetryService;
            yield* telemetry.withNamespace('ui.chat').log('error', 'agent.reset_failed', {
              message: err instanceof Error ? err.message : String(err),
            });
          }),
        )
        .catch(() => {});
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
	                  onClick={() => (isEditingBlueprint ? onCancelEditBlueprint() : onStartEditBlueprint())}
	                  disabled={blueprintLoading || !blueprint}
	                  className="text-[10px] font-mono text-text-muted hover:text-text-primary disabled:opacity-50 disabled:cursor-not-allowed focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus rounded px-2 py-1"
	                >
	                  {isEditingBlueprint ? 'Cancel' : 'Edit'}
	                </button>
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
	              ) : isEditingBlueprint ? (
	                <div className="flex flex-col gap-3">
	                  <div className="text-[11px] text-text-dim">
	                    Edit the Blueprint fields below. (Avoid personal info; handle/nickname only.)
	                  </div>

	                  <label className="flex flex-col gap-1">
	                    <span className="text-[10px] text-text-dim uppercase tracking-wider">
	                      Your handle
	                    </span>
	                    <input
	                      value={blueprintDraft?.userHandle ?? ''}
	                      onChange={(e) =>
	                        setBlueprintDraft((d) => (d ? { ...d, userHandle: e.target.value } : d))
	                      }
	                      className="h-8 w-full rounded border border-border-dark bg-surface-primary px-2 text-[12px] text-text-primary outline-none focus:border-border-focus focus:ring-1 focus:ring-border-focus font-mono"
	                    />
	                  </label>

	                  <label className="flex flex-col gap-1">
	                    <span className="text-[10px] text-text-dim uppercase tracking-wider">
	                      Agent name
	                    </span>
	                    <input
	                      value={blueprintDraft?.agentName ?? ''}
	                      onChange={(e) =>
	                        setBlueprintDraft((d) => (d ? { ...d, agentName: e.target.value } : d))
	                      }
	                      className="h-8 w-full rounded border border-border-dark bg-surface-primary px-2 text-[12px] text-text-primary outline-none focus:border-border-focus focus:ring-1 focus:ring-border-focus font-mono"
	                    />
	                  </label>

	                  <label className="flex flex-col gap-1">
	                    <span className="text-[10px] text-text-dim uppercase tracking-wider">
	                      Agent vibe
	                    </span>
	                    <input
	                      value={blueprintDraft?.identityVibe ?? ''}
	                      onChange={(e) =>
	                        setBlueprintDraft((d) => (d ? { ...d, identityVibe: e.target.value } : d))
	                      }
	                      className="h-8 w-full rounded border border-border-dark bg-surface-primary px-2 text-[12px] text-text-primary outline-none focus:border-border-focus focus:ring-1 focus:ring-border-focus font-mono"
	                    />
	                  </label>

	                  <label className="flex flex-col gap-1">
	                    <span className="text-[10px] text-text-dim uppercase tracking-wider">
	                      Soul vibe
	                    </span>
	                    <input
	                      value={blueprintDraft?.soulVibe ?? ''}
	                      onChange={(e) =>
	                        setBlueprintDraft((d) => (d ? { ...d, soulVibe: e.target.value } : d))
	                      }
	                      className="h-8 w-full rounded border border-border-dark bg-surface-primary px-2 text-[12px] text-text-primary outline-none focus:border-border-focus focus:ring-1 focus:ring-border-focus font-mono"
	                    />
	                  </label>

	                  <label className="flex flex-col gap-1">
	                    <span className="text-[10px] text-text-dim uppercase tracking-wider">
	                      Boundaries (one per line)
	                    </span>
	                    <textarea
	                      value={blueprintDraft?.soulBoundaries ?? ''}
	                      onChange={(e) =>
	                        setBlueprintDraft((d) => (d ? { ...d, soulBoundaries: e.target.value } : d))
	                      }
	                      rows={8}
	                      className="w-full resize-y rounded border border-border-dark bg-surface-primary px-2 py-2 text-[12px] leading-4 text-text-primary outline-none focus:border-border-focus focus:ring-1 focus:ring-border-focus font-mono"
	                    />
	                  </label>

	                  <button
	                    type="button"
	                    onClick={() => void onSaveBlueprint()}
	                    disabled={isSavingBlueprint}
	                    className="inline-flex h-9 items-center justify-center rounded px-3 text-xs font-medium bg-accent text-bg-primary border border-accent hover:bg-accent-muted hover:border-accent-muted disabled:opacity-50 disabled:cursor-not-allowed focus:outline-none focus-visible:ring-2 focus-visible:ring-accent font-mono"
	                  >
	                    {isSavingBlueprint ? 'Saving…' : 'Save Blueprint'}
	                  </button>
	                </div>
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
	                <div
	                  ref={scrollRef}
	                  onScroll={recomputeIsAtBottom}
	                  className="flex-1 overflow-y-auto overseer-scroll pr-1 scroll-smooth"
	                >
	                  <div className="flex flex-col gap-3">
	                    {rendered.map((m) => (
	                      <div
	                        key={m.id}
	                        className={[
                          'max-w-[90%] px-3 py-2 text-sm leading-relaxed font-mono',
                          m.role === 'user'
                            ? 'self-end rounded border bg-accent-subtle text-text-primary border-accent-muted'
                            : 'self-start text-text-primary',
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
	                    <div ref={bottomRef} />
	                  </div>
	                </div>

	                <div className="relative mt-3">
	                  {!isAtBottom && rendered.length > 0 ? (
	                    <button
	                      type="button"
	                      onClick={() => scrollToBottom('smooth')}
	                      className="absolute -top-12 left-1/2 -translate-x-1/2 inline-flex h-9 items-center justify-center rounded px-3 text-xs font-medium bg-surface-primary text-text-primary border border-border-dark hover:bg-surface-secondary hover:border-border-hover focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus font-mono"
	                    >
	                      Scroll to bottom
	                    </button>
	                  ) : null}
	                  <form
	                    onSubmit={onSubmit}
	                    className="flex items-center gap-2 rounded border border-border-dark bg-bg-secondary p-3 shadow-[0_0_0_1px_rgba(255,255,255,0.04)_inset]"
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
