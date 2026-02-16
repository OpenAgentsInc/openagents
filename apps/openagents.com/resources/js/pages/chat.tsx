import { useChat } from '@ai-sdk/react';
import { Head } from '@inertiajs/react';
import {
    DefaultChatTransport,
    type ToolUIPart,
    type UIMessage,
} from 'ai';
import { useCallback, useMemo, useRef } from 'react';
import {
    Conversation,
    ConversationContent,
    ConversationEmptyState,
    ConversationScrollButton,
} from '@/components/ai-elements/conversation';
import {
    Message,
    MessageContent,
    MessageResponse,
} from '@/components/ai-elements/message';
import {
    PromptInput,
    PromptInputBody,
    PromptInputFooter,
    PromptInputProvider,
    PromptInputSubmit,
    PromptInputTextarea,
    usePromptInputController,
} from '@/components/ai-elements/prompt-input';
import {
    Tool,
    ToolContent,
    ToolHeader,
    ToolInput,
    ToolOutput,
    type ToolPart,
} from '@/components/ai-elements/tool';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import AppLayout from '@/layouts/app-layout';
import type { BreadcrumbItem } from '@/types';

type Props = {
    conversationId: string;
    conversationTitle: string;
    initialMessages: Array<{ id: string; role: string; content: string }>;
};

const TOOL_STATES: ReadonlyArray<ToolPart['state']> = [
    'approval-requested',
    'approval-responded',
    'input-available',
    'input-streaming',
    'output-available',
    'output-denied',
    'output-error',
];

function isKnownToolState(value: unknown): value is ToolPart['state'] {
    return typeof value === 'string' && TOOL_STATES.includes(value as ToolPart['state']);
}

function normalizeToolState(value: unknown): ToolPart['state'] {
    return isKnownToolState(value) ? value : 'output-available';
}

function toolNameFromPart(part: unknown): string {
    if (typeof part !== 'object' || part === null) return 'tool';

    const p = part as Record<string, unknown>;
    const type = p.type;

    if (type === 'dynamic-tool' && typeof p.toolName === 'string') {
        return p.toolName;
    }

    if (typeof type === 'string' && type.startsWith('tool-')) {
        return type.slice('tool-'.length);
    }

    return 'tool';
}

function parseL402Output(output: unknown): Record<string, unknown> | null {
    let normalized: unknown = output;

    if (typeof normalized === 'string') {
        try {
            normalized = JSON.parse(normalized);
        } catch {
            return null;
        }
    }

    if (typeof normalized !== 'object' || normalized === null) {
        return null;
    }

    return normalized as Record<string, unknown>;
}

function l402Status(output: unknown): string | null {
    const o = parseL402Output(output);
    return o && typeof o.status === 'string' ? o.status : null;
}

function l402TaskId(output: unknown): string | null {
    const o = parseL402Output(output);
    return o && typeof o.taskId === 'string' && o.taskId.trim() !== '' ? o.taskId : null;
}

function l402ToolStateFromOutput(output: unknown, fallback: ToolPart['state']): ToolPart['state'] {
    const status = l402Status(output);
    if (!status) return fallback;

    if (status === 'approval_requested') return 'approval-requested';
    if (status === 'blocked') return 'output-denied';
    if (status === 'failed') return 'output-error';

    return 'output-available';
}

function l402ToolSummary(output: unknown): string | null {
    const o = parseL402Output(output);
    if (!o) return null;

    const host = typeof o.host === 'string' ? o.host : 'unknown';
    const status = typeof o.status === 'string' ? o.status : null;
    const paid = o.paid === true;
    const cacheHit = o.cacheHit === true;
    const cacheStatus =
        typeof o.cacheStatus === 'string' ? o.cacheStatus : cacheHit ? 'hit' : null;
    const amountMsats = typeof o.amountMsats === 'number' ? o.amountMsats : null;
    const quotedAmountMsats = typeof o.quotedAmountMsats === 'number' ? o.quotedAmountMsats : null;
    const msats = amountMsats ?? quotedAmountMsats;
    const sats = typeof msats === 'number' ? Math.round(msats / 1000) : null;
    const proofReference =
        typeof o.proofReference === 'string' ? o.proofReference : null;
    const denyCode = typeof o.denyCode === 'string' ? o.denyCode : null;

    if (status === 'approval_requested') {
        return `L402 payment intent · ${host}${sats != null ? ` · ${sats} sats max` : ''}`;
    }

    if (status === 'blocked') {
        return `L402 blocked · ${host}${denyCode ? ` · ${denyCode}` : ''}`;
    }

    if (status === 'failed') {
        return `L402 failed · ${host}${denyCode ? ` · ${denyCode}` : ''}`;
    }

    if (status === 'cached' || cacheStatus === 'hit') {
        return `L402 cached · ${host}${sats != null ? ` · ${sats} sats` : ''}${proofReference ? ` · ${proofReference}` : ''}`;
    }

    if (paid) {
        return `L402 paid · ${host}${sats != null ? ` · ${sats} sats` : ''}${proofReference ? ` · ${proofReference}` : ''}`;
    }

    return `L402 fetch · ${host}`;
}

