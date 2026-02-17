import { useChat } from '@ai-sdk/react';
import { Head, usePage } from '@inertiajs/react';
import { DefaultChatTransport, type ToolUIPart, type UIMessage } from 'ai';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
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
import { usePostHogEvent } from '@/hooks/use-posthog-event';
import type { BreadcrumbItem } from '@/types';

type GuestOnboardingStep = 'email' | 'code';

type GuestOnboarding = {
    enabled: boolean;
    step: GuestOnboardingStep | null;
    pendingEmail: string | null;
};

type Props = {
    conversationId: string;
    conversationTitle: string;
    initialMessages: Array<{ id: string; role: string; content: string }>;
    guestOnboarding?: GuestOnboarding;
};

type SharedPageProps = {
    csrfToken?: string;
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
    const type = p.type;

    if (type === 'dynamic-tool' && typeof p.toolName === 'string') {
        return p.toolName;
    }

    if (typeof type === 'string' && type.startsWith('tool-')) {
        return type.slice('tool-'.length);
    }

    return 'tool';
}

function isL402ToolName(name: string): boolean {
    return name === 'lightning_l402_fetch' || name === 'lightning_l402_approve';
}

function compactProofReference(value: string | null): string | null {
    if (!value) return null;
    if (value.length <= 28) return value;

    return value.slice(0, 24) + '...';
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
    return o && typeof o.taskId === 'string' && o.taskId.trim() !== ''
        ? o.taskId
        : null;
}

function l402ToolStateFromOutput(
    output: unknown,
    fallback: ToolPart['state'],
): ToolPart['state'] {
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
        typeof o.cacheStatus === 'string'
            ? o.cacheStatus
            : cacheHit
              ? 'hit'
              : null;
    const amountMsats =
        typeof o.amountMsats === 'number' ? o.amountMsats : null;
    const quotedAmountMsats =
        typeof o.quotedAmountMsats === 'number' ? o.quotedAmountMsats : null;
    const msats = amountMsats ?? quotedAmountMsats;
    const sats = typeof msats === 'number' ? Math.round(msats / 1000) : null;
    const proofReference = compactProofReference(
        typeof o.proofReference === 'string' ? o.proofReference : null,
    );
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
    const toolType =
        typeof part.type === 'string' ? part.type : `tool-${toolName}`;
    const rawToolState = normalizeToolState(part.state);
    const summary = isL402ToolName(toolName)
        ? l402ToolSummary(part.output)
        : null;

    const toolState = isL402ToolName(toolName)
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
    const approvalRequested =
        toolState === 'approval-requested' && taskId !== null;

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
                                if (taskId && onApproveTask)
                                    onApproveTask(taskId);
                            }}
                        >
                            Approve payment
                        </Button>
                        <span className="text-xs text-muted-foreground">
                            task: {taskId}
                        </span>
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
                    className="border-l-2 border-muted-foreground/30 pl-2 text-sm text-muted-foreground italic"
                >
                    <MessageResponse>{text}</MessageResponse>
                </div>
            );
        }

        return <MessageResponse key={idx}>{text}</MessageResponse>;
    }

    if (
        type === 'dynamic-tool' ||
        (typeof type === 'string' && type.startsWith('tool-'))
    ) {
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
                    <MessagePart
                        key={idx}
                        part={part}
                        idx={idx}
                        onApproveTask={onApproveTask}
                    />
                ))}
            </MessageContent>
        </Message>
    );
}

function looksLikeEmail(input: string): boolean {
    return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(input);
}

function isSixDigitCode(input: string): boolean {
    return /^\d{6}$/.test(input.replace(/\s+/g, ''));
}

function makeUiTextMessage(role: UIMessage['role'], text: string): UIMessage {
    const id =
        typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function'
            ? crypto.randomUUID()
            : `${Date.now()}-${Math.random().toString(16).slice(2)}`;

    return {
        id,
        role,
        parts: [{ type: 'text', text }],
    };
}

