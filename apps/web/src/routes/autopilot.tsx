import { createFileRoute, redirect, useRouter } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { useAgent } from 'agents/react';
import { useAgentChat } from '@cloudflare/ai-chat/react';
import { Effect } from 'effect';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { DotsGridBackground, whitePreset } from '@openagentsinc/hud/react';
import { AutopilotSidebar } from '../components/layout/AutopilotSidebar';
import { EffuseMount } from '../components/EffuseMount';
import { runAutopilotChat } from '../effuse-pages/autopilot';
import { TelemetryService } from '../effect/telemetry';
import { AgentApiService } from '../effect/agentApi';
import type { UIMessage } from 'ai';
import type { AgentToolContract } from '../effect/agentApi';
import type { RenderedMessage as EffuseRenderedMessage } from '../effuse-pages/autopilot';

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
        yield* telemetry.withNamespace('app').event('chat_view', { chatId: userId });

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

type UiPart = UIMessage['parts'][number];

type RenderPart =
  | { kind: 'text'; text: string; state?: 'streaming' | 'done' }
  | {
      kind: 'tool';
      toolName: string;
      toolCallId: string;
      state:
        | 'input-streaming'
        | 'input-available'
        | 'approval-requested'
        | 'approval-responded'
        | 'output-available'
        | 'output-error'
        | 'output-denied'
        | string;
      input: unknown;
      output?: unknown;
      errorText?: string;
      preliminary?: boolean;
      approval?: { id: string; approved?: boolean; reason?: string };
    };

function isTextPart(part: unknown): part is Extract<UiPart, { type: 'text' }> {
  return (
    Boolean(part) &&
    typeof part === 'object' &&
    (part as any).type === 'text' &&
    typeof (part as any).text === 'string'
  );
}

function isToolPart(
  part: unknown,
): part is
  | (Extract<UiPart, { type: `tool-${string}` }> & {
      toolCallId: string;
      state: string;
      input?: unknown;
      output?: unknown;
      errorText?: string;
      preliminary?: boolean;
      approval?: { id: string; approved?: boolean; reason?: string };
      rawInput?: unknown;
    })
  | (Extract<UiPart, { type: 'dynamic-tool' }> & {
      toolName: string;
      toolCallId: string;
      state: string;
      input?: unknown;
      output?: unknown;
      errorText?: string;
      preliminary?: boolean;
      approval?: { id: string; approved?: boolean; reason?: string };
      rawInput?: unknown;
    }) {
  if (!part || typeof part !== 'object') return false;
  const type = (part as any).type;
  if (type === 'dynamic-tool') {
    return (
      typeof (part as any).toolName === 'string' &&
      typeof (part as any).toolCallId === 'string' &&
      typeof (part as any).state === 'string'
    );
  }
  return (
    typeof type === 'string' &&
    type.startsWith('tool-') &&
    typeof (part as any).toolCallId === 'string' &&
    typeof (part as any).state === 'string'
  );
}

function getToolPartName(part: { type: string; toolName?: string }): string {
  if (part.type === 'dynamic-tool') return String(part.toolName ?? 'tool');
  if (part.type.startsWith('tool-')) return part.type.slice('tool-'.length);
  return part.type;
}

function safeStableStringify(value: unknown, indent = 2): string {
  if (value == null) return String(value);
  if (typeof value === 'string') return value;
  try {
    return JSON.stringify(value, null, indent);
  } catch {
    return String(value);
  }
}

