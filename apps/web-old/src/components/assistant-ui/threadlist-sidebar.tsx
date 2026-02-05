import * as React from 'react';
import { Link, useRouterState } from '@tanstack/react-router';
import { useAuth } from '@workos/authkit-tanstack-react-start/client';
import { useQuery } from 'convex/react';
import { BookOpen, ServerIcon, Shield } from 'lucide-react';
import { api } from '../../../convex/_generated/api';
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarRail,
  SidebarTrigger,
} from '@/components/ui/sidebar';
import { Avatar, AvatarFallback } from '@/components/ui/avatar';
import { Button } from '@/components/ui/button';
import { Skeleton } from '@/components/ui/skeleton';
import { cn } from '@/lib/utils';

/** Renders Lucide icon only after mount to avoid SSR/client hydration mismatch. */
function SidebarIcon({ children }: { children: React.ReactNode }) {
  const [mounted, setMounted] = React.useState(false);
  React.useEffect(() => setMounted(true), []);
  if (!mounted) {
    return <span className="size-4 shrink-0" aria-hidden />;
  }
  return <>{children}</>;
}

function useIsActive(path: string) {
  const { location } = useRouterState();
  return (
    location.pathname === path ||
    (path !== '/' && location.pathname.startsWith(path + '/'))
  );
}

const SITE_TITLE = 'OpenAgents';

function getInitials(
  name: string | null | undefined,
  email: string | null | undefined,
): string {
  const trimmed = (name ?? '').trim();
  if (trimmed) {
    const parts = trimmed.split(/\s+/);
    return parts
      .slice(0, 2)
      .map((p) => p[0].toUpperCase())
      .join('');
  }
  if (email) return email.slice(0, 2).toUpperCase();
  return '?';
}

function SidebarNavUser() {
  const { user, loading, signOut } = useAuth();

  if (loading) {
    return (
      <SidebarMenuItem>
        <div className="flex h-11 items-center gap-3 px-2">
          <Skeleton className="size-8 shrink-0 rounded-full" />
          <div className="flex flex-1 flex-col gap-1 group-data-[collapsible=icon]:hidden">
            <Skeleton className="h-3.5 w-20" />
          </div>
        </div>
      </SidebarMenuItem>
    );
  }

  if (!user) return null;

  const initials = getInitials(user.firstName, user.email);
  const displayName =
    [user.firstName, user.lastName].filter(Boolean).join(' ') ||
    user.email ||
    'Account';

  return (
    <SidebarMenuItem>
      <div className="flex h-11 w-full items-center gap-3 px-2 group-data-[collapsible=icon]:justify-center">
        <Avatar className="size-8 shrink-0">
          <AvatarFallback className="rounded-full border border-sidebar-border bg-sidebar text-sidebar-foreground text-xs font-medium">
            {initials}
          </AvatarFallback>
        </Avatar>
        <div className="flex min-w-0 flex-1 flex-col text-left leading-tight group-data-[collapsible=icon]:hidden">
          <span className="truncate text-sm font-medium">{displayName}</span>
          <Button
            variant="ghost"
            size="sm"
            className="h-auto w-fit p-0 text-xs text-sidebar-foreground/70 hover:text-sidebar-foreground"
            onClick={() => signOut()}
          >
            Sign out
          </Button>
        </div>
      </div>
    </SidebarMenuItem>
  );
}

function SidebarAdminLink() {
  const { user } = useAuth();
  const adminStatus = useQuery(api.admin.getAdminStatus);
  const fallbackAdmin = user?.email.toLowerCase() === 'chris@openagents.com';
  if (!adminStatus?.isAdmin && !fallbackAdmin) return null;

  return (
    <SidebarMenuItem>
      <SidebarMenuButton asChild>
        <Link to="/admin">
          <SidebarIcon>
            <Shield className="size-4" />
          </SidebarIcon>
          <span>Admin</span>
        </Link>
      </SidebarMenuButton>
    </SidebarMenuItem>
  );
}

export function ThreadListSidebar(
  props: React.ComponentProps<typeof Sidebar>,
) {
  const autopilotActive = useIsActive('/assistant') || useIsActive('/chat');

  return (
    <Sidebar collapsible="icon" {...props}>
      <SidebarHeader className="relative flex h-12 shrink-0 flex-row items-center gap-2 border-b border-sidebar-border px-3">
        <SidebarTrigger className="absolute left-2 z-50" />
        <Link
          to="/"
          className={cn(
            'flex h-8 flex-1 items-center justify-center text-md font-semibold text-foreground transition-opacity duration-200 ease-linear',
            'group-data-[collapsible=icon]:opacity-0 group-data-[collapsible=icon]:w-0 group-data-[collapsible=icon]:overflow-hidden',
          )}
        >
          {SITE_TITLE}
        </Link>
      </SidebarHeader>
      <div className="hidden flex-1 group-data-[collapsible=icon]:block" />
      <SidebarContent className="aui-sidebar-content px-2 py-3 group-data-[collapsible=icon]:hidden">
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton asChild isActive={autopilotActive}>
              <Link to="/assistant">
                <SidebarIcon>
                  <ServerIcon className="size-4" />
                </SidebarIcon>
                <span>Autopilot</span>
              </Link>
            </SidebarMenuButton>
          </SidebarMenuItem>
          <SidebarMenuItem>
            <SidebarMenuButton asChild isActive={useIsActive('/kb')}>
              <Link to="/kb">
                <SidebarIcon>
                  <BookOpen className="size-4" />
                </SidebarIcon>
                <span>Knowledge Base</span>
              </Link>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarContent>
      <SidebarRail />
      <SidebarFooter className="aui-sidebar-footer group-data-[collapsible=icon]:hidden">
        <SidebarMenu className="w-full">
          <SidebarAdminLink />
          <SidebarNavUser />
        </SidebarMenu>
      </SidebarFooter>
    </Sidebar>
  );
}

