import React, { useCallback, useEffect, useState } from "react";
// Restored persistence with our wrapper
import { usePersistentChat, Thread } from "@openagents/core";
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

  // Use the persistence layer with the correct configuration
  const {
    messages,
    input,
    handleInputChange,
    handleSubmit,
    isLoading: isGenerating,
    stop,
    currentThreadId,
    switchThread,
    createNewThread,
    deleteThread,
    updateThread,
  } = usePersistentChat({
    // Use the FULL URL of the local HTTP server
    api: `http://localhost:3001/api/chat`,
    // Configuration that we know works
    streamProtocol: 'data',
    body: {
      model: "claude-3-5-sonnet-20240620"
    },
    headers: {
      'Content-Type': 'application/json',
      'Accept': 'text/event-stream',
      'X-Requested-With': 'XMLHttpRequest',
    },
    // Enable persistence
    persistenceEnabled: true,
    maxSteps: 10,
    // Event handlers
    onResponse: (response) => {
      console.log('Chat response received:', response);
    },
    onFinish: (message) => {
      console.log('Chat finished with message:', message);
    },
    onThreadChange: (threadId: string) => {
      console.log('Thread changed to:', threadId);
    },
    // Handle errors from the AI SDK hook itself
    onError: (error) => {
      console.error('Chat hook onError:', error);
    }
  });


  const handleCreateThread = useCallback(() => {
    createNewThread();
  }, [createNewThread]);

  const handleSelectThread = useCallback((threadId: string) => {
    switchThread(threadId);
  }, [switchThread]);

  const handleDeleteThread = useCallback((threadId: string) => {
    deleteThread(threadId);
  }, [deleteThread]);

  const handleRenameThread = useCallback((threadId: string, title: string) => {
    updateThread(threadId, title);
  }, [updateThread]);

  return (
    <SidebarProvider defaultOpen={true}>
      <div className="flex h-full w-full flex-col text-primary font-mono">
        <div className="relative flex h-full w-full flex-1 overflow-hidden z-0">
          <div className="relative flex h-full w-full flex-row overflow-hidden">
            <Sidebar>
              <SidebarHeader className="border-y h-14 mt-[30px]">
                <div className="flex items-center h-full justify-between px-3">
                  <span className="flex items-center text-sm font-semibold">
                    OpenAgents Coder
                    <Badge
                      variant="outline"
                      className="text-[10px] px-1 py-0 ml-1 mt-[1px]"
                    >
                      v0.0.1
                    </Badge>
                  </span>
                </div>
              </SidebarHeader>

              <SidebarContent>
                <SidebarGroup>
                  <ThreadList
                    currentThreadId={currentThreadId ?? ''}
                    onSelectThread={handleSelectThread}
                    onDeleteThread={handleDeleteThread}
                    onRenameThread={handleRenameThread}
                    onCreateThread={handleCreateThread}
                  />
                </SidebarGroup>
              </SidebarContent>

              <SidebarFooter>
                <div className="px-3 py-2">
                  <div className="text-xs text-muted-foreground">
                    <ToggleTheme />
                  </div>
                </div>
              </SidebarFooter>
            </Sidebar>

            <SidebarInset>
              <div className="grid grid-rows-[auto_1fr_auto] h-screen">
                <div className="border-y bg-background p-3 flex items-center justify-between z-10 h-14">
                  <div className="flex items-center gap-2 overflow-hidden">
                    <button
                      aria-label="Model selector"
                      type="button"
                      className="select-none group flex cursor-pointer items-center gap-1 rounded-lg py-1.5 px-3 text-sm hover:bg-muted overflow-hidden whitespace-nowrap"
                    >
                      <div>
                        Claude 3.5 Sonnet
                      </div>
                      <svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" className="icon-md">
                        <path fillRule="evenodd" clipRule="evenodd" d="M5.29289 9.29289C5.68342 8.90237 6.31658 8.90237 6.70711 9.29289L12 14.5858L17.2929 9.29289C17.6834 8.90237 18.3166 8.90237 18.7071 9.29289C19.0976 9.68342 19.0976 10.3166 18.7071 10.7071L12.7071 16.7071C12.5196 16.8946 12.2652 17 12 17C11.7348 17 11.4804 16.8946 11.2929 16.7071L5.29289 10.7071C4.90237 10.3166 4.90237 9.68342 5.29289 9.29289Z" fill="currentColor" />
                      </svg>
                    </button>
                    <div className="flex items-center ml-auto">
                      {/* Status display removed */}
                    </div>
                  </div>
                </div>

                <div className="overflow-y-auto">
                  <div className="h-full p-4">
                    <div className="mx-auto md:max-w-3xl lg:max-w-[40rem] xl:max-w-[48rem]">

                      <MessageList
                        messages={messages}
                        isTyping={isGenerating}
                      />
                    </div>
                  </div>
                </div>

                <div className="border-t bg-background p-4">
                  <div className="mx-auto md:max-w-3xl lg:max-w-[40rem] xl:max-w-[48rem]">
                    <ChatForm
                      isPending={isGenerating}
                      handleSubmit={handleSubmit}
                      className="relative"
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
                    <div className="mt-2 text-center text-xs text-muted-foreground">
                      <div>Coder will make mistakes. Commit to git regularly.</div>
                    </div>
                  </div>
                </div>
              </div>
            </SidebarInset>
          </div>
        </div>
      </div>
    </SidebarProvider>
  );
}
