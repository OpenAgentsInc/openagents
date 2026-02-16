import { Link, usePage } from '@inertiajs/react';
import { BookOpen, Github, MessageSquare, Plus } from 'lucide-react';
import { NavFooter } from '@/components/nav-footer';
import { NavUser } from '@/components/nav-user';
import {
    Sidebar,
    SidebarContent,
    SidebarFooter,
    SidebarGroup,
    SidebarGroupLabel,
    SidebarHeader,
    SidebarMenu,
    SidebarMenuButton,
    SidebarMenuItem,
} from '@/components/ui/sidebar';
import { useCurrentUrl } from '@/hooks/use-current-url';
import type { NavItem } from '@/types';
import AppLogo from './app-logo';

type SharedChatThread = {
    id: string;
    title: string;
    updatedAt: string | null;
};

type SharedProps = {
    chatThreads?: SharedChatThread[];
};

const footerNavItems: NavItem[] = [
    {
        title: 'Repository',
        href: 'https://github.com/OpenAgentsInc/openagents',
        icon: Github,
    },
    {
        title: 'Documentation',
        href: 'https://docs.openagents.com',
        icon: BookOpen,
    },
];

function toThreadLabel(value: string): string {
    const trimmed = value.trim();
    return trimmed.length > 0 ? trimmed : 'New conversation';
}

export function AppSidebar() {
    const { chatThreads = [] } = usePage<SharedProps>().props;
    const { isCurrentUrl } = useCurrentUrl();

    return (
        <Sidebar collapsible="icon" variant="inset">
            <SidebarHeader className="flex h-14 shrink-0 flex-row items-center px-3">
                <SidebarMenu>
                    <SidebarMenuItem>
                        <SidebarMenuButton size="lg" asChild className="flex-1 justify-center group-data-[collapsible=icon]:justify-center">
                            <Link href="/chat" prefetch>
                                <AppLogo />
                            </Link>
                        </SidebarMenuButton>
                    </SidebarMenuItem>
                </SidebarMenu>
            </SidebarHeader>

            <SidebarContent className="group-data-[collapsible=icon]:overflow-hidden">
                <SidebarGroup className="mb-1">
                    <SidebarMenu>
                        <SidebarMenuItem>
                            <SidebarMenuButton asChild className="w-full justify-start font-medium">
                                <Link href="/chat" prefetch className="gap-2">
                                    <Plus className="size-5 shrink-0" />
                                    <span>New chat</span>
                                </Link>
                            </SidebarMenuButton>
                        </SidebarMenuItem>
                    </SidebarMenu>
                </SidebarGroup>

                <SidebarGroup className="px-2 py-0 group-data-[collapsible=icon]:hidden">
                    <SidebarGroupLabel>Recent chats</SidebarGroupLabel>
                    <SidebarMenu>
                        {chatThreads.length === 0 ? (
                            <SidebarMenuItem>
                                <div className="px-2 py-1 text-xs text-muted-foreground">No chats yet.</div>
                            </SidebarMenuItem>
                        ) : (
                            chatThreads.map((thread) => {
                                const href = `/chat/${thread.id}`;

                                return (
                                    <SidebarMenuItem key={thread.id}>
                                        <SidebarMenuButton
                                            asChild
                                            isActive={isCurrentUrl(href)}
                                            tooltip={{ children: toThreadLabel(thread.title) }}
                                        >
                                            <Link href={href} prefetch>
                                                <MessageSquare className="size-4" />
                                                <span className="truncate">{toThreadLabel(thread.title)}</span>
                                            </Link>
                                        </SidebarMenuButton>
                                    </SidebarMenuItem>
                                );
                            })
                        )}
                    </SidebarMenu>
                </SidebarGroup>
            </SidebarContent>

            <SidebarFooter>
                <NavFooter items={footerNavItems} className="mt-auto" />
                <NavUser />
            </SidebarFooter>
        </Sidebar>
    );
}
