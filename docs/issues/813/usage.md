# RxDB Persistence Usage Guide

This document provides guidance on how to use the RxDB persistence implementation in the OpenAgents platform.

## Basic Usage

### Using the Persistent Chat Hook

Replace the standard Vercel AI SDK chat hook with our persistent version:

```tsx
// Before:
import { useChat } from "@ai-sdk/react"

// After:
import { usePersistentChat } from "@openagents/core"

function ChatComponent() {
  const {
    messages,
    input,
    handleInputChange,
    handleSubmit,
    isLoading,
    // New thread management capabilities:
    currentThreadId,
    switchThread,
    createNewThread,
    getAllThreads,
    deleteThread
  } = usePersistentChat({
    api: "https://chat.openagents.com",
    maxSteps: 10,
    // Optionally specify a thread ID to load:
    id: "thread-123"
  })
  
  // Rest of component remains the same
}
```

### Thread Management

```tsx
import { usePersistentChat, useThreads } from "@openagents/core"

function ChatWithThreads() {
  // Hook for thread management only
  const { threads, isLoading: threadsLoading } = useThreads()
  
  // Full chat hook with a specific thread
  const { 
    messages,
    // Other chat methods...
    currentThreadId,
    switchThread,
    createNewThread
  } = usePersistentChat({ id: threads[0]?.id })
  
  // Create a new thread
  const handleNewThread = async () => {
    const newThread = await createNewThread("New Conversation")
    // Thread is automatically selected
  }
  
  // Switch to another thread
  const handleSwitchThread = (threadId) => {
    switchThread(threadId)
  }
  
  return (
    <div className="flex">
      {/* Thread sidebar */}
      <div className="w-64 border-r">
        <button onClick={handleNewThread}>New Thread</button>
        
        {threadsLoading ? (
          <div>Loading threads...</div>
        ) : (
          <ul>
            {threads.map(thread => (
              <li 
                key={thread.id}
                className={thread.id === currentThreadId ? "bg-muted" : ""}
                onClick={() => handleSwitchThread(thread.id)}
              >
                {thread.title || "Untitled"} - {formatDate(thread.updatedAt)}
              </li>
            ))}
          </ul>
        )}
      </div>
      
      {/* Chat area */}
      <div className="flex-1">
        {/* Existing message list and input components */}
      </div>
    </div>
  )
}
```

### Settings Management

```tsx
import { useSettings } from "@openagents/core"

function SettingsComponent() {
  const { 
    settings, 
    updateSettings,
    isLoading
  } = useSettings()
  
  const handleThemeChange = (theme) => {
    updateSettings({ theme })
  }
  
  const handleModelChange = (model) => {
    updateSettings({ defaultModel: model })
  }
  
  return (
    <div>
      <h2>Settings</h2>
      
      {isLoading ? (
        <div>Loading settings...</div>
      ) : (
        <>
          <div>
            <label>Theme</label>
            <select 
              value={settings.theme || "light"} 
              onChange={e => handleThemeChange(e.target.value)}
            >
              <option value="light">Light</option>
              <option value="dark">Dark</option>
              <option value="system">System</option>
            </select>
          </div>
          
          <div>
            <label>Default Model</label>
            <select 
              value={settings.defaultModel || "claude-3-5-sonnet"} 
              onChange={e => handleModelChange(e.target.value)}
            >
              <option value="claude-3-5-sonnet">Claude 3.5 Sonnet</option>
              <option value="claude-3-haiku">Claude 3 Haiku</option>
              <option value="gpt-4">GPT-4</option>
            </select>
          </div>
        </>
      )}
    </div>
  )
}
```

## Advanced Usage

### Direct Database Access

For advanced scenarios, you can access the database repositories directly:

