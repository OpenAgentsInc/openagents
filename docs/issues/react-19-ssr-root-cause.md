# React 19 Server-Side Rendering: Root Cause Analysis

## Issue

After updating to React 19, we encountered the following error in our server-side rendering:

```
TypeError: ReactDOMServer.renderToReadableStream is not a function
```

## Root Cause Analysis

The issue is **not** just a path resolution problem, but a fundamental change in how React 19 exposes its server-side rendering APIs across different runtime environments.

### 1. API Availability by Environment

In React 19, server-side rendering APIs are split across different runtime-specific modules:

- `react-dom/server.browser.js` - For browser environments
- `react-dom/server.node.js` - For Node.js environments
- `react-dom/server.edge.js` - For edge runtimes
- `react-dom/server.bun.js` - For Bun runtime

Each environment-specific module **exposes different methods**:

### 2. Method Availability

Looking at the actual exports from `server.node.js`:

```javascript
exports.version = l.version;
exports.renderToString = l.renderToString;
exports.renderToStaticMarkup = l.renderToStaticMarkup;
exports.renderToPipeableStream = s.renderToPipeableStream;
```

We can see that:
- `renderToReadableStream` is NOT exposed in the Node.js module
- `renderToPipeableStream` is used for Node.js streaming SSR
- `renderToReadableStream` is likely only available in other environments

### 3. Runtime Environment Considerations

This is a critical change from React 18, where the APIs were more unified. In React 19:

- Each JavaScript runtime has specific optimized methods
- The methods are not interchangeable between environments
- Using the wrong method for your environment will cause errors

## Solution

The correct solution is to:

1. Use the appropriate streaming method for the Node.js environment:
   ```javascript
   import { renderToPipeableStream } from 'react-dom/server.node';
   ```

2. Switch from `renderToReadableStream` to `renderToPipeableStream`, which has a different API:
   ```javascript
   const { pipe, abort } = renderToPipeableStream(
     <App />,
     {
       onShellReady() {
         // Handle initial render
       },
       onAllReady() {
         // Handle when all data is ready
       }
     }
   );
   ```

3. Convert the Node.js stream to a Web stream for use in a Response:
   ```javascript
   import { Readable } from 'node:stream';

   const nodeStream = new Readable();
   pipe(nodeStream);
   const webStream = Readable.toWeb(nodeStream);
   return new Response(webStream, { ... });
   ```

## Implementation Details

The complete implementation requires handling:

1. Different response timing for bots vs. browsers
2. Error handling during streaming
3. Converting Node.js streams to Web streams
4. Proper cleanup and timeouts

## Key Lessons

1. React 19 has environment-specific server rendering modules
2. You must use the correct method for your environment
3. The streaming APIs have changed significantly
4. Always check the actual exports of modules when upgrading major versions

## Further Reading

- [React 19 Server Components documentation](https://react.dev/reference/react-dom/server)
- [React 19 renderToPipeableStream API](https://react.dev/reference/react-dom/server/renderToPipeableStream)