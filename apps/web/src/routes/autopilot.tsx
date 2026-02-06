import { createFileRoute, redirect, useRouter } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { useAgent } from 'agents/react';
import { useAgentChat } from '@cloudflare/ai-chat/react';
import { Effect } from 'effect';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Streamdown } from 'streamdown';
import { DotsGridBackground, whitePreset } from '@openagentsinc/hud/react';
import { AutopilotSidebar } from '../components/layout/AutopilotSidebar';
import { KranoxFrame } from '../components/KranoxFrame';
import { TelemetryService } from '../effect/telemetry';
import { AgentApiService } from '../effect/agentApi';
import type { UIMessage } from 'ai';
import type { FormEvent } from 'react';
import type { AgentToolContract } from '../effect/agentApi';

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

function toolStateSummary(state: string): { label: string; badge: string } {
  switch (state) {
    case 'output-available':
      return { label: 'done', badge: 'OK' };
    case 'output-error':
      return { label: 'error', badge: 'ERR' };
    case 'output-denied':
      return { label: 'denied', badge: 'DENY' };
    case 'approval-requested':
      return { label: 'approval', badge: 'ASK' };
    case 'approval-responded':
      return { label: 'approval', badge: 'ACK' };
    case 'input-streaming':
    case 'input-available':
      return { label: 'running', badge: '...' };
    default:
      return { label: state, badge: '?' };
  }
}

function ToolCard({
  part,
  meta,
}: {
  part: Extract<RenderPart, { kind: 'tool' }>;
  meta?: AgentToolContract;
}) {
  const [isCollapsed, setIsCollapsed] = useState(true);
  const summary = toolStateSummary(part.state);

  const inputText = safeStableStringify(part.input);
  const outputText =
    part.state === 'output-available' ? safeStableStringify(part.output) : '';

  const headerText =
    part.state === 'output-available' ? 'Used tool:' : 'Using tool:';

  const borderTone =
    part.state === 'output-error' || part.state === 'output-denied'
      ? 'border-red-500/40 bg-red-500/5'
      : 'border-border-dark bg-surface-primary/35';

  return (
    <div
      className={[
        'w-full rounded border shadow-[0_0_0_1px_rgba(255,255,255,0.02)_inset]',
        borderTone,
      ].join(' ')}
    >
      <div className="flex items-center justify-between gap-3 px-3 py-2">
        <div className="flex items-baseline gap-2 min-w-0">
          <span className="text-[10px] uppercase tracking-[0.16em] text-text-dim shrink-0">
            {summary.badge}
          </span>
          <span className="text-xs text-text-muted shrink-0">{headerText}</span>
          <span className="text-xs font-semibold text-text-primary truncate">
            {part.toolName}
          </span>
          <span className="text-[10px] text-text-dim shrink-0">({summary.label})</span>
        </div>
        <button
          type="button"
          onClick={() => setIsCollapsed((v) => !v)}
          className="text-[10px] font-mono text-text-muted hover:text-text-primary focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus rounded px-2 py-1 shrink-0"
          aria-expanded={!isCollapsed}
        >
          {isCollapsed ? 'Show' : 'Hide'}
        </button>
      </div>

      {!isCollapsed ? (
        <div className="border-t border-border-dark/70 px-3 py-2">
          {meta?.usage || meta?.description ? (
            <div className="mb-3">
              {meta.usage ? (
                <div className="text-[10px] font-mono text-text-dim whitespace-pre-wrap break-words">
                  {meta.usage}
                </div>
              ) : null}
              {meta.description ? (
                <div className="text-[11px] text-text-muted whitespace-pre-wrap break-words">
                  {meta.description}
                </div>
              ) : null}
            </div>
          ) : null}

          <div className="text-[10px] text-text-dim uppercase tracking-wider mb-1">
            Input
          </div>
          <pre className="text-[11px] leading-4 whitespace-pre-wrap break-words text-text-primary">
            {inputText}
          </pre>

          {part.state === 'output-error' ? (
            <div className="mt-3">
              <div className="text-[10px] text-text-dim uppercase tracking-wider mb-1">
                Error
              </div>
              <pre className="text-[11px] leading-4 whitespace-pre-wrap break-words text-red-300">
                {part.errorText ?? 'Tool failed.'}
              </pre>
            </div>
          ) : null}

          {part.state === 'output-denied' ? (
            <div className="mt-3">
              <div className="text-[10px] text-text-dim uppercase tracking-wider mb-1">
                Denied
              </div>
              <div className="text-[11px] text-text-primary">
                Tool execution denied
                {part.approval?.reason ? `: ${part.approval.reason}` : '.'}
              </div>
            </div>
          ) : null}

          {part.state === 'output-available' ? (
            <div className="mt-3 border-t border-border-dark/60 border-dashed pt-2">
              <div className="text-[10px] text-text-dim uppercase tracking-wider mb-1">
                Output{part.preliminary ? ' (preliminary)' : ''}
              </div>
              <pre className="text-[11px] leading-4 whitespace-pre-wrap break-words text-text-primary">
                {outputText}
              </pre>
            </div>
          ) : null}
        </div>
      ) : null}
    </div>
  );
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

  const renderedMessages = useMemo(() => {
    return messages
      .filter((msg) => msg.role !== 'system')
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

	        {/* Center: header + chat */}
	        <div className="flex-1 min-h-0 flex flex-col overflow-hidden">
	          <header className="flex items-center h-12 px-4 gap-3 border-b border-border-dark bg-bg-secondary shrink-0 shadow-[0_1px_0_rgba(255,255,255,0.06)]">
	            <span className="text-xs text-text-dim uppercase tracking-wider">Autopilot</span>
	          </header>

	          <div className="flex-1 min-h-0 flex overflow-hidden">
	            {/* Chat - center */}
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
	                    {renderedMessages.map((m) => {
                        const userText = m.renderParts
                          .filter((p): p is Extract<RenderPart, { kind: 'text' }> => p.kind === 'text')
                          .map((p) => p.text)
                          .join('');

                        return (
                          <div
                            key={m.id}
                            className={[
                              'max-w-[90%] px-3 py-2 text-sm leading-relaxed font-mono',
                              m.role === 'user'
                                ? 'self-end rounded border bg-accent-subtle text-text-primary border-accent-muted'
                                : 'self-start text-text-primary',
                            ].join(' ')}
                          >
                            {m.role === 'user' ? (
                              <div className="whitespace-pre-wrap">{userText}</div>
                            ) : (
                              <div className="flex flex-col gap-2">
                                {m.renderParts.map((p, idx) => {
                                  if (p.kind === 'text') {
                                    return (
                                      <Streamdown
                                        key={`t:${idx}`}
                                        mode={p.state === 'streaming' ? 'streaming' : 'static'}
                                        isAnimating={p.state === 'streaming'}
                                      >
                                        {p.text}
                                      </Streamdown>
                                    );
                                  }

                                  return (
                                    <ToolCard
                                      key={`tool:${p.toolCallId}`}
                                      part={p}
                                      meta={toolContractsByName?.[p.toolName]}
                                    />
                                  );
                                })}
                              </div>
                            )}
                          </div>
                        );
                      })}
	                    <div ref={bottomRef} />
	                  </div>
	                </div>

	                <div className="relative mt-3">
	                  {!isAtBottom && renderedMessages.length > 0 ? (
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
	          </div>
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
