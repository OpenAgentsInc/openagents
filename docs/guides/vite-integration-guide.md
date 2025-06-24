# Vite Integration Guide

**Audience**: Coding Agents  
**Purpose**: Comprehensive guide to understanding and working with Vite in the OpenAgents codebase  
**Last Updated**: 2025-06-23

## Table of Contents
1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Configuration](#configuration)
4. [Development Workflow](#development-workflow)
5. [Build Process](#build-process)
6. [Module System](#module-system)
7. [Integration with Effect/Psionic](#integration-with-effectpsionic)
8. [Common Patterns](#common-patterns)
9. [Troubleshooting](#troubleshooting)

## Overview

### What is Vite?
Vite is a modern build tool that provides:
- **Lightning-fast HMR** (Hot Module Replacement) for instant client-side updates
- **Native ES modules** in development for better debugging
- **Optimized production builds** using Rollup
- **Built-in TypeScript support** with fast transpilation

### Why Vite in OpenAgents?
We use Vite specifically for **client-side assets only**:
- Server-side code runs on **Bun** (no changes)
- Psionic SSR remains unchanged
- Effect architecture is unaffected
- Vite handles only browser JavaScript and assets

## Architecture

### Dual-Server Development Setup
```
┌─────────────────┐         ┌──────────────────┐
│   Vite Server   │ <-----> │   Bun Server     │
│   Port: 5173    │  proxy  │   Port: 3000     │
│                 │         │                  │
│ - Client JS     │         │ - SSR Pages      │
│ - HMR Updates   │         │ - API Routes     │
│ - Asset serving │         │ - WebSockets     │
└─────────────────┘         └──────────────────┘
```

### File Organization
```
apps/openagents.com/
├── src/
│   ├── client/           # Vite-bundled client code
│   │   ├── index.ts      # Main client entry
│   │   ├── chat.ts       # Chat functionality
│   │   └── model-selector.ts  # Model dropdown
│   ├── components/       # Server-side Psionic components
│   ├── routes/           # Server-side route handlers
│   └── index.ts          # Bun server entry
├── public/               # Static assets
│   └── js/               # Vite build output
└── vite.config.ts        # Vite configuration
```

## Configuration

### vite.config.ts
```typescript
import { defineConfig } from "vite"
import path from "path"

export default defineConfig({
  root: __dirname,
  base: "/",
  publicDir: "static", // Avoid conflict with build output
  
  // Entry points for client code
  build: {
    outDir: "public/js",
    emptyOutDir: false, // Don't clear existing assets
    rollupOptions: {
      input: {
        client: path.resolve(__dirname, "src/client/index.ts"),
        chat: path.resolve(__dirname, "src/client/chat.ts"),
        "model-selector": path.resolve(__dirname, "src/client/model-selector.ts")
      },
      output: {
        entryFileNames: "[name].js",
        chunkFileNames: "[name]-[hash].js",
        assetFileNames: "../assets/[name]-[hash][extname]"
      }
    },
    target: "es2020",
    sourcemap: true,
    minify: process.env.NODE_ENV === "production"
  },
  
  // Development server
  server: {
    port: 5173,
    strictPort: true,
    proxy: {
      "/api": {
        target: "http://localhost:3000",
        changeOrigin: true
      },
      "/chat": {
        target: "http://localhost:3000",
        changeOrigin: true
      }
    }
  },
  
  // Workspace package resolution
  resolve: {
    alias: {
      "@openagentsinc/psionic": path.resolve(__dirname, "../../packages/psionic/src"),
      "@openagentsinc/sdk": path.resolve(__dirname, "../../packages/sdk/src"),
      // Add other workspace packages as needed
    }
  }
})
```

### Package.json Scripts
```json
{
  "scripts": {
    "dev": "concurrently \"bun run dev:server\" \"bun run dev:client\"",
    "dev:server": "bun run --hot --env-file=../../.env src/index.ts",
    "dev:client": "vite",
    "build:client": "vite build",
    "preview": "vite preview"
  }
}
```

## Development Workflow

### Starting Development
```bash
# From repository root
pnpm site

# Or directly from openagents.com
pnpm dev
```

This starts:
1. **Bun server** on http://localhost:3000 (main site)
2. **Vite server** on http://localhost:5173 (client assets)

### How It Works
1. Browser loads page from Bun (port 3000)
2. HTML includes script tags pointing to Vite-served files
3. Vite injects HMR client for live updates
4. Changes to client code trigger instant updates

### Writing Client Code

#### Module Structure
Each client module should:
1. Be a proper ES module with exports
2. Initialize itself when needed
3. Expose global functions only when necessary

```typescript
// src/client/my-feature.ts

// Internal state
let state = { initialized: false }

// Private functions
function internalLogic() {
  // Implementation
}

// Public API
export function initializeFeature() {
  if (state.initialized) return
  
  // Set up event listeners
  document.addEventListener('click', handler)
  
  state.initialized = true
}

// Global exposure (only when needed for onclick handlers)
if (typeof window !== 'undefined') {
  (window as any).myGlobalFunction = publicFunction
}
```

#### Component Integration
Server-side components load client modules:

```typescript
// src/components/my-component/index.ts
import { html, css, document } from "@openagentsinc/psionic"

export function myComponent() {
  return document({
    title: "My Page",
    body: html`
      <div id="app">
        <!-- Component HTML -->
      </div>
      
      <script type="module">
        // Import Vite-bundled module
        import { initializeFeature } from '/js/my-feature.js'
        
        // Initialize when ready
        initializeFeature()
      </script>
    `
  })
}
```

## Build Process

### Development Build
- Vite serves files from memory
- No bundling, pure ES modules
- Source maps for debugging
- HMR client injected automatically

### Production Build
```bash
pnpm build
```

This runs:
1. TypeScript compilation
2. Vite build (client bundles)
3. SDK browser build (separate process)
4. Asset generation

### Build Output
```
public/js/
├── client.js           # Main client bundle
├── chat.js             # Chat functionality
├── model-selector.js   # Model selector
├── *.js.map           # Source maps
└── openagents-sdk-browser.js  # SDK bundle (not Vite)
```

## Module System

### ES Modules Only
Vite works with ES modules exclusively:

```typescript
// ✅ Good - ES modules
import { something } from './module'
export function myFunction() {}

// ❌ Bad - CommonJS
const something = require('./module')
module.exports = myFunction
```

### Dynamic Imports
For code splitting:

```typescript
// Lazy load heavy dependencies
async function loadEditor() {
  const { Editor } = await import('./editor')
  return new Editor()
}
```

### Import Assertions
For non-JS assets:

```typescript
// Import CSS
import styles from './styles.css'

// Import JSON
import config from './config.json'

// Import with query params
import Worker from './worker.js?worker'
```

## Integration with Effect/Psionic

### Key Principles
1. **Vite handles client code only** - no Effect on client side
2. **Server remains Effect-based** - no changes to SSR
3. **API calls use fetch** - not Effect HttpClient
4. **No shared state** - client/server are separate

### Client-Server Communication

#### Client Side (Vite)
```typescript
// src/client/api.ts
export async function callAPI(endpoint: string, data: any) {
  const response = await fetch(`/api/${endpoint}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data)
  })
  return response.json()
}
```

#### Server Side (Effect)
```typescript
// src/routes/api/endpoint.ts
export function handler(ctx: RouteContext) {
  return Effect.gen(function* () {
    const body = yield* ctx.request.json
    const result = yield* processWithEffect(body)
    return HttpServerResponse.json(result)
  })
}
```

### Loading Scripts in Psionic Components

Always use ES module syntax:

```typescript
// ✅ Correct - ES modules
html`
  <script type="module">
    import { initChat } from '/js/chat.js'
    initChat()
  </script>
`

// ❌ Wrong - Classic scripts
html`
  <script src="/js/chat.js"></script>
`
```

## Common Patterns

### Pattern 1: Feature Initialization
```typescript
// Client module
export function initializeFeature(config: FeatureConfig) {
  // Store config
  state.config = config
  
  // Set up DOM
  const container = document.getElementById('feature-container')
  if (!container) return
  
  // Attach event listeners
  container.addEventListener('click', handleClick)
  
  // Start any necessary processes
  startPolling()
}

// Server component
html`
  <div id="feature-container"></div>
  <script type="module">
    import { initializeFeature } from '/js/feature.js'
    
    initializeFeature({
      apiEndpoint: '/api/feature',
      refreshInterval: 5000
    })
  </script>
`
```

### Pattern 2: Global Function Exposure
```typescript
// When onclick handlers need global functions
export function handleAction(id: string) {
  console.log('Handling action:', id)
}

// Make available globally
(window as any).handleAction = handleAction

// Usage in HTML
html`
  <button onclick="handleAction('123')">Click me</button>
`
```

### Pattern 3: Streaming Responses
```typescript
// Client-side SSE handling
export async function streamChat(message: string) {
  const response = await fetch('/api/chat', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ message })
  })
  
  const reader = response.body!.getReader()
  const decoder = new TextDecoder()
  
  while (true) {
    const { done, value } = await reader.read()
    if (done) break
    
    const chunk = decoder.decode(value)
    // Process SSE data
    processSSEChunk(chunk)
  }
}
```

## Troubleshooting

### Issue: "Cannot find module" in browser
**Cause**: Import path is wrong or module not built  
**Solution**: 
- Check import uses `/js/` prefix
- Ensure module is in vite.config.ts inputs
- Run build to generate files

### Issue: HMR not working
**Cause**: Module doesn't have proper exports  
**Solution**:
- Add at least one export to the module
- Check browser console for HMR errors
- Ensure Vite server is running

### Issue: Proxy not working
**Cause**: API calls going to wrong port  
**Solution**:
- Verify proxy config in vite.config.ts
- Use relative URLs (`/api/...`) not absolute
- Check both servers are running

### Issue: Build creates empty chunks
**Cause**: Module has no exports or isn't imported  
**Solution**:
- Add default export if needed
- Ensure module is actually used somewhere
- Check for tree-shaking removing code

### Issue: Production build differs from dev
**Cause**: Environment-specific code or missing polyfills  
**Solution**:
- Test with `vite preview` after building
- Check for `process.env.NODE_ENV` usage
- Add necessary polyfills for older browsers

## Best Practices

### DO ✅
- Keep client modules focused and small
- Use TypeScript for all client code
- Test both dev and production builds
- Keep Effect code on server only
- Use ES modules throughout

### DON'T ❌
- Mix server and client code
- Use require() or module.exports
- Import Effect in client modules
- Assume HMR state persists
- Bundle server code with Vite

## Migration Guide

### Converting Inline Scripts
Before (inline script):
```typescript
html`
  <script>
    function handleClick() {
      // inline logic
    }
  </script>
`
```

After (Vite module):
```typescript
// src/client/feature.ts
export function handleClick() {
  // extracted logic
}
(window as any).handleClick = handleClick

// Component
html`
  <script type="module">
    import '/js/feature.js'
  </script>
`
```

### Extracting from Components
1. Identify inline scripts in components
2. Create new file in `src/client/`
3. Extract logic to module
4. Add to vite.config.ts inputs
5. Update component to import module

## Future Considerations

### Potential Improvements
1. **CSS Modules**: Better style isolation
2. **Web Workers**: Offload heavy computations
3. **PWA Support**: Service worker integration
4. **Build Optimization**: Chunk splitting strategies
5. **Asset Optimization**: Image/font processing

### What Not to Change
- Server-side Effect architecture
- Psionic SSR approach
- API route patterns
- Database interactions
- Service layer structure

---

**Remember**: Vite is for client-side development experience only. The core Effect-based architecture remains unchanged. When in doubt, keep client and server concerns completely separated.