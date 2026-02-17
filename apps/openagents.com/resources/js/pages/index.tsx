import { useChat } from '@ai-sdk/react';
import { Head, usePage } from '@inertiajs/react';
import { ArrowUpIcon } from '@radix-ui/react-icons';
import { DefaultChatTransport, type UIMessage } from 'ai';
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
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

/** Extract plain text from UIMessage parts for display (text parts only). */
function textFromParts(parts: UIMessage['parts']): string {
    if (!Array.isArray(parts)) return '';
    return parts
        .filter((p): p is { type: 'text'; text: string } => p?.type === 'text' && typeof (p as { text?: unknown }).text === 'string')
        .map((p) => p.text)
        .join('');
}

type IndexPageProps = { auth?: { user?: unknown } };

/**
 * Index chat: works for guests and authenticated users. Everyone stays on the homepage.
 * Guests: GET /api/chat/guest-session for conversation id. Authed: POST /api/chats.
 */
export default function Index() {
    const { auth } = usePage<IndexPageProps>().props;
    const isGuest = !auth?.user;

    const textareaRef = useRef<HTMLTextAreaElement>(null);
    const scrollContainerRef = useRef<HTMLDivElement>(null);
    const [value, setValue] = useState('');
    const [isComposing, setIsComposing] = useState(false);
    const [conversationId, setConversationId] = useState<string | null>(null);
    const [createFailedAuth, setCreateFailedAuth] = useState(false);
    const initAttemptedRef = useRef(false);

    // Resolve conversation id: guests use guest-session; authed users create via POST /api/chats
    useEffect(() => {
        if (initAttemptedRef.current || conversationId) return;
        initAttemptedRef.current = true;

        if (isGuest) {
            fetch('/api/chat/guest-session', { credentials: 'include' })
                .then((res) => (res.ok ? res.json() : null))
                .then((data: { conversationId?: string } | null) => {
                    const id = data?.conversationId;
                    if (typeof id === 'string' && id.trim() !== '') {
                        setConversationId(id.trim());
                    }
                })
                .catch(() => {});
            return;
        }

        fetch('/api/chats', {
            method: 'POST',
            credentials: 'include',
            headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
            body: JSON.stringify({ title: 'Chat' }),
        })
            .then((res) => {
                if (res.status === 401 || res.status === 419) {
                    setCreateFailedAuth(true);
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
        () => (conversationId ? `/api/chat?conversationId=${encodeURIComponent(conversationId)}` : ''),
        [conversationId],
    );
    const transport = useMemo(
        () => (api ? new DefaultChatTransport({ api, credentials: 'include' }) : null),
        [api],
    );

    const { messages, sendMessage, status, error, clearError } = useChat({
        id: conversationId ?? undefined,
        messages: [],
        transport: transport ?? undefined,
    });

    // Focus the textarea when conversation is ready (input becomes enabled)
    useEffect(() => {
        if (!conversationId || authRequired) return;
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

    const handleKeyDown = useCallback(
        (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
            if (e.key !== 'Enter') return;
            if (isComposing || e.nativeEvent.isComposing) return;
            if (e.shiftKey) return;
            e.preventDefault();
            const trimmed = value.trim();
            if (!trimmed || status === 'streaming' || status === 'submitted' || !conversationId) return;
            const form = e.currentTarget.form;
            const submitButton = form?.querySelector<HTMLButtonElement>('button[type="submit"]');
            if (submitButton?.disabled) return;
            form?.requestSubmit();
        },
        [isComposing, value, status, conversationId],
    );

    const handleSubmit = useCallback(
        async (e: React.FormEvent<HTMLFormElement>) => {
            e.preventDefault();
            const trimmed = value.trim();
            if (!trimmed || status === 'streaming' || status === 'submitted' || !conversationId) return;
            setValue('');
            await sendMessage({ text: trimmed });
            requestAnimationFrame(() => {
                textareaRef.current?.focus({ preventScroll: true });
                textareaRef.current?.setSelectionRange(0, 0);
            });
        },
        [value, status, conversationId, sendMessage],
    );

    const isStreaming = status === 'submitted' || status === 'streaming';
    const isSubmitDisabled = !value.trim() || isStreaming || !conversationId || authRequired;

    return (
        <>
            <Head title="" />
            <div className="chat-page-root min-h-pwa flex h-dvh flex-col overflow-hidden bg-black">
                <main className="firefox-scrollbar-margin-fix relative flex min-h-0 w-full flex-1 flex-col overflow-hidden transition-[width,height] print:absolute print:left-0 print:top-0 print:h-auto print:min-h-auto print:overflow-visible">
                    {/* Scrollable content: ai-elements Conversation + Messages from useChat */}
                    <div
                        ref={scrollContainerRef}
                        className="absolute inset-0 overflow-y-auto print:static print:inset-auto print:block print:h-auto print:overflow-visible print:pb-0"
                        style={{ paddingBottom: 144, scrollbarGutter: 'stable both-edges' }}
                    >
                        <Conversation className="mx-auto w-full max-w-3xl">
                            <ConversationContent className="min-h-[calc(100vh-20rem)] px-4 pt-8 pb-10">
                                {authRequired && (
                                    <div className="flex flex-col items-center justify-center gap-2 py-12 text-center text-muted-foreground">
                                        <p>Sign in to chat.</p>
                                        <Button asChild variant="outline" size="sm">
                                            <a href="/login">Sign in</a>
                                        </Button>
                                    </div>
                                )}
                                {!authRequired && conversationId && messages.length > 0 && (
                                    <>
                                        {messages.map((m) => (
                                            <Message key={m.id} from={m.role}>
                                                <MessageContent>
                                                    <MessageResponse>{textFromParts(m.parts)}</MessageResponse>
                                                </MessageContent>
                                            </Message>
                                        ))}
                                    </>
                                )}
                            </ConversationContent>
                            <ConversationScrollButton />
                        </Conversation>
                    </div>

                    {/* Error banner */}
                    {error && (
                        <div className="absolute left-0 right-0 top-0 z-20 flex items-center justify-between gap-2 bg-destructive/90 px-4 py-2 text-destructive-foreground text-sm">
                            <span>{error.message}</span>
                            <Button variant="ghost" size="sm" onClick={() => clearError()}>
                                Dismiss
                            </Button>
                        </div>
                    )}

                    {/* Fixed bottom input bar (unchanged look) */}
                    <div className="pointer-events-none absolute bottom-0 z-10 w-full overflow-x-visible px-2">
                        <div className="relative mx-auto flex w-full max-w-3xl flex-col overflow-x-visible text-center">
                            <div className="pointer-events-auto mx-auto w-full max-w-[calc(100%-2rem)]">
                                <div
                                    className={cn(
                                        'chat-input-outer min-w-0 overflow-hidden rounded-t-lg border border-b-0 border-white/20 p-2 pb-0 backdrop-blur-lg',
                                        'dark:border-white/12',
                                    )}
                                >
                                    <form
                                        className={cn(
                                            'chat-input-form pointer-events-auto relative flex w-full min-w-0 flex-col items-stretch gap-2 rounded-t-md border border-b-0 border-white/25 px-3 pt-3 pb-3 text-secondary-foreground outline-none',
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
                                                placeholder={authRequired ? 'Sign in to chat' : 'Type message here'}
                                                value={value}
                                                onChange={(e) => setValue(e.target.value)}
                                                className="chat-input-textarea w-full min-w-0 resize-none bg-transparent text-base leading-6 text-foreground outline-none placeholder:text-muted-foreground/60 disabled:opacity-50"
                                                aria-label="Message input"
                                                autoComplete="off"
                                                autoFocus
                                                rows={1}
                                                onKeyDown={handleKeyDown}
                                                onCompositionStart={() => setIsComposing(true)}
                                                onCompositionEnd={() => setIsComposing(false)}
                                                disabled={authRequired || !conversationId}
                                            />
                                        </div>
                                        <div className="@container -mb-px mt-2 flex w-full min-w-0 flex-row-reverse justify-between">
                                            <div className="-mr-0.5 -mt-0.5 flex shrink-0 items-center justify-center gap-2">
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
