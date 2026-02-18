import { useChat } from '@ai-sdk/react';
import { Head, usePage } from '@inertiajs/react';
import { ArrowUpIcon } from '@radix-ui/react-icons';
import { DefaultChatTransport, type UIMessage } from 'ai';
import type { ReactNode } from 'react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
    Conversation,
    ConversationContent,
    ConversationScrollButton,
} from '@/components/ai-elements/conversation';
import {
    Message,
    MessageContent,
    MessageResponse,
} from '@/components/ai-elements/message';
import { Shimmer } from '@/components/ai-elements/shimmer';
import { Suggestion } from '@/components/ai-elements/suggestion';
import {
    Tool,
    ToolContent,
    ToolHeader,
    ToolInput,
    ToolOutput,
    type ToolPart,
} from '@/components/ai-elements/tool';
import { Button } from '@/components/ui/button';
import { usePostHogEvent } from '@/hooks/use-posthog-event';
import { cn } from '@/lib/utils';

const TOOL_STATES: ReadonlyArray<ToolPart['state']> = [
    'approval-requested',
    'approval-responded',
    'input-available',
    'input-streaming',
    'output-available',
    'output-denied',
    'output-error',
];

const QUICK_SUGGESTIONS = [
    'What tools do you have?',
    'How do I create an account?',
    'What can you do with bitcoin?',
    'Explain what you can do with the OpenAgents API',
] as const;

function isKnownToolState(value: unknown): value is ToolPart['state'] {
    return (
        typeof value === 'string' &&
        TOOL_STATES.includes(value as ToolPart['state'])
    );
}

function normalizeToolState(value: unknown): ToolPart['state'] {
    return isKnownToolState(value) ? value : 'output-available';
}

function toolNameFromPart(part: unknown): string {
    if (typeof part !== 'object' || part === null) return 'tool';
    const p = part as Record<string, unknown>;
    if (p.type === 'dynamic-tool' && typeof p.toolName === 'string')
        return p.toolName;
    if (typeof p.type === 'string' && p.type.startsWith('tool-'))
        return p.type.slice('tool-'.length);
    return 'tool';
}

/** Extract plain text from UIMessage parts for display (text parts only). */
function textFromParts(parts: UIMessage['parts']): string {
    if (!Array.isArray(parts)) return '';
    return parts
        .filter(
            (p): p is { type: 'text'; text: string } =>
                p?.type === 'text' &&
                typeof (p as { text?: unknown }).text === 'string',
        )
        .map((p) => p.text)
        .join('');
}


function parseToolOutputRecord(output: unknown): Record<string, unknown> | null {
    if (typeof output === 'object' && output !== null) {
        return output as Record<string, unknown>;
    }

    if (typeof output !== 'string') {
        return null;
    }

    try {
        const parsed = JSON.parse(output) as unknown;
        return typeof parsed === 'object' && parsed !== null
            ? (parsed as Record<string, unknown>)
            : null;
    } catch {
        return null;
    }
}

function chatLoginStatusFromPart(part: unknown): string | null {
    if (typeof part !== 'object' || part === null) return null;

    const p = part as Record<string, unknown>;
    const output = parseToolOutputRecord(p.output);

    if (!output) return null;
    if (output.toolName !== 'chat_login') return null;

    const status = output.status;
    return typeof status === 'string' ? status : null;
}

function formatChatErrorMessage(raw: string | undefined): string {
    const text = typeof raw === 'string' ? raw.trim() : '';
    if (text === '') return 'Chat request failed. Please try again.';

    if (/<!doctype html/i.test(text) || /<html/i.test(text)) {
        return 'Chat request failed due to a server response error. Please retry or start a new chat.';
    }

    try {
        const parsed = JSON.parse(text) as { message?: unknown };
        if (typeof parsed.message === 'string' && parsed.message.trim() !== '') {
            return parsed.message.trim();
        }
    } catch {}

    return text.length > 400 ? text.slice(0, 400) + '...' : text;
}

