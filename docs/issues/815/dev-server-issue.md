# Development Server Issue with Electron IPC

## Problem

When testing the implementation of the local MCP client in the Coder app, we discovered that the IPC communication for fetch requests is not working properly in development mode. Specifically:

1. The `/api/ping` test is failing
2. Requests are being sent to `http://localhost:5173/api/chat` (the Vite dev server) instead of using our IPC mechanism
3. Even though we implemented `window.electron.fetch`, it appears the normal `fetch` is being used

## Root Cause

In development mode, Vite intercepts all fetch requests and routes them through its dev server. This behavior is beneficial for regular web development but causes problems with our Electron IPC implementation.

When we call:
```typescript
api: "/api/chat"
```

In Vercel AI SDK's `useChat` hook, the hook is prepending `http://localhost:5173` to all relative URLs, which is standard behavior for browser-based fetch, but not what we want in our Electron app.

## Solutions

### Solution 1: Use Absolute URLs for Electron IPC

Modify the `usePersistentChat` hook to recognize a special URL scheme for local IPC calls:

```typescript
api: "electron:///api/chat"
```

Then in the hook implementation, detect this scheme and use `window.electron.fetch` for these URLs.

### Solution 2: Use a Custom Fetch Function 

Pass a custom fetch function to the hook that uses our IPC mechanism:

```typescript
fetchFn: window.electron.fetch
```

This is the approach we're currently trying, but it appears that it's either not being used or there's an issue with how it's implemented.

### Solution 3: Use a Custom URL Base

Configure Vite to recognize a special URL pattern that doesn't get intercepted:

```typescript
api: "/_electron/api/chat"
```

And have the dev server ignore requests to `/_electron/...`.

## Implementation Plan

1. **Immediate Fix**: Add more debug logging to:
   - The `fetch-context.ts` to confirm if our `window.electron.fetch` is being called
   - The IPC handler in the main process to confirm if requests are reaching it
   - The `usePersistentChat` hook to verify which fetch function is actually being used

2. **Short-term Solution**: 
   - Modify `usePersistentChat` to explicitly use `window.electron.fetch` for all requests
   - Add special handling for API URLs that start with a defined scheme (e.g., `electron://`)

3. **Long-term Solution**:
   - Create a proper proxy in the main process that handles both development and production environments
   - Add configuration UI to switch between local and remote MCP servers

## Issues and Fixes

### 1. URL Resolution Fix

When implementing the Electron IPC mechanism, we encountered an issue with relative URLs. In the main process, the `Request` constructor requires absolute URLs, but our app was using relative URLs like `/api/chat`.

The error we encountered was:
```
[IPC Fetch Handler] Error: TypeError: Failed to parse URL from /api/chat
    at new Request (node:internal/deps/undici/undici:9580:19)
```

To fix this issue, we:

1. Modified the fetch handler in the main process to convert relative URLs to absolute URLs:
   ```typescript
   if (typeof input === 'string' && input.startsWith('/')) {
     // Use a dummy base URL for relative paths
     url = new URL(input, 'http://localhost');
     console.log('[IPC Fetch Handler] Converted relative URL to:', url.toString());
   }
   ```

2. Enhanced the custom fetch function in the renderer process to better handle various URL formats, particularly for relative URLs.

This approach ensures that our Electron IPC mechanism works correctly even with relative URLs.

### 2. AbortSignal Type Issues

We encountered another error related to the `signal` property in the Request constructor:

```
Failed to construct 'Request': member signal is not of type AbortSignal.
```

This happened because the serialization of the signal object through the IPC channel resulted in an invalid object type. To fix this issue, we:

1. Removed the signal property completely from the init object before creating the Request:
   ```typescript
   const { signal, ...safeInit } = init || {};
   const request = new Request(url, safeInit);
   ```

### 3. Response Methods Missing

We also encountered issues with the reconstructed Response object in the renderer process:

```
TypeError: response.text is not a function
```

This was happening because when we reconstructed the Response object in the IPC bridge, it didn't maintain all the expected methods. We fixed this by:

1. Using a Blob to properly construct the response body:
   ```typescript
   const bodyBlob = new Blob([responseBody], { 
     type: responseHeaders.get('content-type') || 'text/plain' 
   });
   const response = new Response(bodyBlob, {...});
   ```

2. Adding fallback mechanisms in the custom fetch function to check if the response has the expected methods and recreate it if necessary.

### 4. Handling SSE Streaming Responses

Since the AI chat functionality relies on Server-Sent Events (SSE) for streaming responses, we had to implement special handling for these streaming responses:

1. In the server endpoint:
   ```typescript
   // Set SSE headers
   c.header('Content-Type', 'text/event-stream');
   c.header('Cache-Control', 'no-cache');
   c.header('Connection', 'keep-alive');
   
   // Use Hono's stream helper
   return stream(c, async (s) => {
     // Send events in Vercel AI format
     await s.write(`data: 0:${JSON.stringify(metadata)}\n\n`);
     // Text chunks
     await s.write(`data: 1:${JSON.stringify(chunk)}\n\n`);
     // End marker
     await s.write(`data: 2:[DONE]\n\n`);
   });
   ```

2. In the IPC fetch handler:
   ```typescript
   // Check if this is a streaming response (SSE)
   if (response.headers.get('content-type')?.includes('text/event-stream')) {
     // Add a flag to indicate this is a stream
     return { /* response properties */, isStream: true };
   }
   ```

3. In the preload script, created a special Response with a ReadableStream for streaming responses:
   ```typescript
   if (result.isStream) {
     const readableStream = new ReadableStream({
       start(controller) {
         // Set up streaming
         controller.enqueue(encoder.encode(''));
       }
     });
     
     return new Response(readableStream, {
       status: result.status,
       headers: responseHeaders,
     });
   }
   ```

This approach allows the Vercel AI SDK to properly process streaming responses from our local server.

## Testing in Production Build

To properly test our implementation, we should create a production build where Vite doesn't intercept requests:

```bash
# Build a production version
yarn build

# Run the production build
yarn start
```

In production mode, URL resolution works differently and may allow our IPC mechanism to work as intended.