# JSONL Ingestion Guide

This guide covers the JSONL file drag-and-drop ingestion system for Claude Code conversation data in OpenAgents.

## Overview

The JSONL ingestion feature allows users to drag and drop Claude Code conversation files (`.jsonl` format) anywhere on the chat interface to ingest and analyze conversation data. This system provides a seamless way to import Claude Code session logs for analysis, debugging, or archival purposes.

## Features

- **Full-screen drag-and-drop zone**: Drop files anywhere on the chat interface
- **Visual feedback**: Overlay appears during drag operations with clear instructions
- **JSONL parsing**: Comprehensive parsing with error handling for malformed data
- **Console logging**: Development-friendly output of ingested data
- **Type safety**: Complete TypeScript interfaces for Claude Code data structures
- **Error handling**: Graceful handling of invalid files and parsing errors
- **Notifications**: User-friendly success/error messages

## Architecture

### Core Components

#### 1. JSONLIngestion Class (`jsonl-ingestion.ts`)

The main class that handles all drag-and-drop functionality:

```typescript
export class JSONLIngestion {
  private dropZone: HTMLElement | null = null
  private dragCounter = 0

  // Initializes drag-and-drop on the chat interface
  initialize()
  
  // Handles file processing and parsing
  private async handleDrop(e: DragEvent)
  
  // Parses JSONL content
  private parseJSONL(text: string): Array<LogEntry>
  
  // Shows visual feedback
  private showDropOverlay()
  private hideDropOverlay()
  
  // User notifications
  private showNotification(message: string, type: "error" | "success")
}
```

#### 2. TypeScript Interfaces

Complete type definitions for Claude Code conversation data:

```typescript
// Content part types
interface TextContentPart {
  type: "text"
  text: string
}

interface ThinkingContentPart {
  type: "thinking"
  thinking: string
  signature: string
}

interface ToolUseContentPart {
  type: "tool_use"
  id: string
  name: string
  input: any
}

interface ToolResultContentPart {
  type: "tool_result"
  tool_use_id: string
  content: any
  is_error: boolean
}

// Message types
interface UserMessage {
  role: "user"
  content: Array<TextContentPart | ToolResultContentPart>
}

interface AssistantMessage {
  id: string
  type: "message"
  role: "assistant"
  model: string
  content: Array<TextContentPart | ThinkingContentPart | ToolUseContentPart>
  stop_reason: string | null
  stop_sequence: string | null
  usage: AssistantMessageUsage
}

// Log entry types
interface LogEntryBase {
  uuid: string
  timestamp: string
  isSidechain: boolean
  userType: "external"
  cwd: string
  sessionId: string
  version: string
}

interface UserEntry extends LogEntryBase {
  type: "user"
  parentUuid: string | null
  message: UserMessage
  isCompactSummary?: boolean
  toolUseResult?: any
}

interface AssistantEntry extends LogEntryBase {
  type: "assistant"
  parentUuid: string
  message: AssistantMessage
  requestId: string
}

interface SummaryEntry {
  type: "summary"
  summary: string
  leafUuid: string
}

type LogEntry = SummaryEntry | UserEntry | AssistantEntry
```

### Integration Points

#### 1. Chat Interface Integration

The JSONL ingestion is initialized in the chat interface:

```typescript
// In chat.ts
import { jsonlIngestion } from "./jsonl-ingestion"

export function initializeChat() {
  // ... other initialization code
  
  // Initialize JSONL ingestion
  jsonlIngestion.initialize()
}
```

#### 2. CSS Styling

Visual feedback styles are included in `chat-view.css`:

```css
/* Drop overlay */
#jsonl-drop-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.8);
  backdrop-filter: blur(4px);
  z-index: 9999;
  /* ... styling for overlay */
}

/* Notifications */
.jsonl-notification {
  position: fixed;
  top: 20px;
  right: 20px;
  /* ... notification styling */
}
```

## Usage

### Basic Usage

1. **Access the chat interface**: Navigate to any chat page in OpenAgents
2. **Drag a JSONL file**: Drag any `.jsonl` or `.json` file from your file system
3. **Drop anywhere**: Drop the file anywhere on the chat screen
4. **View results**: Check the browser console to see the ingested data

### Supported File Formats

- `.jsonl` files (primary format for Claude Code conversations)
- `.json` files (for compatibility)

### Expected JSONL Format

Each line in the JSONL file should be a valid JSON object representing a log entry:

```jsonl
{"type":"user","uuid":"abc123","timestamp":"2024-01-15T10:30:00Z","message":{"role":"user","content":[{"type":"text","text":"Hello"}]},...}
{"type":"assistant","uuid":"def456","timestamp":"2024-01-15T10:30:05Z","message":{"role":"assistant","content":[{"type":"text","text":"Hi there!"}]},...}
{"type":"summary","summary":"User greeted the assistant","leafUuid":"ghi789"}
```

## Development

### Console Output

When a file is successfully ingested, the console will show:

```javascript
=== JSONL Ingestion Complete ===
File: conversation-2024-01-15.jsonl
Total entries: 42
Entries by type: {summary: 1, user: 20, assistant: 21}
Full data: [/* array of parsed entries */]
```

### Error Handling

The system handles various error scenarios:

1. **Invalid file types**: Shows error notification for non-JSONL/JSON files
2. **Malformed JSON**: Logs warnings for unparseable lines but continues processing
3. **Empty files**: Handles gracefully with appropriate messaging
4. **File read errors**: Catches and displays file reading errors

