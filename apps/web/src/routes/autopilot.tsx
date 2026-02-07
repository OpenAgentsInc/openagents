import { useAtomSet, useAtomValue } from '@effect-atom/atom-react';
import { createFileRoute, useRouter, useRouterState } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { Effect } from 'effect';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { makeEzRegistry, renderToString } from '@openagentsinc/effuse';
import { EffuseMount } from '../components/EffuseMount';
import { cleanupAuthedDotsGridBackground, hydrateAuthedDotsGridBackground } from '../effuse-pages/authedShell';
import { autopilotRouteShellTemplate, runAutopilotRoute } from '../effuse-pages/autopilotRoute';
import { ChatService } from '../effect/chat';
import { AutopilotChatIsAtBottomAtom, ChatSnapshotAtom } from '../effect/atoms/chat';
import { AutopilotSidebarCollapsedAtom, AutopilotSidebarUserMenuOpenAtom } from '../effect/atoms/autopilotUi';
import { SessionAtom } from '../effect/atoms/session';
import { TelemetryService } from '../effect/telemetry';
import { AgentApiService } from '../effect/agentApi';
import { clearRootAuthCache } from './__root';
import type { UIMessage } from 'ai';
import type { AgentToolContract } from '../effect/agentApi';
import type { RenderedMessage as EffuseRenderedMessage } from '../effuse-pages/autopilot';
import type { AutopilotRouteRenderInput } from '../effuse-pages/autopilotRoute';
import type { AutopilotSidebarModel } from '../effuse-pages/autopilotSidebar';

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
          return { kind: 'ok' as const, userId: null };
        }

        yield* telemetry.withNamespace('route.autopilot').event('autopilot.open', {
          userId,
        });
        yield* telemetry.withNamespace('app').event('chat_view', { chatId: userId });

        return { kind: 'ok' as const, userId };
      }),
    );

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

/** Minimal shell + sidebar + intro for unauthenticated users (no chat/API). */
function AutopilotIntroPage() {
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const collapsed = useAtomValue(AutopilotSidebarCollapsedAtom);
  const setCollapsed = useAtomSet(AutopilotSidebarCollapsedAtom);
  const userMenuOpen = useAtomValue(AutopilotSidebarUserMenuOpenAtom);
  const setUserMenuOpen = useAtomSet(AutopilotSidebarUserMenuOpenAtom);

  const ezRegistryRef = useRef(makeEzRegistry());
  const ezRegistry = ezRegistryRef.current;
  ezRegistry.set('autopilot.sidebar.toggleCollapse', () =>
    Effect.sync(() => setCollapsed((c) => !c)),
  );
  ezRegistry.set('autopilot.sidebar.toggleUserMenu', () =>
    Effect.sync(() => setUserMenuOpen((o) => !o)),
  );
  ezRegistry.set('autopilot.sidebar.logout', () => Effect.void);

  const sidebarModel = useMemo(
    () => ({
      collapsed,
      pathname,
      user: null as AutopilotSidebarModel['user'],
      userMenuOpen,
    }),
    [collapsed, pathname, userMenuOpen],
  );

  const renderInput = useMemo((): AutopilotRouteRenderInput => {
    const emptyChatData = {
      messages: [],
      isBusy: false,
      isAtBottom: true,
      inputValue: '',
    };
    const emptyBlueprintModel = {
      updatedAtLabel: null,
      isLoading: false,
      isEditing: false,
      canEdit: false,
      isSaving: false,
      errorText: null,
      blueprintText: null,
      draft: null,
    };
    const emptyControlsModel = {
      isExportingBlueprint: false,
      isBusy: false,
      isResettingAgent: false,
    };
    return {
      sidebarModel,
      sidebarKey: `intro:${collapsed ? 1 : 0}:${pathname}`,
      introMode: true,
      chatData: emptyChatData,
      chatKey: 'intro',
      blueprintModel: emptyBlueprintModel,
      blueprintKey: 'intro',
      controlsModel: emptyControlsModel,
      controlsKey: 'intro',
    };
  }, [sidebarModel, collapsed, pathname]);

  const ssrHtmlRef = useRef<string | null>(null);
  if (ssrHtmlRef.current === null) {
    ssrHtmlRef.current = renderToString(autopilotRouteShellTemplate());
  }

  const hydrate = useCallback(
    (el: Element) =>
      Effect.gen(function* () {
        yield* hydrateAuthedDotsGridBackground(el);
        yield* runAutopilotRoute(el, renderInput);
      }),
    [renderInput],
  );

  return (
    <EffuseMount
      run={(el) => runAutopilotRoute(el, renderInput)}
      deps={[renderInput.sidebarKey, 'intro']}
      ssrHtml={ssrHtmlRef.current}
      hydrate={hydrate}
      onCleanup={cleanupAuthedDotsGridBackground}
      cleanupOn="unmount"
      ezRegistry={ezRegistry}
      className="h-full w-full"
    />
  );
}

