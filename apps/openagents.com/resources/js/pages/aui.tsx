import {
    ActionBarPrimitive,
    AuiIf,
    AssistantRuntimeProvider,
    BranchPickerPrimitive,
    ComposerPrimitive,
    MessagePrimitive,
    ThreadPrimitive,
    useAuiState,
} from '@assistant-ui/react';
import {
    AssistantChatTransport,
    useChatRuntime,
} from '@assistant-ui/react-ai-sdk';
import { useEffect, useMemo, useRef, useState } from 'react';
import * as Avatar from '@radix-ui/react-avatar';
import {
    ArrowUpIcon,
    CheckIcon,
    ChevronLeftIcon,
    ChevronRightIcon,
    CopyIcon,
    Pencil1Icon,
    ReloadIcon,
} from '@radix-ui/react-icons';
import { Head, usePage } from '@inertiajs/react';
import type { FC } from 'react';
import { TooltipIconButton } from '@/components/aui/tooltip-icon-button';
import { cn } from '@/lib/utils';

function AuiRuntimeProvider({
    conversationId,
    children,
}: {
    conversationId: string;
    children: React.ReactNode;
}) {
    const transport = useMemo(
        () =>
            new AssistantChatTransport({
                api: `/api/chat?conversationId=${encodeURIComponent(conversationId)}`,
                credentials: 'include',
            }),
        [conversationId],
    );
    const runtime = useChatRuntime({ transport });
    return (
        <AssistantRuntimeProvider runtime={runtime}>
            {children}
        </AssistantRuntimeProvider>
    );
}

const ChatGPT: FC = () => {
    const inputRef = useRef<HTMLTextAreaElement>(null);
    const isRunning = useAuiState((s) => s.thread.isRunning);
    const wasRunningRef = useRef(false);

    // Focus on first page load (once thread is mounted)
    useEffect(() => {
        const t = setTimeout(() => inputRef.current?.focus(), 100);
        return () => clearTimeout(t);
    }, []);

    // Focus after every message user sends (when run finishes)
    useEffect(() => {
        if (wasRunningRef.current && !isRunning) {
            inputRef.current?.focus();
        }
        wasRunningRef.current = isRunning;
    }, [isRunning]);

    return (
        <ThreadPrimitive.Root className="dark flex h-full flex-col items-stretch bg-[#212121] px-4 text-foreground">
            <ThreadPrimitive.Viewport className="flex grow flex-col gap-8 overflow-y-scroll pt-16">
                <ThreadPrimitive.Empty>
                    <div className="flex grow flex-col items-center justify-center">
                        <Avatar.Root className="flex h-12 w-12 items-center justify-center rounded-3xl border border-white/15 shadow">
                            <Avatar.AvatarFallback>C</Avatar.AvatarFallback>
                        </Avatar.Root>
                        <p className="mt-4 text-white text-xl">
                            How can I help you today?
                        </p>
                    </div>
                </ThreadPrimitive.Empty>

                <ThreadPrimitive.Messages
                    components={{
                        UserMessage,
                        EditComposer,
                        AssistantMessage,
                    }}
                />
            </ThreadPrimitive.Viewport>

            <ComposerPrimitive.Root className="mx-auto mb-2 flex w-full max-w-3xl items-end rounded-3xl bg-white/5 pl-2">
                <ComposerPrimitive.Input
                    ref={inputRef}
                    placeholder="Message Autopilot"
                    className="h-12 max-h-40 grow resize-none bg-transparent p-3.5 text-sm text-white outline-none placeholder:text-white/50"
                />
                <AuiIf condition={({ thread }) => !thread.isRunning}>
                    <ComposerPrimitive.Send className="m-2 flex size-8 items-center justify-center rounded-full bg-white transition-opacity disabled:opacity-10">
                        <ArrowUpIcon className="size-5 text-black [&_path]:stroke-1 [&_path]:stroke-black" />
                    </ComposerPrimitive.Send>
                </AuiIf>
                <AuiIf condition={({ thread }) => thread.isRunning}>
                    <ComposerPrimitive.Cancel className="m-2 flex size-8 items-center justify-center rounded-full bg-white">
                        <div className="size-2.5 bg-black" />
                    </ComposerPrimitive.Cancel>
                </AuiIf>
            </ComposerPrimitive.Root>
        </ThreadPrimitive.Root>
    );
};

const UserMessage: FC = () => {
    return (
        <MessagePrimitive.Root className="relative mx-auto flex w-full max-w-3xl flex-col items-end gap-1">
            <div className="flex items-start gap-4">
                <ActionBarPrimitive.Root
                    hideWhenRunning
                    autohide="not-last"
                    autohideFloat="single-branch"
                    className="mt-2"
                >
                    <ActionBarPrimitive.Edit asChild>
                        <TooltipIconButton
                            tooltip="Edit"
                            className="text-[#b4b4b4]"
                        >
                            <Pencil1Icon />
                        </TooltipIconButton>
                    </ActionBarPrimitive.Edit>
                </ActionBarPrimitive.Root>

                <div className="rounded-3xl bg-white/5 px-5 py-2 text-[#eee]">
                    <MessagePrimitive.Parts />
                </div>
            </div>

            <BranchPicker className="mt-2 mr-3" />
        </MessagePrimitive.Root>
    );
};

