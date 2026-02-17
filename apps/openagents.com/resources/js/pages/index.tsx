import { Head } from '@inertiajs/react';
import { ArrowUpIcon } from '@radix-ui/react-icons';
import { nanoid } from 'nanoid';
import { useCallback, useRef, useState } from 'react';
import {
    Conversation,
    ConversationContent,
    ConversationScrollButton,
} from '@/components/ai-elements/conversation';
import {
    Message,
    MessageBranch,
    MessageBranchContent,
    MessageBranchNext,
    MessageBranchPage,
    MessageBranchPrevious,
    MessageBranchSelector,
    MessageContent,
    MessageResponse,
} from '@/components/ai-elements/message';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

/** Message shape aligned with ai-elements demo (simplified: no sources/reasoning/tools) */
interface IndexMessage {
    key: string;
    from: 'user' | 'assistant';
    versions: { id: string; content: string }[];
}

const MOCK_RESPONSES = [
    "Got it. I'll help with that once we're wired to the API.",
    "Message received. Ready to connect this to the backend.",
    "Noted. The next step is hooking this up to your chat API.",
];

const delay = (ms: number): Promise<void> =>
    new Promise((resolve) => {
        setTimeout(resolve, ms);
    });

/**
 * Submit behavior adapted from assistant-ui:
 * - ComposerInput: Enter submits, Shift+Enter newline, IME composition respected
 * - Conversation/submit flow from ai-elements demo: addUserMessage + streamed assistant reply
 */
export default function Index() {
    const textareaRef = useRef<HTMLTextAreaElement>(null);
    const [value, setValue] = useState('');
    const [isComposing, setIsComposing] = useState(false);
    const [messages, setMessages] = useState<IndexMessage[]>([]);
    const [status, setStatus] = useState<'ready' | 'submitted' | 'streaming'>('ready');

    const updateMessageContent = useCallback((messageId: string, newContent: string) => {
        setMessages((prev) =>
            prev.map((msg) => {
                if (msg.versions.some((v) => v.id === messageId)) {
                    return {
                        ...msg,
                        versions: msg.versions.map((v) =>
                            v.id === messageId ? { ...v, content: newContent } : v,
                        ),
                    };
                }
                return msg;
            }),
        );
    }, []);

    const streamResponse = useCallback(
        async (messageId: string, content: string) => {
            setStatus('streaming');
            const words = content.split(' ');
            let currentContent = '';
            for (const [i, word] of words.entries()) {
                currentContent += (i > 0 ? ' ' : '') + word;
                updateMessageContent(messageId, currentContent);
                await delay(Math.random() * 80 + 40);
            }
            setStatus('ready');
        },
        [updateMessageContent],
    );

    const addUserMessage = useCallback(
        (content: string) => {
            const userMessage: IndexMessage = {
                from: 'user',
                key: `user-${nanoid()}`,
                versions: [{ id: `user-${nanoid()}`, content }],
            };
            setMessages((prev) => [...prev, userMessage]);
            setValue('');
            setStatus('submitted');

            const assistantMessageId = `assistant-${nanoid()}`;
            const assistantMessage: IndexMessage = {
                from: 'assistant',
                key: `assistant-${nanoid()}`,
                versions: [{ id: assistantMessageId, content: '' }],
            };
            setTimeout(() => {
                setMessages((prev) => [...prev, assistantMessage]);
                const response =
                    MOCK_RESPONSES[Math.floor(Math.random() * MOCK_RESPONSES.length)];
                streamResponse(assistantMessageId, response);
            }, 400);
        },
        [streamResponse],
    );

    const handleKeyDown = useCallback(
        (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
            if (e.key !== 'Enter') return;
            if (isComposing || e.nativeEvent.isComposing) return;
            if (e.shiftKey) return;
            e.preventDefault();
            if (!value.trim() || status === 'streaming') return;
            const form = e.currentTarget.form;
            const submitButton = form?.querySelector<HTMLButtonElement>('button[type="submit"]');
            if (submitButton?.disabled) return;
            form?.requestSubmit();
        },
        [isComposing, value, status],
    );

    const handleSubmit = useCallback(
        (e: React.FormEvent<HTMLFormElement>) => {
            e.preventDefault();
            const trimmed = value.trim();
            if (!trimmed || status === 'streaming') return;
            addUserMessage(trimmed);
            requestAnimationFrame(() => {
                textareaRef.current?.focus({ preventScroll: true });
                textareaRef.current?.setSelectionRange(0, 0);
            });
        },
        [value, status, addUserMessage],
    );

    const isSubmitDisabled = !value.trim() || status === 'streaming';

    return (
        <>
            <Head title="" />
            <div className="chat-page-root min-h-pwa flex h-dvh flex-col overflow-hidden bg-black">
                <main className="firefox-scrollbar-margin-fix relative flex min-h-0 w-full flex-1 flex-col overflow-hidden transition-[width,height] print:absolute print:left-0 print:top-0 print:h-auto print:min-h-auto print:overflow-visible">
                    {/* Scrollable content: ai-elements Conversation + Messages */}
                    <div
                        className="absolute inset-0 overflow-y-auto print:static print:inset-auto print:block print:h-auto print:overflow-visible print:pb-0"
                        style={{ paddingBottom: 144, scrollbarGutter: 'stable both-edges' }}
                    >
                        <Conversation className="mx-auto w-full max-w-3xl">
                            <ConversationContent className="min-h-[calc(100vh-20rem)] px-4 pt-8 pb-10">
                                {messages.map(({ versions, ...message }) => (
                                    <MessageBranch defaultBranch={0} key={message.key}>
                                        <MessageBranchContent>
                                            {versions.map((version) => (
                                                <Message from={message.from} key={`${message.key}-${version.id}`}>
                                                    <MessageContent>
                                                        <MessageResponse>{version.content}</MessageResponse>
                                                    </MessageContent>
                                                </Message>
                                            ))}
                                        </MessageBranchContent>
                                        {versions.length > 1 && (
                                            <MessageBranchSelector>
                                                <MessageBranchPrevious />
                                                <MessageBranchPage />
                                                <MessageBranchNext />
                                            </MessageBranchSelector>
                                        )}
                                    </MessageBranch>
                                ))}
                            </ConversationContent>
                            <ConversationScrollButton />
                        </Conversation>
                    </div>

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
                                                placeholder="Type message here"
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