function ChatPage() {
  const { userId } = Route.useLoaderData();
  if (userId === null) {
    return <AutopilotIntroPage />;
  }

  const chatId = userId;
  const router = useRouter();
  const runtime = router.options.context.effectRuntime;
  const pathname = useRouterState({ select: (s) => s.location.pathname });

  const session = useAtomValue(SessionAtom);
  const setSession = useAtomSet(SessionAtom);

  const collapsed = useAtomValue(AutopilotSidebarCollapsedAtom);
  const setCollapsed = useAtomSet(AutopilotSidebarCollapsedAtom);
  const userMenuOpen = useAtomValue(AutopilotSidebarUserMenuOpenAtom);
  const setUserMenuOpen = useAtomSet(AutopilotSidebarUserMenuOpenAtom);

  const signOut = useCallback(async () => {
    try {
      await fetch('/api/auth/signout', {
        method: 'POST',
        credentials: 'include',
      });
    } catch {
      // Best-effort sign-out; always clear local session state.
    } finally {
      clearRootAuthCache();
      setSession({ userId: null, user: null });
      router.navigate({ href: '/' }).catch(() => {});
    }
  }, [router, setSession]);

  useEffect(() => {
    if (!userMenuOpen) return;
    const onDocClick = (e: MouseEvent) => {
      const root = document.querySelector('[data-autopilot-sidebar-root]');
      if (!(root instanceof HTMLElement)) return;
      if (!(e.target instanceof Node)) return;
      if (!root.contains(e.target)) setUserMenuOpen(false);
    };
    document.addEventListener('click', onDocClick);
    return () => document.removeEventListener('click', onDocClick);
  }, [userMenuOpen, setUserMenuOpen]);

  const chatClient = useMemo(
    () =>
      runtime.runSync(
        Effect.gen(function* () {
          return yield* ChatService;
        }),
      ),
    [runtime],
  );
  const chatSnapshot = useAtomValue(ChatSnapshotAtom(chatId));

  // Keep input draft in a ref so keystrokes don't force a React rerender.
  // We still want the draft persisted across Effuse rerenders (when messages stream, etc.).
  const inputDraftRef = useRef('');

  const didMountRef = useRef(false);
  const [isExportingBlueprint, setIsExportingBlueprint] = useState(false);
  const [isResettingAgent, setIsResettingAgent] = useState(false);
  const [blueprint, setBlueprint] = useState<unknown | null>(null);
  const [blueprintError, setBlueprintError] = useState<string | null>(null);
  const [blueprintLoading, setBlueprintLoading] = useState(false);
  const [blueprintUpdatedAt, setBlueprintUpdatedAt] = useState<number | null>(null);
  const [isEditingBlueprint, setIsEditingBlueprint] = useState(false);
  type BlueprintDraft = {
    userHandle: string;
    agentName: string;
    identityVibe: string;
    characterVibe: string;
    characterBoundaries: string;
  };
  const blueprintDraftRef = useRef<BlueprintDraft | null>(null);
  const [toolContractsByName, setToolContractsByName] = useState<
    Record<string, AgentToolContract> | null
  >(null);
  const [isSavingBlueprint, setIsSavingBlueprint] = useState(false);
  const isAtBottom = useAtomValue(AutopilotChatIsAtBottomAtom(chatId));
  const setIsAtBottom = useAtomSet(AutopilotChatIsAtBottomAtom(chatId));
  const isStreaming = chatSnapshot.status === 'streaming';
  const isBusy = chatSnapshot.status === 'submitted' || chatSnapshot.status === 'streaming';

  const messages = chatSnapshot.messages;

  const scrollToBottom = useCallback((behavior: ScrollBehavior = 'auto') => {
    const bottom = document.querySelector('[data-autopilot-bottom]');
    (bottom as HTMLElement | null)?.scrollIntoView({ block: 'end', behavior });
  }, []);

  // Track whether the user is near the bottom of the chat scroll region.
  // Use a capture-phase scroll listener so we don't need to rebind on each Effuse re-render.
  useEffect(() => {
    const thresholdPx = 96;
    const onScrollCapture = (e: Event) => {
      const target = e.target;
      if (!(target instanceof HTMLElement)) return;
      if (target.getAttribute('data-scroll-id') !== 'autopilot-chat-scroll') return;
      const distanceFromBottom = target.scrollHeight - (target.scrollTop + target.clientHeight);
      setIsAtBottom(distanceFromBottom <= thresholdPx);
    };

    document.addEventListener('scroll', onScrollCapture, true);
    return () => document.removeEventListener('scroll', onScrollCapture, true);
  }, [setIsAtBottom]);

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
      inputValue: inputDraftRef.current,
    };
  }, [renderedMessages, toolContractsByName, isBusy, isAtBottom]);

  const makeDraftFromBlueprint = useCallback((value: unknown): BlueprintDraft => {
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
                blueprintDraftRef.current = makeDraftFromBlueprint(json);
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

  const ezRegistryRef = useRef(makeEzRegistry());
  const ezRegistry = ezRegistryRef.current;

  // Keep handlers in a stable Map (EffuseMount expects stable Map identity).
  ezRegistry.set('autopilot.sidebar.toggleCollapse', () =>
    Effect.sync(() => {
      setUserMenuOpen(false);
      setCollapsed((c) => !c);
    }),
  );

  ezRegistry.set('autopilot.sidebar.toggleUserMenu', () =>
    Effect.sync(() => setUserMenuOpen((o) => !o)),
  );

  ezRegistry.set('autopilot.sidebar.logout', () =>
    Effect.sync(() => {
      setUserMenuOpen(false);
      void signOut();
    }),
  );

  ezRegistry.set('autopilot.chat.input', ({ params }) =>
    Effect.sync(() => {
      const paramsMaybe = params as Record<string, string | undefined>;
      inputDraftRef.current = String(paramsMaybe.message ?? '');
    }),
  );

  ezRegistry.set('autopilot.chat.scrollBottom', () =>
    Effect.sync(() => scrollToBottom('smooth')),
  );

  ezRegistry.set('autopilot.chat.stop', () => chatClient.stop(chatId));

  ezRegistry.set('autopilot.chat.send', ({ el, params }) =>
    Effect.gen(function* () {
      if (isBusy) return;
      const form = el instanceof HTMLFormElement ? el : null;
      const inputEl = form?.querySelector<HTMLInputElement>('input[name="message"]') ?? null;

      const paramsMaybe = params as Record<string, string | undefined>;
      const text = String(paramsMaybe.message ?? '').trim();
      if (!text) return;

      yield* Effect.sync(() => {
        if (inputEl) inputEl.value = '';
        inputDraftRef.current = '';
      });

      yield* chatClient.send(chatId, text).pipe(
        Effect.catchAll(() =>
          Effect.sync(() => {
            if (inputEl) inputEl.value = text;
            inputDraftRef.current = text;
          }),
        ),
      );

      yield* Effect.sync(() => setTimeout(() => scrollToBottom('auto'), 0));
    }),
  );

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
    blueprintDraftRef.current = makeDraftFromBlueprint(blueprint);
    setIsEditingBlueprint(true);
  };

  const onCancelEditBlueprint = () => {
    blueprintDraftRef.current = blueprint ? makeDraftFromBlueprint(blueprint) : null;
    setIsEditingBlueprint(false);
  };

  const onSaveBlueprint = async () => {
    const blueprintDraft = blueprintDraftRef.current;
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
      inputDraftRef.current = '';
      const inputEl = document.querySelector<HTMLInputElement>('input[name="message"]');
      if (inputEl) inputEl.value = '';
      await fetchBlueprint();
      const nextMessages = await fetchChatMessages();
      await Effect.runPromise(chatClient.setMessages(chatId, nextMessages)).catch(() => {});
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

  const blueprintPanelModel = useMemo(
    () => ({
      updatedAtLabel: blueprintUpdatedAt ? new Date(blueprintUpdatedAt).toLocaleTimeString() : null,
      isLoading: blueprintLoading,
      isEditing: isEditingBlueprint,
      canEdit: Boolean(blueprint),
      isSaving: isSavingBlueprint,
      errorText: blueprintError,
      blueprintText,
      draft: isEditingBlueprint
        ? (blueprintDraftRef.current ??
            (blueprint
              ? makeDraftFromBlueprint(blueprint)
              : {
                  userHandle: '',
                  agentName: '',
                  identityVibe: '',
                  characterVibe: '',
                  characterBoundaries: '',
                }))
        : null,
    }),
    [
      blueprint,
      blueprintError,
      blueprintLoading,
      blueprintText,
      blueprintUpdatedAt,
      isEditingBlueprint,
      isSavingBlueprint,
      makeDraftFromBlueprint,
    ],
  );

  ezRegistry.set('autopilot.blueprint.toggleEdit', () =>
    Effect.sync(() => {
      if (isEditingBlueprint) onCancelEditBlueprint();
      else onStartEditBlueprint();
    }),
  );

  ezRegistry.set('autopilot.blueprint.refresh', () =>
    Effect.sync(() => {
      fetchBlueprint().catch(() => {});
    }),
  );

  ezRegistry.set('autopilot.blueprint.save', () =>
    Effect.sync(() => {
      onSaveBlueprint().catch(() => {});
    }),
  );

  ezRegistry.set('autopilot.blueprint.draft', ({ params }) =>
    Effect.sync(() => {
      const paramsMaybe = params as Record<string, string | undefined>;
      const draft =
        blueprintDraftRef.current ??
        (blueprint
          ? makeDraftFromBlueprint(blueprint)
          : {
              userHandle: '',
              agentName: '',
              identityVibe: '',
              characterVibe: '',
              characterBoundaries: '',
            });
      blueprintDraftRef.current = draft;

      if (paramsMaybe.userHandle !== undefined) draft.userHandle = String(paramsMaybe.userHandle);
      if (paramsMaybe.agentName !== undefined) draft.agentName = String(paramsMaybe.agentName);
      if (paramsMaybe.identityVibe !== undefined) draft.identityVibe = String(paramsMaybe.identityVibe);
      if (paramsMaybe.characterVibe !== undefined) draft.characterVibe = String(paramsMaybe.characterVibe);
      if (paramsMaybe.characterBoundaries !== undefined) {
        draft.characterBoundaries = String(paramsMaybe.characterBoundaries);
      }
    }),
  );

  const controlsModel = useMemo(
    () => ({
      isExportingBlueprint,
      isBusy,
      isResettingAgent,
    }),
    [isBusy, isExportingBlueprint, isResettingAgent],
  );

  ezRegistry.set('autopilot.controls.exportBlueprint', () =>
    Effect.sync(() => {
      onExportBlueprint().catch(() => {});
    }),
  );

  ezRegistry.set('autopilot.controls.clearMessages', () =>
    Effect.gen(function* () {
      yield* chatClient.clearHistory(chatId);
      yield* Effect.sync(() => {
        inputDraftRef.current = '';
        const inputEl = document.querySelector<HTMLInputElement>('input[name="message"]');
        if (inputEl) inputEl.value = '';
      });
    }),
  );

  ezRegistry.set('autopilot.controls.resetAgent', () =>
    Effect.sync(() => {
      onResetAgent().catch(() => {});
    }),
  );

  const sidebarModel = useMemo(
    () => ({
      collapsed,
      pathname,
      user: session.user
        ? {
            email: session.user.email,
            firstName: session.user.firstName,
            lastName: session.user.lastName,
          }
        : null,
      userMenuOpen,
    }),
    [collapsed, pathname, session.user, userMenuOpen],
  );

  const toolContractsKey = useMemo(() => {
    if (!toolContractsByName) return '';
    return Object.keys(toolContractsByName).sort().join(',');
  }, [toolContractsByName]);

  const sidebarKey = useMemo(
    () =>
      `${collapsed ? 1 : 0}:${pathname}:${session.user?.id ?? 'null'}:${userMenuOpen ? 1 : 0}`,
    [collapsed, pathname, session.user?.id, userMenuOpen],
  );

  const chatKey = useMemo(
    () => `${renderedTailKey}:${isBusy ? 1 : 0}:${isAtBottom ? 1 : 0}:${toolContractsKey}`,
    [renderedTailKey, isBusy, isAtBottom, toolContractsKey],
  );

  const blueprintKey = useMemo(() => {
    if (isEditingBlueprint) {
      // Freeze the Effuse form while editing so background refreshes don't reset the inputs/caret.
      return `edit:${isSavingBlueprint ? 1 : 0}:${blueprintError ?? ''}`;
    }
    return `view:${isSavingBlueprint ? 1 : 0}:${blueprintLoading ? 1 : 0}:${blueprintError ?? ''}:${blueprintUpdatedAt ?? 0}`;
  }, [
    blueprintError,
    blueprintLoading,
    blueprintUpdatedAt,
    isEditingBlueprint,
    isSavingBlueprint,
  ]);

  const controlsKey = useMemo(
    () => `${isExportingBlueprint ? 1 : 0}:${isBusy ? 1 : 0}:${isResettingAgent ? 1 : 0}`,
    [isExportingBlueprint, isBusy, isResettingAgent],
  );

  const renderInput: AutopilotRouteRenderInput = useMemo(
    () => ({
      sidebarModel,
      sidebarKey,
      chatData: autopilotChatData,
      chatKey,
      blueprintModel: blueprintPanelModel,
      blueprintKey,
      controlsModel,
      controlsKey,
    }),
    [
      autopilotChatData,
      blueprintKey,
      blueprintPanelModel,
      chatKey,
      controlsKey,
      controlsModel,
      sidebarKey,
      sidebarModel,
    ],
  );

  const ssrHtmlRef = useRef<string | null>(null);
  if (ssrHtmlRef.current === null) {
    ssrHtmlRef.current = renderToString(autopilotRouteShellTemplate());
  }
  const ssrHtml = ssrHtmlRef.current;

  const hydrate = useCallback(
    (el: Element) =>
      Effect.gen(function* () {
        yield* hydrateAuthedDotsGridBackground(el);
        yield* runAutopilotRoute(el, renderInput);
      }),
    [renderInput],
  );

  return (
    <EffuseMount
      run={(el) => runAutopilotRoute(el, renderInput)}
      deps={[sidebarKey, chatKey, blueprintKey, controlsKey]}
      ssrHtml={ssrHtml}
      hydrate={hydrate}
      onCleanup={cleanupAuthedDotsGridBackground}
      cleanupOn="unmount"
      ezRegistry={ezRegistry}
      className="h-full w-full"
    />
  );
}

// Tool parts are rendered inline as collapsible cards.
