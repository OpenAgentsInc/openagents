import { Link, usePage } from '@inertiajs/react';
import { BookOpen, Github, MessageSquare, Plus, Shield, Zap } from 'lucide-react';
import { ChatWalletSnapshot } from '@/components/l402/chat-wallet-snapshot';
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
    isAdmin?: boolean;
};

const baseFooterNavItems: NavItem[] = [
    {
        title: 'Repository',
        href: 'https://github.com/OpenAgentsInc/openagents',
        icon: Github,
        external: true,
    },
    {
        title: 'Documentation',
        href: 'https://docs.openagents.com',
        icon: BookOpen,
        external: true,
    },
];

function toThreadLabel(value: string): string {
    const trimmed = value.trim();
    return trimmed.length > 0 ? trimmed : 'New conversation';
}

export function AppSidebar() {
    const page = usePage<SharedProps>();
    const { chatThreads = [], isAdmin = false } = page.props;
    const { currentUrl, isCurrentUrl } = useCurrentUrl();

    const footerNavItems: NavItem[] = isAdmin
        ? [
            ...baseFooterNavItems,
            {
                title: 'Admin',
                href: '/admin',
                icon: Shield,
            },
        ]
        : baseFooterNavItems;

    const isLightningActive = currentUrl === '/l402' || currentUrl.startsWith('/l402/');

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
                            <SidebarMenuButton asChild className="w-full justify-start font-medium" isActive={isCurrentUrl('/chat')}>
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
                <SidebarGroup className="px-2 pb-2 pt-0 group-data-[collapsible=icon]:hidden">
                    <ChatWalletSnapshot refreshKey={currentUrl} variant="sidebar" />
                </SidebarGroup>

                <SidebarGroup className="group-data-[collapsible=icon]:p-0">
                    <SidebarGroupLabel className="group-data-[collapsible=icon]:sr-only">Lightning</SidebarGroupLabel>
                    <SidebarMenu>
                        <SidebarMenuItem>
                            <SidebarMenuButton asChild isActive={isLightningActive}>
                                <Link href="/l402" prefetch>
                                    <Zap className="h-4 w-4" />
                                    <span>Lightning</span>
                                </Link>
                            </SidebarMenuButton>
                        </SidebarMenuItem>
                    </SidebarMenu>
                </SidebarGroup>

                <NavFooter items={footerNavItems} className="mt-auto" />
                <NavUser />
            </SidebarFooter>
        </Sidebar>
    );
}
