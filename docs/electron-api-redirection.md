# Electron API URL Redirection System

## Overview

This document explains the URL redirection system used in the Electron app to handle API requests. This system is essential for bridging the renderer process (web content) with the local API server running in the main process.

## Problem Statement

In Electron applications with a local API server, there are two key challenges:

1. **Development vs. Production URLs**: In development, we often use relative URLs (e.g., `/api/chat`), but in production builds, these may fail to resolve correctly due to the file:// protocol or bundling.

2. **CORS and Security Restrictions**: Web content running in the renderer process has security restrictions when trying to access local servers directly.

## Solution Architecture

Our solution uses a multi-layered approach:

### 1. URL Detection and Redirection in Main Process

The main process intercepts web requests using Electron's `webContents.session.webRequest.onBeforeRequest` API to detect API calls and redirect them to the local server:

```javascript
mainWindow.webContents.session.webRequest.onBeforeRequest((details, callback) => {
  try {
    // Check if this is an API request to our local server
    const isApiRequest = details.url.includes('/api/');
    const isLocalhost = details.url.startsWith(`http://localhost:${LOCAL_API_PORT}`);
    
    if (isApiRequest && !isLocalhost) {
      let redirectUrl;
      try {
        // Parse the URL to extract path and query parameters
        const urlObj = new URL(details.url);
        redirectUrl = `http://localhost:${LOCAL_API_PORT}${urlObj.pathname}${urlObj.search || ''}`;
      } catch (parseError) {
        // Fallback for URLs that may not parse correctly
        const urlPath = details.url.split('/api/')[1] || '';
        redirectUrl = `http://localhost:${LOCAL_API_PORT}/api/${urlPath}`;
      }
      
      console.log(`[Main Process] Redirecting API request from ${details.url} to ${redirectUrl}`);
      callback({ redirectURL: redirectUrl });
    } else {
      callback({});
    }
  } catch (error) {
    // Log the error but allow the request to continue
    console.error('[Main Process] Error in web request handler:', error);
    callback({}); // Don't block the request on error
  }
});
```

### 2. Smart URL Selection in API Client

The API client (usePersistentChat) uses environment detection to choose the appropriate URL format:

```javascript
api: options.api || (typeof window !== 'undefined' && window.location.href.includes('app.asar') 
  ? 'http://localhost:3001/api/chat'  // Production build - use absolute URL
  : '/api/chat'),                     // Development build - use relative URL
```

This allows:
- Development builds to use relative URLs (which work well with development servers)
- Production builds to use absolute URLs (which are more reliable in packaged apps)

## How the System Works

1. **Initialization**: On app startup, a local API server is launched on port 3001 (default).

2. **Request Flow**:
   - The renderer makes an API request (e.g., fetch('/api/chat'))
   - In development: The relative URL works as expected with development servers
   - In production: The URL is either:
     - Explicitly set to `http://localhost:3001/api/chat` in the API client
     - Or caught by the request interceptor and redirected appropriately

3. **Error Handling**:
   - The system includes fallbacks for URL parsing errors
   - All exceptions in the redirect handler are caught and logged
   - Requests are allowed to continue even if redirection fails, preventing silent blocking

## Dynamic Port Allocation

To handle cases where the default port (3001) may already be in use by another application, we've implemented a dynamic port allocation system:

1. **Alternative Ports**: The application tries a series of alternative ports (3002, 3003, etc.) if the default port is unavailable.

2. **Port Discovery Flow**:
   - The app first attempts to start the server on port 3001
   - If that port is in use (EADDRINUSE error), it tries the next port in the alternatives list
   - This process continues until a free port is found or all options are exhausted
   - Once a port is successfully bound, it's stored in the `LOCAL_API_PORT` variable

3. **Port Synchronization**: The chosen port is synchronized between:
   - Main process (server)
   - Renderer process (web client)
   - IPC bridge via the `API_PORT` context

4. **Port Information Access**: The renderer process can access the current port through:
   ```javascript
   const port = await window.API_PORT.getPort();
   ```

## Potential Issues and Debugging

If API requests fail despite this system, check:

1. **Local Server Status**: Ensure the local API server is running and listening on some available port:
   ```
   [Main Process] ✅ Local API server listening on http://localhost:3001
   ```
   OR
   ```
   [Main Process] ✅ Local API server listening on http://localhost:3002
   ```

2. **Request Redirection**: Look for log entries showing the redirection:
   ```
   [Main Process] Redirecting API request from file:///path/to/api/chat to http://localhost:3001/api/chat
   ```

3. **URL Format**: Check what URL format is being used by the client. In production builds, it should use the absolute URL.

4. **CORS Headers**: Ensure the server is configured to accept requests from all origins during development.

5. **Network Errors**: Monitor the developer console and main process logs for network-related errors.

## Best Practices

1. **Explicit Routes**: Always use explicit route paths (e.g., `/api/chat`) rather than dynamic routes when possible.

2. **Error Logging**: Include detailed logging in both the main process and renderer to catch issues.

3. **Environment Detection**: Use reliable methods to detect production vs. development environments.

4. **Fallbacks**: Include fallback mechanisms for URL parsing or server connection failures.

5. **Testing**: Test API requests in both development and production builds.

## Recent Improvements

Recent updates to this system include:

1. **Robust URL Parsing**: Better handling of URLs that may not parse correctly using a fallback mechanism.

2. **Environment-Specific URLs**: Automatic detection of production builds to use absolute URLs directly.

3. **Error Handling**: Comprehensive error handling to prevent silent failures.

4. **Logging Enhancements**: More detailed logging to diagnose redirect issues.

These improvements ensure reliable API connectivity in both development and production environments, addressing previously encountered "Failed to fetch" errors during production usage.