# RxDB Persistence Implementation Summary

## Overview

This document summarizes the key points for implementing RxDB-based persistence for chat threads, messages, and settings in the OpenAgents Coder application using Vercel AI SDK 4.2.

## Key Components

### 1. Database Structure

**Collections:**
- **Threads**: Stores conversation metadata
- **Messages**: Stores individual messages with parts
- **Settings**: Stores user preferences

### 2. Key Features

- **Persistent Chat History**: Chat threads and all messages are saved between sessions
- **Thread Management**: Create, view, switch, and delete conversation threads
- **Parts-Based Message Storage**: Support for Vercel AI SDK 4.2's message parts structure
- **Settings Storage**: User preferences and configurations

### 3. Integration Points

- Replace Vercel's `useChat` with a custom `usePersistentChat` hook
- Seamlessly integrate with existing UI components
- Minimal changes required to the Coder app's main UI

## Implementation Approach

### Phase 1: Core Database Layer

1. Set up RxDB with Dexie adapter for IndexedDB
2. Define schemas for threads, messages, and settings
3. Create repositories for database operations

### Phase 2: Persistence Hooks

1. Implement `usePersistentChat` hook that extends Vercel's `useChat`
2. Add thread creation and switching functionality
3. Add message persistence on send and receive

### Phase 3: UI Integration

1. Add thread selection sidebar
2. Add UI for thread management (create/delete)
3. Update HomePage.tsx to use the persistence hooks

## Code Structure

```
packages/core/src/
├── db/
│   ├── index.ts                 # Main exports
│   ├── database.ts              # Database initialization
│   ├── schema.ts                # Collection schemas
│   ├── repositories/            # Data access
│   │   ├── thread-repository.ts # Thread operations
│   │   ├── message-repository.ts # Message operations
│   │   └── settings-repository.ts # Settings operations
│   └── types.ts                 # TypeScript types
└── chat/
    ├── usePersistentChat.ts     # Main persistence hook
    └── useThreads.ts            # Thread management hook
```

## Message Structure (Vercel AI SDK 4.2)

The implementation accommodates the new parts-based message structure in Vercel AI SDK 4.2:

```typescript
interface Message {
  id: string;
  role: 'user' | 'assistant' | 'system' | 'data';
  content: string; // Fallback content
  createdAt?: Date;
  parts?: Array<
    | TextUIPart          // { type: 'text', text: string }
    | ReasoningUIPart     // { type: 'reasoning', reasoning: string, details: Array<...> }
    | ToolInvocationUIPart // { type: 'tool-invocation', toolInvocation: ToolInvocation }
    | SourceUIPart        // { type: 'source', source: LanguageModelV1Source }
    | FileUIPart          // { type: 'file', mimeType: string, data: string }
  >;
  experimental_attachments?: Attachment[];
}
```

## Usage in Coder App

In the Coder app's HomePage.tsx:

```tsx
// Replace this:
const { messages, input, handleInputChange, handleSubmit, isLoading: isGenerating, stop }
  = useChat({ api: "https://chat.openagents.com", maxSteps: 10 })

// With this:
const { 
  messages, 
  input, 
  handleInputChange, 
  handleSubmit, 
  isLoading: isGenerating, 
  stop,
  // New capabilities:
  currentThreadId,
  switchThread,
  createNewThread,
  deleteThread
} = usePersistentChat({ 
  api: "https://chat.openagents.com", 
  maxSteps: 10 
})
```

## Benefits

1. **User Experience**: Users can return to previous conversations after closing the app
2. **Organization**: Conversations are organized into separate threads
3. **Data Ownership**: All data is stored locally on the user's device
4. **Performance**: Leverages RxDB's reactive queries for real-time UI updates
5. **Compatibility**: Works with the latest Vercel AI SDK features

## Next Steps

1. Implement the core database module
2. Create the persistence hooks
3. Update the Coder app UI
4. Add thread management functionality
5. Test thoroughly across different scenarios
6. Consider adding export/import functionality