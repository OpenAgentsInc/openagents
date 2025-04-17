# React 19 Server-Side Rendering Fix

## Issue

After updating to React 19 and implementing the dependency resolution fixes, we encountered an SSR error in the website application:

```
TypeError: renderToReadableStream is not a function
    at handleRequest (/Users/christopherdavid/code/openagents/apps/website/app/entry.server.tsx:16:22)
```

This is due to changes in React 19's module structure, particularly for the server rendering APIs.

## Root Cause

In React 19, the server rendering API has been restructured. The server rendering functionality is now available in specific environment-targeted files like `server.node.js` rather than in a general `server` directory.

## Solution

The solution involves two key changes:

### 1. Update Import Path in Server Entry File

In `apps/website/app/entry.server.tsx`:

```typescript
// Before
import { renderToReadableStream } from "react-dom/server";

// After
import * as ReactDOMServer from "react-dom/server.node";

// And then update the function call
const body = await ReactDOMServer.renderToReadableStream(...)
```

### 2. Add Server Module Alias to Vite Config

In `apps/website/vite.config.ts`, add an alias for the server module:

```typescript
resolve: {
  alias: {
    'react': path.resolve(__dirname, '../../node_modules/react'),
    'react-dom': path.resolve(__dirname, '../../node_modules/react-dom'),
    'react-dom/server.node': path.resolve(__dirname, '../../node_modules/react-dom/server.node.js'),
  },
},
```

This ensures that Vite correctly resolves the import to the React 19 server module.

## Further Considerations

If you encounter similar issues with other React DOM modules, check for their specific paths in the React 19 module structure. React 19 has more environment-specific entry points compared to previous versions.

For any component using server-side rendering, ensure it's using the appropriate React 19 server functions:

- `renderToReadableStream` - For streaming SSR
- `renderToString` - For traditional synchronous SSR
- `renderToStaticMarkup` - For static markup generation

## Testing

After applying these changes:

1. Clean and reinstall dependencies with `./clean-install.sh`
2. Start the website with `yarn website`
3. Verify that the server can successfully render the application

These changes should resolve the server-side rendering issues with React 19.