function renderMessagePart(part: unknown, idx: number): ReactNode {
    if (typeof part !== 'object' || part === null) return null;
    const p = part as Record<string, unknown>;
    const type = p.type;

    if (type === 'text' || type === 'reasoning') {
        const text = typeof p.text === 'string' ? p.text : '';
        if (!text.trim()) return null;
        return <MessageResponse key={idx}>{text}</MessageResponse>;
    }

    if (type === 'step-start') return null;

    if (
        type === 'dynamic-tool' ||
        (typeof type === 'string' && type.startsWith('tool-')) ||
        (typeof p.toolCallId === 'string' && ('input' in p || 'output' in p))
    ) {
        const toolName = toolNameFromPart(part);
        const toolType = typeof type === 'string' ? type : `tool-${toolName}`;
        const toolState = normalizeToolState(p.state);
        const errorText =
            typeof p.errorText === 'string'
                ? p.errorText
                : typeof p.error === 'string'
                  ? p.error
                  : undefined;
        const defaultOpen =
            toolState === 'approval-requested' ||
            toolState === 'output-error' ||
            toolState === 'output-denied' ||
            toolState === 'output-available';

        const header =
            toolType === 'dynamic-tool' ? (
                <ToolHeader
                    type="dynamic-tool"
                    toolName={toolName}
                    state={toolState}
                />
            ) : (
                <ToolHeader
                    type={toolType as Exclude<ToolPart['type'], 'dynamic-tool'>}
                    state={toolState}
                />
            );

        const key =
            typeof p.toolCallId === 'string' ? p.toolCallId : `part-${idx}`;
        return (
            <Tool key={key} defaultOpen={defaultOpen}>
                {header}
                <ToolContent>
                    {p.input !== undefined && (
                        <ToolInput input={p.input as ToolPart['input']} />
                    )}
                    <ToolOutput
                        output={p.output as ToolPart['output']}
                        errorText={errorText as ToolPart['errorText']}
                    />
                </ToolContent>
            </Tool>
        );
    }

    return null;
}

type IndexPageProps = {
    auth?: { user?: unknown };
    conversationId?: string | null;
    conversationTitle?: string | null;
    initialMessages?: Array<{ id: string; role: string; content: string }>;
};

function isGuestConversationId(value: unknown): value is string {
    return typeof value === 'string' && /^g-[a-f0-9]{32}$/i.test(value.trim());
}

function createGuestConversationId(): string {
    const raw =
        typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function'
            ? crypto.randomUUID()
            : `${Date.now()}-${Math.random().toString(16).slice(2)}-${Math.random().toString(16).slice(2)}`;

    const compact = raw
        .replace(/[^a-f0-9]/gi, '')
        .toLowerCase()
        .padEnd(32, '0')
        .slice(0, 32);

    return `g-${compact}`;
}

/**
 * Index chat: works for guests and authenticated users. Everyone stays on the homepage.
 * Guests get an immediate local id so input is usable instantly, then sync it to session.
 * Authenticated users create a conversation via POST /api/chats.
 */
