import React, { useCallback, useEffect, useState } from "react";
// Restored persistence with our wrapper
import { usePersistentChat, useSettings, models } from "@openagents/core";
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
import { Link } from "@tanstack/react-router";
import { ModelSelect } from "@/components/ui/model-select";
import { MessageSquareIcon, SettingsIcon, HelpCircleIcon } from "lucide-react";

export default function HomePage() {
  // Get settings including the default model
  const { settings, isLoading: isLoadingSettings, clearSettingsCache } = useSettings();

  // Force a refresh of settings when the component mounts
  useEffect(() => {
    clearSettingsCache();
  }, [clearSettingsCache]);
  const [selectedModelId, setSelectedModelId] = useState<string>("");

  useEffect(() => {
    // Set the default model from settings when loaded
    if (settings?.defaultModel) {
      console.log(`Loading default model from settings: ${settings.defaultModel}`);

      // Check if the model exists in our models list
      const modelExists = models.some(model => model.id === settings.defaultModel);

      if (modelExists) {
        setSelectedModelId(settings.defaultModel);
      } else {
        // If model doesn't exist, default to first model
        console.warn(`Model ${settings.defaultModel} not found in models list`);
        if (models.length > 0) {
          setSelectedModelId(models[0].id);
        }
      }
    } else {
      // Default to first model if no default is set
      console.log("No default model in settings, using first model");
      if (models.length > 0) {
        setSelectedModelId(models[0].id);
      }
    }
  }, [settings]);

  // Find the selected model
  const selectedModel = models.find(model => model.id === selectedModelId) || models[0];

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
      model: selectedModelId
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

  // Handle model change
  const handleModelChange = (modelId: string) => {
    console.log(`Model changed to: ${modelId}`);
    setSelectedModelId(modelId);

    // We don't update the default model here - this is just for the current session
    // Users need to go to settings to permanently change the default model
  };

  return (
    <SidebarProvider defaultOpen={true}>
      <div className="flex h-full w-full flex-col text-primary font-mono">
        <div className="relative flex h-full w-full flex-1 overflow-hidden z-0">
          <div className="relative flex h-full w-full flex-row overflow-hidden">
            <Sidebar>
              <SidebarHeader className="border-y h-14 mt-[30px]">
                <div className="flex items-center h-full justify-between px-3">
                  <span className="flex items-center text-md font-semibold">
                    Coder
                    <Badge
                      variant="outline"
                      className="text-[11px] px-[4px] py-[2px] ml-2 mt-[1px]"
                    >
                      v0.0.1
                    </Badge>
                  </span>
                </div>
              </SidebarHeader>

              <SidebarContent>
                <SidebarGroup>
                  <SidebarMenu>
                    <SidebarMenuItem>
                      <SidebarMenuButton
                        onClick={handleCreateThread}
                      >
                        <MessageSquareIcon />
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
                </SidebarGroup>

                <SidebarGroup>
                  <SidebarMenu>
                    <SidebarMenuItem>
                      <Link to="/settings/models">
                        <SidebarMenuButton>
                          <SettingsIcon />
                          <span>Models & API Keys</span>
                        </SidebarMenuButton>
                      </Link>
                    </SidebarMenuItem>
                  </SidebarMenu>
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
                    <ModelSelect
                      value={selectedModelId}
                      onChange={handleModelChange}
                      className="w-[240px]"
                    />
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
