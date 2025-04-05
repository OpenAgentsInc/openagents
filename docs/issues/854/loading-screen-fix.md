# Fixing Black Screen Issue in Production Build

## Problem Overview

After implementing the browser compatibility fixes for MCP tools, we encountered a black screen issue in the production build. The application would start loading but would get stuck at the loading screen with no visible errors.

## Root Causes Identified

1. **Hidden App Container**: The app container was set to `display: none` initially and never shown if there were issues
2. **Content Security Policy**: Inline scripts in the HTML were being blocked due to the strict CSP
3. **Failure to Handle Errors**: Loading errors weren't being handled properly in the production environment
4. **Missing Fallback Content**: No fallback content was visible if React failed to render

## Comprehensive Solution

### 1. Changed App Container Visibility Strategy

Instead of hiding the app container initially and showing it later, we changed to:
- Display the app container by default
- Add fallback content that shows while React is loading
- Only hide the loader rather than showing the app

```html
<!-- Main app container - Not display:none anymore to avoid black screen issues in production -->
<div id="app" class="h-full w-full">
  <!-- Fallback content that will be replaced when React loads -->
  <div style="display: flex; justify-content: center; align-items: center; height: 100vh; color: #777; font-family: system-ui, sans-serif;">
    <div style="text-align: center;">
      <p>Loading application content...</p>
      <p style="font-size: 12px; margin-top: 10px;">If this message persists, please check the console for errors</p>
    </div>
  </div>
</div>
```

### 2. Added Forced Display Timeout

Added a timeout that automatically shows the app after a certain period:

```javascript
// Force show app if it takes too long (backup measure)
window.forceShowApp = setTimeout(function() {
  console.log('[Loader] Force showing app after timeout');
  
  const loader = document.getElementById('initial-loader');
  const app = document.getElementById('app');
  
  if (loader && app) {
    loader.style.display = 'none';
    app.style.display = 'block';
  }
  
  // Add a debug message
  // ...
}, 10000); // 10 seconds timeout
```

### 3. Enhanced Visual Debugging

Added visual debugging directly in the UI for production troubleshooting:

```javascript
// Additional debug function to add logs to the page in case console is inaccessible
window.addDebugMessage = function(message) {
  try {
    // Try to find or create a debug log container
    let debugLog = document.getElementById('debug-messages');
    if (!debugLog) {
      debugLog = document.createElement('div');
      debugLog.id = 'debug-messages';
      debugLog.style.position = 'fixed';
      // ... styling ...
      document.body.appendChild(debugLog);
    }

    // Add timestamp and message
    const messageEl = document.createElement('div');
    const time = new Date().toISOString().substring(11, 19); // HH:MM:SS
    messageEl.textContent = `[${time}] ${message}`;
    debugLog.appendChild(messageEl);
    
    // ... additional functionality ...
  } catch (e) {
    console.error('Error adding debug message:', e);
  }
};
```

### 4. Improved Renderer Script Loading

Updated the renderer script loading approach:

```javascript
// Create a script element to load the renderer
const rendererScript = document.createElement('script');
rendererScript.type = 'module';
rendererScript.src = '/src/renderer.ts';

rendererScript.onload = function() {
  window.addDebugMessage('Renderer script loaded successfully');
};

rendererScript.onerror = function(error) {
  console.error('[Bootstrap] Failed to load renderer script:', error);
  window.addDebugMessage(`Error loading renderer: ${error}`);
  
  // ... error handling ...
  
  // Show app element in case of error
  const appEl = document.getElementById('app');
  if (appEl) {
    appEl.style.display = 'block';
  }
};
```

### 5. Enhanced Build Configuration

Updated the Vite build configuration to improve debugging in production:

```typescript
build: {
  // ... existing config ...
  minify: false, // Disable minification for easier debugging
  sourcemap: true, // Enable sourcemaps in production for debugging
  // Configure chunk naming for better error reporting
  rollupOptions: {
    output: {
      manualChunks: {
        react: ['react', 'react-dom'],
        // ... more chunks ...
      },
      chunkFileNames: 'assets/js/[name]-[hash].js',
    },
  },
},
```

## Benefits of This Approach

1. **Fail-Open Design**: The app now has fallback content and will always display something to the user
2. **Multiple Recovery Mechanisms**: Several backup approaches ensure the app can recover from various failure modes
3. **Self-Debugging**: Visual debugging in the UI helps diagnose issues without needing the console
4. **Clear Failure Paths**: Explicit error handling for each potential failure point
5. **Improved Build Artifacts**: Better chunk naming and source maps for production debugging

## Testing and Verification

The solution has been tested in both development and production environments:
- Ensures the app container is visible even if React fails to load
- Provides clear error feedback to users when things go wrong
- Automatically times out and shows content after a reasonable delay
- Adds visual debugging to help identify issues in production

## Recommended Practices Going Forward

1. **Fail-Open Design**: Always design UIs to fail open rather than fail closed (blank screens)
2. **Multiple Loading Indicators**: Have both initial HTML-based loading and React-based loading
3. **Timeout Recovery**: Add timeouts to force-show content if loading takes too long
4. **Visual Debugging**: Include visual debugging tools that can be enabled in production
5. **Source Maps**: Always include source maps in production builds for easier debugging