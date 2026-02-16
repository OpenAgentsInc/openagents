import { Link, usePage } from '@inertiajs/react';
import { BookOpen, CircleDollarSign, Github, Landmark, List, MessageSquare, Plus, Server, Shield, Wallet } from 'lucide-react';
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

const lightningFooterItems: NavItem[] = [
    {
        title: 'L402 Wallet',
        href: '/l402',
        icon: Wallet,
    },
    {
        title: 'L402 Transactions',
        href: '/l402/transactions',
        icon: List,
    },
    {
        title: 'L402 Paywalls',
        href: '/l402/paywalls',
        icon: Landmark,
    },
    {
        title: 'L402 Settlements',
        href: '/l402/settlements',
        icon: CircleDollarSign,
    },
    {
        title: 'L402 Deployments',
        href: '/l402/deployments',
        icon: Server,
    },
];

function toThreadLabel(value: string): string {
    const trimmed = value.trim();
    return trimmed.length > 0 ? trimmed : 'New conversation';
}

export function AppSidebar() {
    const { chatThreads = [], isAdmin = false } = usePage<SharedProps>().props;
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
                            <SidebarMenuButton asChild className="w-full justify-start font-medium" isActive={isCurrentUrl('/chat')}>
                                <Link href="/chat" prefetch className="gap-2">
                                    <Plus className="size-5 shrink-0" />
                                    <span>New chat</span>
                                </Link>
                            </SidebarMenuButton>
                        </SidebarMenuItem>
                        {isAdmin ? (
                            <SidebarMenuItem>
                                <SidebarMenuButton asChild className="w-full justify-start font-medium" isActive={isCurrentUrl('/admin')}>
                                    <Link href="/admin" prefetch className="gap-2">
                                        <Shield className="size-5 shrink-0" />
                                        <span>Admin</span>
                                    </Link>
                                </SidebarMenuButton>
                            </SidebarMenuItem>
                        ) : null}
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
                <SidebarGroup className="group-data-[collapsible=icon]:p-0">
                    <SidebarGroupLabel className="group-data-[collapsible=icon]:sr-only">L402</SidebarGroupLabel>
                    <SidebarMenu>
                        {lightningFooterItems.map((item) => (
                            <SidebarMenuItem key={item.title}>
                                <SidebarMenuButton asChild isActive={isCurrentUrl(item.href)}>
                                    <Link href={item.href} prefetch>
                                        {item.icon ? <item.icon className="h-4 w-4" /> : null}
                                        <span>{item.title}</span>
                                    </Link>
                                </SidebarMenuButton>
                            </SidebarMenuItem>
                        ))}
                    </SidebarMenu>
                </SidebarGroup>
                <NavFooter items={footerNavItems} className="mt-auto" />
                <NavUser />
            </SidebarFooter>
        </Sidebar>
    );
}
