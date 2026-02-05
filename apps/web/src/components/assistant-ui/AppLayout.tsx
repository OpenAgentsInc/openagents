import { useEffect, useRef, useState } from 'react';
import { Outlet, useRouterState } from '@tanstack/react-router';
import { posthogCapture } from '@/lib/posthog';
import { AssistantRuntimeProvider } from '@assistant-ui/react';
import { useMutation } from 'convex/react';
import { useAuth } from '@workos/authkit-tanstack-react-start/client';
import { api } from '../../../convex/_generated/api';
import {
  SidebarInset,
  SidebarProvider,
  SidebarTrigger,
} from '@/components/ui/sidebar';
import { ThreadListSidebar } from '@/components/assistant-ui/threadlist-sidebar';
import {
  RightSidebar,
  RightSidebarTriggerPortal,
} from '@/components/assistant-ui/right-sidebar';
import { AppBreadcrumb } from '@/components/assistant-ui/AppBreadcrumb';
import { useOpenAgentsChatRuntime } from '@/components/assistant-ui/openagents-chat-runtime';
import { CodexConnectDialog } from '@/components/assistant-ui/CodexConnectDialog';

/**
 * App chrome: left sidebar (thread list), center (header + Outlet), right sidebar (community).
 * Used by _app route layout. Wraps in AssistantRuntimeProvider so ThreadListSidebar (and index Thread) have AuiProvider.
 */
export function AppLayout() {
  const [rightTriggerContainer, setRightTriggerContainer] =
    useState<HTMLElement | null>(null);
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const location = useRouterState({ select: (s) => s.location });
  // Right sidebar (Feed/Communities) hidden – social code in packages/social
  const showRightSidebar = false;
  const showCodexConnect =
    pathname.startsWith('/chat') || pathname.startsWith('/assistant');
  const { user, loading } = useAuth();
  const ensureUser = useMutation(api.users.ensureUser);
  const ensuredUserId = useRef<string | null>(null);
  const runtime = useOpenAgentsChatRuntime();

  useEffect(() => {
    // Sync thread from URL: /chat/:chatId or /assistant?threadId=
    if (pathname.startsWith('/chat/')) {
      const match = pathname.match(/^\/chat\/([^/]+)$/);
      const chatId = match?.[1];
      if (chatId && chatId !== 'new') void runtime.switchToThread(chatId);
      return;
    }
    if (pathname.startsWith('/assistant')) {
      const params = new URLSearchParams(location.search ?? '');
      const threadId = params.get('threadId');
      if (threadId) void runtime.switchToThread(threadId);
    }
  }, [pathname, location.search, runtime]);

  useEffect(() => {
    posthogCapture('page_view', { path: pathname });
  }, [pathname]);

  useEffect(() => {
    if (loading) return;
    if (!user) {
      ensuredUserId.current = null;
      return;
    }
    if (ensuredUserId.current === user.id) return;
    ensuredUserId.current = user.id;
    void ensureUser({}).catch((err) => {
      console.error('Failed to ensure user:', err);
    });
  }, [user, loading, ensureUser]);

  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <SidebarProvider className="h-dvh max-h-dvh min-h-0 overflow-hidden">
        <div className="flex h-full min-h-0 w-full flex-1">
          <ThreadListSidebar />
          <SidebarInset className="flex min-h-0 flex-1 flex-col overflow-hidden">
            <header className="sticky top-0 z-10 flex h-12 shrink-0 items-center gap-2 border-b border-border bg-background/80 px-3 md:px-4 backdrop-blur-md supports-[backdrop-filter]:bg-background/70">
              <SidebarTrigger className="md:hidden" />
              <AppBreadcrumb />
              <div className="ml-auto flex items-center gap-2">
                {showCodexConnect && <CodexConnectDialog />}
                <div
                  ref={(el) => setRightTriggerContainer(el ?? null)}
                  className="flex md:hidden"
                  aria-hidden
                />
              </div>
            </header>
            <div className="flex min-h-0 flex-1 flex-col overflow-auto">
              <Outlet />
            </div>
          </SidebarInset>
          {showRightSidebar ? (
            <SidebarProvider
              cookieName="sidebar_right_state"
              className="w-auto shrink-0"
            >
              <RightSidebar />
              <RightSidebarTriggerPortal
                container={rightTriggerContainer}
                className="md:hidden"
              />
            </SidebarProvider>
          ) : null}
        </div>
      </SidebarProvider>
    </AssistantRuntimeProvider>
  );
}
