# JSONL Parser Quick Reference

## Common Patterns

### 1. User Message Formats

```json
// Simple text
{"type": "user", "message": {"text": "Hello"}}

// Content array
{"type": "user", "message": {"content": [{"type": "text", "text": "Hello"}]}}

// Tool result (special case)
{"type": "user", "message": {"content": [{"type": "tool_result", "content": "..."}]}}
```

### 2. Assistant Message Formats

```json
// Text only
{"type": "assistant", "message": {"content": [{"type": "text", "text": "Response"}]}}

// Text + Tool
{"type": "assistant", "message": {"content": [
  {"type": "text", "text": "Let me help"},
  {"type": "tool_use", "name": "Read", "id": "toolu_123"}
]}}

// Tool only (appears empty without fix)
{"type": "assistant", "message": {"content": [{"type": "tool_use", "name": "Grep"}]}}
```

## Quick Fixes

### Empty Assistant Message
```typescript
// Problem: Assistant message shows empty
// Cause: Contains only tool_use, no text
// Fix: Show "🔧 Using tool: [name]"
if (textParts.length === 0 && toolParts.length > 0) {
  return `🔧 Using tool: ${toolParts[0].name}`
}
```

### Empty User Message
```typescript
// Problem: User message shows "[Empty message]"
// Cause: Contains tool_result
// Fix: Parse and format tool result
if (parsed[0]?.type === "tool_result") {
  return `📤 Tool Result: ${parsed[0].content}`
}
```

### Missing Tool Info
```typescript
// Problem: Tool calls not visible
// Cause: Embedded in content array
// Fix: Extract during transformation
const toolUses = content.filter(item => item.type === "tool_use")
if (toolUses.length > 0) {
  metadata.toolName = toolUses[0].name
  metadata.hasEmbeddedTool = true
}
```

## File Locations

- **Parser**: `packages/overlord/src/services/JSONLParser.ts`
- **Mapper**: `packages/overlord/src/services/DatabaseMapper.ts`
- **Display**: `apps/openagents.com/src/lib/chat-client-convex.ts`
- **Claude files**: `~/.claude/projects/*/`

## Debug Commands

```bash
# Find specific message in JSONL files
bun packages/convex/find-specific-messages.ts

# Test parsing logic
bun packages/convex/test-empty-message-fix.ts

# Re-import sessions
cd packages/overlord && bun src/index.ts import --user-id="claude-code-user"
```

## Common Issues

| Symptom | Cause | Solution |
|---------|-------|----------|
| Empty blue box | Assistant message with only tool_use | Show "🔧 Using tool: X" |
| Green "[Empty message]" | User message with tool_result | Parse tool_result content |
| "Project unknown" | Poor path extraction | Improve project name parsing |
| No tool info in debug | Tools embedded in content | Extract tool metadata |
| Missing messages | Not imported to Convex | Run import command |

## Message Flow

```
JSONL Line → JSONLParser.parseJSONL()
    ↓
Extract content based on type
    ↓
Create LogEntry object
    ↓
DatabaseMapper.logEntryToMessageRecord()
    ↓
ConvexSync.saveSession()
    ↓
chat-client-convex.parseMessageContent()
    ↓
Display in UI
```