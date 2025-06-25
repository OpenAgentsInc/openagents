# JSONL Parser Comprehensive Guide

## Overview

The JSONL Parser is a critical component of the Overlord service that processes Claude Code conversation files and transforms them into structured data for storage in Convex. This guide documents the parser's architecture, the Claude Code JSONL format, and all modifications made to handle various edge cases.

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Claude Code JSONL Format](#claude-code-jsonl-format)
3. [Message Types and Structures](#message-types-and-structures)
4. [Parser Implementation](#parser-implementation)
5. [Key Modifications and Fixes](#key-modifications-and-fixes)
6. [Integration with Database Mapper](#integration-with-database-mapper)
7. [Common Issues and Solutions](#common-issues-and-solutions)
8. [Testing and Debugging](#testing-and-debugging)

## Architecture Overview

```
Claude Code JSONL Files → JSONLParser → LogEntry[] → DatabaseMapper → Convex Database
                                ↓                           ↓
                        Parse & Extract              Transform to Records
                          Content                    (Session + Messages)
```

### Key Components

1. **JSONLParser** (`packages/overlord/src/services/JSONLParser.ts`)
   - Parses JSONL files line by line
   - Extracts content from various message formats
   - Handles edge cases and malformed data

2. **DatabaseMapper** (`packages/overlord/src/services/DatabaseMapper.ts`)
   - Transforms LogEntry objects to database records
   - Creates session metadata
   - Handles cost calculations

3. **ConvexSync** (`packages/overlord/src/services/ConvexSync.ts`)
   - Saves parsed data to Convex database
   - Handles updates for existing records
   - Manages session and message relationships

## Claude Code JSONL Format

Claude Code stores conversations in JSONL (JSON Lines) format, where each line is a separate JSON object representing an event in the conversation.

### File Location
```
~/.claude/projects/{encoded-project-path}/{session-uuid}.jsonl
```

### Line Structure
Each line in the JSONL file represents one of these types:
- `user` - User messages
- `assistant` - Claude's responses
- `tool_use` - Tool invocations
- `tool_result` - Results from tool executions
- `summary` - Conversation summaries

## Message Types and Structures

### 1. User Messages

User messages can have multiple formats depending on the content:

```json
// Simple text message
{
  "type": "user",
  "uuid": "75f5d516-754d-4d9a-bb42-a272fa37c30b",
  "timestamp": "2025-06-22T14:29:40.117Z",
  "message": {
    "text": "Hello, Claude"
  }
}

// Message with content array (modern format)
{
  "type": "user",
  "message": {
    "content": [
      {
        "type": "text",
        "text": "Please help me with this code"
      }
    ]
  }
}

// Tool result message (special case)
{
  "type": "user",
  "message": {
    "content": [
      {
        "type": "tool_result",
        "tool_use_id": "toolu_123",
        "content": "File contents here..."
      }
    ]
  }
}
```

### 2. Assistant Messages

Assistant messages contain Claude's responses and can include multiple content types:

```json
// Text-only response
{
  "type": "assistant",
  "uuid": "687da01a-97d6-4acc-ae9f-1ed713513819",
  "message": {
    "content": [
      {
        "type": "text",
        "text": "I'll help you with that."
      }
    ]
  }
}

// Response with tool use
{
  "type": "assistant",
  "message": {
    "content": [
      {
        "type": "text",
        "text": "Let me search for that file."
      },
      {
        "type": "tool_use",
        "id": "toolu_01ABC",
        "name": "Grep",
        "input": {
          "pattern": "function",
          "path": "/src"
        }
      }
    ]
  }
}

// Tool-only message (no text)
{
  "type": "assistant",
  "uuid": "c7b7ab32-5a6d-438b-bd73-f97b2a75ec42",
  "message": {
    "content": [
      {
        "type": "tool_use",
        "id": "toolu_01NBF6ryfwphMM2RiWESkkr5",
        "name": "Read",
        "input": {
          "file_path": "/path/to/file.ts"
        }
      }
    ]
  }
}
```

### 3. Tool Messages

Tool-related messages are stored separately but sometimes embedded in user/assistant messages:

```json
// Standalone tool use (legacy format)
{
  "type": "tool_use",
  "uuid": "tool-use-123",
  "name": "TodoWrite",
  "input": {
    "todos": [...]
  },
  "tool_use_id": "toolu_123"
}

// Standalone tool result (legacy format)
{
  "type": "tool_result",
  "uuid": "tool-result-123",
  "tool_use_id": "toolu_123",
  "output": "Success",
  "is_error": false
}
```

## Parser Implementation

### Core Parsing Logic

The parser handles multiple content formats and edge cases:

```typescript
// Extract user message content
if (parsed.type === "user") {
  let textContent = ""
  if (parsed.message?.text) {
    // Simple text format
    textContent = parsed.message.text
  } else if (parsed.message?.content) {
    if (Array.isArray(parsed.message.content)) {
      // Check if it's a tool_result
      if (parsed.message.content[0]?.type === "tool_result") {
        // Store entire array as JSON for tool results
        textContent = JSON.stringify(parsed.message.content)
      } else {
        // Extract text parts only
        const textParts = parsed.message.content
          .filter((item: any) => item.type === "text")
          .map((item: any) => item.text)
        textContent = textParts.join("\n")
      }
    }
  }
}
```

### Project Name Extraction

The parser extracts project names from the Claude Code directory structure:

```typescript
// Path: ~/.claude/projects/-Users-christopherdavid-code-yt-dlp/session.jsonl
// Extracts: "yt-dlp"

const segments = projectDir.split("-").filter(p => p)
if (segments.length >= 2) {
  const lastTwo = segments.slice(-2)
  if (lastTwo[0] === "code") {
    projectPath = lastTwo[1]  // "yt-dlp"
  } else {
    projectPath = lastTwo.join("-")  // compound names
  }
}
```

## Key Modifications and Fixes

### 1. Empty Message Handling

**Problem**: Messages containing only tool_use or tool_result appeared empty.

**Solution**: Added special parsing for these cases:
- Assistant messages with only tool_use show: "🔧 Using tool: [name]"
- User messages with tool_result show: "📤 Tool Result: [content]"

### 2. Tool Metadata Extraction

**Problem**: Tool information was embedded in content arrays, not visible in UI.

**Solution**: Extract tool metadata during parsing and add to message metadata:
```typescript
if (Array.isArray(parsed.content)) {
  const toolUses = parsed.content.filter(item => item.type === "tool_use")
  if (toolUses.length > 0) {
    metadata.toolName = toolUses[0].name
    metadata.toolUseId = toolUses[0].id
    metadata.toolInput = toolUses[0].input
    metadata.hasEmbeddedTool = true
  }
}
```

### 3. HTML Content Stripping

**Problem**: Claude Code sometimes includes HTML-formatted content.

**Solution**: Added HTML stripping logic:
```typescript
if (content.includes("<span") || content.includes("→")) {
  const plainText = content
    .replace(/<[^>]*>/g, "")  // Remove HTML tags
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&amp;/g, "&")
    .trim()
}
```

### 4. Database Update Implementation

**Problem**: Parser was only logging what it would update, not actually updating.

**Solution**: Replaced placeholder logging with actual database updates:
```typescript
// Before (placeholder)
yield* Effect.log(`Would update message ${id}`)

// After (actual implementation)
yield* client.ConvexClient.messages.update({
  entryUuid: messageRecord.entry_uuid,
  content: messageRecord.content,
  tool_name: messageRecord.tool_name,
  // ... other fields
})
```

## Integration with Database Mapper

The DatabaseMapper transforms parsed entries into database records:

### Session Creation
```typescript
export const createSessionRecord = (
  sessionId: string,
  userId: string,
  projectPath: string,
  entries: ReadonlyArray<LogEntry>
): SessionRecord => {
  const timestamps = entries.map(e => new Date(e.timestamp))
  const messages = entries.filter(e => 
    ["user", "assistant"].includes(e.type)
  )
  
  return {
    id: sessionId,
    user_id: userId,
    project_path: projectPath,
    project_name: extractProjectName(projectPath),
    status: "active",
    started_at: Math.min(...timestamps),
    last_activity: Math.max(...timestamps),
    message_count: messages.length,
    total_cost: calculateTotalCost(entries)
  }
}
```

### Message Record Creation
```typescript
export const logEntryToMessageRecord = (
  entry: LogEntry,
  sessionId: string
): MessageRecord => {
  const base = {
    session_id: sessionId,
    entry_uuid: entry.uuid,
    timestamp: new Date(entry.timestamp)
  }

  switch (entry.type) {
    case "user":
      return {
        ...base,
        entry_type: "user",
        role: "user",
        content: typeof entry.message.text === "string"
          ? entry.message.text
          : JSON.stringify(entry.message.text)
      }
    // ... other cases
  }
}
```

## Common Issues and Solutions

### Issue 1: "Empty Message" Display

**Symptoms**: Messages show as empty boxes or "[Empty message]"

**Causes**:
1. Message contains only tool_use with no text
2. Content stored in unexpected format
3. Tool_result messages stored as user messages

**Solutions**:
- Parse tool-only messages to show tool names
- Handle tool_result format in user messages
- Extract content from all possible locations

### Issue 2: Missing Tool Information

**Symptoms**: Tool calls not visible in debug JSON

**Causes**:
1. Tool info embedded in content arrays
2. Tool metadata not extracted during parsing
3. UI not displaying tool information

**Solutions**:
- Extract tool metadata from content arrays
- Add visual indicators for tool usage
- Display tool info in message body with purple styling

### Issue 3: Project Name Extraction Failures

**Symptoms**: "Project unknown" in sidebar

**Causes**:
1. Complex directory encoding
2. Compound project names (e.g., "yt-dlp")
3. Special characters in paths

**Solutions**:
- Improved path parsing logic
- Handle compound names correctly
- Fallback strategies for edge cases

## Testing and Debugging

### Debug Scripts

1. **Find specific messages**:
```bash
bun find-specific-messages.ts
```

2. **Test message parsing**:
```bash
bun test-empty-message-fix.ts
```

3. **Check database content**:
```bash
bun check-transformed-messages.ts
```

### Debugging Process

1. **Identify problematic messages**:
   - Note the UUID from the UI
   - Check if message exists in database
   - Find original JSONL entry

2. **Trace parsing flow**:
   - JSONLParser.parseJSONL()
   - Content extraction logic
   - DatabaseMapper transformation
   - ConvexSync save/update

3. **Verify fixes**:
   - Re-import sessions
   - Check parsed content
   - Verify UI display

## Best Practices

1. **Always preserve original data**: Store unparsed content when structure is unclear
2. **Handle all content formats**: Claude Code uses various formats across versions
3. **Fail gracefully**: Continue parsing even if individual entries fail
4. **Log extensively**: Debug logging helps trace parsing issues
5. **Test edge cases**: Empty messages, tool-only messages, malformed JSON

## Future Improvements

1. **Streaming parser**: Handle very large JSONL files efficiently
2. **Schema validation**: Validate entries against expected schemas
3. **Batch updates**: Optimize database updates for large imports
4. **Progress tracking**: Show import progress for long operations
5. **Error recovery**: Resume imports after failures

## Conclusion

The JSONL Parser is a crucial component that bridges Claude Code's file format with our database structure. Through iterative improvements, it now handles various edge cases and message formats reliably. The key to its success is flexibility in parsing different content structures while maintaining data integrity.