function renderToolPart(
    part: Record<string, unknown>,
    idx: number,
    onApproveTask?: (taskId: string) => void,
) {
    const toolName = toolNameFromPart(part);
    const toolType = typeof part.type === 'string' ? part.type : `tool-${toolName}`;
    const rawToolState = normalizeToolState(part.state);
    const summary =
        toolName === 'lightning_l402_fetch' || toolName === 'lightning_l402_approve'
            ? l402ToolSummary(part.output)
            : null;

    const toolState =
        toolName === 'lightning_l402_fetch' || toolName === 'lightning_l402_approve'
            ? l402ToolStateFromOutput(part.output, rawToolState)
            : rawToolState;

    const toolCallId =
        typeof part.toolCallId === 'string' ? part.toolCallId : undefined;

    const errorText =
        typeof part.errorText === 'string'
            ? part.errorText
            : typeof part.error === 'string'
              ? part.error
              : undefined;

    const defaultOpen =
        toolState === 'approval-requested' ||
        toolState === 'output-error' ||
        toolState === 'output-denied';

    const header =
        toolType === 'dynamic-tool' ? (
            <ToolHeader
                type="dynamic-tool"
                toolName={toolName}
                state={toolState}
                title={summary ?? undefined}
            />
        ) : (
            <ToolHeader
                type={toolType as ToolUIPart['type']}
                state={toolState}
                title={summary ?? undefined}
            />
        );

    const taskId = l402TaskId(part.output);
    const approvalRequested = toolState === 'approval-requested' && taskId !== null;

    return (
        <Tool key={idx} defaultOpen={defaultOpen}>
            {header}
            <ToolContent>
                {part.input !== undefined && (
                    <ToolInput input={part.input as ToolPart['input']} />
                )}
                <ToolOutput
                    output={part.output as ToolPart['output']}
                    errorText={errorText as ToolPart['errorText']}
                />
                {approvalRequested ? (
                    <div className="flex items-center gap-2">
                        <Button
                            type="button"
                            size="sm"
                            onClick={() => {
                                if (taskId && onApproveTask) onApproveTask(taskId);
                            }}
                        >
                            Approve payment
                        </Button>
                        <span className="text-xs text-muted-foreground">task: {taskId}</span>
                    </div>
                ) : null}
                {toolCallId ? (
                    <div className="font-mono text-[11px] text-muted-foreground">
                        toolCallId: {toolCallId}
                    </div>
                ) : null}
            </ToolContent>
        </Tool>
    );
}

function MessagePart({
    part,
    idx,
    onApproveTask,
}: {
    part: unknown;
    idx: number;
    onApproveTask?: (taskId: string) => void;
}) {
    if (typeof part !== 'object' || part === null) return null;

    const p = part as Record<string, unknown>;
    const type = p.type;

    if (type === 'text' || type === 'reasoning') {
        const text = typeof p.text === 'string' ? p.text : '';

        if (!text.trim()) return null;

        if (type === 'reasoning') {
            return (
                <div
                    key={idx}
                    className="border-l-2 border-muted-foreground/30 pl-2 text-sm italic text-muted-foreground"
                >
                    <MessageResponse>{text}</MessageResponse>
                </div>
            );
        }

        return <MessageResponse key={idx}>{text}</MessageResponse>;
    }

    if (type === 'dynamic-tool' || (typeof type === 'string' && type.startsWith('tool-'))) {
        return renderToolPart(p, idx, onApproveTask);
    }

    return null;
}

function MessageBubble({
    message,
    onApproveTask,
}: {
    message: UIMessage;
    onApproveTask?: (taskId: string) => void;
}) {
    return (
        <Message from={message.role}>
            <MessageContent>
                {message.parts.map((part, idx) => (
                    <MessagePart key={idx} part={part} idx={idx} onApproveTask={onApproveTask} />
                ))}
            </MessageContent>
        </Message>
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

    const inputContainerRef = useRef<HTMLDivElement | null>(null);
    const isLoading = status === 'submitted' || status === 'streaming';
    const controller = usePromptInputController();

    const focusInputSoon = useCallback(() => {
        setTimeout(() => {
            inputContainerRef.current
                ?.querySelector('textarea')
                ?.focus();
        }, 0);
    }, []);

    const handleApproveTask = useCallback(async (taskId: string) => {
        if (!taskId || isLoading) return;

        await sendMessage({ text: `lightning_l402_approve({"taskId":"${taskId}"})` });
        focusInputSoon();
    }, [focusInputSoon, isLoading, sendMessage]);

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

                <div className="relative mx-auto flex w-full max-w-[768px] min-h-0 flex-1 flex-col overflow-hidden">
                    <Conversation>
                        <ConversationContent>
                            {messages.length === 0 ? (
                                <ConversationEmptyState
                                    title="No messages yet"
                                    description="Send a message to start."
                                />
                            ) : (
                                messages.map((m) => (
                                    <MessageBubble key={m.id} message={m} onApproveTask={handleApproveTask} />
                                ))
                            )}
                        </ConversationContent>
                        <ConversationScrollButton />
                    </Conversation>
                </div>

                <div
                    ref={inputContainerRef}
                    className="mx-auto w-full max-w-[768px] shrink-0"
                >
                    <PromptInput
                        className="w-full"
                        onSubmit={async ({ text }) => {
                            const trimmed = text?.trim();
                            if (!trimmed || isLoading) return;

                            await sendMessage({ text: trimmed });
                            focusInputSoon();
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
