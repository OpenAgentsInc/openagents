# HomePage Component Refactoring

## Original Issues

The `HomePage.tsx` component had several issues that needed addressing:

1. **Excessive Size**: At nearly 1000 lines, it was too large and difficult to maintain
2. **Mixed Responsibilities**: Handled UI rendering, data fetching, state management, and event handling all in one file
3. **Performance Issues**: Caused frequent rerenders due to large component state dependencies
4. **Re-rendering Cascade**: Changes to one part of the state caused complete re-rendering of unrelated components
5. **High Cognitive Load**: Difficult to understand and modify due to complexity

## Refactoring Solution

We split the component into smaller, focused components using React's compositional pattern and context-based state management. The implementation follows these principles:

1. **Single Responsibility**: Each component does one thing well
2. **Separation of Concerns**: Isolated UI, state management, and business logic
3. **Memoization**: Used React.memo, useMemo, and useCallback strategically
4. **State Locality**: Kept state as close as possible to where it's used
5. **State Isolation**: Prevented unnecessary re-renders by isolating state changes

## Implementation Details

### 1. Context Providers

#### ModelProvider

The ModelProvider manages all model-related state and operations:

```tsx
// /apps/coder/src/providers/ModelProvider.tsx
import React, { createContext, useContext, useState, useEffect, useCallback, useRef } from 'react';
import { MODELS, useSettings } from '@openagents/core';

type ModelContextType = {
  selectedModelId: string;
  setSelectedModelId: (id: string) => void;
  handleModelChange: (id: string) => void;
  isModelAvailable: boolean;
  modelWarning: string | null;
  checkCurrentModelAvailability: () => Promise<void>;
};

const ModelContext = createContext<ModelContextType | null>(null);

export const useModelContext = () => {
  const context = useContext(ModelContext);
  if (!context) throw new Error('useModelContext must be used within a ModelProvider');
  return context;
};

export const ModelProvider: React.FC<{children: React.ReactNode}> = ({children}) => {
  // State for model selection and availability
  const [selectedModelId, setSelectedModelId] = useState<string>("");
  const [isModelAvailable, setIsModelAvailable] = useState(true);
  const [modelWarning, setModelWarning] = useState<string | null>(null);
  
  const { settings, updateSettings, selectModel, getApiKey } = useSettings();
  
  // Logic for model change handling, availability checking, and persistence
  
  return (
    <ModelContext.Provider value={{
      selectedModelId,
      setSelectedModelId,
      handleModelChange,
      isModelAvailable,
      modelWarning,
      checkCurrentModelAvailability,
    }}>
      {children}
    </ModelContext.Provider>
  );
};
```

#### ApiKeyProvider

The ApiKeyProvider manages API key retrieval and monitoring:

```tsx
// /apps/coder/src/providers/ApiKeyProvider.tsx
import React, { createContext, useContext, useState, useEffect, useCallback } from 'react';
import { useSettings } from '@openagents/core';

type ApiKeyContextType = {
  apiKeys: Record<string, string>;
  loadApiKeys: () => Promise<void>;
};

const ApiKeyContext = createContext<ApiKeyContextType | null>(null);

export const useApiKeyContext = () => {
  const context = useContext(ApiKeyContext);
  if (!context) throw new Error('useApiKeyContext must be used within an ApiKeyProvider');
  return context;
};

export const ApiKeyProvider: React.FC<{children: React.ReactNode}> = ({children}) => {
  const [apiKeys, setApiKeys] = useState<Record<string, string>>({});
  const { settings } = useSettings();
  
  // Logic for loading and monitoring API keys
  
  return (
    <ApiKeyContext.Provider value={{
      apiKeys,
      loadApiKeys
    }}>
      {children}
    </ApiKeyContext.Provider>
  );
};
```

#### ChatStateProvider

The ChatStateProvider centralizes all chat-related state and operations:

