# HTTP Server Solution for Electron-AI Integration with CORS Support (Two Approaches)

## Issue

We were encountering issues with the Vercel AI SDK (`usePersistentChat`) not properly handling streaming responses when bridged through Electron's IPC mechanism. The root issue was that when the SDK detects a streaming response, it:

1. Doesn't read the `response.body` stream directly from the Response object
2. Instead, creates a new EventSource connection to the same URL internally
3. This internal EventSource connection wasn't going through our IPC bridge, causing it to fail

## Solution

We completely redesigned the approach by:

1. Removing the IPC fetch bridge implementation
2. Running a standard HTTP server in the Electron main process
3. Connecting the HTTP server to our existing Hono app
4. Pointing the AI SDK directly to this local HTTP server

This ensures that both the initial `fetch` request AND the subsequent `EventSource` connection initiated by the AI SDK correctly reach the Hono app running in the main process.

## Implementation Details

We tried two different approaches to solve this issue:

### Approach 1: Native Node.js HTTP Server

1. **Pure Node.js HTTP Server**: Implemented a native Node.js HTTP server for maximum control over the request/response cycle
2. **Direct Request Handling**: Handled OPTIONS preflight requests and test endpoints in the HTTP server
3. **Manual Hono Integration**: Forwarded requests to the Hono app and properly handled both regular and streaming responses
4. **Robust CORS Support**: Set CORS headers directly on the Node.js response objects
5. **Error Handling**: Added improved error handling and logging for better debugging

### Approach 2: Hono Node Adapter with CORS Middleware

1. **Hono Node Adapter**: Used `@hono/node-server` to run the Hono app directly
2. **Built-in CORS Middleware**: Added Hono's CORS middleware (`hono/cors`) for simpler CORS handling
3. **Error Handling**: Added additional error logging and validation
4. **Graceful Shutdown**: Added proper handling for server cleanup when the app closes

### Renderer Changes

1. **Removed IPC Bridge**: Removed all custom fetch implementation in the renderer
2. **Updated API Endpoint**: Changed the API endpoint in usePersistentChat to point to `http://localhost:3001/api/chat`
3. **Simplified Code**: Removed all the complexity related to proxying fetch requests

### Cleanup

1. **Removed Files**: Deleted fetch-context.ts, test-api.ts, and fetch.ts
2. **Updated Types**: Removed ElectronAPI interface from types.d.ts
3. **Removed Context Exposer**: Removed the fetch context exposure from the preload script

## Benefits

- **Simpler Architecture**: Using a standard HTTP server is a more robust approach than custom IPC bridging
- **Better SDK Compatibility**: Works with both the initial fetch request and the EventSource stream connection
- **CORS Support**: Properly handles cross-origin requests between the renderer and the main process
- **Easier Debugging**: Network activity now shows up in the DevTools Network tab
- **Reduced Complexity**: Removed a significant amount of complex code

## Database Conflict Resolution

During testing, we also discovered and fixed an issue with RxDB document conflicts:

1. **Optimistic Locking**: Implemented retry mechanism in `ThreadRepository.updateThread`
2. **Debouncing**: Added debouncing to timestamp updates to reduce concurrent write attempts
3. **Graceful Error Handling**: Made the UI more resilient to database errors

## Testing

To test these changes:
1. Start the Electron app with the new HTTP server approach
2. Check the main process logs for `Local API server listening on http://localhost:3001`
3. Send a message through the chat interface
4. Verify that CORS preflight requests (OPTIONS) are handled correctly
5. Verify that responses are correctly streamed and displayed in the UI
6. Check the Network tab in DevTools to see both the POST request and EventSource connection
7. Verify that no RxDB conflicts are occurring in the console