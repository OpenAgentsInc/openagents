import { Link, router, usePage } from '@inertiajs/react';
import { LogIn, MessageSquare, Plus, Rss, Zap } from 'lucide-react';
import { useCallback, useEffect } from 'react';
import type { ReactNode } from 'react';
import { ChatWalletSnapshot } from '@/components/l402/chat-wallet-snapshot';
import { NavUser } from '@/components/nav-user';
import {
    Sidebar,
    SidebarContent,
    SidebarFooter,
    SidebarGroup,
    SidebarGroupLabel,
    SidebarHeader,
    SidebarInset,
    SidebarMenu,
    SidebarMenuButton,
    SidebarMenuItem,
    SidebarProvider,
    SidebarTrigger,
    useSidebar,
} from '@/components/ui/sidebar';

type Props = { children: ReactNode };

type SharedChatThread = {
    id: string;
    title: string;
    updatedAt: string | null;
};

type SharedProps = {
    auth?: {
        user?: unknown;
    };
    chatThreads?: SharedChatThread[];
};

function toThreadLabel(value: string): string {
    const trimmed = value.trim();
    return trimmed.length > 0 ? trimmed : 'New conversation';
}

function SidebarCloseOnNavigate() {
    const { isMobile, setOpenMobile } = useSidebar();
    useEffect(() => {
        const cleanup = router.on('navigate', () => {
            if (isMobile) setOpenMobile(false);
        });
        return cleanup;
    }, [isMobile, setOpenMobile]);
    return null;
}

export function GlobalSidebarLayout({ children }: Props) {
    const page = usePage<SharedProps>();
    const isAuthenticated = Boolean(page.props.auth?.user);
    const chatThreads = page.props.chatThreads ?? [];
    const refreshKey = page.url ?? '/';

    const handleNewChat = useCallback(() => {
        if (window.location.pathname !== '/') {
            window.location.assign('/');
            return;
        }

        window.dispatchEvent(new Event('openagents:new-chat'));
    }, []);

    useEffect(() => {
        const prevHtml = document.documentElement.style.overflow;
        const prevBody = document.body.style.overflow;
        document.documentElement.style.overflow = 'hidden';
        document.body.style.overflow = 'hidden';
        return () => {
            document.documentElement.style.overflow = prevHtml;
            document.body.style.overflow = prevBody;
        };
    }, []);

    return (
        <SidebarProvider>
            <SidebarCloseOnNavigate />
            <div className="fixed left-3 top-3 z-[70]">
                <SidebarTrigger className="h-8 w-8 rounded-md border border-border bg-background/80 backdrop-blur" />
            </div>

            <Sidebar
                collapsible="offcanvas"
                variant="inset"
                className="border-r border-border dark:border-input"
            >
                <SidebarHeader className="flex h-14 justify-center px-2 pt-5">
                    <Link
                        href="/"
                        className="text-sm font-medium tracking-wide text-foreground/90"
                    >
                        OpenAgents
                    </Link>
                </SidebarHeader>

                <SidebarContent>
                    <SidebarGroup>
                        <SidebarMenu>
                            <SidebarMenuItem>
                                <SidebarMenuButton
                                    type="button"
                                    onClick={handleNewChat}
                                    className="w-full justify-start gap-2"
                                >
                                    <Plus className="size-4" />
                                    <span>New Chat</span>
                                </SidebarMenuButton>
                            </SidebarMenuItem>
                            <SidebarMenuItem>
                                <SidebarMenuButton asChild className="w-full justify-start gap-2">
                                    <Link href="/feed">
                                        <Rss className="size-4" />
                                        <span>Global Feed</span>
                                    </Link>
                                </SidebarMenuButton>
                            </SidebarMenuItem>
                            {isAuthenticated ? (
                                <SidebarMenuItem>
                                    <SidebarMenuButton asChild className="w-full justify-start gap-2">
                                        <Link href="/l402">
                                            <Zap className="size-4" />
                                            <span>Lightning</span>
                                        </Link>
                                    </SidebarMenuButton>
                                </SidebarMenuItem>
                            ) : null}
                        </SidebarMenu>
                    </SidebarGroup>

                    {isAuthenticated ? (
                        <SidebarGroup>
                            <SidebarGroupLabel>Recent chats</SidebarGroupLabel>
                            <SidebarMenu>
                                {chatThreads.length === 0 ? (
                                    <SidebarMenuItem>
                                        <div className="px-2 py-1 text-xs text-muted-foreground">
                                            No chats yet.
                                        </div>
                                    </SidebarMenuItem>
                                ) : (
                                    chatThreads.map((thread) => (
                                        <SidebarMenuItem key={thread.id}>
                                            <SidebarMenuButton asChild className="w-full justify-start gap-2">
                                                <Link href={`/chat/${thread.id}`}>
                                                    <MessageSquare className="size-4" />
                                                    <span className="truncate">{toThreadLabel(thread.title)}</span>
                                                </Link>
                                            </SidebarMenuButton>
                                        </SidebarMenuItem>
                                    ))
                                )}
                            </SidebarMenu>
                        </SidebarGroup>
                    ) : null}
                </SidebarContent>

                <SidebarFooter>
                    {isAuthenticated ? (
                        <>
                            <SidebarGroup className="px-2 pt-0">
                                <ChatWalletSnapshot
                                    refreshKey={refreshKey}
                                    variant="sidebar"
                                />
                            </SidebarGroup>
                            <NavUser />
                        </>
                    ) : (
                        <SidebarMenu>
                            <SidebarMenuItem>
                                <SidebarMenuButton
                                    asChild
                                    size="lg"
                                    className="w-full justify-start gap-3"
                                >
                                    <Link href="/login">
                                        <LogIn className="size-4" />
                                        <span>Login</span>
                                    </Link>
                                </SidebarMenuButton>
                            </SidebarMenuItem>
                        </SidebarMenu>
                    )}
                </SidebarFooter>
            </Sidebar>
            <SidebarInset>{children}</SidebarInset>
        </SidebarProvider>
    );
}