```tsx
// /apps/coder/src/providers/ChatStateProvider.tsx
import React, { createContext, useContext, useCallback, useState } from 'react';
import { usePersistentChat, type UIMessage } from '@openagents/core';
import { useModelContext } from './ModelProvider';
import { useApiKeyContext } from './ApiKeyProvider';

type ChatStateContextType = {
  messages: UIMessage[];
  input: string;
  handleInputChange: (value: string) => void;
  handleSubmit: (event?: { preventDefault?: () => void } | undefined, options?: { experimental_attachments?: FileList | undefined } | undefined) => void;
  isGenerating: boolean;
  stop: () => void;
  currentThreadId: string | null;
  handleSelectThread: (threadId: string) => void;
  handleCreateThread: () => Promise<void>;
  handleDeleteThread: (threadId: string) => void;
  handleRenameThread: (threadId: string, title: string) => void;
  threadListKey: number;
};

const ChatStateContext = createContext<ChatStateContextType | null>(null);

export const useChatState = () => {
  const context = useContext(ChatStateContext);
  if (!context) throw new Error('useChatState must be used within a ChatStateProvider');
  return context;
};

export const ChatStateProvider: React.FC<ChatStateProviderProps> = ({
  children,
  systemPrompt,
}) => {
  const { selectedModelId } = useModelContext();
  const { apiKeys } = useApiKeyContext();
  
  // State and chat operations using usePersistentChat
  const [threadListKey, setThreadListKey] = useState(Date.now());
  
  // Chat operations (createThread, selectThread, etc.)
  
  return (
    <ChatStateContext.Provider value={{
      messages: processedMessages,
      input,
      handleInputChange: typeSafeHandleInputChange,
      handleSubmit,
      isGenerating,
      stop,
      currentThreadId: currentThreadId || null,
      handleSelectThread,
      handleCreateThread,
      handleDeleteThread,
      handleRenameThread,
      threadListKey,
    }}>
      {children}
    </ChatStateContext.Provider>
  );
};
```

### 2. UI Components

#### AppHeader

```tsx
// /apps/coder/src/components/AppHeader.tsx
import React, { memo } from 'react';
import { Badge } from '@/components/ui/badge';
import { NewChatIcon } from '@/components/NewChatIcon';

interface AppHeaderProps {
  onCreateThread: () => Promise<void>;
}

export const AppHeader = memo(function AppHeader({ onCreateThread }: AppHeaderProps) {
  return (
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
      <NewChatIcon onClick={onCreateThread} />
    </div>
  );
});
```

#### ModelHeader

```tsx
// /apps/coder/src/components/ModelHeader.tsx
import React, { memo } from 'react';
import { ModelSelect } from '@/components/ui/model-select';
import { useModelContext } from '@/providers/ModelProvider';

export const ModelHeader = memo(function ModelHeader() {
  const { selectedModelId, handleModelChange } = useModelContext();
  
  return (
    <div className="flex items-center gap-2 overflow-hidden">
      <ModelSelect
        value={selectedModelId}
        onChange={handleModelChange}
        className="w-[240px]"
      />
      <div className="flex items-center ml-auto">
        {/* Status display if needed */}
      </div>
    </div>
  );
});
```

#### ModelWarningBanner

```tsx
// /apps/coder/src/components/ModelWarningBanner.tsx
import React, { memo } from 'react';
import { Link } from '@tanstack/react-router';
import { AlertCircle } from 'lucide-react';
import { MODELS } from '@openagents/core';
import { useModelContext } from '@/providers/ModelProvider';

export const ModelWarningBanner = memo(function ModelWarningBanner() {
  const { isModelAvailable, modelWarning, selectedModelId } = useModelContext();
  
  if (isModelAvailable || !modelWarning) return null;
  
  const selectedModelProvider = MODELS.find(m => m.id === selectedModelId)?.provider;
  
  return (
    <div className="mb-2 p-2 text-sm text-yellow-600 dark:text-yellow-400 border border-yellow-400 rounded-md bg-yellow-50 dark:bg-yellow-900/20">
      <div className="flex items-center">
        <AlertCircle className="h-4 w-4 mr-2 flex-shrink-0" />
        <span>{modelWarning}</span>
      </div>
      <div className="mt-1 ml-6">
        {selectedModelProvider === 'ollama' ? (
          <Link to="/settings/local-models" className="underline">Configure Ollama</Link>
        ) : modelWarning?.includes("LMStudio") ? (
          <Link to="/settings/local-models" className="underline">Configure LMStudio</Link>
        ) : (
          <Link to="/settings/models" className="underline">Add API Key</Link>
        )}
      </div>
    </div>
  );
});
```

