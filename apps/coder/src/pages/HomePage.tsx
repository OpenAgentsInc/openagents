import React, { useCallback, useEffect, useState } from "react";
// Restored persistence with our wrapper
import { usePersistentChat, useSettings, MODELS, DEFAULT_SYSTEM_PROMPT } from "@openagents/core";
import { MessageInput } from "@/components/ui/message-input";
import { MessageList } from "@/components/ui/message-list";
import { Chat, ChatForm } from "@/components/ui/chat";
import { ThreadList } from "@/components/ThreadList";
import ToggleTheme from "@/components/ToggleTheme";
import { Badge } from "@/components/ui/badge";
import { NewChatIcon } from "@/components/NewChatIcon";
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
import { MessageSquareIcon, SettingsIcon, HelpCircleIcon, AlertCircle } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Message, type UIPart, type UIMessage } from "@openagents/core";

interface Settings {
  defaultModel?: string;
  selectedModelId?: string;
  visibleModelIds?: string[];
  // Add other settings properties as needed
}

export default function HomePage() {
  // Get settings including the default model
  const { settings, isLoading: isLoadingSettings, clearSettingsCache, updateSettings, refresh: refreshSettings, selectModel, getApiKey } = useSettings();

  // Define state variables first
  const [selectedModelId, setSelectedModelId] = useState<string>("");

  // Force a refresh of settings when the component mounts
  useEffect(() => {
    clearSettingsCache();

    // Initialize database early
    (async () => {
      try {
        // Import directly here to avoid circular dependencies
        const db = await import('@openagents/core/src/db/database');
        await db.getDatabase();
        console.log("Database initialized on startup");
      } catch (error) {
        console.error("Failed to initialize database on startup:", error);
      }
    })();
  }, [clearSettingsCache]);

  // Use a ref to track the last applied model ID to prevent loops
  const lastAppliedModelRef = React.useRef<string | null>(null);

  // Add event listener for page visibility and focus to refresh settings
  useEffect(() => {
    // Store the visibility handler to properly remove it
    const handleVisibilityChange = () => {
      if (document.visibilityState === 'visible') {
        handleFocus();
      }
    };

    // Function to refresh settings from database
    const handleFocus = () => {
      console.log("Page focused, refreshing settings");
      refreshSettings().then(updatedSettings => {
        if (updatedSettings && updatedSettings.selectedModelId) {
          const newModelId = updatedSettings.selectedModelId;
          console.log(`Refreshed settings with model: ${newModelId}`);

          // Check if this model update is new (not just the one we applied)
          // and also different from what's currently selected
          if (selectedModelId !== newModelId && lastAppliedModelRef.current !== newModelId) {
            console.log(`Updating model from ${selectedModelId} to ${newModelId}`);
            // Update our ref to track this change
            lastAppliedModelRef.current = newModelId;
            setSelectedModelId(newModelId);
          }
        }
      });
    };

    // Handle custom event from settings page
    const handleModelSettingsChanged = (event: any) => {
      console.log("Received model-settings-changed event", event.detail);
      if (event.detail && event.detail.selectedModelId) {
        const newModelId = event.detail.selectedModelId;
        console.log(`Setting model from event: ${newModelId}`);

        // Track that we're about to apply this model ID to prevent loops
        lastAppliedModelRef.current = newModelId;

        if (selectedModelId !== newModelId) {
          setSelectedModelId(newModelId);
        }
      } else {
        // If no details provided, just refresh settings
        handleFocus();
      }
    };

    // Add event listeners
    window.addEventListener('focus', handleFocus);
    window.addEventListener('visibilitychange', handleVisibilityChange);
    window.addEventListener('model-settings-changed', handleModelSettingsChanged);

    // Load settings on first mount
    handleFocus();

    // Cleanup event listeners
    return () => {
      window.removeEventListener('focus', handleFocus);
      window.removeEventListener('visibilitychange', handleVisibilityChange);
      window.removeEventListener('model-settings-changed', handleModelSettingsChanged);
    };

    // Only depend on refreshSettings to avoid re-running the effect when selectedModelId changes
  }, [refreshSettings]);

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
            if (activeModel && MODELS.some(model => model.id === activeModel)) {
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
              if (currentModel && MODELS.some(model => model.id === currentModel)) {
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
            // console.log(`Model already selected (${selectedModelId})`);
            return;
          }

          // console.log(`Using user-selected model: ${userSelectedModel}`);
          setSelectedModelId(userSelectedModel);
          return;
        }

        // If no user selection, use selected model from settings (lower priority)
        // First get the model ID from settings, preferring selectedModelId but falling back to defaultModel
        const settingsModelId = settings.selectedModelId || settings.defaultModel;

        // Check if we already have a selected model that matches settings (to prevent unnecessary reselection)
        if (selectedModelId && selectedModelId === settingsModelId) {
          // console.log(`Model already selected (${selectedModelId}) matches settings`);
          return;
        }

        if (settingsModelId) {
          console.log(`Loading model from settings: ${settingsModelId}`);

          // Check if the model exists in our models list
          const modelExists = MODELS.some(model => model.id === settingsModelId);

          if (modelExists) {
            console.log(`Model ${settingsModelId} found, selecting it`);
            setSelectedModelId(settingsModelId);

            // Also save to sessionStorage for resilience
            try {
              if (typeof window !== 'undefined' && window.sessionStorage) {
                window.sessionStorage.setItem('openagents_current_model', settingsModelId);
              }
            } catch (storageError) {
              console.warn("Error storing model in sessionStorage:", storageError);
            }
          } else {
            // If model doesn't exist, default to first model AND update settings
            console.warn(`Model ${settingsModelId} not found in models list, using fallback`);

            if (MODELS.length > 0) {
              const fallbackModel = MODELS[0].id;
              setSelectedModelId(fallbackModel);

              // Update the settings to use a valid model
              console.log(`Automatically updating settings to use valid model: ${fallbackModel}`);
              try {
                // Update both fields for compatibility
                const result = await updateSettings({
                  defaultModel: fallbackModel,
                  selectedModelId: fallbackModel
                }) as Settings;
                console.log("Settings auto-corrected:", result.selectedModelId || result.defaultModel);
              } catch (error) {
                console.error("Failed to auto-correct settings:", error);
              }
            }
          }
        } else {
          // Default to first model if no default is set
          console.log("No model selected in settings, using first model");
          if (MODELS.length > 0) {
            const firstModel = MODELS[0].id;
            setSelectedModelId(firstModel);

            // Save this as the selected model
            console.log(`Setting first model as selected model: ${firstModel}`);
            try {
              await updateSettings({
                defaultModel: firstModel, // For backward compatibility
                selectedModelId: firstModel
              });
              console.log("Selected model saved");
            } catch (error) {
              console.error("Failed to save selected model:", error);
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
  // const selectedModel = MODELS.find(model => model.id === selectedModelId) || models[0];

  // Load the system prompt from preferences
  const [systemPrompt, setSystemPrompt] = useState<string>("");

  // Load system prompt from settings when component mounts
  useEffect(() => {
    const loadSystemPrompt = async () => {
      try {
        if (!settings) return;

        // Use getPreference method from settingsRepository directly
        // since we need to get this value before rendering
        const { settingsRepository } = await import('@openagents/core/src/db/repositories');
        const savedPrompt = await settingsRepository.getPreference("defaultSystemPrompt", DEFAULT_SYSTEM_PROMPT);
        setSystemPrompt(savedPrompt);
        console.log("Loaded system prompt:", savedPrompt === DEFAULT_SYSTEM_PROMPT ? "Using default prompt" : "Custom prompt loaded");
      } catch (error) {
        console.error("Error loading system prompt:", error);
        // Fall back to default prompt on error
        setSystemPrompt(DEFAULT_SYSTEM_PROMPT);
      }
    };

    loadSystemPrompt();
  }, [settings]);

  // Get API keys from settings
  const [apiKeys, setApiKeys] = useState<Record<string, string>>({});

  // Load API keys from settings
  const loadApiKeys = useCallback(async () => {
    try {
      if (!settings) return;

      const { settingsRepository } = await import('@openagents/core/src/db/repositories');
      const providers = ['openrouter', 'anthropic', 'openai', 'google', 'ollama', 'lmstudio'];
      const keys: Record<string, string> = {};
      
      for (const provider of providers) {
        const key = await settingsRepository.getApiKey(provider);
        if (key) {
          keys[provider] = key;
        }
      }
      
      console.log(`Loaded API keys for providers: ${Object.keys(keys).join(', ')}`);
      setApiKeys(keys);
    } catch (error) {
      console.error("Error loading API keys:", error);
    }
  }, [settings]);

  // Load API keys from settings when component mounts and listen for API key changes
  useEffect(() => {
    // Load API keys initially
    loadApiKeys();
    
    // Handle API key changes from settings page
    const handleApiKeyChange = () => {
      console.log("API key changed, refreshing keys");
      loadApiKeys();
    };
    
    // Add event listener for API key changes
    window.addEventListener('api-key-changed', handleApiKeyChange);
    
    // Cleanup event listener
    return () => {
      window.removeEventListener('api-key-changed', handleApiKeyChange);
    };
  }, [loadApiKeys, settings]);

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
      model: selectedModelId,
      // Include system prompt if it's not empty
      ...(systemPrompt ? { systemPrompt } : {}),
      // Include API keys from settings
      apiKeys: Object.keys(apiKeys).length > 0 ? apiKeys : undefined
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

  // State to force ThreadList rerender when creating a new thread
  const [threadListKey, setThreadListKey] = useState(Date.now());

  const handleCreateThread = useCallback(async (): Promise<void> => {
    try {
      // First update the key to force an immediate re-render of ThreadList
      setThreadListKey(Date.now());

      const thread = await createNewThread();

      // Dispatch a custom event to focus the input
      window.dispatchEvent(
        new CustomEvent('new-chat', { detail: { fromButton: true, threadId: thread.id } })
      );

      // Force another update after thread creation
      setThreadListKey(Date.now());

      // No return value (void) to match the expected type signature
    } catch (error) {
      console.error("Failed to create new thread:", error);
      // Create a fallback thread in UI only
      // This can happen if DB fails to initialize
      alert("Could not create a new thread. Database may be initializing. Please try again.");
      throw error;
    }
  }, [createNewThread]);

  const handleSelectThread = useCallback((threadId: string) => {
    switchThread(threadId);
  }, [switchThread]);

  const handleDeleteThread = useCallback((threadId: string) => {
    // Call the delete function from usePersistentChat
    deleteThread(threadId).catch(error => {
      console.error('Failed to delete thread:', error);
    });
  }, [deleteThread]);

  const handleRenameThread = useCallback((threadId: string, title: string) => {
    updateThread(threadId, title).catch(error => {
      console.error('Failed to rename thread:', error);
    });
  }, [updateThread]);

  // Check if the current model is available (has API key if needed)
  const [isModelAvailable, setIsModelAvailable] = useState(true);
  const [modelWarning, setModelWarning] = useState<string | null>(null);

  useEffect(() => {
    async function checkCurrentModelAvailability() {
      try {
        if (!selectedModelId) return;
        
        const selectedModel = MODELS.find(model => model.id === selectedModelId);
        if (!selectedModel) return;
        
        let isAvailable = true;
        let warning = null;
        
        // Function to check if Ollama is running and get available models
        const checkOllamaModels = async (): Promise<string[]> => {
          try {
            const response = await fetch("http://localhost:11434/api/tags");
            if (response.ok) {
              const data = await response.json();
              return data.models.map((model: any) => model.name);
            }
          } catch (error) {
            console.warn("Failed to connect to Ollama API:", error);
          }
          return [];
        };
        
        // Function to check if LMStudio is running - use proxy to avoid CORS issues
        const checkLMStudioAvailable = async (): Promise<boolean> => {
          try {
            // Use our server proxy to avoid CORS issues
            const proxyUrl = `/api/proxy/lmstudio/models?url=${encodeURIComponent("http://localhost:1234/v1/models")}`;
            console.log("Checking LMStudio via proxy:", proxyUrl);
            
            const response = await fetch(proxyUrl);
            return response.ok;
          } catch (error) {
            console.warn("Failed to connect to LMStudio API via proxy:", error);
            return false;
          }
        };
        
        // Check based on provider
        if (selectedModel.provider === 'openrouter') {
          const key = await getApiKey('openrouter');
          isAvailable = !!key;
          if (!key) {
            warning = "OpenRouter API key required. Add it in Settings > API Keys.";
          }
        } 
        else if (selectedModel.provider === 'anthropic') {
          const key = await getApiKey('anthropic');
          isAvailable = !!key;
          if (!key) {
            warning = "Anthropic API key required. Add it in Settings > API Keys.";
          }
        }
        else if (selectedModel.provider === 'openai') {
          const key = await getApiKey('openai');
          isAvailable = !!key;
          if (!key) {
            warning = "OpenAI API key required. Add it in Settings > API Keys.";
          }
        }
        else if (selectedModel.provider === 'google') {
          const key = await getApiKey('google');
          isAvailable = !!key;
          if (!key) {
            warning = "Google API key required. Add it in Settings > API Keys.";
          }
        }
        else if (selectedModel.provider === 'ollama') {
          const ollamaModels = await checkOllamaModels();
          const modelName = selectedModel.id.split('/')[1];
          
          isAvailable = ollamaModels.length > 0 && ollamaModels.some(m => 
            m === modelName || m.startsWith(`${modelName}:`)
          );
          
          if (!isAvailable) {
            if (ollamaModels.length === 0) {
              warning = "Ollama server not running. Configure Ollama in Settings > Local Models.";
            } else {
              warning = `Model '${modelName}' not available. Run 'ollama pull ${modelName}'`;
            }
          }
        }
        else if (selectedModel.provider === 'lmstudio') {
          isAvailable = await checkLMStudioAvailable();
          if (!isAvailable) {
            warning = "LMStudio not running or unreachable.";
          }
        }
        
        setIsModelAvailable(isAvailable);
        setModelWarning(warning);
      } catch (error) {
        console.error("Error checking model availability:", error);
        setIsModelAvailable(true);
        setModelWarning(null);
      }
    }
    
    checkCurrentModelAvailability();
  }, [selectedModelId, getApiKey]);

  // Handle model change - this happens when user selects from dropdown
  const handleModelChange = (modelId: string) => {
    console.log(`Model changed via dropdown to: ${modelId}`);

    // Track that we're about to apply this model to prevent loops
    lastAppliedModelRef.current = modelId;

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

    // Also update the settings to persist this selection
    selectModel(modelId).catch(error => {
      console.error("Failed to update settings with selected model:", error);
    });
  };

  return (
    <SidebarProvider defaultOpen={true}>
      <div className="flex h-screen w-full flex-col text-primary font-mono">
        <div className="relative flex h-full w-full flex-1 overflow-hidden z-0">
          <div className=" mt-[30px] relative flex h-full w-full flex-row overflow-hidden">
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
                  <NewChatIcon onClick={handleCreateThread} />
                </div>
              </SidebarHeader>

              <SidebarContent>
                <ThreadList
                  key={`thread-list-${threadListKey}`} /* Force re-render on new thread */
                  currentThreadId={currentThreadId ?? ''}
                  onSelectThread={handleSelectThread}
                  onDeleteThread={handleDeleteThread}
                  onRenameThread={handleRenameThread}
                  onCreateThread={handleCreateThread}
                />
              </SidebarContent>

              <SidebarFooter>
                <SidebarMenu>
                  <SidebarMenuItem className="flex justify-between items-center">
                    <Link to="/settings/models">
                      <SidebarMenuButton>
                        <SettingsIcon />
                        <span>Settings</span>
                      </SidebarMenuButton>
                    </Link>
                    <ToggleTheme />
                  </SidebarMenuItem>
                </SidebarMenu>
              </SidebarFooter>
            </Sidebar>

            <SidebarInset>
              <div className="mt-[30px] grid grid-rows-[auto_minmax(0,1fr)_auto] h-[calc(100vh-30px)]">
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

                <div className="overflow-y-auto relative">
                  <div className="absolute inset-0 p-4 pt-8">
                    <div className="mx-auto md:max-w-3xl lg:max-w-[40rem] xl:max-w-[48rem]">

                      <MessageList
                        messages={(() => {
                          // BRUTE FORCE FIX - Aggressively correct identical timestamps - Remains as backup in case database fixes still have issues

                          // First, identify if we have timestamp collisions
                          const timestampCounts: Record<number, number> = {};
                          const messagesWithParts: UIMessage[] = messages.map(msg => ({
                            ...msg,
                            parts: msg.parts || [{
                              type: 'text' as const,
                              text: msg.content
                            }]
                          }));
                          messagesWithParts.forEach(msg => {
                            const timestamp = msg.createdAt?.getTime() || 0;
                            timestampCounts[timestamp] = (timestampCounts[timestamp] || 0) + 1;
                          });

                          const hasCollisions = Object.values(timestampCounts).some(count => count > 1);

                          // If no collisions, return messages as-is
                          if (!hasCollisions) return messagesWithParts;

                          // First organize by role to keep conversation flow
                          const userMessages: UIMessage[] = [];
                          const assistantMessages: UIMessage[] = [];

                          messagesWithParts.forEach(msg => {
                            if (msg.role === 'user') userMessages.push(msg);
                            else assistantMessages.push(msg);
                          });

                          // Pair user messages with assistant responses
                          const correctedMessages = [];
                          let baseTime = Date.now() - (messages.length * 10000); // Start 10 seconds ago per message

                          // If we have more user messages than assistant or vice versa, we need to handle that
                          const maxLength = Math.max(userMessages.length, assistantMessages.length);

                          for (let i = 0; i < maxLength; i++) {
                            if (i < userMessages.length) {
                              const userMsg = userMessages[i];
                              userMsg.createdAt = new Date(baseTime);
                              correctedMessages.push(userMsg);
                              baseTime += 2000; // 2 second gap
                            }

                            if (i < assistantMessages.length) {
                              const assistantMsg = assistantMessages[i];
                              assistantMsg.createdAt = new Date(baseTime);
                              correctedMessages.push(assistantMsg);
                              baseTime += 3000; // 3 second gap
                            }
                          }

                          return correctedMessages;
                        })()}
                        isTyping={isGenerating}
                      />
                    </div>
                  </div>
                </div>

                <div className="border-t bg-background p-4">
                  <div className="mx-auto md:max-w-3xl lg:max-w-[40rem] xl:max-w-[48rem]">
                    {!isModelAvailable && modelWarning && (
                      <div className="mb-2 p-2 text-sm text-yellow-600 dark:text-yellow-400 border border-yellow-400 rounded-md bg-yellow-50 dark:bg-yellow-900/20">
                        <div className="flex items-center">
                          <AlertCircle className="h-4 w-4 mr-2 flex-shrink-0" />
                          <span>{modelWarning}</span>
                        </div>
                        <div className="mt-1 ml-6">
                          {selectedModelId && MODELS.find(m => m.id === selectedModelId)?.provider === 'ollama' ? (
                            <Link to="/settings/local-models" className="underline">Configure Ollama</Link>
                          ) : modelWarning?.includes("LMStudio") ? (
                            <Link to="/settings/local-models" className="underline">Configure LMStudio</Link>
                          ) : (
                            <Link to="/settings/models" className="underline">Add API Key</Link>
                          )}
                        </div>
                      </div>
                    )}
                    
                    <ChatForm
                      isPending={isGenerating}
                      handleSubmit={(e) => {
                        // Prevent submission if model is unavailable
                        if (!isModelAvailable) {
                          e.preventDefault();
                          return;
                        }
                        handleSubmit(e);
                      }}
                      className="relative"
                    >
                      {({ files, setFiles }) => (
                        <MessageInput
                          value={input}
                          onChange={handleInputChange}
                          allowAttachments={false}
                          // files={files}
                          // setFiles={setFiles}
                          stop={stop}
                          isGenerating={isGenerating}
                          disabled={!isModelAvailable}
                          placeholder={!isModelAvailable ? "API key required for this model" : "Message..."}
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
