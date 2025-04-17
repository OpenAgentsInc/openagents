# React 19 SSR with renderToReadableStream Fix

## The Root Cause

After extensive testing, we found the exact issue with React 19 SSR in the application:

1. React 19 splits its server-side rendering APIs into environment-specific modules:
   - `react-dom/server.node.js` - For Node.js
   - `react-dom/server.browser.js` - For browsers/edge
   - `react-dom/server.edge.js` - For edge runtimes
   - `react-dom/server.bun.js` - For Bun

2. The default `react-dom/server` in React 19 is just a proxy that redirects to `react-dom/server.node`, as seen in the server.js file:
   ```javascript
   // Contents of react-dom/server.js
   'use strict';
   module.exports = require('./server.node');
   ```

3. The `renderToReadableStream` function is only exported by the browser and edge versions:
   ```javascript
   // From server.browser.js
   exports.renderToReadableStream = s.renderToReadableStream;
   ```

4. But it's NOT exported by the Node.js version, which is the default:
   ```javascript
   // From server.node.js
   exports.renderToString = l.renderToString;
   exports.renderToStaticMarkup = l.renderToStaticMarkup;
   exports.renderToPipeableStream = s.renderToPipeableStream;
   // Note: No renderToReadableStream
   ```

## The Solution

The solution is to import `renderToReadableStream` directly from the browser version:

```javascript
// Instead of
import { renderToReadableStream } from "react-dom/server";

// Use
import { renderToReadableStream } from "react-dom/server.browser";
```

And add an explicit alias in Vite configuration to ensure it can find the file:

```javascript
resolve: {
  alias: {
    'react': path.resolve(__dirname, '../../node_modules/react'),
    'react-dom': path.resolve(__dirname, '../../node_modules/react-dom'),
    'react-dom/server.browser': path.resolve(__dirname, '../../node_modules/react-dom/server.browser.js'),
  },
},
```

## Why This Works

1. Cloudflare Workers/Vite SSR is more similar to an Edge/Browser environment than to Node.js
2. The Browser/Edge version of React 19's server rendering API is what provides `renderToReadableStream`
3. By importing directly from that module, we get the exact function we need

## Implementation Details

The complete implementation uses:

1. Direct import from `react-dom/server.browser`
2. Vite alias to ensure it finds the file
3. Standard async/await pattern with `renderToReadableStream`
4. Proper handling of the `body.allReady` property for bots/SPA

## Lessons Learned

1. React 19's server rendering API is now split across multiple environment-specific modules
2. The default server module may not provide the functions you expect
3. Always check the exact exports of the module you're using
4. For Cloudflare/Edge environments, use the browser or edge server APIs, not the Node.js one