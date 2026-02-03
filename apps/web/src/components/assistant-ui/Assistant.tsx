import { useState } from 'react';
import { AssistantRuntimeProvider } from '@assistant-ui/react';
import {
  useChatRuntime,
  AssistantChatTransport,
} from '@assistant-ui/react-ai-sdk';
import { Thread } from '@/components/assistant-ui/thread';
import {
  SidebarInset,
  SidebarProvider,
  SidebarTrigger,
} from '@/components/ui/sidebar';
import { ThreadListSidebar } from '@/components/assistant-ui/threadlist-sidebar';
import { RightSidebar, RightSidebarTriggerPortal } from '@/components/assistant-ui/right-sidebar';

/** When false, only render runtime provider + Thread (no sidebars). Used by _app layout. */
export function Assistant({ layout = true }: { layout?: boolean }) {
  const [rightTriggerContainer, setRightTriggerContainer] =
    useState<HTMLElement | null>(null);
  const runtime = useChatRuntime({
    transport: new AssistantChatTransport({
      api: '/api/chat',
    }),
  });

  if (!layout) {
    return (
      <AssistantRuntimeProvider runtime={runtime}>
        <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
          <Thread />
        </div>
      </AssistantRuntimeProvider>
    );
  }

  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <SidebarProvider className="h-dvh max-h-dvh min-h-0 overflow-hidden">
        <div className="flex h-full min-h-0 w-full flex-1">
          <ThreadListSidebar />
          <SidebarInset className="flex min-h-0 flex-1 flex-col overflow-hidden">
            <header className="sticky top-0 z-10 flex h-12 shrink-0 items-center gap-2 border-b border-border bg-background/80 px-3 md:px-4 backdrop-blur-md supports-[backdrop-filter]:bg-background/70">
              <SidebarTrigger className="md:hidden" />
              <div
                ref={(el) => setRightTriggerContainer(el ?? null)}
                className="ml-auto flex md:hidden"
                aria-hidden
              />
            </header>
            <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
              <Thread />
            </div>
          </SidebarInset>
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
        </div>
      </SidebarProvider>
    </AssistantRuntimeProvider>
  );
}
