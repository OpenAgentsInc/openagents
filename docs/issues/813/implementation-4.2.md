# RxDB Implementation with Vercel AI SDK 4.2

Based on examining the current codebase and the Vercel AI SDK 4.2 changes, here's the updated implementation plan for adding RxDB persistence to OpenAgents.

## Key Changes in 4.2

The main difference in Vercel AI SDK 4.2 is the more comprehensive parts-based message structure that replaces the previous `toolInvocations` approach. The new structure uses:

- A message contains multiple `parts` with different types
- Part types include: `text`, `reasoning`, `tool-invocation`, `source`, and `file`
- The parts preserve the exact sequence of different output types

## Database Schema Update

### Thread Schema (No Changes)
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

### Message Schema (Updated for 4.2)
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
    
    // Parts array stores the structured message parts
    parts: { 
      type: 'array', 
      items: {
        type: 'object',
        properties: {
          type: { type: 'string' } // 'text', 'reasoning', 'tool-invocation', 'source', 'file'
        }
      },
      optional: true 
    },
    
    // Keep old field for backward compatibility, marked as deprecated
    toolInvocations: { type: 'array', optional: true }, 
    
    // Attachments for user uploads
    attachments: { type: 'array', optional: true }
  },
  required: ['id', 'threadId', 'role', 'content', 'createdAt']
};
```

## Database Storage Considerations

### Storing Message Parts

Since message parts can contain complex objects that can't be directly saved in IndexedDB, they need to be serialized properly. We'll handle this in the repository layer:

```typescript
// In message-repository.ts
export class MessageRepository {
  // ...

  async createMessage(messageData: UIMessage): Promise<any> {
    // Clone the message to avoid modifying the original
    const messageToSave = { ...messageData };
    
    // Convert Date to timestamp number
    if (messageToSave.createdAt instanceof Date) {
      messageToSave.createdAt = messageToSave.createdAt.getTime();
    } else if (!messageToSave.createdAt) {
      messageToSave.createdAt = Date.now();
    }
    
    // Serialize parts if they exist
    if (messageToSave.parts) {
      // Parts need to be properly serialized
      messageToSave.parts = this.serializeMessageParts(messageToSave.parts);
    }
    
    // Insert into database
    return this.collection.insert(messageToSave);
  }
  
  private serializeMessageParts(parts: Array<any>): Array<any> {
    return parts.map(part => {
      // Create a deep copy to avoid reference issues
      const serializedPart = { ...part };
      
      // Handle specific part types
      switch (part.type) {
        case 'file':
          // Ensure file data is serializable
          return {
            ...serializedPart,
            // Keep base64 data as is
          };
        case 'tool-invocation':
          // Ensure tool invocation is serializable
          return {
            ...serializedPart,
            toolInvocation: JSON.parse(JSON.stringify(part.toolInvocation)),
          };
        case 'source':
          // Ensure source is serializable
          return {
            ...serializedPart,
            source: JSON.parse(JSON.stringify(part.source)),
          };
        case 'reasoning':
        case 'text':
        default:
          // These types are already serializable
          return serializedPart;
      }
    });
  }
  
  // When retrieving a message, deserialize the parts
  private deserializeMessageParts(parts: Array<any>): Array<any> {
    if (!parts) return [];
    
    return parts.map(part => {
      // Create a deep copy
      const deserializedPart = { ...part };
      
      // Add any specific deserialization logic if needed
      // Most parts can be used as-is once retrieved
      
      return deserializedPart;
    });
  }
  
  async getMessageById(id: string): Promise<UIMessage | null> {
    const message = await this.collection.findOne(id).exec();
    
    if (!message) return null;
    
    // Convert to UIMessage format
    const uiMessage = message.toJSON() as UIMessage;
    
    // Convert timestamp back to Date
    if (typeof uiMessage.createdAt === 'number') {
      uiMessage.createdAt = new Date(uiMessage.createdAt);
    }
    
    // Deserialize parts if they exist
    if (uiMessage.parts) {
      uiMessage.parts = this.deserializeMessageParts(uiMessage.parts);
    }
    
    return uiMessage;
  }
  
