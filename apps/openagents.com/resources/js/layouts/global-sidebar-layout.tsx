import { Link, usePage } from '@inertiajs/react';
import { LogIn, Plus } from 'lucide-react';
import { useCallback, useEffect } from 'react';
import type { ReactNode } from 'react';
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
            <Sidebar
                collapsible="icon"
                variant="inset"
                className="border-r border-border dark:border-input"
            >
                <SidebarHeader className="h-14 px-2">
                    <div className="flex items-center gap-2">
                        <SidebarTrigger />
                        <Link
                            href="/"
                            className="text-sm font-medium tracking-wide text-foreground/90 group-data-[collapsible=icon]:hidden"
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
                        </SidebarMenu>
                    </SidebarGroup>
                </SidebarContent>

                <SidebarFooter>
                    {isAuthenticated ? (
                        <NavUser />
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
