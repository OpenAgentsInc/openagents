import { useChat } from '@ai-sdk/react';
import { Head } from '@inertiajs/react';
import { DefaultChatTransport, type UIMessage } from 'ai';
import { useEffect, useMemo, useRef } from 'react';
import {
    PromptInput,
    PromptInputBody,
    PromptInputFooter,
    PromptInputProvider,
    PromptInputSubmit,
    PromptInputTextarea,
    usePromptInputController,
} from '@/components/ai-elements/prompt-input';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { MessageResponse } from '@/components/ai-elements/message';
import {
    Collapsible,
    CollapsibleContent,
    CollapsibleTrigger,
} from '@/components/ui/collapsible';
import { ScrollArea } from '@/components/ui/scroll-area';
import AppLayout from '@/layouts/app-layout';
import type { BreadcrumbItem } from '@/types';

type Props = {
    conversationId: string;
    conversationTitle: string;
    initialMessages: Array<{ id: string; role: string; content: string }>;
};

function prettyJson(value: unknown): string {
    if (value == null) return '';
    if (typeof value === 'string') {
        try {
            return JSON.stringify(JSON.parse(value), null, 2);
        } catch {
            return value;
        }
    }
    try {
        return JSON.stringify(value, null, 2);
    } catch {
        return String(value);
    }
}

function toolNameFromPart(part: unknown): string {
    if (typeof part !== 'object' || part === null) return 'tool';
    const p = part as Record<string, unknown>;
    const type = p.type;
    if (type === 'dynamic-tool' && typeof p.toolName === 'string') return p.toolName;
    if (typeof type === 'string' && type.startsWith('tool-')) return type.slice('tool-'.length);
    return 'tool';
}

function l402ToolSummary(output: unknown): string | null {
    let normalized: unknown = output;
    if (typeof normalized === 'string') {
        try {
            normalized = JSON.parse(normalized);
        } catch {
            return null;
        }
    }
    if (typeof normalized !== 'object' || normalized === null) return null;
    const o = normalized as Record<string, unknown>;
    const host = typeof o.host === 'string' ? o.host : 'unknown';
    const status = typeof o.status === 'string' ? o.status : null;
    const paid = o.paid === true;
    const cacheHit = o.cacheHit === true;
    const cacheStatus = typeof o.cacheStatus === 'string' ? o.cacheStatus : cacheHit ? 'hit' : null;
    const amountMsats = typeof o.amountMsats === 'number' ? o.amountMsats : null;
    const quotedAmountMsats = typeof o.quotedAmountMsats === 'number' ? o.quotedAmountMsats : null;
    const msats = amountMsats ?? quotedAmountMsats;
    const sats = typeof msats === 'number' ? Math.round(msats / 1000) : null;
    const proofReference = typeof o.proofReference === 'string' ? o.proofReference : null;
    const denyCode = typeof o.denyCode === 'string' ? o.denyCode : null;
    if (status === 'blocked') return `L402 blocked · ${host}${denyCode ? ` · ${denyCode}` : ''}`;
    if (status === 'failed') return `L402 failed · ${host}`;
    if (status === 'cached' || cacheStatus === 'hit')
        return `L402 cached · ${host}${sats != null ? ` · ${sats} sats` : ''}${proofReference ? ` · ${proofReference}` : ''}`;
    if (paid)
        return `L402 paid · ${host}${sats != null ? ` · ${sats} sats` : ''}${proofReference ? ` · ${proofReference}` : ''}`;
    return `L402 fetch · ${host}`;
}

function MessagePart({ part, idx }: { part: unknown; idx: number }) {
    if (typeof part !== 'object' || part === null) return null;
    const p = part as Record<string, unknown>;
    const type = p.type;

    if (type === 'text' || type === 'reasoning') {
        const text = typeof p.text === 'string' ? p.text : '';
        if (!text.trim()) return null;
        if (type === 'reasoning') {
            return (
                <div key={idx} className="border-l-2 border-muted-foreground/30 pl-2 text-sm italic text-muted-foreground">
                    <MessageResponse>{text}</MessageResponse>
                </div>
            );
        }
        return <MessageResponse key={idx}>{text}</MessageResponse>;
    }

    if (type === 'dynamic-tool' || (typeof type === 'string' && type.startsWith('tool-'))) {
        const toolName = toolNameFromPart(part);
        const state = typeof p.state === 'string' ? p.state : '';
        const toolCallId = typeof p.toolCallId === 'string' ? p.toolCallId : '';
        const summary =
            toolName === 'lightning_l402_fetch' ? l402ToolSummary(p.output) : null;

        return (
            <Collapsible key={idx} defaultOpen={false}>
                <Card className="rounded-md border-sidebar-border/70 bg-muted/10">
                    <CollapsibleTrigger asChild>
                        <button
                            type="button"
                            className="flex w-full cursor-pointer select-none items-center gap-2 rounded-md p-2 text-left text-xs font-mono hover:bg-muted/20"
                        >
                            <span className="truncate">
                                {summary ?? toolName} · {state || 'tool'}
                            </span>
                            <span className="shrink-0 text-muted-foreground">{toolCallId.slice(0, 8)}</span>
                        </button>
                    </CollapsibleTrigger>
                    <CollapsibleContent>
                        <CardContent className="grid gap-2 pt-0 text-xs">
                            {p.input !== undefined && (
                                <div>
                                    <div className="text-[11px] uppercase tracking-wide text-muted-foreground">
                                        input
                                    </div>
                                    <pre className="mt-1 overflow-x-auto rounded bg-muted/30 p-2 font-mono text-[11px] leading-snug">
                                        {prettyJson(p.input)}
                                    </pre>
                                </div>
                            )}
                            {p.output !== undefined && (
                                <div>
                                    <div className="text-[11px] uppercase tracking-wide text-muted-foreground">
                                        output
                                    </div>
                                    <pre className="mt-1 overflow-x-auto rounded bg-muted/30 p-2 font-mono text-[11px] leading-snug">
                                        {prettyJson(p.output)}
                                    </pre>
                                </div>
                            )}
                            {typeof p.errorText === 'string' && p.errorText && (
                                <div>
                                    <div className="text-[11px] uppercase tracking-wide text-destructive">
                                        error
                                    </div>
                                    <pre className="mt-1 overflow-x-auto rounded bg-destructive/10 p-2 font-mono text-[11px] leading-snug text-destructive">
                                        {p.errorText}
                                    </pre>
                                </div>
                            )}
                        </CardContent>
                    </CollapsibleContent>
                </Card>
            </Collapsible>
        );
    }

    return null;
}

