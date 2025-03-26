# Hot Reloading Shared UI Components in Electron

This document explains how to set up hot reloading for shared UI components when using them in an Electron application.

## Problem

When making changes to a shared UI package (such as `@openagents/ui`), the changes are immediately reflected in the React Native app but not in the Electron app. This happens because:

1. Vite's dependency caching prevents detecting changes in linked packages
2. The default aliasing in Vite's configuration doesn't properly watch for changes
3. The path aliasing approach creates a snapshot of the code rather than watching live changes

## Solution Overview

The solution involves using symlinks with `npm link` and configuring Vite to properly watch for changes in linked packages:

1. Use `npm link` to create a symbolic link to the UI package
2. Modify Vite configuration to work with symlinks
3. Adjust dependency optimization and file watching settings

## Detailed Steps

### 1. Create a Symbolic Link to the UI Package

Use `npm link` to create a symbolic link to the UI package:

```bash
cd /path/to/apps/coder
npm link ../../packages/ui
```

This creates a symbolic link in the `node_modules` folder of the Electron app that points to the UI package.

### 2. Modify Vite Configuration

Update the Vite configuration file (`vite.renderer.config.mts`):

```typescript
export default defineConfig({
  // ... other config
  resolve: {
    preserveSymlinks: false,  // Changed to false to ensure symlinks work
    alias: {
      "@": path.resolve(__dirname, "./src"),
      "react-native": "react-native-web",
      "react-native$": "react-native-web",
      // Remove UI package aliases - use npm linked version instead
      // "@openagents/ui": path.resolve(__dirname, "../../packages/ui/src"),
      // "@openagents/ui/*": path.resolve(__dirname, "../../packages/ui/src/*"),
      // Other aliases remain unchanged
      "@expo/vector-icons": path.resolve(__dirname, "./src/shims/expo-vector-icons.ts"),
    },
  },
  optimizeDeps: {
    exclude: ['@openagents/ui'], // Exclude UI package from optimization
    include: [
      'react-native-web',
      'react-native-vector-icons',
      'react-native-vector-icons/Ionicons',
    ],
    // Other optimizeDeps options remain unchanged
  },
  server: {
    watch: {
      usePolling: true,
      interval: 500,
    },
  },
})
```

Key changes:
- Set `preserveSymlinks` to `false` to ensure symlinks work properly
- Remove the direct path resolving for the UI package since we're using npm link
- Exclude the UI package from dependency optimization with `exclude: ['@openagents/ui']`
- Add file watching with polling to detect changes in the UI package

### 3. Clean Vite Cache and Restart

Clear the Vite cache and restart the Electron app:

```bash
cd /path/to/apps/coder
rm -rf .vite
npm start
```

## Benefits of the Fix

- **Live Updates**: Changes to the UI package are immediately reflected in the Electron app
- **No Build Step Required**: Works without requiring a build step for the UI package
- **Proper Development Workflow**: Enables a proper development workflow for cross-platform components
- **Consistency**: Ensures consistent behavior between React Native and Electron apps

## Considerations

- The polling approach may increase CPU usage slightly
- You may need to re-run `npm link` if you reinstall dependencies
- This approach works best in development; for production builds, you should use proper dependency management

## Related Files

- `/apps/coder/vite.renderer.config.mts` - Vite configuration for the renderer process
- `/packages/ui/src/components/*` - UI components that will now hot reload correctly