# JSONL Backend Processing Guide

This guide covers the Overlord service's JSONL parser that processes Claude Code conversation files and syncs them to the Convex database.

## Quick Reference

### Common Patterns

```json
// User message formats
{"type": "user", "message": {"text": "Hello"}}
{"type": "user", "message": {"content": [{"type": "text", "text": "Hello"}]}}
{"type": "user", "message": {"content": [{"type": "tool_result", "content": "..."}]}}

// Assistant message formats
{"type": "assistant", "message": {"content": [{"type": "text", "text": "Response"}]}}
{"type": "assistant", "message": {"content": [{"type": "tool_use", "name": "Read"}]}}
```

### Quick Fixes

| Problem | Cause | Solution |
|---------|-------|----------|
| Empty blue box | Assistant message with only tool_use | Show "🔧 Using tool: X" |
| Green "[Empty message]" | User message with tool_result | Parse tool_result content |
| "Project unknown" | Poor path extraction | Improve project name parsing |
| No tool info in debug | Tools embedded in content | Extract tool metadata |

### Key Files
- Parser: `packages/overlord/src/services/JSONLParser.ts`
- Mapper: `packages/overlord/src/services/DatabaseMapper.ts`
- Sync: `packages/overlord/src/services/ConvexSync.ts`
- Display Logic: `apps/openagents.com/src/lib/chat-client-convex.ts`
- Display Formatting: `apps/openagents.com/src/lib/chat-utils.ts`

### Debug Commands
```bash
# Re-import sessions
cd packages/overlord && bun src/index.ts import --user-id="claude-code-user"

# Find specific message
bun packages/convex/find-specific-messages.ts

# Test parsing logic
bun packages/convex/test-empty-message-fix.ts
```

## Overview

The Overlord service monitors Claude Code's local JSONL files and syncs them to Convex for persistent storage and web display. This backend process handles complex message formats, edge cases, and maintains data integrity.

## Architecture

```
Claude Code JSONL Files → JSONLParser → LogEntry[] → DatabaseMapper → Convex Database
~/.claude/projects/          ↓                           ↓
                     Parse & Extract              Transform to Records
                       Content                    (Session + Messages)
```

### Data Flow

1. **File Discovery**: FileWatcher finds JSONL files in `~/.claude/projects/`
2. **Parsing**: JSONLParser extracts structured data from each line
3. **Transformation**: DatabaseMapper converts to database records
4. **Storage**: ConvexSync saves/updates records in Convex
5. **Display**: Chat UI retrieves and renders messages

## Claude Code JSONL Format

### File Structure
```
~/.claude/projects/{encoded-project-path}/{session-uuid}.jsonl
```

Example:
```
~/.claude/projects/-Users-christopherdavid-code-yt-dlp/466d695f-2808-42f3-97d3-2465cfb138a7.jsonl
```

### Message Types

#### 1. User Messages

User messages have evolved through different Claude Code versions:

```json
// Simple text (older format)
{
  "type": "user",
  "uuid": "75f5d516-754d-4d9a-bb42-a272fa37c30b",
  "timestamp": "2025-06-22T14:29:40.117Z",
  "message": {
    "text": "Hello, Claude"
  }
}

// Content array (modern format)
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

// Tool result (special case - appears as user message)
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

#### 2. Assistant Messages

Assistant responses can contain text, tool invocations, or both:

```json
// Text only
{
  "type": "assistant",
  "message": {
    "content": [
      {
        "type": "text",
        "text": "I'll help you with that."
      }
    ]
  }
}

// Tool only (causes empty message without proper handling)
{
  "type": "assistant",
  "message": {
    "content": [
      {
        "type": "tool_use",
        "id": "toolu_01NBF6ryfwphMM2RiWESkkr5",
        "name": "Read",
        "input": {"file_path": "/path/to/file.ts"}
      }
    ]
  }
}

// Mixed content
{
  "type": "assistant",
  "message": {
    "content": [
      {"type": "text", "text": "Let me search for that."},
      {"type": "tool_use", "name": "Grep", "id": "toolu_123"}
    ]
  }
}
```

## Parser Implementation

### Core Parsing Logic

The parser handles multiple content formats:

```typescript
// In JSONLParser.ts
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

Claude Code encodes project paths in directory names:

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

## Key Fixes and Modifications

### 1. Empty Message Handling

**Problem**: Messages containing only tool_use or tool_result appeared empty.

**Solution**: 
```typescript
// In chat-client-convex.ts
if (textParts.length === 0 && toolParts.length > 0) {
  return `🔧 Using tool: ${toolParts[0].name}`
}

// For tool results
if (parsed[0]?.type === "tool_result") {
  return `📤 Tool Result: ${parsed[0].content}`
}
```

### 2. Tool Metadata Extraction

**Problem**: Tool information embedded in content arrays wasn't visible.

**Solution**:
```typescript
// Extract during message transformation
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

### 3. Database Update Implementation

**Problem**: Code was only logging "would update" instead of actually updating.

**Solution**: Replaced placeholder code with actual implementation:
```typescript
// Before
yield* Effect.log(`Would update message ${id}`)

