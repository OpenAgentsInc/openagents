# Local Models Integration Guide

This documentation outlines how local models are integrated in the OpenAgents Coder application, with a specific focus on LMStudio integration. It covers the architecture, implementation details, and solutions to common issues like CORS restrictions.

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Server-Side Components](#server-side-components)
4. [Client-Side Components](#client-side-components)
5. [Cross-Origin Resource Sharing (CORS)](#cross-origin-resource-sharing-cors)
6. [Model Detection](#model-detection)
7. [User Interface](#user-interface)
8. [Troubleshooting](#troubleshooting)
9. [Future Improvements](#future-improvements)

## Overview

OpenAgents Coder can connect to various model providers, including locally hosted models through applications like LMStudio and Ollama. This guide focuses primarily on LMStudio integration, although similar principles apply to other local model providers.

Local model integration allows users to:
- Run models on their own hardware
- Access models without requiring API keys
- Work offline with locally installed models
- Configure model parameters directly

## Architecture

The integration follows a client-server architecture:

```
User Interface
      ↓
Client Components (React)
      ↓
Server-Side Proxy (Hono.js)
      ↓
Local Model Server (LMStudio)
```

This architecture addresses several challenges:
- CORS restrictions when browser components access local servers
- Consistent error handling for connection issues
- Unified interface for both cloud and local models

## Server-Side Components

### Proxy Endpoint

A key component is the proxy endpoint in `server.ts` that handles requests to local model servers:

```javascript
// Proxy endpoint for LMStudio API requests
app.get('/api/proxy/lmstudio/models', async (c) => {
  try {
    const url = c.req.query('url') || 'http://localhost:1234/v1/models';
    
    console.log(`[Server] Proxying request to LMStudio at: ${url}`);
    
    const response = await fetch(url, {
      method: 'GET',
      headers: {
        'Accept': 'application/json',
      },
    });
    
    if (!response.ok) {
      console.error(`[Server] LMStudio proxy request failed with status: ${response.status}`);
      return c.json({ 
        error: `Failed to connect to LMStudio server: ${response.statusText}`,
        status: response.status
      }, response.status);
    }
    
    const data = await response.json();
    console.log(`[Server] LMStudio proxy request successful`);
    
    return c.json(data);
  } catch (error) {
    console.error('[Server] LMStudio proxy error:', error);
    return c.json({ 
      error: 'Failed to connect to LMStudio server',
      details: error instanceof Error ? error.message : String(error)
    }, 500);
  }
});
```

This endpoint:
- Accepts requests from the client
- Forwards them to the LMStudio server
- Returns the response back to the client
- Provides proper error handling and logging

### Chat API Integration

The main chat endpoint in `server.ts` also supports local models by using the URL specified in the user settings. This ensures the application remains flexible and can connect to the user's LMStudio instance regardless of network configuration:

```javascript
// Example of how the LMStudio provider should be configured using settings
if (provider === "lmstudio") {
  console.log(`[Server] Using LMStudio provider`);

  // The baseURL should come from user settings, not be hardcoded
  const lmstudio = createOpenAICompatible({
    name: 'lmstudio',
    baseURL: userSettings.lmstudioUrl || "http://localhost:1234/v1",
  });

  model = lmstudio(MODEL);
}
```

## Client-Side Components

### Model Selection Interface

The `ModelSelect` component handles detecting and selecting LMStudio models:

```typescript
// Function to check if LMStudio is running and get available models
const checkLMStudioModels = async (): Promise<boolean> => {
  try {
    // Use our server-side proxy to avoid CORS issues
    // The URL should come from user settings, not be hardcoded
    const lmStudioUrl = settings?.lmstudioUrl || "http://localhost:1234";
    const proxyUrl = `/api/proxy/lmstudio/models?url=${encodeURIComponent(`${lmStudioUrl}/v1/models`)}`;
    console.log("Checking LMStudio via proxy:", proxyUrl);
    
    const controller = new AbortController();
    const timeoutId = setTimeout(() => {
      controller.abort();
      console.error("LMStudio connection timed out");
    }, 10000); // 10 second timeout
    
    const response = await fetch(proxyUrl, {
      method: 'GET',
      headers: { 'Accept': 'application/json' },
      signal: controller.signal
    });
    
    clearTimeout(timeoutId);
    console.log("LMStudio proxy response status:", response.status, "ok:", response.ok);
    
    if (response.ok) {
      const data = await response.json();
      console.log("LMStudio models data:", data);
      
      // Handle different response formats
      if (data) {
        if (data.data && Array.isArray(data.data) && data.data.length > 0) {
          console.log("Found LMStudio models in data.data array:", data.data);
          return true;
        } else if (Array.isArray(data) && data.length > 0) {
          console.log("Found LMStudio models in root array:", data);
          return true;
        } else if (data.models && Array.isArray(data.models) && data.models.length > 0) {
          console.log("Found LMStudio models in data.models array:", data.models);
          return true;
        } else if (typeof data === 'object' && Object.keys(data).length > 0) {
          // Consider any response with data as valid
          console.log("Found some data from LMStudio:", data);
          return true;
        }
      }
      
      // If we didn't find any recognizable model data, but the server responded successfully
      console.log("LMStudio server responded but no model data found. Considering it running anyway");
      return true;
    }
    
    return false; // Response was not OK
    
  } catch (error) {
    console.warn("Failed to connect to LMStudio API via proxy:", error);
    
    if (error instanceof DOMException && error.name === 'AbortError') {
      console.error("LMStudio connection timed out");
    }
    
    return false;
  }
};
```

### Local Models Configuration Page

The `LocalModelsPage` component provides a user interface for configuring LMStudio, with a user-configurable URL input:

```typescript
export default function LocalModelsPage() {
  const [lmStudioStatus, setLmStudioStatus] = useState<'checking' | 'running' | 'not-running'>('checking');
  const [lmStudioModels, setLmStudioModels] = useState<string[]>([]);
  const [lmStudioUrl, setLmStudioUrl] = useState("http://localhost:1234");
  const [refreshing, setRefreshing] = useState(false);
  const [lmStudioError, setLmStudioError] = useState<string | null>(null);

  // Check LMStudio status using the user-configured URL
  const checkLmStudioStatus = async () => {
    try {
      // Reset error state
      setLmStudioError(null);
      setLmStudioStatus('checking');
      
      // Use our server proxy to avoid CORS issues with the user-specified URL
      const proxyUrl = `/api/proxy/lmstudio/models?url=${encodeURIComponent(`${lmStudioUrl}/v1/models`)}`;
      console.log("Using proxy URL:", proxyUrl);
      
      // Request implementation with timeout handling and response parsing...
    } catch (error) {
      // Error handling with helpful user messages...
    }
  };

  // UI components including URL input field allowing users to change connection target
  return (
    <Card>
      {/* Other UI components */}
      <div className="space-y-2">
        <div className="text-sm font-medium">LMStudio Server URL</div>
        <div className="flex gap-2">
          <Input
            value={lmStudioUrl}
            onChange={(e) => setLmStudioUrl(e.target.value)}
            placeholder="http://localhost:1234"
          />
          <Button onClick={checkLmStudioStatus}>
            Connect
          </Button>
        </div>
      </div>
      {/* Additional UI components */}
    </Card>
  );
}
```

This page allows users to:
- Check the status of their LMStudio server
- Configure the server URL
- View available models
- See helpful error messages

## Cross-Origin Resource Sharing (CORS)

### The CORS Challenge

When a web application tries to make direct requests to a local server like LMStudio (which typically runs on http://localhost:1234), browsers block these requests due to CORS policies. These policies prevent websites from making requests to domains other than the one the site is hosted on.

### Solution: Server-Side Proxy

Our solution uses a server-side proxy that:
1. Receives requests from the client app
2. Makes requests to the local model server on behalf of the client
3. Returns the responses back to the client

This approach works because server-side code is not subject to the same CORS restrictions as browser JavaScript.

```
Browser → Our Server → LMStudio Server
           ↑     ↓
           └─────┘
```

## Model Detection

The application includes a robust approach to detecting whether local models are available:

1. Proxy requests to LMStudio's API endpoint
2. Handle various response formats
3. Implement timeout and error handling
4. Make availability information accessible to UI components

This detection happens in two main places:
- On application startup and model selection
- When explicitly checking in the Local Models settings page

## User Interface

The user interface provides several components that facilitate local model usage:

1. **Model Selection Dropdown**: Shows available models and status indicators
2. **Local Models Settings Page**: Configuration interface for LMStudio
3. **Warning Messages**: Clear indicators when models are unavailable
4. **Configuration Guidance**: Installation and setup instructions

## Troubleshooting

Common issues and solutions:

### Connection Issues

**Problem**: "LMStudio is not running" despite the server being active
**Solution**: 
- Verify LMStudio is running on the correct port (default: 1234)
- Check that the server is started in LMStudio's UI (Local Server tab)
- Ensure the proxy endpoint in server.ts is functioning correctly
- Verify the URL in settings matches the actual address where LMStudio is running
- Try using `http://localhost:1234` or the actual IP address if running on a different machine
- Check if network/firewall settings are blocking connections

### Network Configuration

**Problem**: Can't connect to LMStudio running on another machine
**Solution**:
- In LMStudio, ensure "Start server" is clicked in the Local Server tab
- Make sure "Allow remote connections" is enabled in LMStudio
- Use the machine's actual IP address rather than localhost in the URL field
- Check if firewall is blocking port 1234 (the default LMStudio port)
- Verify that the network allows the connection between machines

### CORS Errors

**Problem**: CORS errors in the console
**Solution**: 
- Ensure requests are going through the proxy endpoint 
- Check proxy endpoint implementation in server.ts
- Verify LMStudio isn't blocking requests (rarely happens)
- Make sure the correct URL is being passed to the proxy endpoint

### Model Not Showing

**Problem**: Models don't appear in dropdown despite server running
**Solution**:
- Check model parsing logic in model-select.tsx
- Examine server response format in browser console (look for specific model data format)
- Ensure models are properly loaded in LMStudio
- Try restarting the LMStudio server
- Check that at least one model is loaded in LMStudio before trying to connect

### Timeout Issues

**Problem**: Connection attempts to LMStudio time out
**Solution**:
- Verify that LMStudio is not busy processing other requests
- Check if the model is still loading in LMStudio (large models take time to load)
- Increase the timeout value in the code if consistently timing out
- Try using a smaller, faster model in LMStudio
- Restart both the application and LMStudio

## Future Improvements

Planned enhancements for the local model integration:

1. **Settings Integration**: Pass user settings from UI to server for model configuration
2. **Dynamic Configuration**: Allow updating LMStudio URL without restarting the server
3. **Model Parameter Controls**: Allow adjusting temperature, top-p, etc.
4. **Model Install Management**: Install/remove models directly from the UI
5. **Performance Metrics**: Track inference speed and resource usage
6. **Multi-Modal Support**: Handle image input/output for local vision models
7. **Response Streaming**: Improve streaming implementation for local models
8. **Restore Ollama Support**: Re-implement and improve Ollama integration
9. **Automatic Discovery**: Detect local model servers on the network automatically
10. **Health Monitoring**: Add endpoint to monitor LMStudio health and model status

## LMStudio Configuration Best Practices

When configuring LMStudio for use with this application, follow these best practices:

### Local Usage (Same Machine)
1. Use `http://localhost:1234` as the URL in settings
2. Make sure the LMStudio server is started before connecting
3. Choose smaller models if performance is an issue (7B models recommended)
4. Ensure only one instance of LMStudio is running to avoid port conflicts
5. Set "auto start" in LMStudio settings for easier reconnection

### Remote Usage (Different Machine)
1. In LMStudio settings, enable "Allow remote connections"
2. Use the actual IP address of the machine running LMStudio (e.g., `http://192.168.1.100:1234`)
3. Ensure the machine running LMStudio has a static IP or use mDNS/Bonjour if available
4. Check firewall settings to allow connections on port 1234
5. For public networks, consider using a reverse proxy with TLS for security

### Model Loading
1. Load models in LMStudio before attempting to connect
2. Monitor memory usage - LMStudio is memory-intensive with large models
3. Consider using quantized models (GGUF format) for better performance
4. Restart LMStudio after extended periods of inactivity
5. Check the console for error messages if models don't appear

---

## Development Notes

- The proxy strategy is a temporary solution; a more robust approach might involve using a native app bridge or WebSockets.
- Error logging is comprehensive to help diagnose connection issues.
- Local model detection intentionally allows different response formats since these APIs are still evolving.
- Thorough console logging helps with diagnostics during development.