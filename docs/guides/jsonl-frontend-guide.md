# JSONL Frontend Processing Guide

This guide covers the browser-based drag-and-drop functionality for viewing Claude Code JSONL files in the OpenAgents.com chat interface.

## Quick Reference

### Basic Usage
1. Navigate to any chat page on OpenAgents.com
2. Drag a `.jsonl` file from Claude Code onto the page
3. View parsed conversation in browser console

### Key Files
- Implementation: `apps/openagents.com/src/lib/jsonl-ingestion.ts`
- Integration: `apps/openagents.com/src/components/chat-view/index.ts`
- Styles: `apps/openagents.com/public/css/client.css`

### Common Issues

| Issue | Solution |
|-------|----------|
| Drop zone not appearing | Check if JSONLIngestion is initialized |
| File rejected | Ensure it's a .jsonl file |
| No console output | Open browser DevTools before dropping |
| Parsing errors | Check for valid JSON on each line |

## Overview

The JSONL ingestion feature allows users to drag and drop Claude Code conversation files directly into the web interface for immediate viewing and analysis. This is a client-side only feature - files are processed in the browser without server upload.

## Architecture

```
User drags file → Drop zone appears → File validation → Parse JSONL → Display in console
                         ↓
                  Visual feedback
                  (green border)
```

## Implementation Details

### Core Class: JSONLIngestion

```typescript
export class JSONLIngestion {
  private dropZone: HTMLElement | null = null
  private isInitialized = false

  initialize() {
    if (this.isInitialized || typeof window === 'undefined') return
    
    this.setupDropZone()
    this.setupDragAndDrop()
    this.isInitialized = true
  }
}
```

### Drop Zone Creation

The drop zone is dynamically created when a drag is detected:

```typescript
private setupDropZone() {
  this.dropZone = document.createElement('div')
  this.dropZone.id = 'jsonl-drop-zone'
  this.dropZone.innerHTML = `
    <div class="drop-message">
      <span class="drop-icon">📁</span>
      <p>Drop Claude Code JSONL file here</p>
    </div>
  `
  this.dropZone.style.display = 'none'
  document.body.appendChild(this.dropZone)
}
```

### File Processing

```typescript
private async handleFile(file: File) {
  if (!file.name.endsWith('.jsonl')) {
    this.showNotification('Please drop a .jsonl file', 'error')
    return
  }

  const content = await file.text()
  const lines = content.trim().split('\n')
  const entries: LogEntry[] = []

  for (const line of lines) {
    try {
      const parsed = JSON.parse(line)
      entries.push(this.parseLogEntry(parsed))
    } catch (error) {
      console.error('Failed to parse line:', error)
    }
  }

  this.displayParsedData(file.name, entries)
}
```

## Data Structures

### Claude Code JSONL Format

Each line in a Claude Code JSONL file is a JSON object representing an interaction:

```typescript
interface LogEntry {
  uuid: string
  timestamp: string
  type: 'user' | 'assistant' | 'tool_use' | 'tool_result' | 'summary'
  message?: {
    text?: string
    content?: Array<{
      type: 'text' | 'tool_use' | 'tool_result'
      text?: string
      // ... other fields
    }>
  }
  // ... other fields based on type
}
```

### Message Types

1. **User Messages**: User inputs with optional images
2. **Assistant Messages**: Claude's responses with optional tool use
3. **Tool Use**: Invocation of tools like Read, Edit, etc.
4. **Tool Result**: Results from tool execution
5. **Summary**: Conversation summaries

## Styling

The drop zone uses specific styles for visual feedback:

```css
#jsonl-drop-zone {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.9);
  z-index: 9999;
  border: 2px dashed #4a5568;
  transition: all 0.3s ease;
}

#jsonl-drop-zone.drag-over {
  border-color: #48bb78;
  background: rgba(72, 187, 120, 0.1);
}
```

## Console Output

The parsed data is displayed in a structured format:

```javascript
console.log('📚 Claude Code Conversation:', {
  file: 'session-id.jsonl',
  totalEntries: 45,
  breakdown: {
    user: 15,
    assistant: 15,
    tool_use: 10,
    tool_result: 5
  },
  messages: [
    // Formatted messages
  ],
  rawEntries: [
    // Original parsed entries
  ]
})
```

## Integration with Chat View

The feature is automatically initialized when the chat view loads:

```typescript
// In chat-view/index.ts
import { JSONLIngestion } from '../../lib/jsonl-ingestion'

const ingestion = new JSONLIngestion()
ingestion.initialize()
```

## Security Considerations

- Files are processed entirely in the browser
- No data is sent to servers
- File size limits should be considered for large conversations
- Sensitive data remains on the user's machine

## Development Tips

1. **Testing**: Use Claude Code export files from `~/.claude/projects/`
2. **Debugging**: Enable verbose logging with `console.log` statements
3. **Performance**: Consider Web Workers for large files
4. **Error Handling**: Gracefully handle malformed JSON lines

## Future Enhancements

1. **Visual Display**: Show conversations in the UI instead of console
2. **Search**: Add ability to search through conversations
3. **Export**: Allow exporting in different formats
4. **Persistence**: Optional local storage of imported conversations
5. **Batch Import**: Support dropping multiple files

## Troubleshooting

### Drop Zone Not Appearing
- Ensure JavaScript is enabled
- Check browser console for errors
- Verify JSONLIngestion is initialized

### File Processing Fails
- Validate JSONL format (one JSON object per line)
- Check for UTF-8 encoding
- Ensure file isn't corrupted

### Performance Issues
- Consider file size (large conversations may be slow)
- Use Chrome DevTools Performance tab
- Implement progressive parsing for large files

## Related Documentation

- For backend JSONL processing, see: [JSONL Backend Guide](./jsonl-backend-guide.md)
- For Claude Code format details, see the backend guide's format section