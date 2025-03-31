# Adding Local MCP Client Support to Coder App

## Update: Actual Implementation with OpenRouter API

After implementing the basic IPC bridge for local API connections, we've integrated the OpenRouter API service to provide the actual AI capabilities. This approach:

1. Uses the same Vercel AI SDK and OpenRouter integration from the chatserver app
2. Processes requests locally in the Electron app
3. Communicates with OpenRouter for the AI model responses
4. Streams the responses back through our IPC bridge

This implementation requires an OpenRouter API key (to be added by the user) and uses Claude 3.5 Sonnet by default.

## Update: Initial Implementation Issues

During the initial implementation, we encountered issues with the Electron IPC communication in development mode. The Vite dev server was intercepting our fetch requests, preventing them from being processed by our IPC handlers. 

The preload script output shows:
```
[Preload] Exposing fetch context through contextBridge
[Preload] Existing electron API keys: []
[Preload] Exposed electron.fetch
[Preload] Calling test-ipc channel
IPC error: Error invoking remote method 'test-ipc': Error: No handler registered for 'test-ipc'
```

This indicates that our IPC handlers in the main process are not being registered correctly. We implemented the following solution:

1. Created a custom fetch function in the HomePage component that attempts to use window.electron.fetch and falls back to standard fetch if needed
2. Passed this custom function directly to the usePersistentChat hook using the fetch parameter (rather than fetchFn)
3. Continued using the local API endpoint ("/api/chat") with this custom fetch function to test the local implementation

See the [dev-server-issue.md](./dev-server-issue.md) document for detailed analysis of the issue.

# Original Documentation

## Issue Overview

The Coder app was initially designed to rely on a remote API at `api.openagents.com` for its chat functionality. This implementation had several limitations:

1. Required internet connectivity even for local development
2. Limited privacy as all interactions were processed through remote servers
3. Dependency on external services for basic functionality
4. No offline capabilities

Issue #815 addresses these limitations by adding local MCP (Model Context Protocol) client support directly within the Electron app, similar to how the ChatServer works but running locally.

## Implementation Approach

The implementation follows a phased approach:

### Phase 1: Set up Local Hono Server & IPC Communication

1. **Create Server Directory Structure**
   - Created `/apps/coder/src/server/` directory
   - Added basic server files: `server.ts`, `fetch.ts`, and `index.ts`

2. **Implement Basic Hono Server**
   - Set up a simple Hono app with logger middleware
   - Added an `/api/ping` endpoint for testing connectivity
   - Created a placeholder `/api/chat` endpoint for future MCP integration

3. **Implement IPC Fetch Handler**
   - Created a bridge between the Electron main process and renderer
   - Used an IPC channel (`electron-fetch`) to communicate between processes
   - Mapped Hono responses to standard Response objects in the renderer

4. **Add Preload Script Support**
   - Created `/apps/coder/src/helpers/ipc/fetch/fetch-context.ts`
   - Exposed `window.electron.fetch` API to renderer process
   - Ensured Response objects are properly reconstructed in the renderer

5. **Update Main Process to Initialize Server**
   - Modified `main.ts` to import and initialize the Hono server
   - Set up IPC handlers when the app starts

6. **Add TypeScript Definitions**
   - Updated types in `/apps/coder/src/types.d.ts`
   - Added proper typing for the `window.electron` API

7. **Add Testing UI**
   - Created a test helper at `/apps/coder/src/helpers/ipc/fetch/test-api.ts`
   - Added a "Test Local API" button to HomePage to verify connectivity

## Code Structure

```
apps/coder/src/
├── server/
│   ├── server.ts       # Main Hono app with routes
│   ├── fetch.ts        # IPC handler for fetch requests
│   └── index.ts        # Barrel file to export components
├── helpers/ipc/
│   ├── fetch/
│   │   ├── fetch-context.ts  # Expose fetch API to renderer
│   │   └── test-api.ts       # Helper to test API connection
│   └── context-exposer.ts    # Updated to include fetch context
└── main.ts             # Updated to initialize Hono server
```

## Future Phases

### Phase 2: Port MCP Client Logic
- Create dedicated MCP client directory for coder app
- Adapt MCP client from chatserver
- Integrate with AI SDK for ChatGPT-like streaming 

### Phase 3: Update UI
- Modify `usePersistentChat` hook to use local API
- Update server connection settings in UI

### Phase 4: Configuration & Testing
- Add UI controls to configure MCP servers
- Support switching between local and remote modes
- End-to-end testing of chat functionality

## Technical Details

1. **Hono Server:**
   The server runs in the Electron main process and provides API endpoints similar to those from the remote server.

2. **IPC Communication:**
   - Uses Electron's contextBridge for secure communication
   - Provides `window.electron.fetch` API that works like standard fetch

3. **Response Handling:**
   - Properly reconstructs Response objects in the renderer
   - Maintains all standard fetch API behavior

## Benefits

1. **Improved Privacy:** Chat interactions can stay local to the user's machine
2. **Offline Support:** Works without internet connection
3. **Flexibility:** Can use either local or remote MCP servers
4. **Reduced Latency:** Local operations are faster than remote API calls
5. **Development Friendly:** Easier to develop and test without external dependencies

## Testing

The implementation adds a "Test Local API" button that calls the `/api/ping` endpoint to verify the local Hono server is working correctly.