#### ChatInputArea

```tsx
// /apps/coder/src/components/ChatInputArea.tsx
import React, { memo } from 'react';
import { ChatForm } from '@/components/ui/chat';
import { MessageInput } from '@/components/ui/message-input';
import { ModelWarningBanner } from './ModelWarningBanner';
import { useModelContext } from '@/providers/ModelProvider';
import { useChatState } from '@/providers/ChatStateProvider';

export const ChatInputArea = memo(function ChatInputArea() {
  const { isModelAvailable } = useModelContext();
  const { 
    input, 
    handleInputChange, 
    handleSubmit,
    isGenerating,
    stop 
  } = useChatState();
  
  return (
    <div className="border-t bg-background p-4">
      <div className="mx-auto md:max-w-3xl lg:max-w-[40rem] xl:max-w-[48rem]">
        <ModelWarningBanner />
        
        <ChatForm
          isPending={isGenerating}
          handleSubmit={(event) => {
            if (!isModelAvailable && event?.preventDefault) {
              event.preventDefault();
              return;
            }
            handleSubmit(event);
          }}
          className="relative"
        >
          {({ files, setFiles }) => (
            <MessageInput
              value={input}
              onChange={(e) => {
                // Handle both string and event types
                if (typeof e === 'string') {
                  handleInputChange(e);
                } else if (e && e.target) {
                  handleInputChange(e.target.value);
                }
              }}
              allowAttachments={false}
              stop={stop}
              isGenerating={isGenerating}
              disabled={!isModelAvailable}
              placeholder={!isModelAvailable ? "API key required for this model" : "Message..."}
            />
          )}
        </ChatForm>
      </div>
    </div>
  );
});
```

#### MessageArea

```tsx
// /apps/coder/src/components/MessageArea.tsx
import React, { memo } from 'react';
import { MessageList } from '@/components/ui/message-list';
import { useChatState } from '@/providers/ChatStateProvider';

export const MessageArea = memo(function MessageArea() {
  const { messages, isGenerating } = useChatState();
  
  return (
    <div className="overflow-y-auto relative">
      <div className="absolute inset-0 p-4 pt-8">
        <div className="mx-auto md:max-w-3xl lg:max-w-[40rem] xl:max-w-[48rem]">
          <MessageList
            messages={messages}
            isTyping={isGenerating}
          />
        </div>
      </div>
    </div>
  );
});
```

### 3. Layout Component

```tsx
// /apps/coder/src/components/layout/MainLayout.tsx
import React, { memo } from 'react';
import {
  SidebarProvider,
  Sidebar,
  SidebarHeader,
  SidebarContent,
  SidebarFooter,
  SidebarMenu,
  SidebarMenuItem,
  SidebarMenuButton,
  SidebarInset
} from '@/components/ui/sidebar';
import { Link } from '@tanstack/react-router';
import { SettingsIcon } from 'lucide-react';
import ToggleTheme from '@/components/ToggleTheme';
import { AppHeader } from '@/components/AppHeader';
import { ModelHeader } from '@/components/ModelHeader';
import { ThreadList } from '@/components/ThreadList';
import { MessageArea } from '@/components/MessageArea';
import { ChatInputArea } from '@/components/ChatInputArea';
import { useChatState } from '@/providers/ChatStateProvider';

export const MainLayout = memo(function MainLayout() {
  const { 
    currentThreadId, 
    handleSelectThread,
    handleCreateThread,
    handleDeleteThread,
    handleRenameThread,
    threadListKey
  } = useChatState();
  
  return (
    <SidebarProvider defaultOpen={true}>
      <div className="flex h-screen w-full flex-col text-primary font-mono">
        <div className="relative flex h-full w-full flex-1 overflow-hidden z-0">
          <div className="mt-[30px] relative flex h-full w-full flex-row overflow-hidden">
            <Sidebar>
              <SidebarHeader className="border-y h-14 mt-[30px]">
                <AppHeader onCreateThread={handleCreateThread} />
              </SidebarHeader>

              <SidebarContent>
                <ThreadList
                  key={`thread-list-${threadListKey}`}
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
              <div className="grid grid-rows-[auto_minmax(0,1fr)_auto] h-[calc(100vh-30px)]">
                <div className="border-y bg-background p-3 flex items-center justify-between z-10 h-14">
                  <ModelHeader />
                </div>

                <MessageArea />
                <ChatInputArea />
              </div>
            </SidebarInset>
          </div>
        </div>
      </div>
    </SidebarProvider>
  );
});
```

