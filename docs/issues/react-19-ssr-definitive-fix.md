# React 19 SSR - Definitive Fix for Cloudflare Workers Environment

## Root Cause

After extensive debugging, we identified the fundamental issue with React 19 server-side rendering in a Cloudflare Workers environment:

1. **Module Resolution Mismatch**: React 19 separates server rendering logic into environment-specific modules:
   - `react-dom/server.node.js` - For Node.js environments
   - `react-dom/server.browser.js` - For browser environments
   - `react-dom/server.edge.js` - For edge runtimes like Cloudflare Workers

2. **Default Resolution Problem**: When importing from `"react-dom/server"`, the module resolution system was defaulting to the Node.js version, which:
   - Does NOT export `renderToReadableStream`
   - Exports `renderToPipeableStream` instead, which doesn't work in Worker environments

3. **Environment Type Mismatch**: Cloudflare Workers are edge environments, not Node.js environments, so they need the edge-specific module.

## The Solution

The solution has two critical parts:

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

## Why This Works

1. The `.edge.js` module provides the correct implementation of `renderToReadableStream` designed for edge environments
2. By forcing all imports (even indirect ones from libraries) to resolve to this module, we guarantee the right API is used
3. Edge environments, like Cloudflare Workers, use Web Streams APIs which are compatible with the edge module implementation

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

## Key Takeaways for React 19 Server Rendering

1. React 19 has environment-specific server rendering modules 
2. The default import from `react-dom/server` may not provide the API you need
3. For Cloudflare Workers and other edge environments, always use the `.edge` module
4. For Node.js environments, use the `.node` module
5. For browser environments, use the `.browser` module
6. Force module resolution with build tool aliases to guarantee correct imports