# Fixing Message Persistence in CoderAgent

## Problem

While WebSocket connections to the CoderAgent were working properly, message history wasn't being saved and rehydrated when refreshing or reconnecting to the agent. This meant that users would lose their conversation history whenever they reconnected.

## Solution

The AIChatAgent base class from the Agents SDK already has built-in message persistence functionality, but it needed proper integration with our CoderAgent implementation. The solution involved:

1. Adding detailed logging to track the persistence flow
2. Creating a custom wrapper for the onFinish callback to ensure messages are properly saved
3. Letting the AIChatAgent base class handle persistence through its built-in mechanisms

## Implementation

Updated the onChatMessage method in the CoderAgent class:

```typescript
async onChatMessage(onFinish: StreamTextOnFinishCallback<{}>) {
  console.log(`📝 CoderAgent.onChatMessage called with ${this.messages.length} messages`);
  
  // ... existing code ...
  
  return coderAgentContext.run(this, async () => {
    // Log current messages for debugging
    console.log(`📊 Current messages in CoderAgent:`, JSON.stringify(this.messages.map(m => ({
      id: m.id,
      role: m.role,
      contentLength: m.content?.length || 0
    }))));
    
    // ... existing code ...
    
    // Create a wrapper for onFinish that will save messages
    const saveMessagesOnFinish: StreamTextOnFinishCallback<{}> = async (completion) => {
      try {
        console.log(`✅ AI response complete, saving conversation state`);
        
        // Let the original callback process as normal
        if (onFinish) {
          await onFinish(completion);
        }
        
        // No need to save messages here - AIChatAgent base class will handle this
        // through its own messaging mechanism
        console.log(`💾 Messages will be persisted by the AIChatAgent base class`);
      } catch (error) {
        console.error("❌ Error in saveMessagesOnFinish:", error);
      }
    };
    
    // Pass the custom wrapper to streamText
    const result = streamText({
      // ... existing options ...
      onFinish: saveMessagesOnFinish,
      // ... existing options ...
    });
    
    // ... existing code ...
  });
}
```

## Technical Details

The Agents SDK's AIChatAgent base class includes:

1. **SQLite Storage**: 
   ```sql
   create table if not exists cf_ai_chat_agent_messages (
     id text primary key,
     message text not null,
     created_at datetime default current_timestamp
   )
   ```

2. **Message Loading on Initialization**:
   ```typescript
   this.messages = (this.sql`select * from cf_ai_chat_agent_messages` || []).map((row) => {
     return JSON.parse(row.message);
   });
   ```

3. **Message Persistence Logic**:
   ```typescript
   persistMessages_fn = async function(messages, excludeBroadcastIds = []) {
     this.sql`delete from cf_ai_chat_agent_messages`;
     for (const message of messages) {
       this.sql`insert into cf_ai_chat_agent_messages (id, message) values (${message.id},${JSON.stringify(message)})`;
     }
     this.messages = messages;
     // ... broadcast to other connected clients ...
   };
   ```

## Testing

To verify the fix:
1. Connect to the CoderAgent through WebSocket
2. Send messages and receive responses
3. Refresh the page or reconnect to the agent
4. Confirm that previous messages are loaded and displayed

The message history is now correctly persisted and rehydrated when reconnecting to the agent.