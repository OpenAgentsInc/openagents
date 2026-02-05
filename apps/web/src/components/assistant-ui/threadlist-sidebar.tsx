import * as React from 'react';
import { Link, useNavigate, useRouterState } from '@tanstack/react-router';
import { useAuth } from '@workos/authkit-tanstack-react-start/client';
import { useMutation, useQuery } from 'convex/react';
import { BookOpen, MessageSquarePlus, Plus, ServerIcon, Shield } from 'lucide-react';
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

function useAssistantThreadId(): string | null {
  const { location } = useRouterState();
  const pathname = location.pathname;
  const chatMatch = pathname.match(/^\/chat\/([^/]+)$/);
  if (chatMatch && chatMatch[1] !== 'new') return chatMatch[1];
  const params = new URLSearchParams(location.search);
  return params.get('threadId');
}

const SITE_TITLE = 'OpenAgents';

type ThreadSummary = {
  _id: string;
  title: string;
  kind?: 'chat' | 'project' | 'liteclaw';
};

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

  // if (!user) {
  //   return (
  //     <SidebarMenuItem>
  //       <SidebarMenuButton size="lg" asChild className="h-11">
  //         <Link to="/login" search={{ redirect: '/' }}>
  //           <span className="group-data-[collapsible=icon]:hidden">Log in</span>
  //         </Link>
  //       </SidebarMenuButton>
  //     </SidebarMenuItem>
  //   );
  // }
  if (!user) return null;

  const initials = getInitials(user.firstName, user.email);
  const displayName =
    [user.firstName, user.lastName].filter(Boolean).join(' ') || user.email || 'Account';

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

function SidebarLiteClawSection() {
  const hatcheryActive = useIsActive('/hatchery');
  return (
    <SidebarMenu className="pt-2">
      <div className="px-2 pb-1">
        <span className="text-xs font-medium text-sidebar-foreground/70">LiteClaw</span>
      </div>
      <SidebarMenuItem>
        <SidebarMenuButton asChild isActive={hatcheryActive}>
          <Link to="/hatchery" search={{ focus: undefined }}>
            <SidebarIcon>
              <ServerIcon className="size-4" />
            </SidebarIcon>
            <span>Hatchery</span>
          </Link>
        </SidebarMenuButton>
      </SidebarMenuItem>
    </SidebarMenu>
  );
}

function SidebarChatsSection({ threads }: { threads?: Array<ThreadSummary> }) {
  const navigate = useNavigate();
  const chatActive = useIsActive('/chat') || useIsActive('/assistant');
  const activeThreadId = useAssistantThreadId();
  const chatThreads = React.useMemo(
    () => (threads ?? []).filter((t) => t.kind !== 'project'),
    [threads],
  );

  const handleNewChat = () => {
    navigate({ to: '/chat/$chatId', params: { chatId: 'new' } });
  };

  return (
    <SidebarMenu className="pt-2">
      <div className="flex items-center justify-between px-2 pb-1">
        <Link
          to="/assistant"
          className="text-xs font-medium text-sidebar-foreground/70 transition-colors hover:text-sidebar-foreground"
        >
          Chats
        </Link>
        <Button
          variant="ghost"
          size="sm"
          className="h-6 w-6 p-0 text-sidebar-foreground/70 hover:text-sidebar-foreground"
          onClick={handleNewChat}
          aria-label="New chat"
        >
          <MessageSquarePlus className="size-3.5" />
        </Button>
      </div>
      {chatThreads.length ? (
        chatThreads.slice(0, 10).map((t) => (
          <SidebarMenuItem key={t._id}>
            <SidebarMenuButton
              asChild
              isActive={chatActive && activeThreadId === t._id}
            >
              <Link to="/chat/$chatId" params={{ chatId: t._id }}>
                <span className="truncate text-sm">{t.title}</span>
              </Link>
            </SidebarMenuButton>
          </SidebarMenuItem>
        ))
      ) : null}
    </SidebarMenu>
  );
}

function SidebarProjectsSection({ threads }: { threads?: Array<ThreadSummary> }) {
  const navigate = useNavigate();
  const createThread = useMutation(api.threads.create);
  const activeThreadId = useAssistantThreadId();
  const chatActive = useIsActive('/chat') || useIsActive('/assistant');
  const projectThreads = React.useMemo(
    () => (threads ?? []).filter((t) => t.kind === 'project'),
    [threads],
  );

  const handleNewProject = () => {
    createThread({ title: 'New Project', kind: 'project' })
      .then((threadId: ThreadSummary['_id']) => {
        navigate({ to: '/chat/$chatId', params: { chatId: threadId } });
      })
      .catch((err: unknown) => {
        console.error('Failed to create project:', err);
      });
  };

  return (
    <SidebarMenu className="pt-2">
      <div className="flex items-center justify-between px-2 pb-1">
        <span className="text-xs font-medium text-sidebar-foreground/70">Projects</span>
        <Button
          variant="ghost"
          size="sm"
          className="h-6 w-6 p-0 text-sidebar-foreground/70 hover:text-sidebar-foreground"
          onClick={handleNewProject}
          aria-label="New project"
        >
          <Plus className="size-3.5" />
        </Button>
      </div>
      {projectThreads.length ? (
        projectThreads.slice(0, 10).map((t) => (
          <SidebarMenuItem key={t._id}>
            <SidebarMenuButton
              asChild
              isActive={chatActive && activeThreadId === t._id}
            >
              <Link to="/chat/$chatId" params={{ chatId: t._id }}>
                <span className="truncate text-sm">{t.title}</span>
              </Link>
            </SidebarMenuButton>
          </SidebarMenuItem>
        ))
      ) : (
        <SidebarMenuItem>
          <span className="px-2 text-xs text-sidebar-foreground/60">
            No projects yet
          </span>
        </SidebarMenuItem>
      )}
    </SidebarMenu>
  );
}

export function ThreadListSidebar(
  props: React.ComponentProps<typeof Sidebar>,
) {
  const threads = useQuery(api.threads.list, { archived: false, limit: 50 });

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
            <SidebarMenuButton asChild isActive={useIsActive('/hatchery')}>
              <Link to="/hatchery" search={{ focus: undefined }}>
                <span className="text-base">🦞</span>
                <span>Hatchery</span>
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
        <SidebarChatsSection threads={threads} />
        <SidebarProjectsSection threads={threads} />
        <SidebarLiteClawSection />
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