### Extending the System

#### Adding New Content Types

To support new content part types, extend the interfaces:

```typescript
interface NewContentPart {
  type: "new_type"
  data: any
}

// Update the union types
interface AssistantMessage {
  content: Array<TextContentPart | ThinkingContentPart | ToolUseContentPart | NewContentPart>
}
```

#### Custom Processing

To add custom processing for ingested data:

```typescript
// In the handleDrop method, after parsing:
const entries = this.parseJSONL(text)

// Add custom processing
this.processEntries(entries)

private processEntries(entries: Array<LogEntry>) {
  // Custom logic here
  // e.g., populate chat UI, analyze patterns, etc.
}
```

## Configuration

### Drop Zone Target

By default, the entire layout wrapper serves as the drop zone:

```typescript
// Current implementation
this.dropZone = document.querySelector(".layout-wrapper") || document.body
```

To change the drop zone target:

```typescript
// Custom drop zone
this.dropZone = document.querySelector("#custom-drop-area")
```

### File Type Validation

To support additional file types:

```typescript
// In handleDrop method
if (!file.name.endsWith(".jsonl") && 
    !file.name.endsWith(".json") && 
    !file.name.endsWith(".txt")) {  // Add new type
  // Handle error
}
```

## Testing

### Manual Testing

1. **Valid JSONL file**: Test with a properly formatted Claude Code conversation file
2. **Invalid file type**: Try dropping a `.txt` or `.pdf` file to test error handling
3. **Malformed JSONL**: Test with a file containing some invalid JSON lines
4. **Empty file**: Test with an empty file
5. **Large file**: Test with a large conversation file to verify performance

### Automated Testing

Create test files for different scenarios:

```typescript
// Test valid JSONL
const validJSONL = `
{"type":"user","uuid":"123","message":{"role":"user","content":[{"type":"text","text":"test"}]}}
{"type":"assistant","uuid":"456","message":{"role":"assistant","content":[{"type":"text","text":"response"}]}}
`

// Test malformed JSONL
const malformedJSONL = `
{"valid":"json"}
invalid json line
{"another":"valid","line":true}
`
```

## Performance Considerations

### File Size Limits

The current implementation loads the entire file into memory. For very large conversation files:

1. **Browser memory limits**: Large files may cause memory issues
2. **Parsing performance**: JSON parsing of large files may block the UI
3. **Console output**: Large datasets may overwhelm browser console

### Optimization Strategies

For production use with large files:

1. **Streaming parser**: Implement line-by-line streaming
2. **Web Workers**: Move parsing to a background thread
3. **Progressive display**: Show results as they're parsed
4. **Pagination**: Break large datasets into chunks

## Security Considerations

### File Validation

The system includes basic validation:

1. **File type checking**: Only allows `.jsonl` and `.json` files
2. **JSON parsing**: Safely parses JSON with try-catch blocks
3. **Error containment**: Malformed lines don't break the entire process

### Data Handling

1. **Client-side only**: All processing happens in the browser
2. **No network transmission**: Files are not uploaded to servers
3. **Memory cleanup**: Processed data is available for garbage collection

## Troubleshooting

### Common Issues

1. **No overlay appears**:
   - Check that the drop zone element exists
   - Verify event listeners are properly attached
   - Check browser console for JavaScript errors

2. **Files not processing**:
   - Verify file format is `.jsonl` or `.json`
   - Check browser console for parsing errors
   - Ensure file is not empty or corrupted

3. **Console output missing**:
   - Open browser developer tools
   - Check the Console tab
   - Ensure console.log is not filtered out

### Debug Mode

Enable debug logging by adding to the JSONLIngestion class:

```typescript
private debug = true; // Set to false for production

private log(message: string, ...args: any[]) {
  if (this.debug) {
    console.log(`[JSONL Ingestion] ${message}`, ...args);
  }
}
```

## Future Enhancements

### Planned Features

1. **Chat UI integration**: Display ingested conversations in the chat interface
2. **Conversation merging**: Combine multiple conversation files
3. **Export functionality**: Export processed data in various formats
4. **Search and filtering**: Find specific messages or tool uses
5. **Analytics dashboard**: Visualize conversation patterns and statistics

### API Integration

Future versions may include:

```typescript
interface JSONLProcessor {
  // Process and display in chat UI
  displayInChat(entries: Array<LogEntry>): void
  
  // Search functionality
  searchMessages(query: string): Array<LogEntry>
  
  // Export processed data
  exportData(format: 'json' | 'csv' | 'txt'): string
  
  // Analytics
  analyzeConversation(entries: Array<LogEntry>): ConversationAnalytics
}
```

## Related Documentation

- [Chat Layout Architecture Guide](./chat-layout-architecture.md) - Understanding the chat interface structure
- [Effect Architecture Guide](./effect-architecture-guide.md) - When expanding with Effect-based services
- [Streaming Architecture](./streaming-architecture.md) - For future streaming parser implementation

## Conclusion

The JSONL ingestion system provides a robust foundation for importing Claude Code conversation data. Its modular design allows for easy extension and customization while maintaining type safety and error handling. The system is designed to be developer-friendly with comprehensive console logging and clear error messages.

For immediate use, simply drag and drop your Claude Code JSONL files onto the chat interface and check the browser console to see the ingested data structure.