function extractApiError(payload: unknown): string | null {
    if (!payload || typeof payload !== 'object') return null;

    const data = payload as Record<string, unknown>;

    if (typeof data.message === 'string' && data.message.trim() !== '') {
        return data.message;
    }

    const errors = data.errors;
    if (!errors || typeof errors !== 'object') return null;

    for (const value of Object.values(errors as Record<string, unknown>)) {
        if (Array.isArray(value) && typeof value[0] === 'string') {
            return value[0];
        }
    }

    return null;
}

function hasVisibleAssistantContent(
    message: UIMessage | null | undefined,
): boolean {
    if (
        !message ||
        message.role !== 'assistant' ||
        !Array.isArray(message.parts)
    ) {
        return false;
    }

    return message.parts.some((part) => {
        if (!part || typeof part !== 'object') return false;

        const p = part as Record<string, unknown>;

        if (
            (p.type === 'text' || p.type === 'reasoning') &&
            typeof p.text === 'string'
        ) {
            return p.text.trim().length > 0;
        }

        if (
            typeof p.type === 'string' &&
            (p.type === 'dynamic-tool' || p.type.startsWith('tool-'))
        ) {
            return true;
        }

        return false;
    });
}

function ChatContent({
    conversationId,
    conversationTitle,
    initialMessages,
    guestOnboarding,
}: Props) {
    const page = usePage<SharedPageProps>();
    const capture = usePostHogEvent('chat');
    const csrfToken =
        typeof page.props.csrfToken === 'string' ? page.props.csrfToken : null;

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

    const guestEnabled = guestOnboarding?.enabled === true;
    const [guestMessages, setGuestMessages] =
        useState<UIMessage[]>(normalizedInitial);
    const [guestStep, setGuestStep] = useState<GuestOnboardingStep>(
        guestOnboarding?.step === 'code' ? 'code' : 'email',
    );
    const [guestBusy, setGuestBusy] = useState(false);
    const [guestError, setGuestError] = useState<string | null>(null);

    useEffect(() => {
        capture('chat.page_opened', {
            conversationId,
            guestOnboardingEnabled: guestEnabled,
            initialMessageCount: normalizedInitial.length,
        });
    }, [capture, conversationId, guestEnabled, normalizedInitial.length]);

    useEffect(() => {
        setGuestMessages(normalizedInitial);
        setGuestStep(guestOnboarding?.step === 'code' ? 'code' : 'email');
        setGuestBusy(false);
        setGuestError(null);
    }, [
        conversationId,
        guestOnboarding?.pendingEmail,
        guestOnboarding?.step,
        normalizedInitial,
    ]);

    const inputContainerRef = useRef<HTMLDivElement | null>(null);
    const isStreaming = status === 'submitted' || status === 'streaming';
    const isLoading = guestEnabled ? guestBusy : isStreaming;
    const controller = usePromptInputController();
    const lastStatusRef = useRef(status);

    useEffect(() => {
        const previousStatus = lastStatusRef.current;
        lastStatusRef.current = status;

        if (guestEnabled) return;

        const transitionedFromStreaming =
            previousStatus === 'submitted' || previousStatus === 'streaming';

        if (!transitionedFromStreaming || status !== 'ready') return;

        const lastAssistant =
            [...messages]
                .reverse()
                .find((message) => message.role === 'assistant') ?? null;

        if (!hasVisibleAssistantContent(lastAssistant)) {
            capture('chat.response_empty', {
                conversationId,
                messageCount: messages.length,
            });

            return;
        }

        capture('chat.response_completed', {
            conversationId,
            messageCount: messages.length,
        });
    }, [capture, conversationId, guestEnabled, messages, status]);

    const focusInputSoon = useCallback(() => {
        setTimeout(() => {
            inputContainerRef.current?.querySelector('textarea')?.focus();
        }, 0);
    }, []);

    const postJson = useCallback(
        async (url: string, payload: Record<string, unknown>) => {
            const headers: Record<string, string> = {
                'Content-Type': 'application/json',
                Accept: 'application/json',
                'X-Requested-With': 'XMLHttpRequest',
            };

            if (csrfToken) {
                headers['X-CSRF-TOKEN'] = csrfToken;
            }

            const response = await fetch(url, {
                method: 'POST',
                credentials: 'include',
                headers,
                body: JSON.stringify(payload),
            });

            let body: unknown = null;
            try {
                body = await response.json();
            } catch {
                body = null;
            }

            return { response, body };
        },
        [csrfToken],
    );

    const appendGuestMessage = useCallback((message: UIMessage) => {
        setGuestMessages((current) => [...current, message]);
    }, []);

    const handleGuestSubmit = useCallback(
        async (text: string) => {
            if (guestBusy) return;

            const trimmed = text.trim();
            if (!trimmed) return;

            capture('chat.message_submitted', {
                conversationId,
                source: 'guest',
                guestStep,
                characterCount: trimmed.length,
            });

            setGuestError(null);

            const userDisplayText =
                guestStep === 'code' && isSixDigitCode(trimmed)
                    ? '••••••'
                    : trimmed;

            appendGuestMessage(makeUiTextMessage('user', userDisplayText));

            if (guestStep === 'email') {
                if (!looksLikeEmail(trimmed)) {
                    capture('chat.guest_email_invalid', {
                        conversationId,
                        submittedValueLength: trimmed.length,
                    });
                    appendGuestMessage(
                        makeUiTextMessage(
                            'assistant',
                            'To finish setup, please enter a valid email address.',
                        ),
                    );
                    return;
                }

                setGuestBusy(true);

                try {
                    const email = trimmed.toLowerCase();
                    const { response, body } = await postJson(
                        '/api/auth/email',
                        { email },
                    );

                    if (!response.ok) {
                        capture('chat.guest_code_send_failed', {
                            conversationId,
                            status: response.status,
                        });
                        const errorText =
                            extractApiError(body) ??
                            'Unable to send code right now. Please try again.';
                        appendGuestMessage(
                            makeUiTextMessage('assistant', errorText),
                        );
                        return;
                    }

                    setGuestStep('code');
                    capture('chat.guest_code_sent', {
                        conversationId,
                        destinationDomain: email.split('@')[1] ?? null,
                    });
                    appendGuestMessage(
                        makeUiTextMessage(
                            'assistant',
                            `Check ${email}. Enter your 6-digit verification code to continue.`,
                        ),
                    );
                } catch {
                    capture('chat.guest_code_send_failed', {
                        conversationId,
                        status: null,
                    });
                    appendGuestMessage(
                        makeUiTextMessage(
                            'assistant',
                            'Unable to send code right now. Please try again.',
                        ),
                    );
                } finally {
                    setGuestBusy(false);
                }

                return;
            }

            if (
                trimmed.toLowerCase() === 'change email' ||
                trimmed.toLowerCase() === 'start over'
            ) {
                capture('chat.guest_email_reset', {
                    conversationId,
                });
                setGuestStep('email');
                appendGuestMessage(
                    makeUiTextMessage(
                        'assistant',
                        'No problem. Enter the email address you want to use.',
                    ),
                );
                return;
            }

            if (!isSixDigitCode(trimmed)) {
                capture('chat.guest_code_invalid', {
                    conversationId,
                    submittedValueLength: trimmed.length,
                });
                appendGuestMessage(
                    makeUiTextMessage(
                        'assistant',
                        'Please enter the 6-digit code from your email. You can also reply "change email".',
                    ),
                );
                return;
            }

            setGuestBusy(true);

            try {
                const code = trimmed.replace(/\s+/g, '');
                const { response, body } = await postJson('/api/auth/verify', {
                    code,
                });

                if (!response.ok) {
                    capture('chat.guest_code_verify_failed', {
                        conversationId,
                        status: response.status,
                    });
                    const errorText =
                        extractApiError(body) ??
                        'Verification failed. Request a new code and try again.';
                    appendGuestMessage(
                        makeUiTextMessage('assistant', errorText),
                    );
                    return;
                }

                capture('chat.guest_code_verified', {
                    conversationId,
                });
                appendGuestMessage(
                    makeUiTextMessage(
                        'assistant',
                        'Setup complete. You are signed in. What would you like to do first?',
                    ),
                );

                const redirectTo =
                    body &&
                    typeof body === 'object' &&
                    typeof (body as Record<string, unknown>).redirect ===
                        'string'
                        ? ((body as Record<string, unknown>).redirect as string)
                        : '/chat';

                window.setTimeout(() => {
                    window.location.assign(redirectTo);
                }, 250);
            } catch {
                capture('chat.guest_code_verify_failed', {
                    conversationId,
                    status: null,
                });
                appendGuestMessage(
                    makeUiTextMessage(
                        'assistant',
                        'Verification failed. Request a new code and try again.',
                    ),
                );
            } finally {
                setGuestBusy(false);
            }
        },
        [
            appendGuestMessage,
            capture,
            conversationId,
            guestBusy,
            guestStep,
            postJson,
        ],
    );

    const handleApproveTask = useCallback(
        async (taskId: string) => {
            if (!taskId || isLoading || guestEnabled) return;

            capture('chat.l402_approval_clicked', {
                conversationId,
                taskId,
            });

            await sendMessage({
                text: `lightning_l402_approve({"taskId":"${taskId}"})`,
            });
            focusInputSoon();
        },
        [
            capture,
            conversationId,
            focusInputSoon,
            guestEnabled,
            isLoading,
            sendMessage,
        ],
    );

    const breadcrumbs: BreadcrumbItem[] = [
        { title: 'Chat', href: '/chat' },
        {
            title: conversationTitle || 'New conversation',
            href: `/chat/${conversationId}`,
        },
    ];

    const activeMessages = guestEnabled ? guestMessages : messages;
    const activeStatus = guestEnabled
        ? guestBusy
            ? 'submitted'
            : 'ready'
        : status;
    const errorMessage = guestEnabled ? guestError : (error?.message ?? null);

    useEffect(() => {
        if (!errorMessage) return;

        capture('chat.error_shown', {
            conversationId,
            guestOnboardingEnabled: guestEnabled,
            errorMessage,
        });
    }, [capture, conversationId, errorMessage, guestEnabled]);

    return (
        <AppLayout breadcrumbs={breadcrumbs}>
            <Head title={conversationTitle || 'Chat'} />

            <div className="flex h-full flex-1 flex-col gap-4 overflow-hidden rounded-xl p-4">
                {errorMessage && (
                    <Alert variant="destructive" className="shrink-0">
                        <AlertTitle>Chat failed</AlertTitle>
                        <AlertDescription>
                            <p>{errorMessage}</p>
                            <Button
                                type="button"
                                variant="secondary"
                                size="sm"
                                className="mt-2"
                                onClick={() => {
                                    capture('chat.error_dismissed', {
                                        conversationId,
                                        guestOnboardingEnabled: guestEnabled,
                                    });

                                    if (guestEnabled) {
                                        setGuestError(null);
                                    } else {
                                        clearError();
                                    }
                                }}
                            >
                                Dismiss
                            </Button>
                        </AlertDescription>
                    </Alert>
                )}
                <div className="relative mx-auto flex min-h-0 w-full max-w-[768px] flex-1 flex-col overflow-hidden">
                    <Conversation>
                        <ConversationContent>
                            {activeMessages.length === 0 ? (
                                <ConversationEmptyState
                                    title="No messages yet"
                                    description="Send a message to start."
                                />
                            ) : (
                                activeMessages.map((m) => (
                                    <MessageBubble
                                        key={m.id}
                                        message={m}
                                        onApproveTask={handleApproveTask}
                                    />
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

                            if (guestEnabled) {
                                await handleGuestSubmit(trimmed);
                                focusInputSoon();
                                return;
                            }

                            capture('chat.message_submitted', {
                                conversationId,
                                source: 'authed',
                                characterCount: trimmed.length,
                            });

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
                                status={activeStatus}
                                onStop={guestEnabled ? undefined : stop}
                                disabled={
                                    !controller.textInput.value.trim() &&
                                    activeStatus !== 'submitted' &&
                                    activeStatus !== 'streaming'
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