### 4. Refactored HomePage

The HomePage is now dramatically simplified:

```tsx
// /apps/coder/src/pages/HomePage.tsx
import React, { useState, useEffect } from 'react';
import { useSettings, DEFAULT_SYSTEM_PROMPT } from '@openagents/core';
import { ModelProvider } from '@/providers/ModelProvider';
import { ApiKeyProvider } from '@/providers/ApiKeyProvider';
import { ChatStateProvider } from '@/providers/ChatStateProvider';
import { MainLayout } from '@/components/layout/MainLayout';

export default function HomePage() {
  // Get settings
  const { clearSettingsCache } = useSettings();
  
  // State for system prompt
  const [systemPrompt, setSystemPrompt] = useState<string>("");
  
  // Initialize database and load system prompt
  useEffect(() => {
    clearSettingsCache();
    
    // Initialize database early
    (async () => {
      try {
        // Import directly here to avoid circular dependencies
        const db = await import('@openagents/core/src/db/database');
        await db.getDatabase();
        console.log("Database initialized on startup");
        
        // Load system prompt
        const { settingsRepository } = await import('@openagents/core/src/db/repositories');
        const savedPrompt = await settingsRepository.getPreference("defaultSystemPrompt", DEFAULT_SYSTEM_PROMPT);
        setSystemPrompt(savedPrompt);
        console.log("Loaded system prompt:", savedPrompt === DEFAULT_SYSTEM_PROMPT ? "Using default prompt" : "Custom prompt loaded");
      } catch (error) {
        console.error("Failed to initialize database or load system prompt:", error);
        setSystemPrompt(DEFAULT_SYSTEM_PROMPT);
      }
    })();
  }, [clearSettingsCache]);
  
  return (
    <ModelProvider>
      <ApiKeyProvider>
        <ChatStateProvider systemPrompt={systemPrompt}>
          <MainLayout />
        </ChatStateProvider>
      </ApiKeyProvider>
    </ModelProvider>
  );
}
```

## Performance Optimizations Applied

1. **Comprehensive Memoization**
   - All UI components wrapped with React.memo
   - Event handlers memoized with useCallback
   - Complex calculations wrapped with useMemo

2. **Context Separation**
   - State divided into model, API key, and chat contexts
   - Changes in one context don't trigger rerenders in unrelated components

3. **Proper Type Handling**
   - Fixed type errors in component interactions
   - Created type-safe wrappers for event handlers

4. **Optimized Handler Functions**
   - Event handlers created with useCallback to preserve references
   - Handlers placed in the appropriate context providers

5. **Elimination of Prop Drilling**
   - Props no longer passed through multiple component levels
   - Components pull exactly what they need from context

## Results and Benefits

1. **Code Size Reduction**: Homepage component reduced from ~1000 lines to less than 50
2. **Better Performance**: Isolated state changes prevent cascade rerenders
3. **Improved Maintainability**: Each component has a clear, single responsibility
4. **Enhanced Developer Experience**: Easier to understand and modify
5. **More Resilient Code**: Better error handling and type safety

The refactored code now follows React best practices with proper component composition, context usage, and state management. This approach significantly reduces unnecessary rerenders and makes the codebase much easier to work with.