# Debug Summary: Session and Message Loading Issues

## Issues Found

### 1. All sessions show "Project unknown"
- **Root Cause**: In the Overlord service, when importing sessions, the `projectPath` is being set to "unknown"
- **Location**: `/packages/overlord/src/services/OverlordService.ts` line 315
- **Fix Required**: The `extractProjectPath` function needs to properly parse Claude's file paths

### 2. No messages showing for some sessions
- **Root Cause**: Message count discrepancies - sessions claim to have more messages than actually exist in the database
- **Example**: Session `7644ecb5-b708-4418-9ed5-a08c87a1e72f` claims 1023 messages but only has 206
- **Fix Required**: Re-sync sessions or update message counts to match actual data

### 3. Messages with empty content
- **Root Cause**: Content is being stored as JSON array strings instead of plain text
- **Example**: Content looks like `[{"type":"text","text":"actual content"}]`
- **Fix Required**: Parse JSON content during import in DatabaseMapper

## Code Locations to Fix

### 1. Fix Project Path Extraction
File: `/packages/overlord/src/services/OverlordService.ts`
```typescript
// Line 339-349 - Current implementation
const extractProjectPath = (filePath: string): string => {
  // Claude stores conversations in folders like:
  // ~/Library/Mobile Documents/com~apple~CloudDocs/Claude/<project-hash>/conversations/<session-id>.jsonl
  const parts = filePath.split("/")
  const claudeIndex = parts.findIndex((p) => p === "Claude")
  if (claudeIndex >= 0 && claudeIndex + 1 < parts.length) {
    return parts[claudeIndex + 1]
  }
  // Fallback to parent directory name
  return path.basename(path.dirname(path.dirname(filePath)))
}
```

### 2. Fix Project Name Generation
File: `/packages/overlord/src/services/DatabaseMapper.ts`
```typescript
// Line 192-196 - Current implementation
const extractProjectName = (projectPath: string): string => {
  // Project paths are hashed, so we can't get the real name
  // In Phase 2, we might want to store a mapping
  return `Project ${projectPath.substring(0, 8)}`
}
```

### 3. Fix Content Parsing
File: `/packages/overlord/src/services/DatabaseMapper.ts`
```typescript
// Lines 64-67 - Need to parse JSON content
content: typeof entry.message.text === "string"
  ? entry.message.text
  : JSON.stringify(entry.message.text)
```

## Immediate Actions

1. **Add debug logging** to the Overlord import process to see actual file paths
2. **Create a mutation** to fix existing sessions with better project names
3. **Update DatabaseMapper** to properly parse Claude's content format
4. **Add a re-sync feature** to re-import sessions with correct metadata

## Testing

To verify fixes:
1. Check sessions have meaningful project names (not "Project unknown")
2. Verify message counts match actual messages in database
3. Ensure message content is plain text, not JSON strings