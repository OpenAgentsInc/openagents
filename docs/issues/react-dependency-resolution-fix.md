# React Dependency Resolution Fix

## Current Issue

After moving React to `peerDependencies` in the packages and adding Vite aliases, we're now encountering a different error:

```
✘ [ERROR] Cannot read file: /Users/christopherdavid/code/openagents/apps/website/node_modules/react-dom
✘ [ERROR] Cannot read file: /Users/christopherdavid/code/openagents/apps/website/node_modules/react
✘ [ERROR] Cannot read file: /Users/christopherdavid/code/openagents/apps/website/node_modules/react/jsx-runtime
```

## Analysis

This error indicates that Vite is correctly trying to resolve React from the `apps/website/node_modules` directory as we specified in the aliases, but it cannot find the files there. This likely means:

1. React is not installed in the expected location (`apps/website/node_modules`)
2. The hoisting behavior of yarn workspaces is putting React in the root `node_modules` directory instead
3. The alias paths in Vite need adjustment to match where React is actually installed

## Solution Steps

### 1. Fix the Vite configuration

Update the `apps/website/vite.config.ts` to use the root `node_modules` directory for React:

```typescript
// In apps/website/vite.config.ts
export default defineConfig({
  // ...
  resolve: {
    alias: {
      // Point to the root node_modules instead of the app-specific one
      'react': path.resolve(__dirname, '../../node_modules/react'),
      'react-dom': path.resolve(__dirname, '../../node_modules/react-dom'),
    },
  },
});
```

### 2. Ensure React is correctly installed

Make sure React and ReactDOM are installed in the monorepo root:

```json
// In root package.json
{
  "dependencies": {
    "react": "19.1.0",
    "react-dom": "19.1.0"
  }
}
```

### 3. Adjust the workspace package.json files

For all packages, ensure React is properly defined as a peer dependency:

```json
// In packages/*/package.json
{
  "peerDependencies": {
    "react": "^19.0.0",
    "react-dom": "^19.0.0"
  }
}
```

### 4. Use resolutions to enforce versions

Add resolutions to the root package.json to ensure all packages use the same React version:

```json
// In root package.json
{
  "resolutions": {
    "react": "19.1.0",
    "react-dom": "19.1.0"
  }
}
```

### 5. Fix workspaces setup

Make sure the workspaces field in the root package.json is properly configured:

```json
// In root package.json
{
  "workspaces": [
    "packages/*",
    "apps/*"
  ]
}
```

### 6. Deduplicate dependencies

Run Yarn's dedupe command to remove duplicate dependencies:

```bash
yarn dedupe
```

### 7. Clean and reinstall

Completely clean and reinstall all dependencies:

```bash
# Remove all node_modules
rm -rf node_modules
rm -rf packages/*/node_modules
rm -rf apps/*/node_modules

# Remove lock file
rm -f yarn.lock

# Reinstall
yarn install
```

## Alternative Resolution Strategies

If the above steps don't resolve the issue, consider these alternatives:

### Option 1: Use bundleDependencies

In the website's package.json, add React as a bundled dependency:

```json
{
  "bundleDependencies": ["react", "react-dom"]
}
```

### Option 2: Use external in Vite config

Tell Vite to treat React as an external dependency:

```typescript
export default defineConfig({
  // ...
  build: {
    rollupOptions: {
      external: ['react', 'react-dom'],
    }
  }
});
```

### Option 3: Create symlinks

Create symbolic links to ensure Vite can find the React packages:

```bash
mkdir -p apps/website/node_modules
ln -sf ../../../node_modules/react apps/website/node_modules/react
ln -sf ../../../node_modules/react-dom apps/website/node_modules/react-dom
```

## Diagnosing Dependency Resolution

To understand how dependencies are being resolved, run:

```bash
yarn why react
yarn why react-dom
```

## Monitoring Progress

1. Check if the error messages change after implementing each solution step
2. Look for changes in how Vite resolves React imports in its debug logs
3. Watch for React-related errors in the browser console

## Expected Outcome

Once resolved, the website should start successfully, and the SolverConnector component should be able to use React hooks without encountering the "Cannot read properties of null" error related to useRef.

## Conclusion

This is likely a Yarn workspace/hoisting issue. By ensuring all packages use the same React version and correctly configuring the build system to resolve dependencies, we can fix both the React hooks context issue and the module resolution error.