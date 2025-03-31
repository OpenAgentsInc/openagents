# Implementing RxDB Persistence for OpenAgents

## Overview

Issue #813 calls for implementing local persistence for chat threads, messages, and user settings in the OpenAgents project, specifically starting with the Coder app. The goal is to create a robust database solution using RxDB and IndexedDB that will:

1. Store and persist chat threads and their metadata
2. Store and persist all messages within threads
3. Store user settings and preferences
4. Provide a foundation that can be shared across multiple apps

## Current Architecture

After examining the codebase:
- Chat functionality is in `packages/core/src/chat/useChat.ts`
- Currently uses an in-memory approach with no persistence
- Messages structure is defined in `chat/types.ts`
- The system relies on Vercel's AI SDK (`@ai-sdk/react`)

## Implementation Plan

To implement RxDB persistence, we need to:

1. **Add Dependencies**:
   - RxDB for reactive database operations
   - Dexie.js for IndexedDB storage

2. **Create Database Module**:
   - Setup in `packages/core/src/db` directory
   - Define database connection and initialization
   - Create schema validation

3. **Define Database Collections**:
   - **Threads Collection**: Store thread metadata, timestamps, AI model info
   - **Messages Collection**: Store message content, role, thread association
   - **Settings Collection**: Store user preferences, theme, API keys

4. **Implement Database Services**:
   - Thread operations (CRUD)
   - Message operations (CRUD)
   - Settings operations (CRUD)
   - Query capabilities

5. **Integrate with Existing Chat System**:
   - Modify `useChat.ts` to use the database
   - Ensure proper loading/saving of messages
   - Handle optimistic updates

6. **Add Migration Support**:
   - Prepare for future schema changes
   - Version the database schema

## Technical Considerations

- Ensure the database works across platforms (browser/electron)
- Implement proper error handling
- Create a clean abstraction layer for database operations
- Optimize for performance with large message histories
- Ensure type safety throughout the implementation

## Implementation Focus

The implementation will begin in the Coder app but will be built in the shared `core` package to facilitate reuse in other apps like Onyx. This approach ensures consistency across the OpenAgents ecosystem while minimizing duplicate code.