function toRenderableParts(parts: ReadonlyArray<UiPart>): Array<RenderPart> {
  const out: Array<RenderPart> = [];

  for (const p of parts) {
    if (isTextPart(p)) {
      if (p.text.length === 0) continue;
      const prev = out.at(-1);
      if (prev?.kind === 'text' && prev.state === p.state) {
        prev.text += p.text;
      } else {
        out.push({ kind: 'text', text: p.text, state: p.state });
      }
      continue;
    }

    if (isToolPart(p)) {
      const toolName = getToolPartName(p as any);
      const state = String((p as any).state ?? '');
      const rawInput = (p as any).rawInput;
      const input = (p as any).input ?? rawInput;
      out.push({
        kind: 'tool',
        toolName,
        toolCallId: String((p as any).toolCallId),
        state,
        input,
        output: (p as any).output,
        errorText: typeof (p as any).errorText === 'string' ? (p as any).errorText : undefined,
        preliminary: Boolean((p as any).preliminary),
        approval:
          (p as any).approval && typeof (p as any).approval === 'object'
            ? {
                id: String((p as any).approval.id ?? ''),
                approved:
                  typeof (p as any).approval.approved === 'boolean'
                    ? (p as any).approval.approved
                    : undefined,
                reason:
                  typeof (p as any).approval.reason === 'string' ? (p as any).approval.reason : undefined,
              }
            : undefined,
      });
      continue;
    }
  }

  return out;
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
    characterVibe: string;
    characterBoundaries: string;
  } | null>(null);
  const [toolContractsByName, setToolContractsByName] = useState<
    Record<string, AgentToolContract> | null
  >(null);
  const [isSavingBlueprint, setIsSavingBlueprint] = useState(false);
  const [isAtBottom, setIsAtBottom] = useState(true);
  const isStreaming = chat.status === 'streaming';
  const isBusy = chat.status === 'submitted' || chat.status === 'streaming';

  const messages = chat.messages as ReadonlyArray<UIMessage>;
  const scrollRef = useRef<HTMLElement | null>(null);
  const chatMountRef = useRef<HTMLDivElement>(null);

  const recomputeIsAtBottom = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;
    const thresholdPx = 96;
    const distanceFromBottom = el.scrollHeight - (el.scrollTop + el.clientHeight);
    setIsAtBottom(distanceFromBottom <= thresholdPx);
  }, []);

  const scrollToBottom = useCallback((behavior: ScrollBehavior = 'auto') => {
    const bottom = chatMountRef.current?.firstElementChild?.querySelector('[data-autopilot-bottom]');
    (bottom as HTMLElement | undefined)?.scrollIntoView({ block: 'end', behavior });
  }, []);

  const renderedMessages = useMemo(() => {
    const isUserOrAssistant = (
      msg: UIMessage,
    ): msg is UIMessage & { readonly role: 'user' | 'assistant' } =>
      msg.role === 'user' || msg.role === 'assistant';

    return messages
      .filter(isUserOrAssistant)
      .map((msg) => {
        const parts = Array.isArray((msg as any).parts) ? (msg as any).parts : [];
        const renderParts = toRenderableParts(parts as ReadonlyArray<UiPart>);
        return {
          id: msg.id,
          role: msg.role,
          renderParts,
        };
      })
      .filter((m) => m.role === 'user' || m.renderParts.length > 0);
  }, [messages]);

  const renderedTailKey = useMemo(() => {
    const last = renderedMessages.at(-1);
    if (!last) return '';
    const lastPart = last.renderParts.at(-1);
    if (!lastPart) return `${renderedMessages.length}:${last.id}`;
    if (lastPart.kind === 'text') {
      return `${renderedMessages.length}:${last.id}:text:${lastPart.text.length}:${lastPart.state ?? ''}`;
    }
    return `${renderedMessages.length}:${last.id}:tool:${lastPart.toolName}:${lastPart.state}:${safeStableStringify(lastPart.output ?? '').length}`;
  }, [renderedMessages]);

  const autopilotChatData = useMemo((): {
    messages: ReadonlyArray<EffuseRenderedMessage>;
    isBusy: boolean;
    isAtBottom: boolean;
    inputValue: string;
  } => {
    return {
      messages: renderedMessages.map((m) => ({
        id: m.id,
        role: m.role,
        renderParts: m.renderParts.map((p) => {
          if (p.kind === 'text') {
            return { kind: 'text' as const, text: p.text, state: p.state };
          }
          const meta = toolContractsByName?.[p.toolName];
          return {
            kind: 'tool' as const,
            toolName: p.toolName,
            toolCallId: p.toolCallId,
            state: p.state,
            inputJson: safeStableStringify(p.input),
            outputJson: p.output !== undefined ? safeStableStringify(p.output) : undefined,
            errorText: p.errorText,
            preliminary: p.preliminary,
            usage: meta?.usage ?? null,
            description: meta?.description ?? null,
          };
        }),
      })),
      isBusy,
      isAtBottom,
      inputValue: input,
    };
  }, [renderedMessages, toolContractsByName, isBusy, isAtBottom, input]);

  const makeDraftFromBlueprint = useCallback((value: unknown) => {
    const b: any = value ?? {};
    const docs = b?.docs ?? {};
    const identity = docs.identity ?? {};
    const user = docs.user ?? {};
    const character = docs.character ?? {};

    const boundaries: string = Array.isArray(character.boundaries)
      ? character.boundaries
          .map((s: unknown) => (typeof s === 'string' ? s : ''))
          .filter(Boolean)
          .join('\n')
      : '';

    return {
      userHandle: typeof user.addressAs === 'string' ? user.addressAs : '',
      agentName: typeof identity.name === 'string' ? identity.name : '',
      identityVibe: typeof identity.vibe === 'string' ? identity.vibe : '',
      characterVibe: typeof character.vibe === 'string' ? character.vibe : '',
      characterBoundaries: boundaries,
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
    let cancelled = false;
    runtime
      .runPromise(
        Effect.gen(function* () {
          const api = yield* AgentApiService;
          const contracts = yield* api.getToolContracts(chatId);
          yield* Effect.sync(() => {
            if (cancelled) return;
            const map: Record<string, AgentToolContract> = {};
            for (const c of contracts) map[c.name] = c;
            setToolContractsByName(map);
          });
        }),
      )
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [chatId, runtime]);

  useEffect(() => {
    recomputeIsAtBottom();
  }, [recomputeIsAtBottom, renderedTailKey]);

  useEffect(() => {
    if (!isAtBottom) return;
    if (renderedMessages.length === 0) return;
    scrollToBottom(isStreaming ? 'auto' : 'smooth');
  }, [isAtBottom, isStreaming, renderedMessages.length, renderedTailKey, scrollToBottom]);

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

  const runAutopilotChatRef = useCallback(
    (el: Element) => runAutopilotChat(el, autopilotChatData),
    [autopilotChatData],
  );

  const onChatRendered = useCallback(() => {
    const wrapper = chatMountRef.current;
    if (!wrapper) return;
    const container = wrapper.firstElementChild;
    if (!container) return;

    const form = container.querySelector('#chat-form');
    if (form instanceof HTMLFormElement) {
      form.addEventListener('submit', (e) => {
        e.preventDefault();
        const inputEl = form.querySelector<HTMLInputElement>('input[name="message"]');
        if (!inputEl || isBusy) return;
        const text = inputEl.value.trim();
        if (!text) return;
        inputEl.value = '';
        setInput('');
        void chat.sendMessage({ text }).catch(() => {
          inputEl.value = text;
          setInput(text);
        });
        setTimeout(() => scrollToBottom('auto'), 0);
      });
    }

    const stopBtn = container.querySelector('[data-action="stop"]');
    stopBtn?.addEventListener('click', () => void chat.stop());

    const scrollBottomBtn = container.querySelector('[data-action="scroll-bottom"]');
    scrollBottomBtn?.addEventListener('click', () => scrollToBottom('smooth'));

    const inputEl = container.querySelector<HTMLInputElement>('input[name="message"]');
    if (inputEl) {
      inputEl.addEventListener('input', () => setInput(inputEl.value));
    }

    const scrollEl = container.querySelector('[data-scroll-id="autopilot-chat-scroll"]');
    if (scrollEl instanceof HTMLElement) {
      scrollRef.current = scrollEl;
      scrollEl.addEventListener('scroll', recomputeIsAtBottom);
    }
  }, [isBusy, chat, scrollToBottom, recomputeIsAtBottom]);

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
    const nextBoundaries = blueprintDraft.characterBoundaries
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

    // CHARACTER
    next.docs.character.vibe = blueprintDraft.characterVibe.trim() || next.docs.character.vibe;
    next.docs.character.boundaries = nextBoundaries;
    next.docs.character.updatedAt = nowIso;
    next.docs.character.updatedBy = 'user';
    next.docs.character.version = Number(next.docs.character.version ?? 1) + 1;

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
              'character.vibe',
              'character.boundaries',
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
	      {/* Main area: left sidebar (nav) | center (header + chat) | right (Blueprint) */}
	      <main className="flex-1 min-h-0 w-full flex overflow-hidden">
	        <AutopilotSidebar />

	        {/* Center: header + chat (Effuse) */}
	        <div ref={chatMountRef} className="flex-1 min-h-0 flex flex-col overflow-hidden">
	          <EffuseMount
	            run={runAutopilotChatRef}
	            deps={[autopilotChatData]}
	            onRendered={onChatRendered}
	            className="flex-1 min-h-0 flex flex-col overflow-hidden"
	          />
	        </div>

	        {/* Blueprint - right sidebar */}
	        <aside className="hidden lg:flex lg:w-[360px] shrink-0 border-l border-border-dark bg-bg-secondary shadow-[0_0_0_1px_rgba(255,255,255,0.04)]">
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
	                      Character vibe
	                    </span>
	                    <input
	                      value={blueprintDraft?.characterVibe ?? ''}
	                      onChange={(e) =>
	                        setBlueprintDraft((d) =>
	                          d ? { ...d, characterVibe: e.target.value } : d,
	                        )
	                      }
	                      className="h-8 w-full rounded border border-border-dark bg-surface-primary px-2 text-[12px] text-text-primary outline-none focus:border-border-focus focus:ring-1 focus:ring-border-focus font-mono"
	                    />
	                  </label>

	                  <label className="flex flex-col gap-1">
	                    <span className="text-[10px] text-text-dim uppercase tracking-wider">
	                      Boundaries (one per line)
	                    </span>
	                    <textarea
	                      value={blueprintDraft?.characterBoundaries ?? ''}
	                      onChange={(e) =>
	                        setBlueprintDraft((d) =>
	                          d ? { ...d, characterBoundaries: e.target.value } : d,
	                        )
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

// Tool parts are rendered inline as collapsible cards.