  // ...
}
```

## Integration with useChat

The integration with useChat needs to handle the new parts-based message structure:

```typescript
// In usePersistentChat.ts
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
        if (messages.length > 0) {
          chatState.setMessages(messages);
        }
      };
      loadThread();
    }
  }, [threadId]);
  
  // Use Vercel's useChat with our options
  const chatState = vercelUseChat({
    ...options,
    onFinish: (message) => {
      // Save message to database when streaming completes
      messageRepository.createMessage({
        threadId,
        ...message
      });
      
      // Update thread's updatedAt timestamp
      threadRepository.updateThread(threadId, {
        updatedAt: Date.now()
      });
      
      // Call original onFinish if provided
      options.onFinish?.(message);
    }
  });
  
  // Override append to save user messages immediately
  const originalAppend = chatState.append;
  const append = async (message) => {
    // If this is a user message, save it immediately
    if (message.role === 'user') {
      await messageRepository.createMessage({
        threadId,
        id: message.id || crypto.randomUUID(),
        createdAt: message.createdAt || new Date(),
        ...message
      });
      
      // Update thread's updatedAt timestamp
      await threadRepository.updateThread(threadId, {
        updatedAt: Date.now()
      });
    }
    
    // Call original append
    return originalAppend(message);
  };
  
  // Add thread management functions
  return {
    ...chatState,
    append,
    // Thread management
    currentThreadId: threadId,
    switchThread: setThreadId,
    createNewThread: async (title) => {
      const newThread = await threadRepository.createThread({ 
        title,
        createdAt: Date.now(),
        updatedAt: Date.now()
      });
      setThreadId(newThread.id);
      return newThread;
    },
    getAllThreads: () => threadRepository.getAllThreads(),
    deleteThread: (id) => threadRepository.deleteThread(id),
    // Message management extensions
    deleteMessage: (id) => messageRepository.deleteMessage(id)
  };
};
```

## UI Implementation for Parts

When displaying messages in the UI, we need to handle the new parts-based structure:

```tsx
function MessageDisplay({ message }) {
  // If the message has parts, render each part individually
  if (message.parts && message.parts.length > 0) {
    return (
      <div className="message">
        {message.parts.map((part, index) => {
          switch (part.type) {
            case 'text':
              return <MarkdownRenderer key={index}>{part.text}</MarkdownRenderer>;
            case 'reasoning':
              return <ReasoningBlock key={index} reasoning={part.reasoning} />;
            case 'tool-invocation':
              return <ToolInvocationBlock key={index} toolInvocation={part.toolInvocation} />;
            case 'source':
              return <SourceBlock key={index} source={part.source} />;
            case 'file':
              return <FileDisplay key={index} file={part} />;
            default:
              return null;
          }
        })}
      </div>
    );
  }
  
  // Fallback to content for backward compatibility
  return <MarkdownRenderer>{message.content}</MarkdownRenderer>;
}
```

## Thread Management UI

```tsx
function ThreadSidebar({ currentThreadId, onSelectThread, onCreateThread, onDeleteThread }) {
  const { threads, isLoading } = useThreads();
  
  return (
    <div className="thread-sidebar">
      <button 
        className="new-thread-button"
        onClick={() => onCreateThread("New Chat")}
      >
        New Chat
      </button>
      
      {isLoading ? (
        <div>Loading...</div>
      ) : (
        <ul className="thread-list">
          {threads.map(thread => (
            <li 
              key={thread.id}
              className={thread.id === currentThreadId ? "active" : ""}
              onClick={() => onSelectThread(thread.id)}
            >
              <span className="thread-title">{thread.title || "Untitled"}</span>
              <span className="thread-date">{formatDate(thread.updatedAt)}</span>
              <button 
                className="delete-thread"
                onClick={(e) => {
                  e.stopPropagation();
                  onDeleteThread(thread.id);
                }}
              >
                Delete
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
```

## Enhanced Integration

The integration with the Coder app should be straightforward:

```tsx
// In HomePage.tsx
import { usePersistentChat } from "@openagents/core";
import { ThreadSidebar } from "../components/ThreadSidebar";

export default function HomePage() {
  const { 
    messages, 
    input, 
    handleInputChange, 
    handleSubmit, 
    isLoading: isGenerating, 
    stop,
    // Thread management
    currentThreadId,
    switchThread,
    createNewThread,
    deleteThread
  } = usePersistentChat({ 
    api: "https://chat.openagents.com", 
    maxSteps: 10 
  });

  return (
    <SidebarProvider defaultOpen={true}>
      <div className="flex h-full w-full flex-col text-primary font-mono">
        <div className="relative flex h-full w-full flex-1 overflow-hidden z-0">
          {/* Add the thread sidebar */}
          <ThreadSidebar 
            currentThreadId={currentThreadId}
            onSelectThread={switchThread}
            onCreateThread={createNewThread}
            onDeleteThread={deleteThread}
          />
          
          {/* Rest of the UI stays the same */}
          <div className="grid grid-rows-[auto_1fr_auto] h-screen">
            {/* Message list and input components */}
            <MessageList
              messages={messages}
              isTyping={isGenerating}
            />
            
            {/* Chat form */}
            <ChatForm
              isPending={isGenerating}
              handleSubmit={handleSubmit}
            >
              {/* Message input */}
            </ChatForm>
          </div>
        </div>
      </div>
    </SidebarProvider>
  );
}
```

This implementation takes full advantage of the new parts-based message structure in Vercel AI SDK 4.2 while also providing robust persistence via RxDB.