```tsx
import { 
  threadRepository, 
  messageRepository, 
  settingsRepository 
} from "@openagents/core/db"

// Example: Export all chat history for a thread
async function exportThreadHistory(threadId) {
  const thread = await threadRepository.getThreadById(threadId)
  const messages = await messageRepository.getMessagesByThreadId(threadId)
  
  return {
    thread,
    messages
  }
}

// Example: Import chat history
async function importChatHistory(data) {
  const { thread, messages } = data
  
  // Create a new thread with the provided data
  const newThread = await threadRepository.createThread({
    ...thread,
    id: undefined, // Generate a new ID
    createdAt: Date.now(),
    updatedAt: Date.now()
  })
  
  // Import all messages
  for (const message of messages) {
    await messageRepository.createMessage({
      ...message,
      id: undefined, // Generate a new ID
      threadId: newThread.id
    })
  }
  
  return newThread.id
}
```

### Subscribing to Data Changes

RxDB provides reactive queries that you can use to subscribe to data changes:

```tsx
import { threadRepository } from "@openagents/core/db"
import { useEffect, useState } from "react"

function ThreadList() {
  const [threads, setThreads] = useState([])
  
  useEffect(() => {
    // Get all threads, sorted by update time
    const query = threadRepository.createQuery({
      sort: [{ updatedAt: 'desc' }]
    })
    
    // Subscribe to changes
    const subscription = query.$.subscribe(results => {
      setThreads(results)
    })
    
    // Clean up subscription
    return () => subscription.unsubscribe()
  }, [])
  
  return (
    <ul>
      {threads.map(thread => (
        <li key={thread.id}>{thread.title}</li>
      ))}
    </ul>
  )
}
```

### Handling Migrations

The database is designed to handle migrations automatically, but for major changes, you may need to implement a custom migration:

```tsx
import { db } from "@openagents/core/db"

// Example: Migrate from v0 to v1 of the thread schema
async function migrateThreadSchema() {
  // Get all threads
  const threads = await db.threads.find().exec()
  
  // Update each thread to add a new field
  for (const thread of threads) {
    await thread.update({
      $set: {
        newField: 'default value',
      }
    })
  }
}
```

## Implementation Requirements

### Required in HomePage.tsx

To implement the persistent chat in the Coder app's HomePage.tsx:

1. Import the persistence hooks:
```tsx
import { usePersistentChat, useThreads } from "@openagents/core"
```

2. Replace the existing useChat with usePersistentChat:
```tsx
const { 
  messages, 
  input, 
  handleInputChange, 
  handleSubmit, 
  isLoading: isGenerating, 
  stop,
  // Thread management
  currentThreadId,
  getAllThreads,
  switchThread,
  createNewThread
} = usePersistentChat({ 
  api: "https://chat.openagents.com", 
  maxSteps: 10 
})
```

3. Add thread management UI to the sidebar:
```tsx
<SidebarContent>
  <SidebarGroup>
    <button 
      className="w-full text-left px-3 py-2 text-sm font-semibold hover:bg-muted"
      onClick={() => createNewThread("New Chat")}
    >
      + New Chat
    </button>
    
    <ThreadList 
      currentThreadId={currentThreadId}
      onSelectThread={switchThread}
    />
  </SidebarGroup>
</SidebarContent>
```

4. Create a ThreadList component:
```tsx
function ThreadList({ currentThreadId, onSelectThread }) {
  const { threads, isLoading } = useThreads()
  
  if (isLoading) {
    return <div className="p-3 text-sm">Loading...</div>
  }
  
  return (
    <ul className="space-y-1">
      {threads.map(thread => (
        <li 
          key={thread.id}
          className={`px-3 py-2 text-sm cursor-pointer hover:bg-muted ${
            thread.id === currentThreadId ? 'bg-muted' : ''
          }`}
          onClick={() => onSelectThread(thread.id)}
        >
          {thread.title || "Untitled"}
        </li>
      ))}
    </ul>
  )
}
```

This implementation provides a solid foundation for chat persistence in the OpenAgents Coder application while maintaining compatibility with the existing UI components.