// After
yield* client.ConvexClient.messages.update({
  entryUuid: messageRecord.entry_uuid,
  content: messageRecord.content,
  tool_name: messageRecord.tool_name,
  // ... other fields
})
```

## Database Integration

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

### Message Storage

Messages are stored with full metadata:
- Original content (preserved for data integrity)
- Extracted tool information
- Token usage and costs
- Timestamps and relationships

## Common Issues and Solutions

### Issue: "Empty Message" Display

**Symptoms**: 
- Blue box with no content (assistant)
- Green box with "[Empty message]" (user)

**Root Causes**:
1. Assistant message contains only tool_use
2. User message contains tool_result
3. Content parsing failed

**Solutions**:
1. Parse tool-only messages to show tool names
2. Extract tool_result content for display
3. Add fallback for unparseable content

### Issue: Long Tool Results Breaking Layout

**Symptoms**:
- Tool results with file listings showing hundreds of lines
- Debug JSON expanding off screen to the right
- Layout breaking with horizontal scrollbars

**Solutions Implemented**:

1. **Collapsible Tool Results** (in `chat-utils.ts`):
   ```typescript
   // File listings show summary with expandable details
   if (lines.length > 10 && /* file pattern check */) {
     return `<details><summary>Show all ${count} files</summary>...`
   }
   
   // Long content shows preview with full view
   if (content.length > 500) {
     return `<div>preview...</div><details><summary>Show full content</summary>...`
   }
   ```

2. **Constrained Debug JSON**:
   ```css
   .debug-json {
     white-space: pre-wrap;
     word-break: break-word;
     max-width: 100%;
   }
   ```

3. **Visual Distinction**:
   - Tool invocations: Purple border/text (#a855f7)
   - Tool results: Green border/text (#9ece6a)
   - Both have collapsible sections for long content

### Issue: Debug JSON Breaking Layout

**Symptom**: Debug JSON sections causing horizontal overflow

**Solution**: Debug sections disabled by default in production:
```typescript
const includeDebug = false // Set to true only for debugging
```

When enabled, debug JSON is constrained with CSS to prevent overflow.

### Issue: Tool Results Not Detected Due to Markdown Rendering

**Symptom**: Tool results showing as massive text blocks instead of formatted collapsible sections

**Root Cause**: Markdown rendering wraps JSON content in `<p>` tags before tool result detection

**Solution**: Process tool results BEFORE markdown rendering:
```typescript
// Check raw content for tool results BEFORE using rendered markdown
const rawContent = message.content || ''
const isToolResultJson = rawContent.startsWith('[{') || 
  rawContent.includes('<p>[{') ||
  rawContent.includes('<p>[&lt;{')

if (message.role === "user" && rawContent && isToolResultJson) {
  // Process tool result from raw content
  // Set isToolResultProcessed = true
}

// Only use rendered content if we didn't process a tool result
if (!isToolResultProcessed) {
  content = message.rendered || escapeHtml(message.content)
}
```

This ensures tool results are detected and formatted correctly regardless of markdown processing.

### Issue: Missing Tool Information

**Symptoms**: Tool calls not visible in debug JSON

**Solution**: Extract embedded tool metadata during parsing and add to message metadata.

### Issue: Project Name "Unknown"

**Symptoms**: Sidebar shows "Project unknown"

**Solution**: Improved path parsing to handle various encoding formats.

## Testing and Debugging

### Debug Scripts

```bash
# Find specific messages in JSONL files
bun packages/convex/find-specific-messages.ts

# Test message parsing
bun packages/convex/test-empty-message-fix.ts

# Check database content
bun packages/convex/check-transformed-messages.ts
```

### Debugging Process

1. **Identify Issue**: Note message UUID from UI
2. **Find Source**: Locate in JSONL file
3. **Trace Flow**: Follow through parser → mapper → database
4. **Test Fix**: Use debug scripts to verify
5. **Re-import**: Run import command to apply fixes

## Best Practices

1. **Preserve Original Data**: Store raw content when structure is unclear
2. **Handle All Formats**: Claude Code uses various formats across versions
3. **Fail Gracefully**: Continue parsing even if individual entries fail
4. **Log Extensively**: Debug logging helps trace issues
5. **Test Edge Cases**: Empty messages, tool-only messages, malformed JSON

## Performance Considerations

- **Batch Processing**: Process multiple entries together
- **Update Optimization**: Only update changed fields
- **Memory Management**: Stream large files instead of loading entirely
- **Concurrent Imports**: Handle multiple sessions in parallel

## Security Notes

- JSONL files may contain sensitive code/data
- Ensure proper access controls on Convex
- Sanitize content for display (HTML escaping)
- Don't expose file paths in production logs

## Future Improvements

1. **Incremental Sync**: Only sync new entries
2. **Conflict Resolution**: Handle concurrent edits
3. **Schema Validation**: Validate against TypeScript types
4. **Progress Tracking**: Show import progress
5. **Error Recovery**: Resume failed imports

## Related Documentation

- For frontend drag-and-drop UI, see: [JSONL Frontend Guide](./jsonl-frontend-guide.md)
- For Effect patterns, see: [Effect Architecture Guide](./effect-architecture-guide.md)