export default function Index() {
    const page = usePage<IndexPageProps>();
    const {
        auth,
        conversationId: serverConversationIdRaw,
        initialMessages: serverInitialMessages,
    } = page.props;
    const isGuest = !auth?.user;
    const capture = usePostHogEvent('home_chat');

    const textareaRef = useRef<HTMLTextAreaElement>(null);
    const scrollContainerRef = useRef<HTMLDivElement>(null);
    const [value, setValue] = useState('');
    const [isComposing, setIsComposing] = useState(false);

    const initialConversationId = useMemo(() => {
        if (isGuest) {
            return isGuestConversationId(serverConversationIdRaw)
                ? serverConversationIdRaw
                : createGuestConversationId();
        }

        if (typeof serverConversationIdRaw === 'string' && serverConversationIdRaw.trim() !== '') {
            return serverConversationIdRaw.trim();
        }

        return null;
    }, [isGuest, serverConversationIdRaw]);

    const initialMessagesFromServer = useMemo<UIMessage[]>(() => {
        if (!Array.isArray(serverInitialMessages)) {
            return [];
        }

        return serverInitialMessages.map((m) => ({
            id: String(m.id),
            role: m.role as UIMessage['role'],
            parts: [{ type: 'text' as const, text: String(m.content ?? '') }],
        }));
    }, [serverInitialMessages]);

    const [conversationId, setConversationId] = useState<string | null>(initialConversationId);
    const [createFailedAuth, setCreateFailedAuth] = useState(false);
    const initAttemptedRef = useRef(false);

    // Resolve conversation id once:
    // Guests: sync current guest id into session (non-blocking for input).
    // Auth users: create via POST /api/chats.
    useEffect(() => {
        if (initAttemptedRef.current) return;

        if (isGuest) {
            initAttemptedRef.current = true;

            const requestedId = isGuestConversationId(conversationId)
                ? conversationId
                : createGuestConversationId();

            if (requestedId !== conversationId) {
                setConversationId(requestedId);
            }

            fetch(
                `/api/chat/guest-session?conversationId=${encodeURIComponent(requestedId)}`,
                {
                    credentials: 'include',
                    headers: { Accept: 'application/json' },
                },
            )
                .then((res) => (res.ok ? res.json() : null))
                .then((data: { conversationId?: string } | null) => {
                    const id = data?.conversationId;
                    if (isGuestConversationId(id) && id !== requestedId) {
                        setConversationId(id);
                    }
                })
                .catch(() => {});

            return;
        }

        if (conversationId) return;

        initAttemptedRef.current = true;

        fetch('/api/chats', {
            method: 'POST',
            credentials: 'include',
            headers: {
                'Content-Type': 'application/json',
                Accept: 'application/json',
            },
            body: JSON.stringify({ title: 'Chat' }),
        })
            .then((res) => {
                if (res.status === 401 || res.status === 419) {
                    window.location.assign('/login');
                    return null;
                }
                if (!res.ok) return null;
                return res.json();
            })
            .then((data: { data?: { id?: string } } | null | undefined) => {
                const id = data?.data?.id;
                if (typeof id === 'string' && id.trim() !== '') {
                    setConversationId(id.trim());
                }
            })
            .catch(() => {});
    }, [isGuest, conversationId]);
    const authRequired = createFailedAuth;

    const api = useMemo(
        () =>
            conversationId
                ? `/api/chat?conversationId=${encodeURIComponent(conversationId)}`
                : '',
        [conversationId],
    );
    const transport = useMemo(
        () =>
            api
                ? new DefaultChatTransport({ api, credentials: 'include' })
                : null,
        [api],
    );

    const { messages, setMessages, sendMessage, status, error, clearError, stop } = useChat({
        id: conversationId ?? undefined,
        messages: initialMessagesFromServer,
        transport: transport ?? undefined,
    });

    useEffect(() => {
        setConversationId(initialConversationId);
        setMessages(initialMessagesFromServer);
        initAttemptedRef.current = false;
    }, [initialConversationId, initialMessagesFromServer, setMessages]);

    useEffect(() => {
        capture('home_chat.page_opened', {
            isGuest,
            conversationId,
        });
    }, [capture, conversationId, isGuest]);

    // After successful chat login, reload page data so sidebar shows authenticated user immediately.
    const didReloadForAuthRef = useRef(false);
    useEffect(() => {
        if (!isGuest || didReloadForAuthRef.current) return;
        for (const message of messages) {
            if (message.role !== 'assistant') continue;
            const parts = message.parts ?? [];
            for (const part of parts) {
                const p = part as Record<string, unknown>;
                const status = chatLoginStatusFromPart(part);
                if (status === 'authenticated' || status === 'already_authenticated') {
                    didReloadForAuthRef.current = true;
                    window.location.assign('/');
                    return;
                }
            }
        }
    }, [isGuest, messages]);

    useEffect(() => {
        const handleNewChat = () => {
            stop();
            clearError();
            setMessages([]);
            setValue('');
            setCreateFailedAuth(false);
            initAttemptedRef.current = false;
            setConversationId(isGuest ? createGuestConversationId() : null);

            requestAnimationFrame(() => {
                textareaRef.current?.focus({ preventScroll: true });
            });
        };

        window.addEventListener('openagents:new-chat', handleNewChat);

        return () => {
            window.removeEventListener('openagents:new-chat', handleNewChat);
        };
    }, [clearError, isGuest, setMessages, stop]);

    // Focus the textarea when conversation is ready.
    // For guests, conversationId is immediate and should not wait on network.
    useEffect(() => {
        if (authRequired) return;
        if (!conversationId) return;

        const el = textareaRef.current;
        if (!el) return;

        const id = requestAnimationFrame(() => {
            el.focus({ preventScroll: true });
        });

        return () => cancelAnimationFrame(id);
    }, [conversationId, authRequired]);

    // Auto-scroll message container to bottom when messages change (new message or streaming update)
    useEffect(() => {
        const el = scrollContainerRef.current;
        if (!el || messages.length === 0) return;
        requestAnimationFrame(() => {
            el.scrollTo({ top: el.scrollHeight, behavior: 'smooth' });
        });
    }, [messages]);

    const lastStatusRef = useRef(status);

    useEffect(() => {
        const previousStatus = lastStatusRef.current;
        lastStatusRef.current = status;

        const transitionedFromStreaming =
            previousStatus === 'submitted' || previousStatus === 'streaming';

        if (!transitionedFromStreaming || status !== 'ready') return;

        const lastAssistant =
            [...messages]
                .reverse()
                .find((message) => message.role === 'assistant') ?? null;
        const hasContent =
            lastAssistant !== null &&
            textFromParts(lastAssistant.parts).trim().length > 0;

        if (!hasContent) {
            capture('home_chat.response_empty', {
                conversationId,
                messageCount: messages.length,
            });

            return;
        }

        capture('home_chat.response_completed', {
            conversationId,
            messageCount: messages.length,
        });
    }, [capture, conversationId, messages, status]);

    const handleKeyDown = useCallback(
        (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
            if (e.key !== 'Enter') return;
            if (isComposing || e.nativeEvent.isComposing) return;
            if (e.shiftKey) return;
            e.preventDefault();

            const trimmed = value.trim();
            if (
                !trimmed ||
                status === 'streaming' ||
                status === 'submitted' ||
                !conversationId
            )
                return;

            const form = e.currentTarget.form;
            const submitButton = form?.querySelector<HTMLButtonElement>(
                'button[type="submit"]',
            );
            if (submitButton?.disabled) return;
            form?.requestSubmit();
        },
        [isComposing, value, status, conversationId],
    );

    const handleSubmit = useCallback(
        async (e: React.FormEvent<HTMLFormElement>) => {
            e.preventDefault();
            const trimmed = value.trim();
            if (
                !trimmed ||
                status === 'streaming' ||
                status === 'submitted' ||
                !conversationId
            )
                return;
            setValue('');
            capture('home_chat.message_submitted', {
                conversationId,
                source: isGuest ? 'guest' : 'authed',
                characterCount: trimmed.length,
            });
            await sendMessage({ text: trimmed });
            requestAnimationFrame(() => {
                textareaRef.current?.focus({ preventScroll: true });
                textareaRef.current?.setSelectionRange(0, 0);
            });
        },
        [capture, conversationId, isGuest, sendMessage, status, value],
    );

    const handleSuggestionClick = useCallback(
        async (suggestion: string) => {
            if (
                authRequired ||
                !conversationId ||
                status === 'streaming' ||
                status === 'submitted'
            ) {
                return;
            }

            capture('home_chat.suggestion_clicked', {
                conversationId,
                suggestion,
            });
            setValue('');
            await sendMessage({ text: suggestion });
            requestAnimationFrame(() => {
                textareaRef.current?.focus({ preventScroll: true });
                textareaRef.current?.setSelectionRange(0, 0);
            });
        },
        [authRequired, capture, conversationId, sendMessage, status],
    );

    const isStreaming = status === 'submitted' || status === 'streaming';
    const isSubmitDisabled =
        !value.trim() || isStreaming || !conversationId || authRequired;

    const lastMessage =
        messages.length > 0 ? messages[messages.length - 1] : null;
    const showThinking =
        !authRequired &&
        conversationId &&
        (status === 'submitted' ||
            (status === 'streaming' &&
                lastMessage?.role === 'assistant' &&
                !textFromParts(lastMessage.parts)));

    const shouldShowQuickSuggestions =
        !authRequired && messages.length === 0 && !showThinking;

    const displayErrorMessage = formatChatErrorMessage(error?.message);

    useEffect(() => {
        if (!error?.message) return;

        capture('home_chat.error_shown', {
            conversationId,
            errorMessage: displayErrorMessage,
        });
    }, [capture, conversationId, error?.message, displayErrorMessage]);

    return (
        <>
            <Head title="" />
            <div className="chat-page-root min-h-pwa flex h-dvh flex-col overflow-hidden bg-black">
                <main className="firefox-scrollbar-margin-fix relative flex min-h-0 w-full flex-1 flex-col overflow-hidden transition-[width,height] print:absolute print:top-0 print:left-0 print:h-auto print:min-h-auto print:overflow-visible">
                    {/* Scrollable content: ai-elements Conversation + Messages from useChat */}
                    <div
                        ref={scrollContainerRef}
                        className="absolute inset-0 overflow-y-auto print:static print:inset-auto print:block print:h-auto print:overflow-visible print:pb-0"
                        style={{
                            paddingBottom: 144,
                            scrollbarGutter: 'stable both-edges',
                        }}
                    >
                        <Conversation className="mx-auto w-full max-w-3xl">
                            <ConversationContent className="min-h-[calc(100vh-20rem)] px-4 pt-8 pb-10">
                                {authRequired && (
                                    <div className="flex flex-col items-center justify-center gap-2 py-12 text-center text-muted-foreground">
                                        <p>Sign in to chat.</p>
                                        <Button
                                            asChild
                                            variant="outline"
                                            size="sm"
                                        >
                                            <a href="/login">Sign in</a>
                                        </Button>
                                    </div>
                                )}
                                {!authRequired &&
                                    conversationId &&
                                    (messages.length > 0 || showThinking) && (
                                        <>
                                            {messages.map((m) => (
                                                <Message
                                                    key={m.id}
                                                    from={m.role}
                                                >
                                                    <MessageContent>
                                                        {Array.isArray(m.parts)
                                                            ? m.parts.map(
                                                                  (part, idx) =>
                                                                      renderMessagePart(
                                                                          part,
                                                                          idx,
                                                                      ),
                                                              )
                                                            : textFromParts(
                                                                  m.parts,
                                                              ) && (
                                                                  <MessageResponse>
                                                                      {textFromParts(
                                                                          m.parts,
                                                                      )}
                                                                  </MessageResponse>
                                                              )}
                                                    </MessageContent>
                                                </Message>
                                            ))}
                                            {showThinking && (
                                                <div className="is-assistant flex w-full max-w-[95%] flex-col gap-2">
                                                    <div className="flex w-fit max-w-full min-w-0 flex-col gap-2 overflow-hidden text-sm text-foreground">
                                                        <Shimmer>
                                                            Thinking
                                                        </Shimmer>
                                                    </div>
                                                </div>
                                            )}
                                        </>
                                    )}
                                {!authRequired &&
                                    conversationId &&
                                    messages.length === 0 &&
                                    !showThinking && (
                                        <div className="flex min-h-[40vh] flex-col items-center justify-center gap-2 pt-16 text-center">
                                            <h1 className="text-3xl font-semibold tracking-tight text-foreground sm:text-4xl">
                                                Autopilot online.
                                            </h1>
                                            <p className="text-base text-muted-foreground sm:text-lg">
                                                Send a message to start
                                            </p>
                                        </div>
                                    )}
                            </ConversationContent>
                            <ConversationScrollButton />
                        </Conversation>
                    </div>

                    {/* Error banner */}
                    {error && (
                        <div className="absolute top-0 right-0 left-0 z-20 flex items-center justify-between gap-2 bg-destructive/90 px-4 py-2 text-sm text-destructive-foreground">
                            <span>{displayErrorMessage}</span>
                            <Button
                                variant="ghost"
                                size="sm"
                                onClick={() => {
                                    capture('home_chat.error_dismissed', {
                                        conversationId,
                                    });
                                    clearError();
                                }}
                            >
                                Dismiss
                            </Button>
                        </div>
                    )}

                    {/* Fixed bottom input bar */}
                    <div className="pointer-events-none absolute bottom-0 z-10 w-full overflow-x-visible px-2">
                        <div className="relative mx-auto flex w-full max-w-3xl flex-col overflow-x-visible text-center">
                            <div className="pointer-events-auto mx-auto w-full max-w-[calc(100%-2rem)]">
                                {shouldShowQuickSuggestions && (
                                    <div className="mb-2 grid grid-cols-2 gap-2">
                                        {QUICK_SUGGESTIONS.map((suggestion) => (
                                            <Suggestion
                                                key={suggestion}
                                                suggestion={suggestion}
                                                onClick={handleSuggestionClick}
                                                disabled={
                                                    authRequired ||
                                                    !conversationId ||
                                                    isStreaming
                                                }
                                                className="h-auto w-full justify-start rounded-lg px-3 py-2 text-left whitespace-normal"
                                            />
                                        ))}
                                    </div>
                                )}
                                <div className="chat-input-outer min-w-0 overflow-hidden rounded-t-lg">
                                    <form
                                        className={cn(
                                            'chat-input-form pointer-events-auto relative flex w-full min-w-0 flex-col items-stretch gap-2 rounded-t-lg border border-b-0 border-white/25 px-3 pt-3 pb-3 text-secondary-foreground outline-none',
                                            'max-sm:pb-6 sm:max-w-3xl dark:border-white/15',
                                        )}
                                        id="chat-input-form"
                                        onSubmit={handleSubmit}
                                    >
                                        <div className="flex min-w-0 grow flex-row items-start">
                                            <textarea
                                                ref={textareaRef}
                                                name="input"
                                                id="chat-input"
                                                placeholder={
                                                    authRequired
                                                        ? 'Sign in to chat'
                                                        : 'Type message here'
                                                }
                                                value={value}
                                                onChange={(e) =>
                                                    setValue(e.target.value)
                                                }
                                                className="chat-input-textarea w-full min-w-0 resize-none bg-transparent text-base leading-6 text-foreground outline-none placeholder:text-muted-foreground/60 disabled:opacity-50"
                                                aria-label="Message input"
                                                autoComplete="off"
                                                autoFocus
                                                rows={1}
                                                onKeyDown={handleKeyDown}
                                                onCompositionStart={() =>
                                                    setIsComposing(true)
                                                }
                                                onCompositionEnd={() =>
                                                    setIsComposing(false)
                                                }
                                                disabled={
                                                    authRequired ||
                                                    !conversationId
                                                }
                                            />
                                        </div>
                                        <div className="@container mt-2 -mb-px flex w-full min-w-0 flex-row-reverse justify-between">
                                            <div className="-mt-0.5 -mr-0.5 flex shrink-0 items-center justify-center gap-2">
                                                <Button
                                                    type="submit"
                                                    size="icon"
                                                    className="size-9 rounded-lg bg-zinc-600 text-zinc-100 hover:bg-zinc-500 disabled:bg-zinc-600 disabled:hover:bg-zinc-600"
                                                    aria-label="Send message"
                                                    disabled={isSubmitDisabled}
                                                >
                                                    <ArrowUpIcon className="size-5" />
                                                </Button>
                                            </div>
                                        </div>
                                    </form>
                                </div>
                            </div>
                        </div>
                    </div>
                </main>
            </div>
        </>
    );
}
