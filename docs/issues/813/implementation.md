# RxDB Persistence Implementation Plan

Based on examining the current codebase, here's a detailed implementation plan for adding RxDB persistence to OpenAgents.

## 1. Dependencies to Add

Add these dependencies to the `packages/core` package:
```
rxdb
@rxdb/utils
dexie
```

## 2. Database Module Structure

Create the following file structure in `packages/core/src/db`:

```
db/
├── index.ts                 # Main entry point, exports everything
├── schema.ts                # Database and collection schemas
├── database.ts              # Database initialization and connection
├── repositories/            # Repository pattern implementation
│   ├── index.ts             # Exports all repositories
│   ├── thread-repository.ts # Thread operations
│   ├── message-repository.ts # Message operations
│   └── settings-repository.ts # Settings operations
└── types.ts                 # Type definitions for database objects
```

## 3. Schema Design

### Thread Schema
```typescript
export const threadSchema = {
  version: 0,
  primaryKey: 'id',
  type: 'object',
  properties: {
    id: { type: 'string' },
    title: { type: 'string' },
    createdAt: { type: 'number' },
    updatedAt: { type: 'number' },
    modelId: { type: 'string', optional: true },
    systemPrompt: { type: 'string', optional: true },
    metadata: { type: 'object', optional: true }
  },
  required: ['id', 'createdAt', 'updatedAt']
};
```

### Message Schema
```typescript
export const messageSchema = {
  version: 0,
  primaryKey: 'id',
  type: 'object',
  properties: {
    id: { type: 'string' },
    threadId: { type: 'string' },
    role: { type: 'string' }, // 'user', 'assistant', 'system'
    content: { type: 'string' },
    createdAt: { type: 'number' },
    parts: { type: 'array', optional: true }, // Serialized message parts
    toolInvocations: { type: 'array', optional: true }, // Serialized tool invocations
    attachments: { type: 'array', optional: true } // Serialized attachments
  },
  required: ['id', 'threadId', 'role', 'content', 'createdAt']
};
```

### Settings Schema
```typescript
export const settingsSchema = {
  version: 0,
  primaryKey: 'id',
  type: 'object',
  properties: {
    id: { type: 'string' },
    theme: { type: 'string', optional: true },
    apiKeys: { type: 'object', optional: true },
    defaultModel: { type: 'string', optional: true },
    preferences: { type: 'object', optional: true }
  },
  required: ['id']
};
```

## 4. Database Initialization

The database initialization module should:
- Create a singleton database instance
- Handle connection/creation
- Set up schema migrations
- Provide access to collection repositories

## 5. Repository Implementation

Each repository will provide CRUD operations:

### ThreadRepository
- createThread(threadData)
- getThreadById(id)
- getAllThreads()
- updateThread(id, updates)
- deleteThread(id)
- getThreadWithMessages(id)

### MessageRepository
- createMessage(messageData)
- getMessageById(id)
- getMessagesByThreadId(threadId)
- updateMessage(id, updates)
- deleteMessage(id)
- bulkAddMessages(messages)

### SettingsRepository
- getSettings()
- updateSettings(updates)
- setApiKey(provider, key)
- getApiKey(provider)

## 6. Integration with useChat

Modify `useChat.ts` to integrate with the database:

1. Add a new parameter `persistenceEnabled` (default true)
2. On initialization, load thread and messages if an ID is provided
3. On message append, save to the database
4. Create utility functions for thread management:
   - loadThread(id)
   - createNewThread(options)
   - deleteThread(id)
   - updateThreadMetadata(id, metadata)

## 7. Migration Strategy

Implement a versioned schema approach:
- Add version numbers to each schema
- Create migrations for future schema changes
- Auto-migrate on database initialization

## 8. Additional Features

Consider implementing:
- Message search capabilities
- Thread export/import
- Message attachment storage
- Thread tagging
- Thread title auto-generation

## 9. Testing Strategy

- Create unit tests for each repository
- Add integration tests for database operations
- Test thread persistence across page reloads
- Test message saving/loading
- Test large message history performance

## Implementation Timeline

1. Set up basic database structure and schemas (1 day)
2. Implement repositories with CRUD operations (1-2 days)
3. Integrate with useChat (1 day)
4. Add thread management UI components (1 day)
5. Testing and refinement (1-2 days)

This implementation plan provides a solid foundation for adding persistent storage to the OpenAgents platform, focusing first on the Coder app with the ability to extend to other apps in the future.