function MessageBubble({ message }: { message: UIMessage }) {
    const isUser = message.role === 'user';
    return (
        <div
            className={`group flex flex-col gap-2 ${isUser ? 'is-user ml-auto w-fit max-w-[85%] items-end' : 'is-assistant w-full max-w-[95%]'}`}
        >
            <div
                className={
                    isUser
                        ? 'flex w-fit min-w-0 max-w-full flex-col gap-2 overflow-hidden rounded-lg bg-muted px-4 py-3 text-sm text-foreground'
                        : 'flex w-fit min-w-0 max-w-full flex-col gap-2 overflow-hidden text-sm text-foreground'
                }
            >
                {message.parts.map((part, idx) => (
                    <MessagePart key={idx} part={part} idx={idx} />
                ))}
            </div>
        </div>
    );
}

function ChatContent({
    conversationId,
    conversationTitle,
    initialMessages,
}: Props) {
    const api = useMemo(
        () => `/api/chat?conversationId=${encodeURIComponent(conversationId)}`,
        [conversationId],
    );

    const normalizedInitial: UIMessage[] = useMemo(
        () =>
            initialMessages.map((m) => ({
                id: String(m.id),
                role: m.role as UIMessage['role'],
                parts: [{ type: 'text' as const, text: String(m.content) }],
            })),
        [initialMessages],
    );

    const transport = useMemo(
        () => new DefaultChatTransport({ api, credentials: 'include' }),
        [api],
    );

    const { messages, sendMessage, status, error, clearError, stop } = useChat({
        id: conversationId,
        messages: normalizedInitial,
        transport,
    });

    const scrollRef = useRef<HTMLDivElement | null>(null);
    const inputContainerRef = useRef<HTMLDivElement | null>(null);
    const isLoading = status === 'submitted' || status === 'streaming';
    const controller = usePromptInputController();

    useEffect(() => {
        scrollRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [messages.length, isLoading]);

    const breadcrumbs: BreadcrumbItem[] = [
        { title: 'Chat', href: '/chat' },
        {
            title: conversationTitle || 'New conversation',
            href: `/chat/${conversationId}`,
        },
    ];

    return (
        <AppLayout breadcrumbs={breadcrumbs}>
            <Head title={conversationTitle || 'Chat'} />

            <div className="flex h-full flex-1 flex-col gap-4 overflow-hidden rounded-xl p-4">
                {error && (
                    <Alert variant="destructive" className="shrink-0">
                        <AlertTitle>Chat failed</AlertTitle>
                        <AlertDescription>
                            <p>{error.message}</p>
                            <Button
                                type="button"
                                variant="secondary"
                                size="sm"
                                className="mt-2"
                                onClick={() => clearError()}
                            >
                                Dismiss
                            </Button>
                        </AlertDescription>
                    </Alert>
                )}

                <div className="relative mx-auto flex w-full max-w-[768px] min-h-0 flex-1 flex-col overflow-y-hidden" role="log">
                    <ScrollArea className="min-h-0 flex-1 overflow-hidden">
                        <div className="flex flex-col gap-8 p-4">
                            {messages.length === 0 ? (
                                <p className="py-8 text-center text-sm text-muted-foreground">
                                    Send a message to start.
                                </p>
                            ) : (
                                messages.map((m) => (
                                    <MessageBubble key={m.id} message={m} />
                                ))
                            )}
                            <div ref={scrollRef} />
                        </div>
                    </ScrollArea>
                </div>

                <div ref={inputContainerRef} className="mx-auto w-full max-w-[768px] shrink-0">
                <PromptInput
                    className="w-full"
                    onSubmit={async ({ text }) => {
                        const trimmed = text?.trim();
                        if (!trimmed || isLoading) return;
                        await sendMessage({ text: trimmed });
                        setTimeout(() => {
                            inputContainerRef.current?.querySelector('textarea')?.focus();
                        }, 0);
                    }}
                >
                    <PromptInputBody>
                        <PromptInputTextarea
                            placeholder="Type a message…"
                            disabled={isLoading}
                            aria-label="Message"
                            autoFocus={initialMessages.length === 0}
                        />
                    </PromptInputBody>
                    <PromptInputFooter className="justify-end">
                        <PromptInputSubmit
                            status={status}
                            onStop={stop}
                            disabled={
                                !controller.textInput.value.trim() &&
                                status !== 'submitted' &&
                                status !== 'streaming'
                            }
                        />
                    </PromptInputFooter>
                </PromptInput>
                </div>
            </div>
        </AppLayout>
    );
}

export default function Chat(props: Props) {
    return (
        <PromptInputProvider>
            <ChatContent {...props} />
        </PromptInputProvider>
    );
}
