import { Link, usePage } from '@inertiajs/react';
import { LogIn, Plus, Rss, Zap } from 'lucide-react';
import { useCallback, useEffect } from 'react';
import type { ReactNode } from 'react';
import { ChatWalletSnapshot } from '@/components/l402/chat-wallet-snapshot';
import { NavUser } from '@/components/nav-user';
import {
    Sidebar,
    SidebarContent,
    SidebarFooter,
    SidebarGroup,
    SidebarHeader,
    SidebarInset,
    SidebarMenu,
    SidebarMenuButton,
    SidebarMenuItem,
    SidebarProvider,
    SidebarTrigger,
} from '@/components/ui/sidebar';

type Props = { children: ReactNode };
type SharedProps = {
    auth?: {
        user?: unknown;
    };
};

export function GlobalSidebarLayout({ children }: Props) {
    const page = usePage<SharedProps>();
    const isAuthenticated = Boolean(page.props.auth?.user);
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
            <div className="fixed top-2 left-2 z-50 md:hidden">
                <SidebarTrigger className="h-8 w-8 rounded-md border border-border bg-background/80 backdrop-blur" />
            </div>

            <Sidebar
                collapsible="offcanvas"
                variant="inset"
                className="border-r border-border dark:border-input"
            >
                <SidebarHeader className="h-14 px-2">
                    <div className="flex items-center gap-2">
                        <SidebarTrigger className="hidden md:inline-flex" />
                        <Link
                            href="/"
                            className="text-sm font-medium tracking-wide text-foreground/90"
                        >
                            OpenAgents
                        </Link>
                    </div>
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
