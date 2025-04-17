# React Hooks Context Error Fix

## Issue

We experienced the following error when using the `useAgent` hook from the agents/react package:

```
Error: Cannot read properties of null (reading 'useRef')
    at useAgent (http://localhost:5173/@fs/Users/christopherdavid/code/openagents/node_modules/agents/dist/react.js?v=4fc9613b:19:27)
```

This is a classic React hooks context error, which occurs when:

1. Using hooks outside of a React function component context
2. Having multiple instances of React in the same application
3. Hooks not following the rules of hooks (conditional calls, etc.)

## Root Cause

The primary issue is having multiple instances of React in our monorepo. When the Agents package has its own React instance and our app has a different one, React hooks break because the React context is not properly shared across these instances.

## Solution

We've implemented the following fixes:

### 1. Fixed Package Dependencies

Updated the `packages/agents/package.json` to move React from dependencies to peerDependencies:

```json
"peerDependencies": {
  "react": "^19.1.0",
  "react-dom": "^19.1.0"
}
```

This ensures that Agents uses the consumer application's React instance rather than bundling its own.

### 2. Updated Vite Configuration

Modified the `apps/website/vite.config.ts` to use alias paths for React that point to the root node_modules:

```javascript
resolve: {
  alias: {
    // Point to the root node_modules instead of the app-specific one
    'react': path.resolve(__dirname, '../../node_modules/react'),
    'react-dom': path.resolve(__dirname, '../../node_modules/react-dom'),
  },
},
```

This ensures that all React imports resolve to the same instance.

### 3. Improved the useOpenAgent Implementation

- Added explicit React import to ensure proper module resolution
- Added robust null checking throughout the implementation
- Consolidated duplicate event listeners
- Added better error handling for all agent methods
- Simplified the React useEffect dependencies

### 4. Dependencies in Root package.json

Ensured that the root package.json has:

```json
"resolutions": {
  "react": "19.1.0",
  "react-dom": "19.1.0"
},
"overrides": {
  "react": "19.1.0",
  "react-dom": "19.1.0"
}
```

This enforces consistent React versions across the entire monorepo.

## Testing the Fix

After implementing these changes, run the clean-install script to ensure all dependencies are properly installed:

```bash
./clean-install.sh
```

Then start the application and verify that the hooks error no longer occurs when using the SolverConnector component.

## Additional Notes

- If you see errors with Vite not finding React in the app's node_modules, this is expected - our aliases now point to the root node_modules
- All React-dependent components should now work consistently across the monorepo
- All packages that use React should declare it as a peer dependency, not a direct dependency

## Conclusion

This solution addresses the root cause by ensuring we have only one React instance throughout the application, which is critical for hooks to work properly. By enforcing peer dependencies and consistent module resolution, we prevent the "multiple React instances" problem that was breaking the hooks context.