# JSONL Ingestion Quick Reference

Quick reference for using the JSONL file drag-and-drop ingestion system.

## Quick Start

1. **Drag**: Take any Claude Code conversation file (`.jsonl` format)
2. **Drop**: Drop it anywhere on the chat interface
3. **Check**: Open browser console to see ingested data

## File Requirements

- **Supported formats**: `.jsonl`, `.json`
- **Expected content**: Claude Code conversation logs
- **File size**: No hard limit, but large files may impact performance

## Visual Feedback

- **Drop overlay**: Dark overlay with upload icon appears during drag
- **Notifications**: Success/error messages appear in top-right corner
- **Console output**: Detailed data logged to browser console

## Example Console Output

```javascript
=== JSONL Ingestion Complete ===
File: my-conversation.jsonl
Total entries: 25
Entries by type: {summary: 1, user: 12, assistant: 12}
Full data: [/* parsed conversation data */]
```

## Data Structure

Ingested JSONL files contain these entry types:

### User Entries
```json
{
  "type": "user",
  "uuid": "abc123",
  "timestamp": "2024-01-15T10:30:00Z",
  "message": {
    "role": "user",
    "content": [{"type": "text", "text": "Hello"}]
  }
}
```

### Assistant Entries
```json
{
  "type": "assistant", 
  "uuid": "def456",
  "timestamp": "2024-01-15T10:30:05Z",
  "message": {
    "role": "assistant",
    "content": [{"type": "text", "text": "Hi there!"}]
  }
}
```

### Summary Entries
```json
{
  "type": "summary",
  "summary": "User greeted the assistant",
  "leafUuid": "ghi789"
}
```

## Content Types

The system recognizes these content part types:

- **text**: Plain text content
- **thinking**: Claude's reasoning process
- **tool_use**: Tool/function calls
- **tool_result**: Tool execution results

## Error Handling

- **Invalid file types**: Shows error notification
- **Malformed JSON lines**: Logs warnings, continues processing
- **Empty files**: Handles gracefully
- **Large files**: May show performance warnings

## Troubleshooting

### File Not Processing
1. Check file extension is `.jsonl` or `.json`
2. Verify file is not empty
3. Check browser console for errors

### No Visual Feedback
1. Ensure JavaScript is enabled
2. Check browser console for script errors
3. Verify files are being dragged over the chat area

### Console Output Missing
1. Open Developer Tools (F12)
2. Navigate to Console tab
3. Ensure console logging is not filtered

## Development Notes

- **Client-side processing**: No data sent to servers
- **Memory usage**: Large files loaded entirely into memory
- **Type safety**: Full TypeScript interfaces available
- **Error boundaries**: Malformed data won't crash the system

## Related Files

- Implementation: `apps/openagents.com/src/client/jsonl-ingestion.ts`
- Styling: `apps/openagents.com/src/components/chat-view/chat-view.css`
- Integration: `apps/openagents.com/src/client/chat.ts`

For detailed documentation, see [JSONL Ingestion Guide](./jsonl-ingestion-guide.md).