const EditComposer: FC = () => {
    return (
        <ComposerPrimitive.Root className="mx-auto flex w-full max-w-3xl flex-col justify-end gap-1 rounded-3xl bg-white/15">
            <ComposerPrimitive.Input className="flex h-8 w-full resize-none bg-transparent p-5 pb-0 text-white outline-none" />

            <div className="m-3 mt-2 flex items-center justify-center gap-2 self-end">
                <ComposerPrimitive.Cancel className="rounded-full bg-zinc-900 px-3 py-2 font-semibold text-sm text-white hover:bg-zinc-800">
                    Cancel
                </ComposerPrimitive.Cancel>
                <ComposerPrimitive.Send className="rounded-full bg-white px-3 py-2 font-semibold text-black text-sm hover:bg-white/90">
                    Send
                </ComposerPrimitive.Send>
            </div>
        </ComposerPrimitive.Root>
    );
};

const AssistantMessage: FC = () => {
    return (
        <MessagePrimitive.Root className="relative mx-auto flex w-full max-w-3xl gap-3">
            <Avatar.Root className="flex size-8 shrink-0 items-center justify-center rounded-3xl border border-white/15 shadow">
                <Avatar.AvatarFallback className="text-white text-xs">
                    C
                </Avatar.AvatarFallback>
            </Avatar.Root>

            <div className="pt-1">
                <div className="text-[#eee]">
                    <MessagePrimitive.Parts />
                </div>

                <div className="flex pt-2">
                    <BranchPicker />

                    <ActionBarPrimitive.Root
                        hideWhenRunning
                        autohide="not-last"
                        autohideFloat="single-branch"
                        className="flex items-center gap-1 rounded-lg data-floating:absolute data-floating:border-2 data-floating:p-1"
                    >
                        <ActionBarPrimitive.Reload asChild>
                            <TooltipIconButton
                                tooltip="Reload"
                                className="text-[#b4b4b4]"
                            >
                                <ReloadIcon />
                            </TooltipIconButton>
                        </ActionBarPrimitive.Reload>
                        <ActionBarPrimitive.Copy asChild>
                            <TooltipIconButton
                                tooltip="Copy"
                                className="text-[#b4b4b4]"
                            >
                                <AuiIf condition={({ message }) => message.isCopied}>
                                    <CheckIcon />
                                </AuiIf>
                                <AuiIf condition={({ message }) => !message.isCopied}>
                                    <CopyIcon />
                                </AuiIf>
                            </TooltipIconButton>
                        </ActionBarPrimitive.Copy>
                    </ActionBarPrimitive.Root>
                </div>
            </div>
        </MessagePrimitive.Root>
    );
};

const BranchPicker: FC<{ className?: string }> = ({ className }) => {
    return (
        <BranchPickerPrimitive.Root
            hideWhenSingleBranch
            className={cn(
                'inline-flex items-center font-semibold text-[#b4b4b4] text-sm',
                className,
            )}
        >
            <BranchPickerPrimitive.Previous asChild>
                <TooltipIconButton
                    tooltip="Previous"
                    className="text-[#b4b4b4]"
                >
                    <ChevronLeftIcon />
                </TooltipIconButton>
            </BranchPickerPrimitive.Previous>
            <BranchPickerPrimitive.Number />/
            <BranchPickerPrimitive.Count />
            <BranchPickerPrimitive.Next asChild>
                <TooltipIconButton
                    tooltip="Next"
                    className="text-[#b4b4b4]"
                >
                    <ChevronRightIcon />
                </TooltipIconButton>
            </BranchPickerPrimitive.Next>
        </BranchPickerPrimitive.Root>
    );
};

export default function AuiPage() {
    const { props } = usePage<{ csrfToken?: string }>();
    const [conversationId, setConversationId] = useState<string | null>(null);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        let cancelled = false;
        setError(null);
        const headers: Record<string, string> = {
            'Content-Type': 'application/json',
            Accept: 'application/json',
        };
        if (props.csrfToken) {
            headers['X-CSRF-TOKEN'] = props.csrfToken;
        }
        fetch('/api/chats', {
            method: 'POST',
            headers,
            credentials: 'include',
            body: JSON.stringify({ title: 'AUI chat' }),
        })
            .then((res) => {
                if (cancelled) return;
                if (!res.ok) {
                    if (res.status === 401) setError('Sign in to chat');
                    else setError('Could not start chat');
                    return;
                }
                return res.json();
            })
            .then((data) => {
                if (cancelled || !data?.data?.id) return;
                setConversationId(data.data.id);
            })
            .catch(() => {
                if (!cancelled) setError('Could not start chat');
            });
        return () => {
            cancelled = true;
        };
    }, []);

    return (
        <>
            <Head title="Assistant UI – ChatGPT clone" />
            <div className="fixed inset-0 flex h-screen flex-col bg-[#212121]">
                {error && (
                    <div className="flex items-center justify-center gap-2 border-b border-white/10 bg-white/5 px-4 py-2 text-sm text-white">
                        {error}
                        {error === 'Sign in to chat' && (
                            <a href="/login" className="underline">
                                Log in
                            </a>
                        )}
                    </div>
                )}
                {!conversationId && !error && (
                    <div className="flex flex-1 items-center justify-center text-white/70">
                        Preparing chat…
                    </div>
                )}
                {conversationId && (
                    <AuiRuntimeProvider conversationId={conversationId}>
                        <ChatGPT />
                    </AuiRuntimeProvider>
                )}
            </div>
        </>
    );
}
