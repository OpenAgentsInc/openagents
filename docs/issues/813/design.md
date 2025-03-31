# RxDB Persistence Design Document

## Overview

This document outlines the design for implementing RxDB persistence in the OpenAgents Coder application, focusing on chat threads, messages, and user settings.

## Current Implementation

The current implementation in Coder uses:
- Vercel's AI SDK (`@ai-sdk/react`) for chat functionality
- In-memory message storage with no persistence
- A simple UI with a single chat thread
- No ability to switch between threads or manage history

Key code locations:
- `apps/coder/src/pages/HomePage.tsx` - Main chat interface using Vercel's `useChat` hook
- `apps/coder/src/components/ui/message-list.tsx` - Displays messages
- `apps/coder/src/components/ui/chat-message.tsx` - Renders individual messages

## RxDB Implementation Plan

### 1. Database Structure

The database will have three main collections:

**Threads Collection:**
- `id`: Unique thread identifier
- `title`: Thread title (auto-generated or user-defined)
- `createdAt`: Creation timestamp
- `updatedAt`: Last update timestamp
- `modelId`: The AI model used for this thread
- `systemPrompt`: Optional system prompt for the thread
- `metadata`: Additional thread metadata

**Messages Collection:**
- `id`: Unique message identifier
- `threadId`: Foreign key to thread
- `role`: Message role ('user', 'assistant', 'system')
- `content`: Message content
- `createdAt`: Creation timestamp
- `parts`: Serialized message parts (for advanced message rendering)
- `toolInvocations`: Serialized tool invocation data
- `attachments`: Serialized attachment data

**Settings Collection:**
- `id`: Settings identifier ('global' for app-wide settings)
- `theme`: UI theme preference
- `apiKeys`: Encrypted API keys
- `defaultModel`: Default model selection
- `preferences`: Other user preferences

### 2. Implementation Components

#### A. Database Module (`packages/core/src/db`)

**Database Initialization (`database.ts`):**
```typescript
import { createRxDatabase, addRxPlugin } from 'rxdb';
import { getRxStorageDexie } from 'rxdb/plugins/dexie';
import { schemas } from './schema';

// Create database singleton
export const createDatabase = async () => {
  const db = await createRxDatabase({
    name: 'openagents',
    storage: getRxStorageDexie(),
  });
  
  // Add collections
  await db.addCollections({
    threads: { schema: schemas.threadSchema },
    messages: { schema: schemas.messageSchema },
    settings: { schema: schemas.settingsSchema },
  });
  
  return db;
};
```

**Schema Definitions (`schema.ts`):**
Define all collection schemas with proper types and validation rules.

**Repository Pattern:**
Create repositories for each collection to abstract database operations.

#### B. Chat Store Extension

**Custom useChat Hook (`packages/core/src/chat/usePersistentChat.ts`):**
```typescript
import { useChat as vercelUseChat } from '@ai-sdk/react';
import { useState, useEffect } from 'react';
import { threadRepository, messageRepository } from '../db/repositories';

export const usePersistentChat = (options) => {
  const { id } = options;
  const [threadId, setThreadId] = useState(id || 'default');
  
  // Load messages for the thread
  useEffect(() => {
    if (threadId) {
      const loadThread = async () => {
        const messages = await messageRepository.getMessagesByThreadId(threadId);
        // Set initial messages
      };
      loadThread();
    }
  }, [threadId]);
  
  // Use Vercel's useChat with our options
  const chatState = vercelUseChat({
    ...options,
    onFinish: (message) => {
      // Save message to database
      messageRepository.createMessage({
        threadId,
        ...message
      });
      options.onFinish?.(message);
    }
  });
  
  // Add thread management functions
  return {
    ...chatState,
    // Thread management
    currentThreadId: threadId,
    switchThread: setThreadId,
    createNewThread: async (title) => {
      const newThread = await threadRepository.createThread({ title });
      setThreadId(newThread.id);
      return newThread;
    },
    getAllThreads: () => threadRepository.getAllThreads(),
    deleteThread: (id) => threadRepository.deleteThread(id),
    // Message management extensions
    deleteMessage: (id) => messageRepository.deleteMessage(id),
  };
}
```

#### C. UI Components for Thread Management

**Thread Selector Component:**
Create a component to list and select threads, with the ability to create new ones and delete existing ones.

**Thread Management UI:**
Add UI elements for:
- Creating new threads
- Renaming threads
- Deleting threads
- Viewing thread history

### 3. Integration Plan

1. First implement the database layer with RxDB and repositories
2. Create the custom hook that extends Vercel's useChat with persistence
3. Update HomePage.tsx to use the new hook
4. Add UI components for thread management
5. Implement settings persistence

### 4. Cross-Platform Considerations

The implementation will work in both Electron and browser environments:
- RxDB with Dexie adapter for browser-based storage
- Proper serialization/deserialization of complex objects
- Encryption for sensitive data (API keys)

### 5. Migration Strategy

The schema includes version numbers to support future migrations:
- Initial version set to 0
- RxDB migration capabilities for future schema updates
- Data export/import options for major version changes

## Implementation Phases

1. **Phase 1 - Core Database Layer:**
   - Implement RxDB with schemas and repositories
   - Create basic thread/message persistence

2. **Phase 2 - Extended Hook:**
   - Create persistent chat hook extending Vercel's useChat
   - Add thread management functions

3. **Phase 3 - UI Integration:**
   - Update Coder UI to support thread management
   - Add thread selection sidebar

4. **Phase 4 - Settings & Preferences:**
   - Implement settings persistence
   - Add UI for customizing preferences

## Conclusion

This design provides a solid foundation for adding persistence to the OpenAgents Coder application. By implementing at the core package level and using RxDB, we ensure flexibility for future enhancements and integration with other applications in the ecosystem.