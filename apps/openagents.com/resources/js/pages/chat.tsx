import { useChat } from '@ai-sdk/react';
import { Head } from '@inertiajs/react';
import { DefaultChatTransport, type UIMessage } from 'ai';
import { useEffect, useMemo, useRef, useState } from 'react';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import AppLayout from '@/layouts/app-layout';
import type { BreadcrumbItem } from '@/types';

type Props = {
    conversationId: string;
    conversationTitle: string;
    initialMessages: Array<{ id: string; role: string; content: string }>;
};

const breadcrumbs: BreadcrumbItem[] = [
    {
        title: 'Chat',
        href: '/chat',
    },
];

function messageToText(message: UIMessage): string {
    return message.parts
        .map((p) => {
            if (p.type === 'text') return p.text;
            if (p.type === 'reasoning') return p.text;
            return '';
        })
        .join('')
        .trim();
}

export default function Chat({ conversationId, conversationTitle, initialMessages }: Props) {
    const api = useMemo(() => `/api/chat?conversationId=${encodeURIComponent(conversationId)}`, [conversationId]);

    const normalizedInitial: UIMessage[] = useMemo(() => {
        return initialMessages.map((m) => ({
            id: String(m.id),
            role: m.role as UIMessage['role'],
            parts: [{ type: 'text' as const, text: String(m.content) }],
        }));
    }, [initialMessages]);

    const transport = useMemo(() => new DefaultChatTransport({ api }), [api]);

    const { messages, sendMessage, status, error, clearError } = useChat({
        id: conversationId,
        messages: normalizedInitial,
        transport,
    });

    const [input, setInput] = useState('');
    const isLoading = status === 'submitted' || status === 'streaming';

    const bottomRef = useRef<HTMLDivElement | null>(null);

    useEffect(() => {
        bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [messages.length, isLoading]);

    return (
        <AppLayout breadcrumbs={breadcrumbs}>
            <Head title={conversationTitle || 'Chat'} />

            <div className="flex h-full flex-1 flex-col gap-4 overflow-hidden rounded-xl p-4">
                <div className="flex items-center justify-between">
                    <div>
                        <div className="text-sm text-muted-foreground">Conversation</div>
                        <div className="font-medium">{conversationTitle || conversationId}</div>
                    </div>
                    <div className="text-xs text-muted-foreground">{isLoading ? 'Streaming…' : 'Ready'}</div>
                </div>

                {error ? (
                    <Alert variant="destructive">
                        <AlertTitle>Chat failed</AlertTitle>
                        <AlertDescription>
                            <p>{error.message}</p>
                            <div className="mt-2">
                                <Button type="button" variant="secondary" size="sm" onClick={() => clearError()}>
                                    Dismiss
                                </Button>
                            </div>
                        </AlertDescription>
                    </Alert>
                ) : null}

                <div className="flex flex-1 flex-col gap-3 overflow-y-auto rounded-lg border border-sidebar-border/70 p-3">
                    {messages.length === 0 ? (
                        <div className="text-sm text-muted-foreground">Send a message to start.</div>
                    ) : null}

                    {messages.map((m) => (
                        <div key={m.id} className="flex flex-col gap-1">
                            <div className="text-xs uppercase tracking-wide text-muted-foreground">{m.role}</div>
                            <div className="whitespace-pre-wrap rounded-md bg-muted/40 p-2 text-sm">{messageToText(m)}</div>
                        </div>
                    ))}

                    <div ref={bottomRef} />
                </div>

                <form
                    onSubmit={(e) => {
                        e.preventDefault();
                        const trimmed = input.trim();
                        if (!trimmed || isLoading) return;
                        void sendMessage({ text: trimmed });
                        setInput('');
                    }}
                    className="flex gap-2"
                >
                    <Input
                        value={input}
                        onChange={(e) => setInput(e.target.value)}
                        placeholder="Type a message…"
                        disabled={isLoading}
                        aria-label="Message"
                    />
                    <Button type="submit" disabled={isLoading || input.trim().length === 0}>
                        Send
                    </Button>
                </form>
            </div>
        </AppLayout>
    );
}
