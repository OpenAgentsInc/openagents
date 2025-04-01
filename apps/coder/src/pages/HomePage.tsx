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
import { MessageSquareIcon, SettingsIcon, HelpCircleIcon, Plus } from "lucide-react";
import { Button } from "@/components/ui/button";

export default function HomePage() {
  // Get settings including the default model
  const { settings, isLoading: isLoadingSettings, clearSettingsCache, updateSettings } = useSettings();

  // Force a refresh of settings when the component mounts
  useEffect(() => {
    clearSettingsCache();
  }, [clearSettingsCache]);
  const [selectedModelId, setSelectedModelId] = useState<string>("");

  useEffect(() => {
    async function setupModel() {
      try {
        // Only proceed when settings are loaded
        if (!settings) return;

        // Look for a user-selected model first (highest priority)
        let userSelectedModel = null;

        // Check active localStorage model (selected by user in this or another tab)
        try {
          if (typeof window !== 'undefined' && window.localStorage) {
            const activeModel = window.localStorage.getItem('openagents_active_model');
            if (activeModel && models.some(model => model.id === activeModel)) {
              // console.log(`Using active model from localStorage: ${activeModel}`);
              userSelectedModel = activeModel;
            }
          }
        } catch (storageError) {
          console.warn("Error reading active model from localStorage:", storageError);
        }

        // If no localStorage model, check sessionStorage (selected in this tab only)
        if (!userSelectedModel) {
          try {
            if (typeof window !== 'undefined' && window.sessionStorage) {
              const currentModel = window.sessionStorage.getItem('openagents_current_model');
              if (currentModel && models.some(model => model.id === currentModel)) {
                console.log(`Using current model from sessionStorage: ${currentModel}`);
                userSelectedModel = currentModel;
              }
            }
          } catch (storageError) {
            console.warn("Error reading from sessionStorage:", storageError);
          }
        }

        // If user has manually selected a model, use it and skip default logic
        if (userSelectedModel) {
          // Skip if already selected
          if (selectedModelId === userSelectedModel) {
            console.log(`Model already selected (${selectedModelId})`);
            return;
          }

          // console.log(`Using user-selected model: ${userSelectedModel}`);
          setSelectedModelId(userSelectedModel);
          return;
        }

        // If no user selection, use default from settings (lower priority)
        // Check if we already have a selected model and matches settings (to prevent unnecessary reselection)
        if (selectedModelId && selectedModelId === settings.defaultModel) {
          console.log(`Model already selected (${selectedModelId}) matches settings`);
          return;
        }

        if (settings.defaultModel) {
          console.log(`Loading default model from settings: ${settings.defaultModel}`);

          // Check if the model exists in our models list
          const modelExists = models.some(model => model.id === settings.defaultModel);

          if (modelExists) {
            console.log(`Model ${settings.defaultModel} found, selecting it`);
            setSelectedModelId(settings.defaultModel);

            // Also save to sessionStorage for resilience
            try {
              if (typeof window !== 'undefined' && window.sessionStorage) {
                window.sessionStorage.setItem('openagents_current_model', settings.defaultModel);
              }
            } catch (storageError) {
              console.warn("Error storing model in sessionStorage:", storageError);
            }
          } else {
            // If model doesn't exist, default to first model AND update settings
            console.warn(`Model ${settings.defaultModel} not found in models list, using fallback`);

            if (models.length > 0) {
              const fallbackModel = models[0].id;
              setSelectedModelId(fallbackModel);

              // Update the settings to use a valid model
              console.log(`Automatically updating settings to use valid model: ${fallbackModel}`);
              try {
                // Update the default model
                const result = await updateSettings({ defaultModel: fallbackModel });
                console.log("Settings auto-corrected:", result.defaultModel);
              } catch (error) {
                console.error("Failed to auto-correct settings:", error);
              }
            }
          }
        } else {
          // Default to first model if no default is set
          console.log("No default model in settings, using first model");
          if (models.length > 0) {
            const firstModel = models[0].id;
            setSelectedModelId(firstModel);

            // Save this as the default
            console.log(`Setting first model as default: ${firstModel}`);
            try {
              await updateSettings({ defaultModel: firstModel });
              console.log("Default model saved");
            } catch (error) {
              console.error("Failed to save default model:", error);
            }
          }
        }
      } catch (error) {
        console.error("Error setting up model:", error);
      }
    }

    setupModel();
  }, [settings, clearSettingsCache, updateSettings]);

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
    // onResponse: (response) => {
    //   console.log('Chat response received:', response);
    // },
    // onFinish: (message) => {
    //   console.log('Chat finished with message:', message);
    // },
    // onThreadChange: (threadId: string) => {
    //   console.log('Thread changed to:', threadId);
    // },
    // Handle errors from the AI SDK hook itself
    onError: (error) => {
      console.error('Chat hook onError:', error);
    }
  });


  const handleCreateThread = useCallback(() => {
    createNewThread();

    // Dispatch a custom event to focus the input
    window.dispatchEvent(
      new CustomEvent('new-chat', { detail: { fromButton: true } })
    );
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

  // Handle model change - this happens when user selects from dropdown
  const handleModelChange = (modelId: string) => {
    console.log(`Model changed via dropdown to: ${modelId}`);

    // Set the model ID for current session
    setSelectedModelId(modelId);

    // Save the selection to sessionStorage for persistence within this tab
    try {
      if (typeof window !== 'undefined' && window.sessionStorage) {
        window.sessionStorage.setItem('openagents_current_model', modelId);
      }
    } catch (storageError) {
      console.warn("Error storing model in sessionStorage:", storageError);
    }

    // Also save to localStorage for persistence across tabs (but not as default)
    try {
      if (typeof window !== 'undefined' && window.localStorage) {
        window.localStorage.setItem('openagents_active_model', modelId);
      }
    } catch (localStorageError) {
      console.warn("Error storing model in localStorage:", localStorageError);
    }

    // We don't update the default model here - this is just for the current session
    // Users need to go to settings to permanently change the default model
  };

  return (
    <SidebarProvider defaultOpen={true}>
      <div className="flex h-[calc(100vh-30px)] w-full flex-col text-primary font-mono">
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
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={handleCreateThread}
                    className="flex gap-1 items-center"
                  >
                    <Plus className="size-4" />
                    <span>New</span>
                  </Button>
                </div>
              </SidebarHeader>

              <SidebarContent>
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
                    <Link to="/settings/models">
                      <SidebarMenuButton>
                        <SettingsIcon />
                        <span>Settings</span>
                      </SidebarMenuButton>
                    </Link>
                  </SidebarMenuItem>
                </SidebarMenu>
              </SidebarFooter>
            </Sidebar>

            <SidebarInset>
              <div className="grid grid-rows-[auto_1fr_auto] h-[calc(100vh-30px)]">
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
                  <div className="h-full p-4 pt-8 mt-4">
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
                    {/* Removed the disclaimer text */}
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
