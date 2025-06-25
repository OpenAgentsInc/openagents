# Claude Code Message Display Fixes

## Issue Summary
The Claude Code chat UI was not displaying user messages properly because:
1. User messages in the Convex database had empty content (`""`)
2. The parsing logic wasn't handling different entry types correctly
3. Tool calls were not being displayed as separate system messages

## Debug Findings

### Database State
Running analysis on the Convex database revealed:
- 38 Claude Code sessions imported
- 491 user messages with empty content
- Assistant messages have content, but user messages were lost during import

### Root Cause
The import process from JSONL to Convex didn't properly extract user message content. The JSONL format stores user messages like:
```json
{
  "type": "user",
  "message": {
    "role": "user", 
    "content": [
      {
        "type": "text",
        "text": "actual user message here"
      }
    ]
  }
}
```

But the DatabaseMapper was trying to access `entry.message.text` directly, which doesn't exist in this structure.

## Fixes Applied

### 1. Enhanced Debug Logging
Added extensive logging in `chat-client-convex.ts` to trace message parsing:
- Log first 5 messages in detail
- Log role determination logic
- Log content parsing steps
- Log transformations

### 2. Improved Message Parsing
Updated `parseMessageContent()` function to:
- Handle empty user messages with placeholder text
- Format tool use/results with emojis and code blocks
- Parse assistant messages correctly
- Add proper role detection

### 3. Message Filtering
Added filtering to remove empty summary entries that shouldn't be displayed.

### 4. UI Styling Updates
Enhanced `chat-utils.ts` to:
- Support system role messages
- Add specific styling for tool use/results
- Add color-coded borders for different message types

### 5. Database Mapper Fix
Fixed `DatabaseMapper.ts` to properly extract user message content from the nested structure.

## Testing

### Debug Endpoints
Created debug endpoints to inspect data:
- `/api/debug-conversations` - List all conversations
- `/api/debug-messages/:id` - Inspect messages for a conversation

### Sample JSONL
Found example data at `/docs/ingest/claude-code-example.jsonl` showing the correct format.

## Remaining Work

To fully fix the issue:
1. Re-import all JSONL files with the corrected DatabaseMapper
2. Or create a Convex mutation to update existing messages
3. Or use the Convex dashboard to manually fix key conversations

## Temporary Workaround
User messages now display `[User message content was not properly imported]` instead of being blank, making the conversation flow visible even without the original content.