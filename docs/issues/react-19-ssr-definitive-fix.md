# React 19 SSR & Hooks - Definitive Fix for Cloudflare Workers Environment

## Root Causes

We identified two fundamental issues with React 19 in our Cloudflare Workers environment:

### 1. Server-Side Rendering Module Resolution

1. **Module Resolution Mismatch**: React 19 separates server rendering logic into environment-specific modules:
   - `react-dom/server.node.js` - For Node.js environments
   - `react-dom/server.browser.js` - For browser environments
   - `react-dom/server.edge.js` - For edge runtimes like Cloudflare Workers

2. **Default Resolution Problem**: When importing from `"react-dom/server"`, the module resolution system was defaulting to the Node.js version, which:
   - Does NOT export `renderToReadableStream`
   - Exports `renderToPipeableStream` instead, which doesn't work in Worker environments

3. **Environment Type Mismatch**: Cloudflare Workers are edge environments, not Node.js environments, so they need the edge-specific module.

### 2. Multiple React Instances Causing "Cannot read properties of null (reading 'useRef')"

Even after fixing the SSR rendering, hooks from packages like `agents/react` were failing with "Cannot read properties of null (reading 'useRef')". This happens because:

1. **Multiple React Instances**: When different parts of the application use different instances of React, hooks break
2. **SSR Bundling Issue**: Vite's SSR build was potentially bundling React separately for some modules
3. **Library Integration**: External packages might not properly respect React as a peer dependency in the SSR context

## The Solution

The solution has three critical parts:

### 1. Explicit Import

In `apps/website/app/entry.server.tsx`, explicitly import from the edge module:

```javascript
// Instead of
import { renderToReadableStream } from "react-dom/server";

// Use the explicit edge module import
import { renderToReadableStream } from "react-dom/server.edge";
```

### 2. Force Module Resolution via Vite Aliases

In `apps/website/vite.config.ts`, add aliases that force both generic and specific imports to resolve to the edge module:

```javascript
resolve: {
  alias: {
    'react': path.resolve(__dirname, '../../node_modules/react'),
    'react-dom': path.resolve(__dirname, '../../node_modules/react-dom'),
    // FORCE resolution to the .edge version for both generic and specific imports
    'react-dom/server': path.resolve(__dirname, '../../node_modules/react-dom/server.edge.js'),
    'react-dom/server.edge': path.resolve(__dirname, '../../node_modules/react-dom/server.edge.js'),
  },
},
```

### 3. Enhanced Module Deduplication and Aliasing

Add enhanced module resolution settings to `apps/website/vite.config.ts` to ensure consistent React usage across the application:

```javascript
resolve: {
  alias: {
    // Point to the root node_modules instead of the app-specific one
    'react': path.resolve(__dirname, '../../node_modules/react'),
    'react-dom': path.resolve(__dirname, '../../node_modules/react-dom'),
    // FORCE resolution to the .edge version for both generic and specific imports
    'react-dom/server': path.resolve(__dirname, '../../node_modules/react-dom/server.edge.js'),
    'react-dom/server.edge': path.resolve(__dirname, '../../node_modules/react-dom/server.edge.js'),
    // Ensure agents/react resolves consistently
    'agents/react': path.resolve(__dirname, '../../node_modules/agents/dist/react.js'),
  },
  // This helps ensure proper deduplication of packages
  dedupe: ['react', 'react-dom', 'agents', 'agents/react'],
},
```

**Note:** We initially tried using `ssr.external`, but the Cloudflare Vite plugin doesn't support this configuration. Using `resolve.dedupe` achieves a similar goal by ensuring packages are properly deduplicated.

## Why This Works

### For the SSR Rendering Issue

1. The `.edge.js` module provides the correct implementation of `renderToReadableStream` designed for edge environments
2. By forcing all imports (even indirect ones from libraries) to resolve to this module, we guarantee the right API is used
3. Edge environments, like Cloudflare Workers, use Web Streams APIs which are compatible with the edge module implementation

### For the React Hooks Error

1. Using explicit aliases for all React-related modules and the agents/react package ensures resolution to the same instances
2. The `resolve.dedupe` setting forces Vite to use only one copy of these packages in the bundle
3. This prevents the "multiple React instances" problem that causes hook errors
4. When all components (including those from libraries) use the same React instance, hooks work correctly

## Implementation Details

### Entry Server Implementation

```javascript
import type { AppLoadContext, EntryContext } from "react-router";
import { ServerRouter } from "react-router";
import { isbot } from "isbot";
import { renderToReadableStream } from "react-dom/server.edge";

export default async function handleRequest(
  request: Request,
  responseStatusCode: number,
  responseHeaders: Headers,
  routerContext: EntryContext,
  _loadContext: AppLoadContext
) {
  let shellRendered = false;
  const userAgent = request.headers.get("user-agent");

  const body = await renderToReadableStream(
    <ServerRouter context={routerContext} url={request.url} />,
    {
      onError(error: unknown) {
        responseStatusCode = 500;
        if (shellRendered) {
          console.error(error);
        }
      },
    }
  );
  shellRendered = true;

  // Wait for all content for bots and SPA Mode
  if ((userAgent && isbot(userAgent)) || routerContext.isSpaMode) {
    await body.allReady;
  }

  responseHeaders.set("Content-Type", "text/html");
  return new Response(body, {
    headers: responseHeaders,
    status: responseStatusCode,
  });
}
```

## Essential After Applying This Fix

After applying this fix, it's critical to:

1. Clear Vite's cache: `rm -rf node_modules/.vite`
2. Clear app-specific node_modules: `rm -rf apps/website/node_modules` (if it exists)
3. Reinstall all dependencies: `./clean-install.sh` or equivalent

This ensures Vite picks up the new aliases and doesn't use cached, incorrect resolutions.

## Key Takeaways

### For React 19 Server Rendering

1. React 19 has environment-specific server rendering modules 
2. The default import from `react-dom/server` may not provide the API you need
3. For Cloudflare Workers and other edge environments, always use the `.edge` module
4. For Node.js environments, use the `.node` module
5. For browser environments, use the `.browser` module
6. Force module resolution with build tool aliases to guarantee correct imports

### For React Hooks Usage Across Libraries

1. Ensure React and ReactDOM are marked as peer dependencies in all packages
2. Use a single definitive resolution point for React in your monorepo
3. Use explicit aliases to force consistent module resolution
4. Use `resolve.dedupe` to ensure packages are properly deduplicated
5. When using Cloudflare Workers, be aware that standard Vite SSR configurations like `ssr.external` are not supported
6. For libraries/packages imported from node_modules, explicitly alias them to ensure consistent resolution