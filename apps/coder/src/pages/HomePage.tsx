import React, { useState, useCallback } from "react";
import { usePersistentChat, Thread, UIMessage } from "@openagents/core";
import { MessageInput } from "@/components/ui/message-input";
import { MessageList } from "@/components/ui/message-list";
import { Chat, ChatForm } from "@/components/ui/chat";
import { ThreadList } from "@/components/ThreadList";
import ToggleTheme from "@/components/ToggleTheme";
import { Badge } from "@/components/ui/badge";
import {
  Sidebar,
  SidebarContent,
  SidebarHeader,
  SidebarProvider,
  SidebarMenu,
  SidebarMenuItem,
  SidebarMenuButton,
  SidebarFooter,
  SidebarGroup,
  SidebarInset
} from "@/components/ui/sidebar";
import { MessageSquareIcon, SettingsIcon, HelpCircleIcon } from "lucide-react";

export default function HomePage() {
  // Use persistent chat hook
  const {
    messages,
    input,
    handleInputChange,
    handleSubmit: originalHandleSubmit,
    isLoading: isGenerating,
    stop,
    // Thread management capabilities
    currentThreadId,
    switchThread,
    createNewThread,
    deleteThread,
    updateThread
  } = usePersistentChat({
    api: "https://chat.openagents.com",
    maxSteps: 10,
    persistenceEnabled: true,
    onThreadChange: (threadId: string) => {
      console.log(`Switched to thread: ${threadId}`);
    }
  });

  // Wrap handleSubmit to match the expected signature
  const handleSubmit = useCallback((
    event?: { preventDefault?: () => void },
    options?: { experimental_attachments?: FileList }
  ) => {
    event?.preventDefault?.();
    return originalHandleSubmit(event as any);
  }, [originalHandleSubmit]);

  // Handler for creating a new thread
  const handleCreateThread = useCallback(() => {
    createNewThread();
  }, [createNewThread]);

  // Handler for selecting a thread
  const handleSelectThread = useCallback((threadId: string) => {
    switchThread(threadId);
  }, [switchThread]);

  // Handler for deleting a thread
  const handleDeleteThread = useCallback((threadId: string) => {
    deleteThread(threadId);
  }, [deleteThread]);

  // Handler for renaming a thread
  const handleRenameThread = useCallback((threadId: string, title: string) => {
    updateThread(threadId, title);
  }, [updateThread]);

  // Mock threads for now since they're not provided by the hook
  const [threads, setThreads] = useState<Thread[]>([]);

  return (
    <SidebarProvider defaultOpen>
      <div className="flex h-screen">
        <Sidebar>
          <SidebarHeader>
            <h2 className="text-lg font-semibold">OpenAgents</h2>
          </SidebarHeader>

          <SidebarContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton onClick={handleCreateThread}>
                  <MessageSquareIcon className="mr-2" />
                  <span>New Chat</span>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>

            <ThreadList
              currentThreadId={currentThreadId ?? ''}
              onSelectThread={handleSelectThread}
              onDeleteThread={handleDeleteThread}
              onRenameThread={handleRenameThread}
              onCreateThread={handleCreateThread}
            />
          </SidebarContent>

          <SidebarFooter>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton>
                  <SettingsIcon className="mr-2" />
                  <span>Settings</span>
                </SidebarMenuButton>
              </SidebarMenuItem>
              <SidebarMenuItem>
                <SidebarMenuButton>
                  <HelpCircleIcon className="mr-2" />
                  <span>Help</span>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
            <ToggleTheme />
          </SidebarFooter>
        </Sidebar>

        <main className="flex-1 flex flex-col">
          <div className="flex-1 overflow-y-auto">
            <MessageList messages={messages} />
          </div>

          <div className="p-4 border-t">
            <ChatForm
              isPending={isGenerating}
              handleSubmit={handleSubmit}
              className="mt-auto"
            >
              {({ files, setFiles }) => (
                <MessageInput
                  value={input}
                  onChange={handleInputChange}
                  allowAttachments
                  files={files}
                  setFiles={setFiles}
                  stop={stop}
                  isGenerating={isGenerating}
                />
              )}
            </ChatForm>
          </div>
        </main>
      </div>
    </SidebarProvider>
  );
}
